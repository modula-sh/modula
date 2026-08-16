use modula_client::{
    CreateTask, CreatedTask, ModulaClient, ResetOutcome, UpdateTask, UpsertOutcome, UpsertTask,
};
use modula_types::{Task, TaskAgentSetting};
use serde_json::Value;
use tauri::State;

#[tauri::command]
pub async fn task_list(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
) -> Result<Vec<Task>, String> {
    Ok(engine.list_tasks(&workspace_id).await?)
}

#[tauri::command]
pub async fn task_create(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    title: String,
    description: String,
) -> Result<CreatedTask, String> {
    Ok(engine
        .create_task(CreateTask {
            workspace_id,
            title,
            description: Some(description),
            approved: None,
            max_variants: None,
            worktree: None,
            source_data: None,
        })
        .await?)
}

/// External upsert for the import flow; the engine defaults `synced_at` to
/// today and dedups on `(source, external_id)`.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn task_upsert(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    source: String,
    external_id: String,
    title: String,
    description: Option<String>,
    source_data: Option<Value>,
    url: Option<String>,
) -> Result<UpsertOutcome, String> {
    Ok(engine
        .upsert_task(UpsertTask {
            workspace_id,
            external_id,
            source,
            title,
            description,
            source_data,
            status: None,
            url,
            synced_at: None,
            approved: None,
            max_variants: None,
            worktree: None,
        })
        .await?)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn task_update(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    task_id: String,
    title: Option<String>,
    description: Option<String>,
    approved: Option<bool>,
    max_variants: Option<i64>,
    worktree: Option<bool>,
) -> Result<(), String> {
    engine
        .update_task(UpdateTask {
            workspace_id,
            task_id,
            title,
            description,
            approved,
            max_variants,
            worktree,
        })
        .await?;
    Ok(())
}

#[tauri::command]
pub async fn task_delete(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    task_id: String,
) -> Result<(), String> {
    engine.delete_task(&workspace_id, &task_id).await?;
    Ok(())
}

#[tauri::command]
pub async fn task_reset(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    task_id: String,
) -> Result<ResetOutcome, String> {
    Ok(engine.reset_task(&workspace_id, &task_id).await?)
}

#[tauri::command]
pub async fn task_agent_settings(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    task_id: String,
) -> Result<Vec<TaskAgentSetting>, String> {
    Ok(engine.list_agent_settings(&workspace_id, &task_id).await?)
}

#[tauri::command]
pub async fn task_agent_setting_set(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    task_id: String,
    agent_id: String,
    amount: i64,
) -> Result<(), String> {
    engine
        .set_agent_settings(&workspace_id, &task_id, &agent_id, amount)
        .await?;
    Ok(())
}

#[tauri::command]
pub async fn task_agent_setting_delete(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    task_id: String,
    agent_id: String,
) -> Result<(), String> {
    engine
        .delete_agent_settings(&workspace_id, &task_id, &agent_id)
        .await?;
    Ok(())
}

#[tauri::command]
pub async fn variant_update(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    task_id: String,
    variant_id: String,
    status: Option<String>,
    action: Option<String>,
) -> Result<(), String> {
    engine
        .update_variant(&workspace_id, &task_id, &variant_id, status, action)
        .await?;
    Ok(())
}
