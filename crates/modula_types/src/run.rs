use modula_rpc::json::{json_to_struct, struct_to_json};
use modula_rpc::v1 as pb;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::event::{event_types, opt_i64, opt_str, str_at};

/// An `agent_runs` entry (frontend `AgentRun`). Proto `task_id`/`variant_id`
/// are surfaced flat as `task`/`variant`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentRun {
    pub id: i64,
    pub agent_id: String,
    pub agent_name: String,
    pub event_id: Option<i64>,
    pub status: String,
    pub attempts: i64,
    pub data: Value,
    pub task: Option<String>,
    pub variant: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub created_at: String,
    pub log_path: Option<String>,
    pub loop_iter: i64,
    pub loop_total: i64,
    pub loop_group_id: Option<i64>,
}

impl From<pb::AgentRun> for AgentRun {
    fn from(r: pb::AgentRun) -> Self {
        Self {
            id: r.id,
            agent_id: r.agent_id,
            agent_name: r.agent_name,
            event_id: r.event_id,
            status: r.status,
            attempts: r.attempts,
            data: r.data.map(struct_to_json).unwrap_or_else(|| json!({})),
            task: r.task_id,
            variant: r.variant_id,
            started_at: r.started_at,
            finished_at: r.finished_at,
            created_at: r.created_at,
            log_path: r.log_path,
            loop_iter: r.loop_iter,
            loop_total: r.loop_total,
            loop_group_id: r.loop_group_id,
        }
    }
}

impl From<AgentRun> for pb::AgentRun {
    fn from(r: AgentRun) -> Self {
        Self {
            id: r.id,
            agent_id: r.agent_id,
            agent_name: r.agent_name,
            event_id: r.event_id,
            status: r.status,
            attempts: r.attempts,
            data: json_to_struct(r.data),
            task_id: r.task,
            variant_id: r.variant,
            started_at: r.started_at,
            finished_at: r.finished_at,
            created_at: r.created_at,
            log_path: r.log_path,
            loop_iter: r.loop_iter,
            loop_total: r.loop_total,
            loop_group_id: r.loop_group_id,
        }
    }
}

/// One status frame from the run watch stream (`dto::run_status`). `phase` is
/// the wire string (`spawned`/`running`/`exited`/`unspecified`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunStatus {
    pub run_id: String,
    pub agent_id: String,
    pub agent_name: String,
    pub phase: String,
    pub task: Option<String>,
    pub variant: Option<String>,
    pub loop_iter: i64,
    pub loop_total: i64,
    pub updated_at: String,
}

impl RunStatus {
    /// Decode a run-lifecycle bus event (`run.spawned` / `run.exited` +
    /// schemaless JSON payload) into a status frame. Returns `None` for
    /// non-run events. The exit payload carries only `run_id` (no agent
    /// identity), so those fields come through empty on exit.
    pub fn from_parts(type_: &str, data: &Value) -> Option<Self> {
        let phase = match type_ {
            event_types::RUN_SPAWNED => "spawned",
            event_types::RUN_EXITED => "exited",
            _ => return None,
        };
        Some(Self {
            run_id: data
                .get("run_id")
                .map(|v| v.as_i64().map(|n| n.to_string()).unwrap_or_default())
                .unwrap_or_default(),
            agent_id: str_at(data, "agent_id"),
            agent_name: str_at(data, "agent_name"),
            phase: phase.to_string(),
            task: opt_str(data, "task_id"),
            variant: opt_str(data, "variant_id"),
            loop_iter: opt_i64(data, "iter"),
            loop_total: 0,
            updated_at: String::new(),
        })
    }
}

fn phase_to_str(v: i32) -> &'static str {
    match pb::RunPhase::try_from(v).unwrap_or(pb::RunPhase::Unspecified) {
        pb::RunPhase::Spawned => "spawned",
        pb::RunPhase::Running => "running",
        pb::RunPhase::Exited => "exited",
        pb::RunPhase::Unspecified => "unspecified",
    }
}

fn str_to_phase(s: &str) -> i32 {
    let phase = match s {
        "spawned" => pb::RunPhase::Spawned,
        "running" => pb::RunPhase::Running,
        "exited" => pb::RunPhase::Exited,
        _ => pb::RunPhase::Unspecified,
    };
    phase as i32
}

impl From<pb::RunStatus> for RunStatus {
    fn from(s: pb::RunStatus) -> Self {
        Self {
            run_id: s.run_id,
            agent_id: s.agent_id,
            agent_name: s.agent_name,
            phase: phase_to_str(s.phase).to_string(),
            task: s.task_id,
            variant: s.variant_id,
            loop_iter: s.loop_iter,
            loop_total: s.loop_total,
            updated_at: s.updated_at,
        }
    }
}

impl From<RunStatus> for pb::RunStatus {
    fn from(s: RunStatus) -> Self {
        Self {
            run_id: s.run_id,
            agent_id: s.agent_id,
            agent_name: s.agent_name,
            phase: str_to_phase(&s.phase),
            task_id: s.task,
            variant_id: s.variant,
            loop_iter: s.loop_iter,
            loop_total: s.loop_total,
            updated_at: s.updated_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run() -> AgentRun {
        AgentRun {
            id: 1,
            agent_id: "a1".into(),
            agent_name: "worker".into(),
            event_id: Some(5),
            status: "running".into(),
            attempts: 1,
            data: json!({"args": {"task-id": "t1"}}),
            task: Some("t1".into()),
            variant: None,
            started_at: Some("2026-01-01T00:00:00Z".into()),
            finished_at: None,
            created_at: "2026-01-01T00:00:00Z".into(),
            log_path: Some("run.log".into()),
            loop_iter: 0,
            loop_total: 1,
            loop_group_id: None,
        }
    }

    #[test]
    fn run_round_trip() {
        let d = run();
        assert_eq!(d, AgentRun::from(pb::AgentRun::from(d.clone())));
    }

    #[test]
    fn run_serde_matches_snapshot() {
        let want = json!({
            "id": 1, "agent_id": "a1", "agent_name": "worker", "event_id": 5,
            "status": "running", "attempts": 1, "data": {"args": {"task-id": "t1"}},
            "task": "t1", "variant": null, "started_at": "2026-01-01T00:00:00Z",
            "finished_at": null, "created_at": "2026-01-01T00:00:00Z", "log_path": "run.log",
            "loop_iter": 0, "loop_total": 1, "loop_group_id": null,
        });
        assert_eq!(serde_json::to_value(run()).unwrap(), want);
    }

    #[test]
    fn run_status_round_trip_and_serde() {
        let d = RunStatus {
            run_id: "r1".into(),
            agent_id: "a1".into(),
            agent_name: "worker".into(),
            phase: "running".into(),
            task: Some("t1".into()),
            variant: Some("v1".into()),
            loop_iter: 1,
            loop_total: 3,
            updated_at: "2026-01-01T00:00:00Z".into(),
        };
        assert_eq!(d, RunStatus::from(pb::RunStatus::from(d.clone())));
        let want = json!({
            "run_id": "r1", "agent_id": "a1", "agent_name": "worker", "phase": "running",
            "task": "t1", "variant": "v1", "loop_iter": 1, "loop_total": 3,
            "updated_at": "2026-01-01T00:00:00Z",
        });
        assert_eq!(serde_json::to_value(d).unwrap(), want);
    }

    #[test]
    fn run_status_from_parts_spawned() {
        let data = json!({"run_id": 5, "agent_id": "A1", "agent_name": "worker", "iter": 2});
        let s = RunStatus::from_parts(event_types::RUN_SPAWNED, &data).unwrap();
        assert_eq!(s.run_id, "5");
        assert_eq!(s.agent_id, "A1");
        assert_eq!(s.phase, "spawned");
        assert_eq!(s.loop_iter, 2);
        assert_eq!(pb::RunStatus::from(s).phase, pb::RunPhase::Spawned as i32);
    }

    #[test]
    fn run_status_from_parts_exited_has_empty_agent() {
        let s = RunStatus::from_parts(event_types::RUN_EXITED, &json!({"run_id": 9, "pid": 100}))
            .unwrap();
        assert_eq!(s.run_id, "9");
        assert!(s.agent_id.is_empty());
        assert_eq!(s.phase, "exited");
    }

    #[test]
    fn run_status_from_parts_skips_non_run_events() {
        assert!(RunStatus::from_parts(event_types::TASK_UPDATE, &json!({})).is_none());
    }
}
