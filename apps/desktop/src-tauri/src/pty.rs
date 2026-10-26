//! Bridges a terminal view to its conversation's engine-hosted PTY. The engine
//! owns the process, so a paired phone shares the same session; this side only
//! forwards one view's stream per conversation.

use std::collections::HashMap;

use modula_client::ModulaClient;
use modula_types::TerminalOutput;
use tauri::async_runtime::JoinHandle;
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::State;
use tokio_stream::StreamExt;

/// The forwarding task of each conversation's mounted view.
#[derive(Default)]
pub struct TerminalViews(tokio::sync::Mutex<HashMap<String, JoinHandle<()>>>);

/// Attach to the conversation's PTY, replaying its buffered output, or launch
/// it. A newer view of the same conversation replaces the older one's stream.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn pty_open(
    engine: State<'_, ModulaClient>,
    views: State<'_, TerminalViews>,
    workspace_id: String,
    conversation_id: String,
    prompt: Option<String>,
    cols: u16,
    rows: u16,
    on_data: Channel<InvokeResponseBody>,
    on_exit: Channel<u32>,
) -> Result<(), String> {
    let mut views = views.0.lock().await;
    if let Some(old) = views.remove(&conversation_id) {
        old.abort();
    }
    let mut stream = engine
        .terminal_open(&workspace_id, &conversation_id, prompt, cols, rows)
        .await
        .map_err(|e| e.to_string())?;
    let task = tauri::async_runtime::spawn(async move {
        while let Some(item) = stream.next().await {
            match item {
                Ok(TerminalOutput::Data(data)) => {
                    let _ = on_data.send(InvokeResponseBody::Raw(data));
                }
                Ok(TerminalOutput::Exit(code)) => {
                    let _ = on_exit.send(code);
                    return;
                }
                Err(_) => break,
            }
        }
        // The engine went away mid-session; offer Resume like any other exit.
        let _ = on_exit.send(1);
    });
    views.insert(conversation_id, task);
    Ok(())
}

#[tauri::command]
pub async fn pty_write(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    conversation_id: String,
    data: String,
) -> Result<(), String> {
    engine
        .terminal_write(&workspace_id, &conversation_id, data.into_bytes())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pty_resize(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    conversation_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    engine
        .terminal_resize(&workspace_id, &conversation_id, cols, rows)
        .await
        .map_err(|e| e.to_string())
}
