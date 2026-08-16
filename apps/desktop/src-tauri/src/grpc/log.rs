use modula_client::ModulaClient;
use tauri::ipc::Channel as IpcChannel;
use tauri::State;
use tokio_stream::StreamExt;

/// Tail a run log file, forwarding each chunk to the frontend over `on_chunk`.
/// The client drains existing lines then follows; when the frontend channel is
/// gone (webview reload/navigation) the forward fails and we drop the stream,
/// which detaches from the engine without affecting the run.
#[tauri::command]
pub async fn log_stream(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    log_name: String,
    on_chunk: IpcChannel<String>,
) -> Result<(), String> {
    let mut stream = std::pin::pin!(engine.stream_log(&workspace_id, &log_name).await?);
    while let Some(chunk) = stream.next().await {
        if on_chunk.send(chunk?).is_err() {
            break;
        }
    }
    Ok(())
}
