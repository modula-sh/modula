//! The all-in-one snapshot the dashboard polls (and SSE-streams).
//!
//! `SnapshotService` is a cross-domain aggregator: it DIs the domain services
//! (`tasks`, `config`, `runs`, `conversations`, `processes`, `workspaces`) and
//! stitches their results into the single JSON blob the dashboard consumes. It
//! owns no repositories — every read flows through a peer service, so there is
//! no direct DB access here.

use std::path::PathBuf;

use chrono::{Local, SecondsFormat};
use serde_json::{json, Value as JsonValue};

use super::projects;
use crate::core::error::ApiResult;
use crate::services::agents::next_fire;
use crate::services::config::ConfigService;
use crate::services::conversations::ConversationService;
use crate::services::processes::ProcessesService;
use crate::services::runs::RunService;
use crate::services::tasks::TaskService;
use crate::services::workspaces::WorkspaceService;

/// Assembles the dashboard snapshot from the domain services. Cheaply cloneable
/// (each field is a cloneable service handle).
#[derive(Clone)]
pub struct SnapshotService {
    tasks: TaskService,
    config: ConfigService,
    runs: RunService,
    conversations: ConversationService,
    processes: ProcessesService,
    workspaces: WorkspaceService,
}

impl SnapshotService {
    pub fn new(
        tasks: TaskService,
        config: ConfigService,
        runs: RunService,
        conversations: ConversationService,
        processes: ProcessesService,
        workspaces: WorkspaceService,
    ) -> Self {
        Self {
            tasks,
            config,
            runs,
            conversations,
            processes,
            workspaces,
        }
    }

    pub async fn workspace_snapshot(&self, ws_id: &str) -> ApiResult<JsonValue> {
        // Reject unknown workspaces with 404 before any aggregation.
        self.workspaces.workspace_dir(ws_id).await?;

        let mut config = self.config.get(ws_id).await?;

        let project_paths: Vec<(String, PathBuf)> = config
            .projects
            .iter()
            .map(|p| (p.name.clone(), PathBuf::from(&p.path)))
            .collect();
        let tasks = self.tasks.list(ws_id).await?;
        let task_ids: Vec<String> = tasks.iter().map(|t| t.id.clone()).collect();
        let tp = projects::task_projects(&project_paths, &task_ids);
        let tasks_json: Vec<JsonValue> = tasks
            .into_iter()
            .map(|task| {
                let mut entry = serde_json::to_value(&task).unwrap_or_else(|_| json!({}));
                if let Some(arr) = tp.get(&task.id) {
                    if let Some(map) = entry.as_object_mut() {
                        map.insert("projects_touched".into(), arr.clone());
                    }
                }
                entry
            })
            .collect();

        let roadmap_json: Vec<JsonValue> = self
            .tasks
            .list_roadmap(ws_id)
            .await?
            .into_iter()
            .map(|r| serde_json::to_value(&r).unwrap_or_else(|_| json!({})))
            .collect();

        // Config's `serde` is the shape-locked frontend contract, so the
        // snapshot embeds it directly. The dashboard wants each agent's
        // scheduler-derived `next_fire` (which `config.get` leaves `None`), so
        // fill it here before serializing.
        for agent in &mut config.agents {
            agent.next_fire = next_fire(agent.schedule.as_ref());
        }
        let config = serde_json::to_value(&config).unwrap_or_else(|_| json!({}));
        let agents = self.processes.list_running(ws_id).await;
        let runs: Vec<JsonValue> = self
            .runs
            .list_recent(ws_id)
            .await?
            .into_iter()
            .map(|r| serde_json::to_value(&r).unwrap_or_else(|_| json!({})))
            .collect();
        let conversations: Vec<JsonValue> = self
            .conversations
            .list(ws_id)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|r| {
                json!({
                    "id": r.id,
                    "title": r.title,
                    "provider_id": r.provider_id,
                    "model": r.model,
                    "context": r.context,
                    "updated_at": r.updated_at,
                })
            })
            .collect();

        Ok(json!({
            "tasks": tasks_json,
            "roadmap": roadmap_json,
            "config": config,
            "agents": agents,
            "runs": runs,
            "conversations": conversations,
            "ts": Local::now().to_rfc3339_opts(SecondsFormat::Secs, true),
        }))
    }
}
