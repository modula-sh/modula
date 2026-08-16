use modula_client::ModulaClient;
use modula_types::UsageEntry;
use tauri::State;

#[tauri::command]
pub async fn usage_get(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
) -> Result<Vec<UsageEntry>, String> {
    Ok(engine.get_usage(&workspace_id).await?)
}
