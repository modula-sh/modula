use modula_client::ModulaClient;
use modula_types::OverviewWorkspace;
use tauri::State;

#[tauri::command]
pub async fn overview_get(
    engine: State<'_, ModulaClient>,
) -> Result<Vec<OverviewWorkspace>, String> {
    Ok(engine.overview().await?)
}
