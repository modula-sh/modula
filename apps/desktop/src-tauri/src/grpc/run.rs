use modula_client::ModulaClient;
use modula_types::RunStatus;
use tauri::ipc::Channel as IpcChannel;
use tauri::State;
use tokio_stream::StreamExt;

/// Live run/agent status watch (spawn → running → exited). Forwards each
/// `RunStatus` to `on_status` so active-agent UI updates live instead of
/// polling. Dropping the frontend channel ends the watch.
#[tauri::command]
pub async fn run_watch(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    agent_id: Option<String>,
    on_status: IpcChannel<RunStatus>,
) -> Result<(), String> {
    let mut stream = std::pin::pin!(engine.watch_run_status(&workspace_id, agent_id).await?);
    while let Some(status) = stream.next().await {
        if on_status.send(status?).is_err() {
            break;
        }
    }
    Ok(())
}
