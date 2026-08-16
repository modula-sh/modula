use modula_client::{ModulaClient, SetRoadmapStatus};
use tauri::State;

#[tauri::command]
pub async fn roadmap_set_status(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    task_id: String,
    status: String,
) -> Result<(), String> {
    engine
        .set_roadmap_status(SetRoadmapStatus {
            workspace_id,
            task_id,
            status,
            depends_on: Vec::new(),
            notes: None,
        })
        .await?;
    Ok(())
}
