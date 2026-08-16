use modula_client::ModulaClient;
use modula_types::SystemTool;
use tauri::State;

#[tauri::command]
pub async fn system_tools(engine: State<'_, ModulaClient>) -> Result<Vec<SystemTool>, String> {
    Ok(engine.list_system_tools().await?)
}
