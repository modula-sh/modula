//! Fixture builders shared by every integration binary — the engine's own and
//! a plugin's, which live in a different workspace and so cannot reach a
//! `tests/common` module.

use crate::Harness;
use anyhow::Result;
use modula_rpc::json::struct_to_json;
use modula_rpc::v1::{
    CreateAgentRequest, CreateProviderRequest, CreateTaskRequest, CreateVariantsRequest,
    GetSnapshotRequest, ListEventsRequest, ListTasksRequest, Task,
};
use serde_json::{json, Value as Json};

/// Create a workspace with the given display name; returns the UUID id.
pub async fn fresh_workspace(h: &Harness, name: &str) -> Result<String> {
    h.create_workspace(name).await
}

/// Create a provider; returns the provider UUID.
pub async fn create_provider(
    h: &Harness,
    ws_id: &str,
    name: &str,
    config_dir: &std::path::Path,
) -> Result<String> {
    let resp = h
        .providers()
        .create(CreateProviderRequest {
            workspace_id: ws_id.to_string(),
            name: name.to_string(),
            r#type: String::new(),
            config_dir: config_dir.to_string_lossy().to_string(),
            description: None,
            mcp_servers: vec![],
        })
        .await?
        .into_inner();
    Ok(resp.id)
}

/// Create an agent; returns the agent UUID.
pub async fn create_agent(
    h: &Harness,
    ws_id: &str,
    provider_id: &str,
    name: &str,
    rules: &[&str],
    manual: bool,
) -> Result<String> {
    let resp = h
        .agents()
        .create(CreateAgentRequest {
            workspace_id: ws_id.to_string(),
            name: name.to_string(),
            description: name.to_string(),
            provider_id: provider_id.to_string(),
            model: None,
            manual,
            schedule: None,
            rules: rules.iter().map(|r| r.to_string()).collect(),
            args: vec![],
            prompt: "test".to_string(),
            spawn_per_variant: false,
            skills: vec![],
        })
        .await?
        .into_inner();
    Ok(resp.id)
}

/// Create a task; returns the task UUID.
pub async fn create_task(h: &Harness, ws_id: &str, title: &str) -> Result<String> {
    let resp = h
        .tasks()
        .create(CreateTaskRequest {
            workspace_id: ws_id.to_string(),
            title: title.to_string(),
            description: None,
            approved: None,
            max_variants: None,
            worktree: None,
            source_data: None,
        })
        .await?
        .into_inner();
    Ok(resp.id)
}

/// Create N variants for a task; returns vec of (uuid, position).
pub async fn create_variants(
    h: &Harness,
    ws_id: &str,
    task_id: &str,
    count: u32,
) -> Result<Vec<(String, i64)>> {
    let resp = h
        .variants()
        .create(CreateVariantsRequest {
            workspace_id: ws_id.to_string(),
            task_id: task_id.to_string(),
            count,
        })
        .await?
        .into_inner();
    Ok(resp
        .created
        .into_iter()
        .map(|v| (v.id, v.position))
        .collect())
}

/// A single task by UUID — TaskService exposes only List, so this filters it.
pub async fn get_task(h: &Harness, ws: &str, task_id: &str) -> Result<Task> {
    h.tasks()
        .list(ListTasksRequest {
            workspace_id: ws.to_string(),
        })
        .await?
        .into_inner()
        .tasks
        .into_iter()
        .find(|t| t.id == task_id)
        .ok_or_else(|| anyhow::anyhow!("task {task_id} not found"))
}

/// The assembled workspace snapshot as JSON, via the unary `SnapshotService.Get`.
pub async fn snapshot(h: &Harness, ws: &str) -> Result<Json> {
    let bytes = h
        .snapshots()
        .get(GetSnapshotRequest {
            workspace_id: ws.to_string(),
        })
        .await?
        .into_inner()
        .snapshot_json;
    Ok(serde_json::from_slice(&bytes)?)
}

/// All DB events for a workspace as `(type, data)` pairs, with the schemaless
/// `data` Struct decoded to a `serde_json::Value` — mirrors the old REST
/// `/events` payload so event assertions stay terse.
pub async fn list_events(h: &Harness, ws: &str) -> Result<Vec<(String, Json)>> {
    let events = h
        .events()
        .list(ListEventsRequest {
            workspace_id: ws.to_string(),
        })
        .await?
        .into_inner()
        .events;
    Ok(events
        .into_iter()
        .map(|e| {
            (
                e.r#type,
                e.data.map(struct_to_json).unwrap_or_else(|| json!({})),
            )
        })
        .collect())
}
