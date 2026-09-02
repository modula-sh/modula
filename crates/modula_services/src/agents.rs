//! Agent CRUD + manual trigger + skill catalog as a typed service layer.
//! `AgentService` owns its repositories and the scheduler handle; gRPC
//! handlers are thin adapters over it. Validation and the schedule/arg
//! normalization live here, off the edge.

use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;
use std::sync::Arc;

use chrono::SecondsFormat;
use chrono_tz::Tz;
use cron::Schedule;
use modula_db::agent_skills::AgentSkillRepository;
use modula_db::agents::ScheduleFields;
use modula_types::{Agent, AgentArgDef, AgentSchedule, AgentSkill, RunningAgent};
use serde_json::{json, Value as JsonValue};
use sqlx::{Executor, Sqlite};

use crate::dispatcher::expr;
use crate::events::{self, EventSink};
use crate::loop_registry::LoopRegistry;
use crate::processes::ProcessesService;
use crate::scheduler::SchedulerHandle;
use crate::spawn::{self, SpawnParams};
use crate::workspaces::WorkspaceService;
use modula_core::error::{ApiError, ApiResult};
use modula_core::repositories::Repositories;
use modula_core::validation::ARG_FLAG_RE;

/// Arg-definition input: proto `AgentArgDef` maps into this before
/// validation.
pub struct ArgInput {
    pub flag: String,
    pub required: bool,
    pub help: Option<String>,
}

/// Transport-neutral `{cron, timezone, enabled}` schedule block.
pub struct ScheduleParam {
    pub cron: String,
    pub timezone: String,
    pub enabled: bool,
}

pub struct CreateParams {
    pub name: String,
    pub description: String,
    pub provider_id: String,
    pub model: Option<String>,
    pub manual: bool,
    pub schedule: Option<ScheduleParam>,
    pub rules: Vec<String>,
    pub args: Vec<ArgInput>,
    pub prompt: Option<String>,
    pub spawn_per_variant: bool,
    pub skills: Vec<String>,
}

/// `field: Some(None)` clears an optional field; `None` leaves it unchanged.
#[derive(Default)]
pub struct UpdateParams {
    pub description: Option<String>,
    pub provider_id: Option<String>,
    pub model: Option<Option<String>>,
    pub manual: Option<bool>,
    pub schedule: Option<Option<ScheduleParam>>,
    pub rules: Option<Vec<String>>,
    pub args: Option<Vec<ArgInput>>,
    pub prompt: Option<String>,
    pub spawn_per_variant: Option<bool>,
    pub skills: Option<Vec<String>>,
}

pub struct CreatedAgent {
    pub id: String,
    pub name: String,
}

pub struct TriggerResult {
    pub id: String,
    pub name: String,
    pub pid: u32,
    pub args: Vec<String>,
}

/// Agent CRUD, manual trigger, and running-process control as a service. Owns
/// the repositories plus the engine runtime handles (`scheduler`, `loops`,
/// `events`, `engine_socket`) its business methods need: mutating CRUD reconfigures
/// the cron scheduler; `trigger` reaches the spawn runtime. It DIs
/// [`WorkspaceService`] (workspace-dir resolution) and [`ProcessesService`]
/// (running-agent list + kill) so that logic lives in one place.
#[derive(Clone)]
pub struct AgentService {
    repos: Repositories,
    scheduler: SchedulerHandle,
    loops: LoopRegistry,
    events: Arc<dyn EventSink>,
    engine_socket: String,
    processes: ProcessesService,
    workspaces: WorkspaceService,
}

impl AgentService {
    pub fn new(
        repos: Repositories,
        scheduler: SchedulerHandle,
        loops: LoopRegistry,
        events: Arc<dyn EventSink>,
        engine_socket: String,
        processes: ProcessesService,
        workspaces: WorkspaceService,
    ) -> Self {
        Self {
            repos,
            scheduler,
            loops,
            events,
            engine_socket,
            processes,
            workspaces,
        }
    }

    /// Fetch one agent, enriched with its scheduler-derived `next_fire`. 404s
    /// when it doesn't exist in the workspace.
    pub async fn get(&self, ws: &str, id: &str) -> ApiResult<Agent> {
        let mut agent = self.repos.agents.get(&self.repos.pool, ws, id).await?;
        agent.next_fire = next_fire(agent.schedule.as_ref());
        Ok(agent)
    }

    /// Every agent in the workspace, each enriched with its `next_fire`.
    pub async fn list(&self, ws: &str) -> ApiResult<Vec<Agent>> {
        let mut agents = self.repos.agents.list(&self.repos.pool, ws).await?;
        for agent in &mut agents {
            agent.next_fire = next_fire(agent.schedule.as_ref());
        }
        Ok(agents)
    }

    /// The workspace's skill catalog (including hidden skills).
    pub async fn list_skills(&self, ws: &str) -> ApiResult<Vec<AgentSkill>> {
        Ok(self.repos.agent_skills.list(&self.repos.pool, ws).await?)
    }

    /// Live spawned agent processes in the workspace.
    pub async fn list_running(&self, ws: &str) -> Vec<RunningAgent> {
        self.processes
            .running_agents(ws)
            .await
            .into_iter()
            .map(|a| RunningAgent {
                pid: a.pid as i32,
                agent_id: a.agent_id,
                agent_name: a.name,
                task: a.task,
                variant: a.variant,
                started_at: a.started_at,
            })
            .collect()
    }

    /// Manually fire a `manual=true` agent. `raw_args` is a JSON object of
    /// `{key: value}` pairs keyed by flag name (without the leading `--`).
    pub async fn trigger(
        &self,
        ws: &str,
        id: &str,
        raw_args: JsonValue,
    ) -> ApiResult<TriggerResult> {
        let ws_dir = self.workspaces.workspace_dir(ws).await?;
        let agent = self.repos.agents.get(&self.repos.pool, ws, id).await?;
        if !agent.manual {
            return Err(ApiError::Forbidden(format!(
                "agent {:?} is not manually triggerable",
                agent.name
            )));
        }
        let arg_map = build_trigger_args(&agent.args, &raw_args)?;
        let spawned = spawn::spawn_tracked(
            &self.repos,
            &self.loops,
            SpawnParams {
                ws_id: ws.to_string(),
                ws_dir,
                agent_id: agent.id.clone(),
                agent_name: agent.name.clone(),
                arg_map: arg_map.clone(),
                engine_socket: self.engine_socket.clone(),
            },
            None,
            &json!({ "args": &arg_map, "trigger": "manual" }),
            &self.events,
        )
        .await?;
        let args = arg_map
            .iter()
            .flat_map(|(k, v)| vec![format!("--{k}"), v.clone()])
            .collect();
        Ok(TriggerResult {
            id: agent.id,
            name: agent.name,
            pid: spawned.pid,
            args,
        })
    }

    /// Kill a running spawned agent by pid, cancelling any loop it drives.
    pub async fn kill(&self, ws: &str, pid: i32, escalate: bool) -> ApiResult<JsonValue> {
        self.processes.kill(ws, pid, escalate).await
    }

    pub async fn create(&self, ws: &str, params: CreateParams) -> ApiResult<CreatedAgent> {
        let name = require_nonempty("name", &params.name)?.to_string();
        let description = require_nonempty("description", &params.description)?.to_string();
        let provider_id = require_nonempty("provider_id", &params.provider_id)?.to_string();
        let model = params
            .model
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        if let Some(s) = &params.schedule {
            validate_cron(&s.cron)?;
        }
        let args_json = validate_args(&params.args)?;
        let rules_json = validate_rules(&params.rules)?;
        let prompt = params
            .prompt
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| ApiError::BadRequest("prompt is required for new agents".into()))?;

        if !self
            .repos
            .agents
            .provider_exists(&self.repos.pool, ws, &provider_id)
            .await?
        {
            return Err(ApiError::BadRequest(format!(
                "provider_id {provider_id:?} is not defined in this workspace"
            )));
        }
        let skills_json = validate_skills(
            &self.repos.pool,
            &self.repos.agent_skills,
            ws,
            &params.skills,
        )
        .await?;

        let (sched_cron, sched_tz, sched_enabled) = match &params.schedule {
            Some(s) => (
                Some(s.cron.trim().to_string()),
                Some(default_tz(&s.timezone)),
                s.enabled,
            ),
            None => (None, None, false),
        };

        let id = self
            .repos
            .agents
            .create(
                &self.repos.pool,
                ws,
                &name,
                &description,
                &provider_id,
                model,
                sched_cron.as_deref(),
                sched_tz.as_deref(),
                sched_enabled,
                params.manual,
                &rules_json,
                &args_json,
                prompt,
                params.spawn_per_variant,
                &skills_json,
            )
            .await?;
        self.scheduler.reconfigure().await?;
        self.events
            .publish(ws, events::AGENT_CREATE, json!({ "agent_id": id }))
            .await;
        Ok(CreatedAgent { id, name })
    }

    pub async fn update(&self, ws: &str, id: &str, params: UpdateParams) -> ApiResult<()> {
        let description = params
            .description
            .as_deref()
            .map(|d| require_nonempty("description", d).map(str::to_string))
            .transpose()?;
        let provider_id = params
            .provider_id
            .as_deref()
            .map(|p| require_nonempty("provider_id", p).map(str::to_string))
            .transpose()?;
        if let Some(pid) = &provider_id {
            if !self
                .repos
                .agents
                .provider_exists(&self.repos.pool, ws, pid)
                .await?
            {
                return Err(ApiError::BadRequest(format!(
                    "provider_id {pid:?} is not defined in this workspace"
                )));
            }
        }
        let model = params
            .model
            .map(|m| m.and_then(|s| if s.trim().is_empty() { None } else { Some(s) }));
        let schedule = match params.schedule {
            Some(Some(s)) => {
                validate_cron(&s.cron)?;
                Some(Some(ScheduleFields {
                    cron: s.cron.trim().to_string(),
                    tz: default_tz(&s.timezone),
                    enabled: s.enabled,
                }))
            }
            Some(None) => Some(None),
            None => None,
        };
        let rules_json = params
            .rules
            .as_ref()
            .map(|r| validate_rules(r))
            .transpose()?;
        let args_json = params.args.as_ref().map(|a| validate_args(a)).transpose()?;
        let prompt = params
            .prompt
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let skills_json = match &params.skills {
            Some(s) => {
                Some(validate_skills(&self.repos.pool, &self.repos.agent_skills, ws, s).await?)
            }
            None => None,
        };

        self.repos
            .agents
            .patch(
                &self.repos.pool,
                ws,
                id,
                description.as_deref(),
                provider_id.as_deref(),
                model,
                schedule,
                params.manual,
                rules_json.as_ref(),
                args_json.as_ref(),
                prompt,
                params.spawn_per_variant,
                skills_json.as_ref(),
            )
            .await?;
        self.scheduler.reconfigure().await?;
        self.events
            .publish(ws, events::AGENT_UPDATE, json!({ "agent_id": id }))
            .await;
        Ok(())
    }

    pub async fn delete(&self, ws: &str, id: &str) -> ApiResult<()> {
        self.repos.agents.delete(&self.repos.pool, ws, id).await?;
        self.scheduler.reconfigure().await?;
        self.events
            .publish(ws, events::AGENT_DELETE, json!({ "agent_id": id }))
            .await;
        Ok(())
    }
}

pub fn require_nonempty<'a>(field: &str, v: &'a str) -> ApiResult<&'a str> {
    let t = v.trim();
    if t.is_empty() {
        return Err(ApiError::BadRequest(format!("{field} is required")));
    }
    Ok(t)
}

/// Parse the cron expression (5- or 6-field) to surface a 400 at write time
/// rather than at scheduler-reconfigure time.
pub fn validate_cron(cron: &str) -> ApiResult<()> {
    let cron = cron.trim();
    if cron.is_empty() {
        return Err(ApiError::BadRequest(
            "schedule.cron is required when schedule is set".into(),
        ));
    }
    let six_field = if cron.split_whitespace().count() == 5 {
        format!("0 {cron}")
    } else {
        cron.to_string()
    };
    Schedule::from_str(&six_field)
        .map_err(|e| ApiError::BadRequest(format!("invalid cron expression: {e}")))?;
    Ok(())
}

pub fn default_tz(tz: &str) -> String {
    let t = tz.trim();
    if t.is_empty() {
        "UTC".into()
    } else {
        t.into()
    }
}

pub fn validate_args(args: &[ArgInput]) -> ApiResult<JsonValue> {
    let mut seen: BTreeSet<String> = Default::default();
    let mut out: Vec<JsonValue> = Vec::new();
    for def in args {
        let flag = def.flag.trim().to_string();
        if flag.is_empty() {
            continue;
        }
        if !ARG_FLAG_RE.is_match(&flag) {
            return Err(ApiError::BadRequest(format!(
                "invalid arg flag: {flag:?} (use --lowercase-with-hyphens)"
            )));
        }
        if !seen.insert(flag.clone()) {
            return Err(ApiError::BadRequest(format!(
                "duplicate arg flag: {flag:?}"
            )));
        }
        let mut entry = serde_json::Map::new();
        entry.insert("flag".into(), json!(flag));
        entry.insert("required".into(), json!(def.required));
        if let Some(help) = def.help.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            entry.insert("help".into(), json!(help));
        }
        out.push(JsonValue::Object(entry));
    }
    Ok(JsonValue::Array(out))
}

pub fn validate_rules(rules: &[String]) -> ApiResult<JsonValue> {
    let mut out: Vec<JsonValue> = Vec::new();
    for r in rules {
        let t = r.trim();
        if t.is_empty() {
            continue;
        }
        expr::parse(t).map_err(|e| ApiError::BadRequest(format!("invalid rule {t:?}: {e}")))?;
        out.push(json!(t));
    }
    Ok(JsonValue::Array(out))
}

/// Validate opted-in skill slugs against the workspace catalog. Trims, dedupes,
/// and rejects any slug that isn't a known optional skill. Hidden skills are
/// injected at spawn time and must never be stored in `agents.skills`.
pub async fn validate_skills<'e, E>(
    exec: E,
    repo: &AgentSkillRepository,
    ws: &str,
    skills: &[String],
) -> ApiResult<JsonValue>
where
    E: Executor<'e, Database = Sqlite>,
{
    let known: BTreeSet<String> = repo
        .list(exec, ws)
        .await?
        .into_iter()
        .filter(|s| !s.hidden)
        .map(|s| s.slug)
        .collect();
    let mut seen: BTreeSet<String> = Default::default();
    let mut out: Vec<JsonValue> = Vec::new();
    for s in skills {
        let t = s.trim();
        if t.is_empty() {
            continue;
        }
        if !known.contains(t) {
            return Err(ApiError::BadRequest(format!("unknown skill slug: {t:?}")));
        }
        if seen.insert(t.to_string()) {
            out.push(json!(t));
        }
    }
    Ok(JsonValue::Array(out))
}

pub fn build_trigger_args(
    arg_defs: &[AgentArgDef],
    body: &JsonValue,
) -> ApiResult<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    for def in arg_defs {
        let key = def.flag.trim_start_matches('-').to_string();
        let value = body
            .get(&key)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if value.is_empty() {
            if def.required {
                return Err(ApiError::BadRequest(format!("missing required arg: {key}")));
            }
            continue;
        }
        if (key == "spec" || key == "branch") && (value.contains('\n') || value.contains(' ')) {
            return Err(ApiError::BadRequest(format!("invalid {key} value")));
        }
        out.insert(key, value);
    }
    Ok(out)
}

/// Next scheduled fire time (RFC3339) for an enabled cron schedule, or `None`.
/// Takes the schedule alone so both the `Agent` and `ConfigAgent` paths (which
/// each expose one) can compute it.
pub fn next_fire(schedule: Option<&AgentSchedule>) -> Option<String> {
    let sched = schedule.filter(|s| s.enabled)?;
    let six_field = if sched.cron.split_whitespace().count() == 5 {
        format!("0 {}", sched.cron)
    } else {
        sched.cron.clone()
    };
    let schedule = Schedule::from_str(&six_field).ok()?;
    let tz: Tz = sched.timezone.parse().unwrap_or(Tz::UTC);
    schedule
        .upcoming(tz)
        .next()
        .map(|dt| dt.to_rfc3339_opts(SecondsFormat::Secs, true))
}
