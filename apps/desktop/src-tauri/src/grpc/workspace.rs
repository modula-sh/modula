use modula_client::{CreatedWorkspace, ModulaClient};
use modula_types::Workspace;
use tauri::State;

#[tauri::command]
pub async fn workspace_list(engine: State<'_, ModulaClient>) -> Result<Vec<Workspace>, String> {
    Ok(engine.list_workspaces().await?)
}

#[tauri::command]
pub async fn workspace_create(
    engine: State<'_, ModulaClient>,
    name: String,
    description: Option<String>,
) -> Result<CreatedWorkspace, String> {
    Ok(engine.create_workspace(&name, description).await?)
}
