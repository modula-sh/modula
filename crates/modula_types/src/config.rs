use modula_rpc::v1 as pb;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfigLimits {
    pub max_spawns_per_run: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PipelineStatus {
    pub key: String,
    pub label: String,
    pub tone: String,
    pub station: Option<String>,
    pub terminal: bool,
    pub error: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfigProvider {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub r#type: String,
    pub config_dir: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfigProject {
    pub id: String,
    pub name: String,
    pub path: String,
    pub base_branch: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentSchedule {
    pub cron: String,
    pub timezone: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentArgDef {
    pub flag: String,
    pub required: bool,
    pub help: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfigAgent {
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
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    pub limits: ConfigLimits,
    pub pipeline: Vec<PipelineStatus>,
    pub providers: Vec<ConfigProvider>,
    pub projects: Vec<ConfigProject>,
    pub agents: Vec<ConfigAgent>,
}

impl From<pb::ConfigLimits> for ConfigLimits {
    fn from(l: pb::ConfigLimits) -> Self {
        Self {
            max_spawns_per_run: l.max_spawns_per_run,
        }
    }
}

impl From<ConfigLimits> for pb::ConfigLimits {
    fn from(l: ConfigLimits) -> Self {
        Self {
            max_spawns_per_run: l.max_spawns_per_run,
        }
    }
}

impl From<pb::PipelineStatus> for PipelineStatus {
    fn from(p: pb::PipelineStatus) -> Self {
        Self {
            key: p.key,
            label: p.label,
            tone: p.tone,
            station: p.station,
            terminal: p.terminal,
            error: p.error,
        }
    }
}

impl From<PipelineStatus> for pb::PipelineStatus {
    fn from(p: PipelineStatus) -> Self {
        Self {
            key: p.key,
            label: p.label,
            tone: p.tone,
            station: p.station,
            terminal: p.terminal,
            error: p.error,
        }
    }
}

impl From<pb::ConfigProvider> for ConfigProvider {
    fn from(p: pb::ConfigProvider) -> Self {
        Self {
            id: p.id,
            name: p.name,
            r#type: p.r#type,
            config_dir: p.config_dir,
            description: p.description,
        }
    }
}

impl From<ConfigProvider> for pb::ConfigProvider {
    fn from(p: ConfigProvider) -> Self {
        Self {
            id: p.id,
            name: p.name,
            r#type: p.r#type,
            config_dir: p.config_dir,
            description: p.description,
        }
    }
}

/// Project the full [`Provider`](crate::Provider) domain model onto its config
/// shape, dropping the per-detail enrichment (`config_dir_exists`, `mcp_*`,
/// `agents_using`) the config surface never exposes.
impl From<crate::Provider> for ConfigProvider {
    fn from(p: crate::Provider) -> Self {
        Self {
            id: p.id,
            name: p.name,
            r#type: p.r#type,
            config_dir: p.config_dir,
            description: p.description,
        }
    }
}

impl From<pb::ConfigProject> for ConfigProject {
    fn from(p: pb::ConfigProject) -> Self {
        Self {
            id: p.id,
            name: p.name,
            path: p.path,
            base_branch: p.base_branch,
        }
    }
}

/// Project the full [`Project`](crate::Project) domain model onto its config
/// shape, dropping the on-disk enrichment (`exists`, `worktrees`) the config
/// surface never exposes.
impl From<crate::Project> for ConfigProject {
    fn from(p: crate::Project) -> Self {
        Self {
            id: p.id,
            name: p.name,
            path: p.path,
            base_branch: p.base_branch,
        }
    }
}

impl From<ConfigProject> for pb::ConfigProject {
    fn from(p: ConfigProject) -> Self {
        Self {
            id: p.id,
            name: p.name,
            path: p.path,
            base_branch: p.base_branch,
        }
    }
}

impl From<pb::AgentSchedule> for AgentSchedule {
    fn from(s: pb::AgentSchedule) -> Self {
        Self {
            cron: s.cron,
            timezone: s.timezone,
            enabled: s.enabled,
        }
    }
}

impl From<AgentSchedule> for pb::AgentSchedule {
    fn from(s: AgentSchedule) -> Self {
        Self {
            cron: s.cron,
            timezone: s.timezone,
            enabled: s.enabled,
        }
    }
}

impl From<pb::AgentArgDef> for AgentArgDef {
    fn from(a: pb::AgentArgDef) -> Self {
        Self {
            flag: a.flag,
            required: a.required,
            help: a.help,
        }
    }
}

impl From<AgentArgDef> for pb::AgentArgDef {
    fn from(a: AgentArgDef) -> Self {
        Self {
            flag: a.flag,
            required: a.required,
            help: a.help,
        }
    }
}

/// Project the full [`Agent`](crate::Agent) domain model onto its config shape,
/// dropping the `prompt` body the config surface never exposes.
impl From<crate::Agent> for ConfigAgent {
    fn from(a: crate::Agent) -> Self {
        Self {
            id: a.id,
            name: a.name,
            description: a.description,
            provider_id: a.provider_id,
            model: a.model,
            manual: a.manual,
            schedule: a.schedule,
            rules: a.rules,
            args: a.args,
            next_fire: a.next_fire,
            spawn_per_variant: a.spawn_per_variant,
            skills: a.skills,
        }
    }
}

impl From<pb::ConfigAgent> for ConfigAgent {
    fn from(a: pb::ConfigAgent) -> Self {
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
        }
    }
}

impl From<ConfigAgent> for pb::ConfigAgent {
    fn from(a: ConfigAgent) -> Self {
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
        }
    }
}

impl From<pb::WorkspaceConfig> for WorkspaceConfig {
    fn from(c: pb::WorkspaceConfig) -> Self {
        Self {
            limits: c.limits.map(ConfigLimits::from).unwrap_or(ConfigLimits {
                max_spawns_per_run: 0,
            }),
            pipeline: c.pipeline.into_iter().map(PipelineStatus::from).collect(),
            providers: c.providers.into_iter().map(ConfigProvider::from).collect(),
            projects: c.projects.into_iter().map(ConfigProject::from).collect(),
            agents: c.agents.into_iter().map(ConfigAgent::from).collect(),
        }
    }
}

impl From<WorkspaceConfig> for pb::WorkspaceConfig {
    fn from(c: WorkspaceConfig) -> Self {
        Self {
            limits: Some(c.limits.into()),
            pipeline: c
                .pipeline
                .into_iter()
                .map(pb::PipelineStatus::from)
                .collect(),
            providers: c
                .providers
                .into_iter()
                .map(pb::ConfigProvider::from)
                .collect(),
            projects: c
                .projects
                .into_iter()
                .map(pb::ConfigProject::from)
                .collect(),
            agents: c.agents.into_iter().map(pb::ConfigAgent::from).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample() -> WorkspaceConfig {
        WorkspaceConfig {
            limits: ConfigLimits {
                max_spawns_per_run: 5,
            },
            pipeline: vec![PipelineStatus {
                key: "planning".into(),
                label: "Planning".into(),
                tone: "zinc".into(),
                station: Some("PLAN".into()),
                terminal: false,
                error: false,
            }],
            providers: vec![ConfigProvider {
                id: "p1".into(),
                name: "Claude".into(),
                r#type: "claude".into(),
                config_dir: "/c".into(),
                description: None,
            }],
            projects: vec![ConfigProject {
                id: "pr1".into(),
                name: "Modula".into(),
                path: "/m".into(),
                base_branch: "main".into(),
            }],
            agents: vec![ConfigAgent {
                id: "a1".into(),
                name: "worker".into(),
                description: "impl".into(),
                provider_id: "p1".into(),
                model: None,
                manual: false,
                schedule: None,
                rules: vec!["r".into()],
                args: vec![AgentArgDef {
                    flag: "task-id".into(),
                    required: true,
                    help: None,
                }],
                next_fire: None,
                spawn_per_variant: true,
                skills: vec![],
            }],
        }
    }

    #[test]
    fn config_round_trip() {
        let d = sample();
        assert_eq!(
            d,
            WorkspaceConfig::from(pb::WorkspaceConfig::from(d.clone()))
        );
    }

    // Locks the JSON the frontend (`WorkspaceConfig`) consumes via `dto::config`.
    #[test]
    fn config_serde_matches_dto() {
        let got = serde_json::to_value(sample()).unwrap();
        let want = json!({
            "limits": {"max_spawns_per_run": 5},
            "pipeline": [{
                "key": "planning", "label": "Planning", "tone": "zinc",
                "station": "PLAN", "terminal": false, "error": false,
            }],
            "providers": [{
                "id": "p1", "name": "Claude", "type": "claude",
                "config_dir": "/c", "description": null,
            }],
            "projects": [{"id": "pr1", "name": "Modula", "path": "/m", "base_branch": "main"}],
            "agents": [{
                "id": "a1", "name": "worker", "description": "impl", "provider_id": "p1",
                "model": null, "manual": false, "schedule": null, "rules": ["r"],
                "args": [{"flag": "task-id", "required": true, "help": null}],
                "next_fire": null, "spawn_per_variant": true, "skills": [],
            }],
        });
        assert_eq!(got, want);
    }
}
