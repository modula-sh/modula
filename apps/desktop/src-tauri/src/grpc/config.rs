use modula_client::ModulaClient;
use modula_types::WorkspaceConfig;
use tauri::State;

#[tauri::command]
pub async fn config_get(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
) -> Result<WorkspaceConfig, String> {
    Ok(engine.get_config(&workspace_id).await?)
}
