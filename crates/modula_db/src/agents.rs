//! Agent rows. Identity = `(workspace_id, id)` where id is a UUID.
//! `name` is a human-readable display label. `rules` and `args` are
//! stored as JSON TEXT and surfaced as `serde_json::Value` to callers.
//!
//! Every method takes a caller-provided executor (`&pool` for a standalone
//! statement, or `&mut *tx` to enlist in a service-owned transaction) so the
//! caller owns the unit of work. The repository is a stateless namespace for
//! the SQL — it never holds the pool.

use modula_types::{Agent, AgentArgDef, AgentSchedule};
use serde_json::{json, Value as Json};
use sqlx::{Executor, QueryBuilder, Sqlite, SqliteConnection};
use uuid::Uuid;

use crate::{Error, Result};

/// Raw `agents` columns. Private serialization detail: the repository maps it
/// into the [`Agent`] domain type at its boundary (JSON-string columns parsed,
/// the flat `schedule_*` columns nested into `Option<AgentSchedule>`).
#[derive(Debug, Clone, sqlx::FromRow)]
struct AgentRecord {
    id: String,
    name: String,
    description: String,
    provider_id: String,
    model: Option<String>,
    schedule_cron: Option<String>,
    schedule_tz: Option<String>,
    schedule_enabled: bool,
    manual: bool,
    /// JSON array of expression strings.
    rules: String,
    /// JSON array of `{flag, required, help?}` objects.
    args: String,
    /// Full prompt body — what the spawn passes to claude via `-p`.
    prompt: String,
    /// When true, task-scoped events that don't carry a `variant_id`
    /// (e.g. a `task.update` carrying `pipeline_status`) are
    /// fanned out by the dispatcher into one spawn per variant.
    spawn_per_variant: bool,
    /// JSON array of opted-in skill slugs (hidden skills are injected at
    /// spawn time regardless and are not listed here).
    skills: String,
}

const SELECT_COLS: &str = "id, name, description, provider_id, model, schedule_cron, schedule_tz, \
     schedule_enabled, manual, rules, args, prompt, spawn_per_variant, skills";

impl From<AgentRecord> for Agent {
    fn from(r: AgentRecord) -> Self {
        let AgentRecord {
            id,
            name,
            description,
            provider_id,
            model,
            schedule_cron,
            schedule_tz,
            schedule_enabled,
            manual,
            rules,
            args,
            prompt,
            spawn_per_variant,
            skills,
        } = r;
        let schedule = schedule_cron.map(|cron| AgentSchedule {
            cron,
            timezone: schedule_tz.unwrap_or_else(|| "UTC".into()),
            enabled: schedule_enabled,
        });
        Agent {
            id,
            name,
            description,
            provider_id,
            model,
            manual,
            schedule,
            rules: serde_json::from_str(&rules).unwrap_or_default(),
            args: serde_json::from_str::<Vec<AgentArgDef>>(&args).unwrap_or_default(),
            // The scheduler-derived next fire time is filled by `AgentService`,
            // not the repository — no such column exists.
            next_fire: None,
            spawn_per_variant,
            skills: serde_json::from_str(&skills).unwrap_or_default(),
            prompt: Some(prompt),
        }
    }
}

pub struct ScheduleFields {
    pub cron: String,
    pub tz: String,
    pub enabled: bool,
}

struct AgentSeed {
    name: &'static str,
    description: &'static str,
    rules: &'static [&'static str],
    args: &'static [SeedArg],
    prompt: &'static str,
    spawn_per_variant: bool,
    /// Opt-in skill slugs. Hidden skills are injected at spawn time for every
    /// agent, so only optional skills are listed here.
    skills: &'static [&'static str],
}

struct SeedArg {
    flag: &'static str,
    required: bool,
    help: Option<&'static str>,
}

const SEED_AGENTS: &[AgentSeed] = &[
    AgentSeed {
        name: "jira-scan",
        description: "Mirrors JIRA tasks into the workspace.",
        rules: &[],
        args: &[],
        prompt: include_str!("../../modula_services/templates/agents/jira-scan.md"),
        spawn_per_variant: false,
        skills: &[],
    },
    AgentSeed {
        name: "linear-scan",
        description: "Mirrors Linear issues into the workspace.",
        rules: &[],
        args: &[],
        prompt: include_str!("../../modula_services/templates/agents/linear-scan.md"),
        spawn_per_variant: false,
        skills: &[],
    },
    AgentSeed {
        name: "github-scan",
        description: "Mirrors GitHub issues into the workspace.",
        rules: &[],
        args: &[],
        prompt: include_str!("../../modula_services/templates/agents/github-scan.md"),
        spawn_per_variant: false,
        skills: &[],
    },
    AgentSeed {
        name: "project-manager",
        description: "Sequences approved tasks onto the roadmap.",
        rules: &[
            "event.type == 'task.create' and event.data.approved == true",
            "event.type == 'task.update' and event.data.approved == true",
        ],
        args: &[],
        prompt: include_str!("../../modula_services/templates/agents/project-manager.md"),
        spawn_per_variant: false,
        skills: &[],
    },
    AgentSeed {
        name: "researcher",
        description: "Investigates a task and drafts up to 3 variant specs.",
        rules: &[
            "event.type == 'task.update' and event.data.pipeline_status == 'ready_for_research'",
        ],
        args: &[SeedArg {
            flag: "--task-id",
            required: true,
            help: Some("Task to research."),
        }],
        prompt: include_str!("../../modula_services/templates/agents/researcher.md"),
        spawn_per_variant: false,
        skills: &["ai-wiki"],
    },
    AgentSeed {
        name: "worker",
        description: "Implements one variant of a task across affected projects.",
        rules: &[
            "event.type == 'variant.update' and event.data.status == 'ready_for_workers'",
            "event.type == 'variant.update' and event.data.status == 'rework'",
        ],
        args: &[
            SeedArg {
                flag: "--task-id",
                required: true,
                help: None,
            },
            SeedArg {
                flag: "--variant-id",
                required: true,
                help: None,
            },
        ],
        prompt: include_str!("../../modula_services/templates/agents/worker.md"),
        spawn_per_variant: true,
        skills: &["ai-wiki"],
    },
    AgentSeed {
        name: "code-reviewer",
        description: "Reviews one variant's code changes and posts feedback.",
        rules: &["event.type == 'variant.update' and event.data.status == 'ready_for_review'"],
        args: &[
            SeedArg {
                flag: "--task-id",
                required: true,
                help: None,
            },
            SeedArg {
                flag: "--variant-id",
                required: true,
                help: None,
            },
        ],
        prompt: include_str!("../../modula_services/templates/agents/code-reviewer.md"),
        spawn_per_variant: true,
        skills: &["ai-wiki"],
    },
    AgentSeed {
        name: "reviewer",
        description: "Reviews all variants together before human acceptance.",
        rules: &[
            "event.type == 'task.update' and event.data.pipeline_status == 'ready_for_review'",
        ],
        args: &[SeedArg {
            flag: "--task-id",
            required: true,
            help: Some("Task id to review."),
        }],
        prompt: include_str!("../../modula_services/templates/agents/reviewer.md"),
        spawn_per_variant: false,
        skills: &["ai-wiki"],
    },
];

pub(crate) async fn seed_defaults(
    conn: &mut SqliteConnection,
    ws_id: &str,
    provider_id: &str,
) -> Result<()> {
    for seed in SEED_AGENTS {
        insert_seed(&mut *conn, ws_id, provider_id, seed).await?;
    }
    Ok(())
}

async fn insert_seed(
    conn: &mut SqliteConnection,
    ws_id: &str,
    provider_id: &str,
    seed: &AgentSeed,
) -> Result<()> {
    let id = Uuid::new_v4().to_string();
    let rules_json = Json::Array(seed.rules.iter().map(|r| json!(r)).collect());
    let args_json = Json::Array(
        seed.args
            .iter()
            .map(|a| {
                let mut m = serde_json::Map::new();
                m.insert("flag".into(), json!(a.flag));
                m.insert("required".into(), json!(a.required));
                if let Some(h) = a.help {
                    m.insert("help".into(), json!(h));
                }
                Json::Object(m)
            })
            .collect(),
    );
    let skills_json = Json::Array(seed.skills.iter().map(|s| json!(s)).collect());
    sqlx::query(
        "INSERT INTO agents (workspace_id, id, name, description, provider_id, model, \
                             schedule_cron, schedule_tz, schedule_enabled, manual, \
                             rules, args, prompt, spawn_per_variant, skills) \
         VALUES (?, ?, ?, ?, ?, NULL, NULL, NULL, 0, 1, ?, ?, ?, ?, ?)",
    )
    .bind(ws_id)
    .bind(&id)
    .bind(seed.name)
    .bind(seed.description)
    .bind(provider_id)
    .bind(rules_json.to_string())
    .bind(args_json.to_string())
    .bind(seed.prompt)
    .bind(seed.spawn_per_variant as i64)
    .bind(skills_json.to_string())
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// An agent row that matched a search. See [`AgentRepository::search`].
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AgentMatch {
    pub id: String,
    pub name: String,
    pub description: String,
    pub prompt: String,
}

#[derive(Clone, Default)]
pub struct AgentRepository;

impl AgentRepository {
    pub fn new() -> Self {
        Self
    }

    pub async fn list<'e, E>(&self, exec: E, ws_id: &str) -> Result<Vec<Agent>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        Ok(sqlx::query_as::<_, AgentRecord>(&format!(
            "SELECT {SELECT_COLS} FROM agents WHERE workspace_id = ? ORDER BY name"
        ))
        .bind(ws_id)
        .fetch_all(exec)
        .await?
        .into_iter()
        .map(Agent::from)
        .collect())
    }

    pub async fn get<'e, E>(&self, exec: E, ws_id: &str, id: &str) -> Result<Agent>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        sqlx::query_as::<_, AgentRecord>(&format!(
            "SELECT {SELECT_COLS} FROM agents WHERE workspace_id = ? AND id = ?"
        ))
        .bind(ws_id)
        .bind(id)
        .fetch_optional(exec)
        .await?
        .map(Agent::from)
        .ok_or_else(|| Error::NotFound(format!("unknown agent: {id}")))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
        name: &str,
        description: &str,
        provider_id: &str,
        model: Option<&str>,
        schedule_cron: Option<&str>,
        schedule_tz: Option<&str>,
        schedule_enabled: bool,
        manual: bool,
        rules: &Json,
        args: &Json,
        prompt: &str,
        spawn_per_variant: bool,
        skills: &Json,
    ) -> Result<String>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO agents (workspace_id, id, name, description, provider_id, model, \
                                 schedule_cron, schedule_tz, schedule_enabled, manual, \
                                 rules, args, prompt, spawn_per_variant, skills) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(ws_id)
        .bind(&id)
        .bind(name)
        .bind(description)
        .bind(provider_id)
        .bind(model)
        .bind(schedule_cron)
        .bind(schedule_tz)
        .bind(schedule_enabled as i64)
        .bind(manual as i64)
        .bind(rules.to_string())
        .bind(args.to_string())
        .bind(prompt)
        .bind(spawn_per_variant as i64)
        .bind(skills.to_string())
        .execute(exec)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db)
                if db.code().as_deref() == Some("2067") || db.code().as_deref() == Some("1555") =>
            {
                Error::Conflict(format!("agent {name:?} already exists"))
            }
            other => Error::Internal(format!("sqlx: {other}")),
        })?;
        Ok(id)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn patch<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
        id: &str,
        description: Option<&str>,
        provider_id: Option<&str>,
        model: Option<Option<String>>,
        schedule: Option<Option<ScheduleFields>>,
        manual: Option<bool>,
        rules: Option<&Json>,
        args: Option<&Json>,
        prompt: Option<&str>,
        spawn_per_variant: Option<bool>,
        skills: Option<&Json>,
    ) -> Result<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new("UPDATE agents SET ");
        let mut sets = qb.separated(", ");
        if let Some(d) = description {
            sets.push("description = ").push_bind_unseparated(d);
        }
        if let Some(p) = provider_id {
            sets.push("provider_id = ").push_bind_unseparated(p);
        }
        if let Some(m) = model {
            sets.push("model = ")
                .push_bind_unseparated(m.filter(|s| !s.is_empty()));
        }
        if let Some(s) = schedule {
            let (cron, tz, enabled) = match s {
                Some(s) => (Some(s.cron), Some(s.tz), s.enabled as i64),
                None => (None, None, 0),
            };
            sets.push("schedule_cron = ").push_bind_unseparated(cron);
            sets.push("schedule_tz = ").push_bind_unseparated(tz);
            sets.push("schedule_enabled = ")
                .push_bind_unseparated(enabled);
        }
        if let Some(m) = manual {
            sets.push("manual = ").push_bind_unseparated(m as i64);
        }
        if let Some(r) = rules {
            sets.push("rules = ").push_bind_unseparated(r.to_string());
        }
        if let Some(a) = args {
            sets.push("args = ").push_bind_unseparated(a.to_string());
        }
        if let Some(p) = prompt {
            sets.push("prompt = ").push_bind_unseparated(p);
        }
        if let Some(s) = spawn_per_variant {
            sets.push("spawn_per_variant = ")
                .push_bind_unseparated(s as i64);
        }
        if let Some(s) = skills {
            sets.push("skills = ").push_bind_unseparated(s.to_string());
        }
        // Nothing set — no-op.
        if qb.sql().ends_with("SET ") {
            return Ok(());
        }
        qb.push(" WHERE workspace_id = ")
            .push_bind(ws_id)
            .push(" AND id = ")
            .push_bind(id);
        let res = qb.build().execute(exec).await?;
        if res.rows_affected() == 0 {
            return Err(Error::NotFound(format!("unknown agent: {id}")));
        }
        Ok(())
    }

    pub async fn delete<'e, E>(&self, exec: E, ws_id: &str, id: &str) -> Result<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let res = sqlx::query("DELETE FROM agents WHERE workspace_id = ? AND id = ?")
            .bind(ws_id)
            .bind(id)
            .execute(exec)
            .await?;
        if res.rows_affected() == 0 {
            return Err(Error::NotFound(format!("unknown agent: {id}")));
        }
        Ok(())
    }

    pub async fn provider_exists<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
        provider_id: &str,
    ) -> Result<bool>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let n: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM providers WHERE workspace_id = ? AND id = ?")
                .bind(ws_id)
                .bind(provider_id)
                .fetch_one(exec)
                .await?;
        Ok(n > 0)
    }

    /// Insert `SEED_AGENTS` entries missing by `name` into every workspace, so
    /// defaults added after a workspace was created still appear. Run at startup.
    ///
    /// Insert-only: existing rows are never touched (agents are user-editable), so
    /// a default the user deleted is re-inserted next startup. Existence is checked
    /// by name in-transaction since there's no UNIQUE on `(workspace_id, name)`.
    pub async fn sync_missing_defaults(&self, conn: &mut SqliteConnection) -> Result<()> {
        let ws_ids: Vec<String> = sqlx::query_scalar("SELECT id FROM workspaces")
            .fetch_all(&mut *conn)
            .await?;
        for ws_id in &ws_ids {
            let provider_id: Option<String> = sqlx::query_scalar(
                "SELECT id FROM providers WHERE workspace_id = ? \
                 ORDER BY (type = 'claude') DESC, created_at ASC LIMIT 1",
            )
            .bind(ws_id)
            .fetch_optional(&mut *conn)
            .await?;
            let Some(provider_id) = provider_id else {
                tracing::warn!(workspace = %ws_id, "skipping seed-agent backfill: no provider");
                continue;
            };
            for seed in SEED_AGENTS {
                let exists: i64 = sqlx::query_scalar(
                    "SELECT COUNT(*) FROM agents WHERE workspace_id = ? AND name = ?",
                )
                .bind(ws_id)
                .bind(seed.name)
                .fetch_one(&mut *conn)
                .await?;
                if exists == 0 {
                    insert_seed(&mut *conn, ws_id, &provider_id, seed).await?;
                }
            }
        }
        Ok(())
    }

    /// Scheduled agents for the cron scheduler. Returns
    /// `(workspace_id, agent_id, agent_name, cron, tz)`.
    pub async fn scheduled_across_workspaces<'e, E>(
        &self,
        exec: E,
    ) -> Result<Vec<(String, String, String, String, String)>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        Ok(
            sqlx::query_as::<_, (String, String, String, String, String)>(
                "SELECT workspace_id, id, name, schedule_cron, COALESCE(schedule_tz, 'UTC') \
             FROM agents \
             WHERE schedule_enabled = 1 AND schedule_cron IS NOT NULL AND schedule_cron <> ''",
            )
            .fetch_all(exec)
            .await?,
        )
    }
    /// Agents whose name, description or prompt matches `query`.
    pub async fn search<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
        query: &str,
        limit: i64,
    ) -> Result<Vec<AgentMatch>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let pattern = crate::search::like_pattern(query);
        Ok(sqlx::query_as::<_, AgentMatch>(
            "SELECT id, name, description, prompt FROM agents \
             WHERE workspace_id = ? \
               AND (name LIKE ? ESCAPE '\\' OR description LIKE ? ESCAPE '\\' \
                    OR prompt LIKE ? ESCAPE '\\') \
             ORDER BY updated_at DESC LIMIT ?",
        )
        .bind(ws_id)
        .bind(&pattern)
        .bind(&pattern)
        .bind(&pattern)
        .bind(limit)
        .fetch_all(exec)
        .await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record() -> AgentRecord {
        AgentRecord {
            id: "a1".into(),
            name: "worker".into(),
            description: "impl".into(),
            provider_id: "p1".into(),
            model: Some("opus".into()),
            schedule_cron: Some("0 9 * * *".into()),
            schedule_tz: None,
            schedule_enabled: true,
            manual: false,
            rules: r#"["event.type == 'x'"]"#.into(),
            args: r#"[{"flag":"--task-id","required":true},{"flag":"--branch","required":false,"help":"b"}]"#.into(),
            prompt: "do work".into(),
            spawn_per_variant: true,
            skills: r#"["ai-wiki"]"#.into(),
        }
    }

    #[test]
    fn record_maps_json_columns_and_nests_schedule() {
        let a = Agent::from(record());
        assert_eq!(a.rules, vec!["event.type == 'x'".to_string()]);
        assert_eq!(a.skills, vec!["ai-wiki".to_string()]);
        assert_eq!(a.args.len(), 2);
        assert_eq!(a.args[0].flag, "--task-id");
        assert!(a.args[0].required);
        assert_eq!(a.args[1].help.as_deref(), Some("b"));
        let sched = a.schedule.expect("schedule present");
        assert_eq!(sched.cron, "0 9 * * *");
        assert_eq!(sched.timezone, "UTC"); // NULL schedule_tz defaults to UTC
        assert!(sched.enabled);
        assert_eq!(a.next_fire, None); // filled by the service, not the repo
        assert_eq!(a.prompt.as_deref(), Some("do work"));
    }

    #[test]
    fn record_without_schedule_and_bad_json_degrades_to_empty() {
        let mut r = record();
        r.schedule_cron = None;
        r.rules = "not json".into();
        r.args = "".into();
        r.skills = "".into();
        let a = Agent::from(r);
        assert!(a.schedule.is_none());
        assert!(a.rules.is_empty());
        assert!(a.args.is_empty());
        assert!(a.skills.is_empty());
    }
}
