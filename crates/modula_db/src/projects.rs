//! Project rows. Identity = `(workspace_id, id)` where id is a UUID.
//! `name` is a human-readable display label, not the identity key.
//!
//! Every method takes a caller-provided executor (`&pool` for a standalone
//! statement, or `&mut *tx` to enlist in a service-owned transaction) so the
//! caller owns the unit of work. The repository is a stateless namespace for
//! the SQL — it never holds the pool.

use modula_types::Project;
use sqlx::{Executor, QueryBuilder, Sqlite};
use uuid::Uuid;

use crate::{Error, Result};

/// Raw `projects` columns. Private serialization detail: the repository maps it
/// into the [`Project`] domain type at its boundary. `exists`/`worktrees` (the
/// on-disk enrichment) are left empty for the owning `ProjectService` to fill.
#[derive(Debug, Clone, sqlx::FromRow)]
struct ProjectRecord {
    id: String,
    name: String,
    path: String,
    base_branch: String,
}

impl From<ProjectRecord> for Project {
    fn from(r: ProjectRecord) -> Self {
        Project {
            id: r.id,
            name: r.name,
            path: r.path,
            base_branch: r.base_branch,
            exists: false,
            worktrees: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ProjectMatch {
    pub id: String,
    pub name: String,
    pub path: String,
}

#[derive(Clone, Default)]
pub struct ProjectRepository;

impl ProjectRepository {
    pub fn new() -> Self {
        Self
    }

    pub async fn list<'e, E>(&self, exec: E, ws_id: &str) -> Result<Vec<Project>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        Ok(sqlx::query_as::<_, ProjectRecord>(
            "SELECT id, name, path, base_branch FROM projects WHERE workspace_id = ? ORDER BY name",
        )
        .bind(ws_id)
        .fetch_all(exec)
        .await?
        .into_iter()
        .map(Project::from)
        .collect())
    }

    pub async fn get<'e, E>(&self, exec: E, ws_id: &str, id: &str) -> Result<Project>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        sqlx::query_as::<_, ProjectRecord>(
            "SELECT id, name, path, base_branch FROM projects WHERE workspace_id = ? AND id = ?",
        )
        .bind(ws_id)
        .bind(id)
        .fetch_optional(exec)
        .await?
        .map(Project::from)
        .ok_or_else(|| Error::NotFound(format!("unknown project: {id}")))
    }

    /// Create a project and return its UUID.
    pub async fn create<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
        name: &str,
        path: &str,
        base_branch: &str,
    ) -> Result<String>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO projects (workspace_id, id, name, path, base_branch) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(ws_id)
        .bind(&id)
        .bind(name)
        .bind(path)
        .bind(base_branch)
        .execute(exec)
        .await?;
        Ok(id)
    }

    pub async fn patch<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
        id: &str,
        name: Option<&str>,
        path: Option<&str>,
        base_branch: Option<&str>,
    ) -> Result<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new("UPDATE projects SET ");
        let mut sets = qb.separated(", ");
        if let Some(n) = name {
            sets.push("name = ").push_bind_unseparated(n);
        }
        if let Some(p) = path {
            sets.push("path = ").push_bind_unseparated(p);
        }
        if let Some(b) = base_branch {
            sets.push("base_branch = ").push_bind_unseparated(b);
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
            return Err(Error::NotFound(format!("unknown project: {id}")));
        }
        Ok(())
    }

    pub async fn delete<'e, E>(&self, exec: E, ws_id: &str, id: &str) -> Result<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let res = sqlx::query("DELETE FROM projects WHERE workspace_id = ? AND id = ?")
            .bind(ws_id)
            .bind(id)
            .execute(exec)
            .await?;
        if res.rows_affected() == 0 {
            return Err(Error::NotFound(format!("unknown project: {id}")));
        }
        Ok(())
    }
    pub async fn search<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
        query: &str,
        limit: i64,
    ) -> Result<Vec<ProjectMatch>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let pattern = crate::search::like_pattern(query);
        Ok(sqlx::query_as::<_, ProjectMatch>(
            "SELECT id, name, path FROM projects \
             WHERE workspace_id = ? \
               AND (name LIKE ? ESCAPE '\\' OR path LIKE ? ESCAPE '\\') \
             ORDER BY name LIMIT ?",
        )
        .bind(ws_id)
        .bind(&pattern)
        .bind(&pattern)
        .bind(limit)
        .fetch_all(exec)
        .await?)
    }
}
