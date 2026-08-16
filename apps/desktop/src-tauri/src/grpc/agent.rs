use modula_client::{CreatedAgent, KillOutcome, ModulaClient, TriggeredAgent, WriteAgent};
use modula_types::{Agent, AgentArgDef, AgentSchedule, AgentSkill, RunningAgent};
use serde::Deserialize;
use serde_json::{json, Value};
use tauri::State;

/// The agent create/update form body (mirrors the frontend `AgentWriteBody`).
/// `name` is present on create and omitted on update (the engine can't rename).
#[derive(Deserialize)]
pub struct AgentWriteInput {
    name: Option<String>,
    description: String,
    provider_id: String,
    model: Option<String>,
    manual: bool,
    schedule: Option<AgentSchedule>,
    rules: Vec<String>,
    args: Vec<AgentArgDef>,
    prompt: String,
    spawn_per_variant: bool,
    skills: Vec<String>,
}

impl From<AgentWriteInput> for WriteAgent {
    fn from(b: AgentWriteInput) -> Self {
        WriteAgent {
            name: b.name,
            description: b.description,
            provider_id: b.provider_id,
            model: b.model,
            manual: b.manual,
            schedule: b.schedule,
            rules: b.rules,
            args: b.args,
            prompt: b.prompt,
            spawn_per_variant: b.spawn_per_variant,
            skills: b.skills,
        }
    }
}

#[tauri::command]
pub async fn agent_list_running(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
) -> Result<Vec<RunningAgent>, String> {
    Ok(engine.list_running_agents(&workspace_id).await?)
}

#[tauri::command]
pub async fn agent_get(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    agent_id: String,
) -> Result<Agent, String> {
    Ok(engine.get_agent(&workspace_id, &agent_id).await?)
}

#[tauri::command]
pub async fn agent_config(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
) -> Result<Vec<Agent>, String> {
    Ok(engine.agent_config(&workspace_id).await?)
}

#[tauri::command]
pub async fn agent_skills(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
) -> Result<Vec<AgentSkill>, String> {
    Ok(engine.list_skills(&workspace_id).await?)
}

#[tauri::command]
pub async fn agent_create(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    body: AgentWriteInput,
) -> Result<CreatedAgent, String> {
    Ok(engine.create_agent(&workspace_id, body.into()).await?)
}

#[tauri::command]
pub async fn agent_update(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    agent_id: String,
    body: AgentWriteInput,
) -> Result<Value, String> {
    let id = engine
        .update_agent(&workspace_id, &agent_id, body.into())
        .await?;
    Ok(json!({ "id": id }))
}

#[tauri::command]
pub async fn agent_delete(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    agent_id: String,
) -> Result<(), String> {
    engine.delete_agent(&workspace_id, &agent_id).await?;
    Ok(())
}

#[tauri::command]
pub async fn agent_trigger(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    agent_id: String,
    args: Option<Value>,
) -> Result<TriggeredAgent, String> {
    Ok(engine.trigger_agent(&workspace_id, &agent_id, args).await?)
}

#[tauri::command]
pub async fn agent_kill(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    pid: i32,
    escalate: bool,
) -> Result<KillOutcome, String> {
    Ok(engine.kill_agent(&workspace_id, pid, escalate).await?)
}
