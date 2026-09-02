//! Agent skill rows. Identity = `(workspace_id, slug)`. A skill is a reusable
//! system-prompt fragment. `hidden` skills are injected into every agent at
//! spawn time; the rest are opt-in via the `agents.skills` slug array.
//! `position` drives deterministic assembly order.

use modula_types::AgentSkill;
use sqlx::{Executor, Sqlite, SqliteConnection};

use crate::Result;

/// Raw `agent_skills` columns. Private serialization detail: `prompt` is the
/// skill body assembled into a spawn, never part of the [`AgentSkill`] catalog
/// shape, so the domain mapping drops it.
#[derive(Debug, Clone, sqlx::FromRow)]
struct AgentSkillRecord {
    slug: String,
    name: String,
    description: String,
    prompt: String,
    hidden: bool,
    position: i64,
}

impl From<AgentSkillRecord> for AgentSkill {
    fn from(r: AgentSkillRecord) -> Self {
        AgentSkill {
            slug: r.slug,
            name: r.name,
            description: r.description,
            hidden: r.hidden,
            position: r.position,
        }
    }
}

const SELECT_COLS: &str = "slug, name, description, prompt, hidden, position";

#[derive(Clone, Default)]
pub struct AgentSkillRepository;

impl AgentSkillRepository {
    pub fn new() -> Self {
        Self
    }

    async fn list_records<'e, E>(&self, exec: E, ws_id: &str) -> Result<Vec<AgentSkillRecord>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        Ok(sqlx::query_as::<_, AgentSkillRecord>(&format!(
            "SELECT {SELECT_COLS} FROM agent_skills WHERE workspace_id = ? ORDER BY position, slug"
        ))
        .bind(ws_id)
        .fetch_all(exec)
        .await?)
    }

    /// The workspace skill catalog, ordered by assembly `position`.
    pub async fn list<'e, E>(&self, exec: E, ws_id: &str) -> Result<Vec<AgentSkill>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        Ok(self
            .list_records(exec, ws_id)
            .await?
            .into_iter()
            .map(AgentSkill::from)
            .collect())
    }

    /// The effective skill prompt bodies for an agent: every `hidden` skill plus
    /// the ones whose slug appears in `opted_in`, ordered by `position`. Unknown
    /// slugs in `opted_in` are skipped silently (a referenced skill may be deleted).
    pub async fn for_agent<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
        opted_in: &[String],
    ) -> Result<Vec<String>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        Ok(self
            .list_records(exec, ws_id)
            .await?
            .into_iter()
            .filter(|s| s.hidden || opted_in.iter().any(|o| o == &s.slug))
            .map(|s| s.prompt)
            .collect())
    }

    /// Sync the default catalog into every workspace. Run at startup so the seeded
    /// skills reach workspaces created before this feature (and pick up any updated
    /// skill prompts); idempotent via [`seed_defaults`]'s upsert. Multi-statement
    /// (list workspaces + upsert each), so it runs on the caller's connection so the
    /// whole sync is one unit of work.
    pub async fn sync_all(&self, conn: &mut SqliteConnection) -> Result<()> {
        let ws_ids: Vec<String> = sqlx::query_scalar("SELECT id FROM workspaces")
            .fetch_all(&mut *conn)
            .await?;
        for ws_id in &ws_ids {
            seed_defaults(conn, ws_id).await?;
        }
        Ok(())
    }
}

struct SkillSeed {
    slug: &'static str,
    name: &'static str,
    description: &'static str,
    prompt: &'static str,
    hidden: bool,
    position: i64,
}

const SEED_SKILLS: &[SkillSeed] = &[
    SkillSeed {
        slug: "engine-api",
        name: "Engine API",
        description: "How to read and write workspace state via the engine HTTP API.",
        prompt: include_str!("../../modula_services/templates/skills/engine-api.md"),
        hidden: true,
        position: 10,
    },
    SkillSeed {
        slug: "tasks",
        name: "Tasks",
        description: "How to read tasks, register and update variants, and comment on threads.",
        prompt: include_str!("../../modula_services/templates/skills/tasks.md"),
        hidden: true,
        position: 20,
    },
    SkillSeed {
        slug: "specs",
        name: "Specs",
        description: "The spec folder layout, phase templates, and ownership rules.",
        prompt: include_str!("../../modula_services/templates/skills/specs.md"),
        hidden: true,
        position: 30,
    },
    SkillSeed {
        slug: "workflows",
        name: "Workflows",
        description: "How to claim a task and move it along the roadmap/pipeline.",
        prompt: include_str!("../../modula_services/templates/skills/workflows.md"),
        hidden: true,
        position: 40,
    },
    SkillSeed {
        slug: "worktrees",
        name: "Worktrees",
        description: "How worktrees are structured, branch/tag naming, and how to inspect diffs.",
        prompt: include_str!("../../modula_services/templates/skills/worktrees.md"),
        hidden: true,
        position: 50,
    },
    SkillSeed {
        slug: "ai-wiki",
        name: "AI Wiki",
        description: "How to read from and contribute durable facts to the workspace wiki.",
        prompt: include_str!("../../modula_services/templates/skills/ai-wiki.md"),
        hidden: false,
        position: 60,
    },
];

/// Upsert the default skill catalog into a workspace. The catalog is
/// developer-owned and rebuilt from `templates/skills/*.md`, so re-running
/// refreshes name/description/prompt/hidden/position for existing slugs.
pub(crate) async fn seed_defaults(conn: &mut SqliteConnection, ws_id: &str) -> Result<()> {
    for seed in SEED_SKILLS {
        sqlx::query(
            "INSERT INTO agent_skills (workspace_id, slug, name, description, prompt, hidden, position) \
             VALUES (?, ?, ?, ?, ?, ?, ?) \
             ON CONFLICT(workspace_id, slug) DO UPDATE SET \
               name = excluded.name, description = excluded.description, \
               prompt = excluded.prompt, hidden = excluded.hidden, position = excluded.position",
        )
        .bind(ws_id)
        .bind(seed.slug)
        .bind(seed.name)
        .bind(seed.description)
        .bind(seed.prompt)
        .bind(seed.hidden as i64)
        .bind(seed.position)
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}
