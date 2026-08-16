use modula_client::ModulaClient;
use modula_types::WorkspaceEvent;
use tauri::ipc::Channel as IpcChannel;
use tauri::State;
use tokio_stream::StreamExt;

/// Live workspace event watch. Forwards every typed `WorkspaceEvent` to
/// `on_event` so the frontend drives TanStack Query invalidation off the stream
/// instead of timer-polling. Dropping the frontend channel ends the watch.
#[tauri::command]
pub async fn event_watch(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    after_seq: i64,
    on_event: IpcChannel<WorkspaceEvent>,
) -> Result<(), String> {
    let mut stream = std::pin::pin!(engine.watch_events(&workspace_id, after_seq).await?);
    while let Some(event) = stream.next().await {
        if on_event.send(event?).is_err() {
            break;
        }
    }
    Ok(())
}
