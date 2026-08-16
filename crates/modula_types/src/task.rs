use modula_rpc::json::{json_to_struct, struct_to_json};
use modula_rpc::v1 as pb;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Variant {
    pub id: String,
    pub status: Option<String>,
    pub position: i64,
}

impl From<pb::Variant> for Variant {
    fn from(v: pb::Variant) -> Self {
        Self {
            id: v.id,
            status: v.status,
            position: v.position,
        }
    }
}

impl From<Variant> for pb::Variant {
    fn from(v: Variant) -> Self {
        Self {
            id: v.id,
            status: v.status,
            position: v.position,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskLabel {
    pub id: String,
    pub name: String,
}

impl From<pb::TaskLabel> for TaskLabel {
    fn from(l: pb::TaskLabel) -> Self {
        Self {
            id: l.id,
            name: l.name,
        }
    }
}

impl From<TaskLabel> for pb::TaskLabel {
    fn from(l: TaskLabel) -> Self {
        Self {
            id: l.id,
            name: l.name,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub external_id: Option<String>,
    pub title: String,
    pub source: String,
    pub status: Option<String>,
    pub source_data: Option<Value>,
    pub url: Option<String>,
    pub approved: Option<bool>,
    pub description: String,
    pub max_variants: Option<i64>,
    pub worktree: Option<bool>,
    pub synced_at: Option<String>,
    pub created_at: Option<String>,
    pub variants: Vec<Variant>,
    pub labels: Vec<TaskLabel>,
}

impl From<pb::Task> for Task {
    fn from(t: pb::Task) -> Self {
        Self {
            id: t.id,
            external_id: t.external_id,
            title: t.title,
            source: t.source,
            status: t.status,
            source_data: t.source_data.map(struct_to_json),
            url: t.url,
            approved: t.approved,
            description: t.description,
            max_variants: t.max_variants,
            worktree: t.worktree,
            synced_at: t.synced_at,
            created_at: t.created_at,
            variants: t.variants.into_iter().map(Variant::from).collect(),
            labels: t.labels.into_iter().map(TaskLabel::from).collect(),
        }
    }
}

impl From<Task> for pb::Task {
    fn from(t: Task) -> Self {
        Self {
            id: t.id,
            external_id: t.external_id,
            title: t.title,
            source: t.source,
            status: t.status,
            source_data: t.source_data.and_then(json_to_struct),
            url: t.url,
            approved: t.approved,
            description: t.description,
            max_variants: t.max_variants,
            worktree: t.worktree,
            synced_at: t.synced_at,
            created_at: t.created_at,
            variants: t.variants.into_iter().map(pb::Variant::from).collect(),
            labels: t.labels.into_iter().map(pb::TaskLabel::from).collect(),
        }
    }
}

/// Per-task agent loop setting. Serialized as `{type: "fixed", amount}` — the
/// only loop kind today; the proto carries just the amount.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentLoop {
    #[serde(rename = "type")]
    pub kind: String,
    pub amount: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskAgentSetting {
    pub agent_id: String,
    #[serde(rename = "loop")]
    pub loop_setting: AgentLoop,
}

impl From<pb::TaskAgentSetting> for TaskAgentSetting {
    fn from(s: pb::TaskAgentSetting) -> Self {
        Self {
            agent_id: s.agent_id,
            loop_setting: AgentLoop {
                kind: "fixed".into(),
                amount: s.loop_amount,
            },
        }
    }
}

impl From<TaskAgentSetting> for pb::TaskAgentSetting {
    fn from(s: TaskAgentSetting) -> Self {
        Self {
            agent_id: s.agent_id,
            loop_amount: s.loop_setting.amount,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample() -> Task {
        Task {
            id: "t1".into(),
            external_id: Some("MOD-1".into()),
            title: "Title".into(),
            source: "internal".into(),
            status: Some("planning".into()),
            source_data: Some(json!({"key": "value"})),
            url: None,
            approved: Some(true),
            description: "desc".into(),
            max_variants: Some(2),
            worktree: None,
            synced_at: Some("2026-01-01".into()),
            created_at: Some("2026-01-01T00:00:00Z".into()),
            variants: vec![Variant {
                id: "v1".into(),
                status: Some("in_progress".into()),
                position: 1,
            }],
            labels: vec![TaskLabel {
                id: "l1".into(),
                name: "refactor".into(),
            }],
        }
    }

    #[test]
    fn task_round_trip() {
        let d = sample();
        assert_eq!(d, Task::from(pb::Task::from(d.clone())));
    }

    #[test]
    fn task_setting_round_trip() {
        let d = TaskAgentSetting {
            agent_id: "a1".into(),
            loop_setting: AgentLoop {
                kind: "fixed".into(),
                amount: 3,
            },
        };
        assert_eq!(
            d,
            TaskAgentSetting::from(pb::TaskAgentSetting::from(d.clone()))
        );
    }

    // Locks the JSON the frontend (`types.ts` Task) consumes today via `dto::task`.
    #[test]
    fn task_serde_matches_dto() {
        let got = serde_json::to_value(sample()).unwrap();
        let want = json!({
            "id": "t1",
            "external_id": "MOD-1",
            "title": "Title",
            "source": "internal",
            "status": "planning",
            "source_data": {"key": "value"},
            "url": null,
            "approved": true,
            "description": "desc",
            "max_variants": 2,
            "worktree": null,
            "synced_at": "2026-01-01",
            "created_at": "2026-01-01T00:00:00Z",
            "variants": [{"id": "v1", "status": "in_progress", "position": 1}],
            "labels": [{"id": "l1", "name": "refactor"}],
        });
        assert_eq!(got, want);
    }

    #[test]
    fn agent_setting_serde_matches_dto() {
        let d = TaskAgentSetting {
            agent_id: "a1".into(),
            loop_setting: AgentLoop {
                kind: "fixed".into(),
                amount: 5,
            },
        };
        let want = json!({"agent_id": "a1", "loop": {"type": "fixed", "amount": 5}});
        assert_eq!(serde_json::to_value(d).unwrap(), want);
    }
}
