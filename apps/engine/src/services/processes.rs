//! Discover + kill running spawned agents. Live PIDs are tracked in
//! `agent_processes` (inserted by `spawn`, reaped by the dispatcher).
//! The dashboard's "running" panel and the kill endpoint both consume
//! that table — no OS-wide process scan.

use modula_db::agent_processes::{AgentProcessRepository, RunningAgentRecord};
use modula_db::Database;
use serde_json::{json, Value as JsonValue};

use crate::core::error::{ApiError, ApiResult};
use crate::platform;
use crate::services::loop_registry::LoopRegistry;

/// A live spawned agent process, resolved from `agent_processes` + its run's
/// args. Richer than the wire `RunningAgent`: the snapshot JSON also carries
/// `pid` / `spec` / `branch`, so both consumers render from this one shape.
pub struct RunningAgentInfo {
    pub pid: i64,
    pub run_id: i64,
    pub agent_id: String,
    pub name: String,
    pub started_at: String,
    pub task: Option<String>,
    pub variant: Option<String>,
    pub spec: Option<String>,
    pub branch: Option<String>,
}

/// Running-agent discovery + termination as a service. Owns the process
/// repository and the loop registry it trips before signalling a kill. DI'd by
/// [`AgentService`](super::agents::AgentService) (kill/list) and
/// [`SnapshotService`](super::snapshot::SnapshotService) (dashboard panel).
#[derive(Clone)]
pub struct ProcessesService {
    pool: Database,
    agent_processes: AgentProcessRepository,
    loops: LoopRegistry,
}

impl ProcessesService {
    pub fn new(
        pool: Database,
        agent_processes: AgentProcessRepository,
        loops: LoopRegistry,
    ) -> Self {
        Self {
            pool,
            agent_processes,
            loops,
        }
    }

    pub async fn running_agents(&self, ws_id: &str) -> Vec<RunningAgentInfo> {
        self.agent_processes
            .list_running_for_workspace(&self.pool, ws_id)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(row_to_info)
            .collect()
    }

    pub async fn list_running(&self, ws_id: &str) -> Vec<JsonValue> {
        self.running_agents(ws_id)
            .await
            .into_iter()
            .map(|a| {
                json!({
                    "pid": a.pid,
                    // The `agent_runs` row this process belongs to (run → pid).
                    "run_id": a.run_id,
                    "name": a.name,
                    "started_at": a.started_at,
                    "task": a.task,
                    "variant": a.variant,
                    "spec": a.spec,
                    "branch": a.branch,
                })
            })
            .collect()
    }

    pub async fn kill(&self, ws_id: &str, pid: i32, escalate: bool) -> ApiResult<JsonValue> {
        if !self
            .agent_processes
            .exists(&self.pool, ws_id, pid as i64)
            .await?
        {
            return Err(ApiError::NotFound(format!(
                "pid {pid} is not a running agent in this workspace"
            )));
        }
        // Trip the loop cancel flag BEFORE signalling so the loop controller
        // doesn't race the kill and spawn the next iteration.
        let loop_cancelled = self.loops.cancel(pid as u32);
        platform::process_manager()
            .kill_tree(pid as u32, escalate)
            .map_err(|e| ApiError::Internal(format!("kill_tree: {e}")))?;
        Ok(json!({
            "pid": pid,
            "signal": if escalate { "SIGKILL" } else { "SIGTERM" },
            "loop_cancelled": loop_cancelled,
        }))
    }
}

fn row_to_info(r: RunningAgentRecord) -> RunningAgentInfo {
    let args = serde_json::from_str::<JsonValue>(&r.data)
        .ok()
        .and_then(|v| v.get("args").cloned())
        .unwrap_or(JsonValue::Null);
    // Agent flags are kebab-case (`--task-id`, `--variant-id`); they're
    // stored under those keys in `agent_runs.data.args`. The dashboard's
    // task panel keys off `task`, so surface both flat keys.
    let pick = |k: &str| args.get(k).and_then(|v| v.as_str()).map(str::to_string);
    RunningAgentInfo {
        pid: r.pid,
        run_id: r.agent_run_id,
        agent_id: r.agent_id,
        name: r.agent_name,
        started_at: r.started_at.unwrap_or(r.run_created_at),
        task: pick("task-id").or_else(|| pick("task")),
        variant: pick("variant-id").or_else(|| pick("variant")),
        spec: pick("spec"),
        branch: pick("branch"),
    }
}
