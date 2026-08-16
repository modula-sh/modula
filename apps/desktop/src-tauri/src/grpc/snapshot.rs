use modula_client::ModulaClient;
use serde_json::Value;
use tauri::State;

/// Unary fetch of the full workspace snapshot. The engine returns the assembled
/// document as JSON; the frontend `SnapshotContext` consumes it directly and
/// refetches on live `EventService` events instead of polling a stream.
#[tauri::command]
pub async fn snapshot_get(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
) -> Result<Value, String> {
    Ok(engine.get_snapshot(&workspace_id).await?)
}
