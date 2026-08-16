use crate::config::{AgentArgDef, AgentSchedule};
use modula_rpc::v1 as pb;
use serde::{Deserialize, Serialize};

/// Full agent detail (`AgentService.Get`): the config shape plus the prompt
/// body. Matches `dto::agent` / frontend `AgentDetail`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Agent {
    pub id: String,
    pub name: String,
    pub description: String,
    pub provider_id: String,
    pub model: Option<String>,
    pub manual: bool,
    pub schedule: Option<AgentSchedule>,
    pub rules: Vec<String>,
    pub args: Vec<AgentArgDef>,
    pub next_fire: Option<String>,
    pub spawn_per_variant: bool,
    pub skills: Vec<String>,
    pub prompt: Option<String>,
}

impl From<pb::Agent> for Agent {
    fn from(a: pb::Agent) -> Self {
        Self {
            id: a.id,
            name: a.name,
            description: a.description,
            provider_id: a.provider_id,
            model: a.model,
            manual: a.manual,
            schedule: a.schedule.map(AgentSchedule::from),
            rules: a.rules,
            args: a.args.into_iter().map(AgentArgDef::from).collect(),
            next_fire: a.next_fire,
            spawn_per_variant: a.spawn_per_variant,
            skills: a.skills,
            prompt: a.prompt,
        }
    }
}

impl From<Agent> for pb::Agent {
    fn from(a: Agent) -> Self {
        Self {
            id: a.id,
            name: a.name,
            description: a.description,
            provider_id: a.provider_id,
            model: a.model,
            manual: a.manual,
            schedule: a.schedule.map(pb::AgentSchedule::from),
            rules: a.rules,
            args: a.args.into_iter().map(pb::AgentArgDef::from).collect(),
            next_fire: a.next_fire,
            spawn_per_variant: a.spawn_per_variant,
            skills: a.skills,
            prompt: a.prompt,
        }
    }
}

/// A live agent process (`dto::running_agent`). Proto `task_id`/`variant_id`
/// are surfaced flat as `task`/`variant`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunningAgent {
    pub pid: i32,
    pub agent_id: String,
    pub agent_name: String,
    pub task: Option<String>,
    pub variant: Option<String>,
    pub started_at: String,
}

impl From<pb::RunningAgent> for RunningAgent {
    fn from(a: pb::RunningAgent) -> Self {
        Self {
            pid: a.pid,
            agent_id: a.agent_id,
            agent_name: a.agent_name,
            task: a.task_id,
            variant: a.variant_id,
            started_at: a.started_at,
        }
    }
}

impl From<RunningAgent> for pb::RunningAgent {
    fn from(a: RunningAgent) -> Self {
        Self {
            pid: a.pid,
            agent_id: a.agent_id,
            agent_name: a.agent_name,
            task_id: a.task,
            variant_id: a.variant,
            started_at: a.started_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentSkill {
    pub slug: String,
    pub name: String,
    pub description: String,
    pub hidden: bool,
    pub position: i64,
}

impl From<pb::AgentSkill> for AgentSkill {
    fn from(s: pb::AgentSkill) -> Self {
        Self {
            slug: s.slug,
            name: s.name,
            description: s.description,
            hidden: s.hidden,
            position: s.position,
        }
    }
}

impl From<AgentSkill> for pb::AgentSkill {
    fn from(s: AgentSkill) -> Self {
        Self {
            slug: s.slug,
            name: s.name,
            description: s.description,
            hidden: s.hidden,
            position: s.position,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SystemTool {
    pub id: String,
    pub installed: bool,
}

impl From<pb::SystemTool> for SystemTool {
    fn from(t: pb::SystemTool) -> Self {
        Self {
            id: t.id,
            installed: t.installed,
        }
    }
}

impl From<SystemTool> for pb::SystemTool {
    fn from(t: SystemTool) -> Self {
        Self {
            id: t.id,
            installed: t.installed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn agent() -> Agent {
        Agent {
            id: "a1".into(),
            name: "worker".into(),
            description: "impl".into(),
            provider_id: "p1".into(),
            model: Some("opus".into()),
            manual: false,
            schedule: None,
            rules: vec!["r".into()],
            args: vec![],
            next_fire: None,
            spawn_per_variant: true,
            skills: vec!["s".into()],
            prompt: Some("do work".into()),
        }
    }

    #[test]
    fn agent_round_trip() {
        let d = agent();
        assert_eq!(d, Agent::from(pb::Agent::from(d.clone())));
    }

    #[test]
    fn agent_serde_matches_dto() {
        let want = json!({
            "id": "a1", "name": "worker", "description": "impl", "provider_id": "p1",
            "model": "opus", "manual": false, "schedule": null, "rules": ["r"], "args": [],
            "next_fire": null, "spawn_per_variant": true, "skills": ["s"], "prompt": "do work",
        });
        assert_eq!(serde_json::to_value(agent()).unwrap(), want);
    }

    #[test]
    fn running_agent_round_trip_and_serde() {
        let d = RunningAgent {
            pid: 42,
            agent_id: "a1".into(),
            agent_name: "worker".into(),
            task: Some("t1".into()),
            variant: None,
            started_at: "2026-01-01T00:00:00Z".into(),
        };
        assert_eq!(d, RunningAgent::from(pb::RunningAgent::from(d.clone())));
        let want = json!({
            "pid": 42, "agent_id": "a1", "agent_name": "worker",
            "task": "t1", "variant": null, "started_at": "2026-01-01T00:00:00Z",
        });
        assert_eq!(serde_json::to_value(d).unwrap(), want);
    }

    #[test]
    fn skill_and_tool_round_trip() {
        let s = AgentSkill {
            slug: "s".into(),
            name: "Skill".into(),
            description: "d".into(),
            hidden: false,
            position: 1,
        };
        assert_eq!(s, AgentSkill::from(pb::AgentSkill::from(s.clone())));
        let t = SystemTool {
            id: "git".into(),
            installed: true,
        };
        assert_eq!(t, SystemTool::from(pb::SystemTool::from(t.clone())));
    }
}
