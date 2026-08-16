//! Workspace row CRUD + default seeding (pipeline + settings + provider + skills + agents).
//!
//! Single-statement methods take a caller-provided executor (`&pool` for a
//! standalone statement, or `&mut *conn` to enlist in a caller-owned unit of
//! work). `create` is multi-statement — it seeds the whole default set — so it
//! takes `&mut SqliteConnection` and the caller owns the surrounding
//! `pool.begin()`/`commit()`. The repository is a stateless namespace for the
//! SQL — it never holds the pool.

use modula_types::Workspace;
use sqlx::{Executor, Sqlite, SqliteConnection};
use uuid::Uuid;

use crate::slug;
use crate::{agent_skills, agents, pipeline, providers, settings};
use crate::{Error, Result};

/// Raw `workspaces` columns. Private serialization detail: the repository maps
/// it into the [`Workspace`] domain type at its boundary. `path` (the on-disk
/// location) is left empty for the owning `WorkspaceService` to fill.
#[derive(Debug, Clone, sqlx::FromRow)]
struct WorkspaceRecord {
    id: String,
    name: String,
    description: Option<String>,
    created_at: String,
    /// Human-readable slug used only for on-disk paths; `id` stays canonical.
    /// Always populated at create time (`slugify` never yields empty), so the
    /// API and CLI can rely on it instead of re-deriving from `name`.
    slug: String,
}

impl From<WorkspaceRecord> for Workspace {
    fn from(r: WorkspaceRecord) -> Self {
        Workspace {
            id: r.id,
            name: r.name,
            slug: r.slug,
            description: r.description,
            path: String::new(),
            created_at: r.created_at,
        }
    }
}

#[derive(Clone, Default)]
pub struct WorkspaceRepository;

impl WorkspaceRepository {
    pub fn new() -> Self {
        Self
    }

    /// The workspace's on-disk slug.
    pub async fn slug_for<'e, E>(&self, exec: E, id: &str) -> Result<String>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        Ok(self.get(exec, id).await?.slug)
    }

    pub async fn exists<'e, E>(&self, exec: E, id: &str) -> Result<bool>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workspaces WHERE id = ?")
            .bind(id)
            .fetch_one(exec)
            .await?;
        Ok(n > 0)
    }

    pub async fn list<'e, E>(&self, exec: E) -> Result<Vec<Workspace>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        Ok(sqlx::query_as::<_, WorkspaceRecord>(
            "SELECT id, name, description, created_at, slug FROM workspaces ORDER BY name",
        )
        .fetch_all(exec)
        .await?
        .into_iter()
        .map(Workspace::from)
        .collect())
    }

    pub async fn get<'e, E>(&self, exec: E, id: &str) -> Result<Workspace>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        sqlx::query_as::<_, WorkspaceRecord>(
            "SELECT id, name, description, created_at, slug FROM workspaces WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(exec)
        .await?
        .map(Workspace::from)
        .ok_or_else(|| Error::NotFound(format!("workspace not found: {id}")))
    }

    /// Create a workspace, seed defaults, and return the generated UUID. The
    /// existence check and every seed insert run on the caller-provided
    /// connection, so the caller's `pool.begin()`/`commit()` makes the whole
    /// workspace-creation one atomic unit of work.
    pub async fn create(
        &self,
        conn: &mut SqliteConnection,
        name: &str,
        description: Option<&str>,
    ) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let slug = slug::slugify(name);

        // The slug is the workspace's on-disk directory name, so it must be unique.
        // Reject a duplicate up front with a clear error rather than letting the
        // UNIQUE index surface as an opaque constraint failure. (Distinct names can
        // still collide here, e.g. "My WS" and "my-ws" both slugify to "my-ws".)
        let taken: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workspaces WHERE slug = ?")
            .bind(&slug)
            .fetch_one(&mut *conn)
            .await?;
        if taken > 0 {
            return Err(Error::Conflict(format!(
                "a workspace named \"{name}\" already exists"
            )));
        }

        sqlx::query("INSERT INTO workspaces (id, name, description, slug) VALUES (?, ?, ?, ?)")
            .bind(&id)
            .bind(name)
            .bind(description)
            .bind(&slug)
            .execute(&mut *conn)
            .await?;

        pipeline::seed_defaults(&mut *conn, &id).await?;
        settings::seed_defaults(&mut *conn, &id).await?;
        let provider_id = providers::seed_default(&mut *conn, &id).await?;
        agent_skills::seed_defaults(&mut *conn, &id).await?;
        agents::seed_defaults(&mut *conn, &id, &provider_id).await?;

        Ok(id)
    }

    pub async fn delete<'e, E>(&self, exec: E, id: &str) -> Result<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let res = sqlx::query("DELETE FROM workspaces WHERE id = ?")
            .bind(id)
            .execute(exec)
            .await?;
        if res.rows_affected() == 0 {
            return Err(Error::NotFound(format!("workspace not found: {id}")));
        }
        Ok(())
    }
}
