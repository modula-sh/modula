//! Task rows. Identity = `(workspace_id, id)` where id is a UUID.
//!
//! Every method takes a caller-provided executor (`&pool` for a standalone
//! statement, or `&mut *tx` to enlist in a service-owned transaction) so the
//! service layer owns the unit of work. The repository is a stateless namespace
//! for the SQL — it never holds the pool.

use modula_types::Task;
use sqlx::{Executor, QueryBuilder, Sqlite, SqliteConnection};
use uuid::Uuid;

use crate::{Error, Result};

/// Derive an internal-task id prefix from a workspace name: the first up-to-3
/// ASCII letters, uppercased ("modula" → "MOD", "hi" → "HI", "h" → "H").
/// Falls back to "TSK" when the name has no letters at all.
fn prefix_from_name(name: &str) -> String {
    let letters: String = name
        .chars()
        .filter(|c| c.is_ascii_alphabetic())
        .take(3)
        .collect();
    if letters.is_empty() {
        "TSK".to_string()
    } else {
        letters.to_ascii_uppercase()
    }
}

/// Format an internal sequence number as a display id (min 3 digits, grows past
/// that naturally): 1 → "MOD-001", 999 → "MOD-999", 1001 → "MOD-1001".
fn format_external_id(prefix: &str, internal_id: i64) -> String {
    format!("{prefix}-{internal_id:03}")
}

/// Raw `tasks` columns. Private serialization detail: the repository maps it
/// into the [`Task`] domain type at its boundary — `source_data` (a JSON string)
/// is parsed to `Option<Value>`, and `variants`/`labels` are left empty for
/// `TaskService` to assemble.
#[derive(Debug, Clone, sqlx::FromRow)]
struct TaskRecord {
    id: String,
    title: String,
    source: String,
    external_id: Option<String>,
    status: Option<String>,
    source_data: String,
    url: Option<String>,
    approved: Option<bool>,
    description: String,
    max_variants: Option<i64>,
    worktree: Option<bool>,
    synced_at: Option<String>,
    created_at: Option<String>,
}

impl From<TaskRecord> for Task {
    fn from(r: TaskRecord) -> Self {
        Task {
            id: r.id,
            external_id: r.external_id,
            title: r.title,
            source: r.source,
            status: r.status,
            source_data: serde_json::from_str(&r.source_data).ok(),
            url: r.url,
            approved: r.approved,
            description: r.description,
            max_variants: r.max_variants,
            worktree: r.worktree,
            synced_at: r.synced_at,
            created_at: r.created_at,
            variants: Vec::new(),
            labels: Vec::new(),
        }
    }
}

const SELECT_COLS: &str =
    "id, title, source, external_id, status, source_data, url, approved, description, \
     max_variants, worktree, synced_at, created_at";

#[derive(Default)]
pub struct TaskPatch {
    pub title: Option<String>,
    pub source_data: Option<String>,
    pub description: Option<String>,
    pub approved: Option<Option<bool>>,
    pub max_variants: Option<Option<i64>>,
    pub worktree: Option<Option<bool>>,
    pub status: Option<Option<String>>,
    pub url: Option<Option<String>>,
    pub synced_at: Option<Option<String>>,
}

/// A task row that matched a search, projected down to what a result row
/// renders. See [`TaskRepository::search`].
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TaskMatch {
    pub id: String,
    pub external_id: Option<String>,
    pub title: String,
    pub description: String,
}

#[derive(Clone, Default)]
pub struct TaskRepository;

impl TaskRepository {
    pub fn new() -> Self {
        Self
    }

    /// Fetch the workspace name and derive its task-id prefix.
    async fn workspace_prefix<'e, E>(&self, exec: E, ws_id: &str) -> Result<String>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let name: String = sqlx::query_scalar("SELECT name FROM workspaces WHERE id = ?")
            .bind(ws_id)
            .fetch_optional(exec)
            .await?
            .ok_or_else(|| Error::NotFound(format!("workspace not found: {ws_id}")))?;
        Ok(prefix_from_name(&name))
    }

    pub async fn list<'e, E>(&self, exec: E, ws_id: &str) -> Result<Vec<Task>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        Ok(sqlx::query_as::<_, TaskRecord>(&format!(
            "SELECT {SELECT_COLS} FROM tasks \
             WHERE workspace_id = ? AND deleted_at IS NULL \
             ORDER BY external_id DESC, created_at DESC, id DESC"
        ))
        .bind(ws_id)
        .fetch_all(exec)
        .await?
        .into_iter()
        .map(Task::from)
        .collect())
    }

    pub async fn get<'e, E>(&self, exec: E, ws_id: &str, id: &str) -> Result<Task>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        sqlx::query_as::<_, TaskRecord>(&format!(
            "SELECT {SELECT_COLS} FROM tasks WHERE workspace_id = ? AND id = ? AND deleted_at IS NULL"
        ))
        .bind(ws_id)
        .bind(id)
        .fetch_optional(exec)
        .await?
        .map(Task::from)
        .ok_or_else(|| Error::NotFound(format!("unknown task: {id}")))
    }

    /// Look up by external id, INCLUDING soft-deleted rows — the external upsert
    /// must update a tombstone in place, not re-insert its (unique) external_id.
    pub async fn get_by_external<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
        external_id: &str,
    ) -> Result<Option<Task>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        Ok(sqlx::query_as::<_, TaskRecord>(&format!(
            "SELECT {SELECT_COLS} FROM tasks WHERE workspace_id = ? AND external_id = ?"
        ))
        .bind(ws_id)
        .bind(external_id)
        .fetch_optional(exec)
        .await?
        .map(Task::from))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
        title: &str,
        source: &str,
        external_id: Option<&str>,
        source_data: &str,
        approved: Option<bool>,
        description: &str,
        max_variants: Option<i64>,
        worktree: Option<bool>,
        synced_at: Option<&str>,
        status: Option<&str>,
        url: Option<&str>,
    ) -> Result<String>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let id = Uuid::new_v4().to_string();
        // internal_id = next per-(workspace, source) value (MAX+1), derived live.
        sqlx::query(
            "INSERT INTO tasks \
               (workspace_id, id, title, source, external_id, source_data, approved, description, \
                max_variants, worktree, synced_at, status, url, internal_id) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, \
                     (SELECT COALESCE(MAX(internal_id), 0) + 1 FROM tasks \
                      WHERE workspace_id = ? AND source = ?))",
        )
        .bind(ws_id)
        .bind(&id)
        .bind(title)
        .bind(source)
        .bind(external_id)
        .bind(source_data)
        .bind(approved)
        .bind(description)
        .bind(max_variants)
        .bind(worktree)
        .bind(synced_at)
        .bind(status)
        .bind(url)
        // re-bind (workspace_id, source) for the subquery
        .bind(ws_id)
        .bind(source)
        .execute(exec)
        .await?;
        Ok(id)
    }

    /// Create an internal task: mint the UUID plus the per-workspace `internal_id`
    /// and its display id ("MOD-001"). Runs on the caller's connection/transaction
    /// so the `internal_id` allocation and the insert share one unit of work.
    /// Returns `(uuid, external_id)`.
    #[allow(clippy::too_many_arguments)]
    pub async fn create_internal(
        &self,
        conn: &mut SqliteConnection,
        ws_id: &str,
        title: &str,
        source_data: &str,
        approved: Option<bool>,
        description: &str,
        max_variants: Option<i64>,
        worktree: Option<bool>,
        synced_at: &str,
    ) -> Result<(String, String)> {
        let prefix = self.workspace_prefix(&mut *conn, ws_id).await?;
        let id = Uuid::new_v4().to_string();

        let internal_id: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(internal_id), 0) + 1 FROM tasks \
             WHERE workspace_id = ? AND source = 'internal'",
        )
        .bind(ws_id)
        .fetch_one(&mut *conn)
        .await?;
        let external_id = format_external_id(&prefix, internal_id);

        sqlx::query(
            "INSERT INTO tasks \
               (workspace_id, id, title, source, external_id, source_data, approved, description, \
                max_variants, worktree, synced_at, status, url, internal_id) \
             VALUES (?, ?, ?, 'internal', ?, ?, ?, ?, ?, ?, ?, NULL, NULL, ?)",
        )
        .bind(ws_id)
        .bind(&id)
        .bind(title)
        .bind(&external_id)
        .bind(source_data)
        .bind(approved)
        .bind(description)
        .bind(max_variants)
        .bind(worktree)
        .bind(synced_at)
        .bind(internal_id)
        .execute(&mut *conn)
        .await?;

        Ok((id, external_id))
    }

    pub async fn patch<'e, E>(&self, exec: E, ws_id: &str, id: &str, p: TaskPatch) -> Result<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new("UPDATE tasks SET ");
        let mut sets = qb.separated(", ");
        if let Some(t) = p.title {
            sets.push("title = ").push_bind_unseparated(t);
        }
        if let Some(sd) = p.source_data {
            sets.push("source_data = ").push_bind_unseparated(sd);
        }
        if let Some(d) = p.description {
            sets.push("description = ").push_bind_unseparated(d);
        }
        if let Some(a) = p.approved {
            sets.push("approved = ").push_bind_unseparated(a);
        }
        if let Some(m) = p.max_variants {
            sets.push("max_variants = ").push_bind_unseparated(m);
        }
        if let Some(w) = p.worktree {
            sets.push("worktree = ").push_bind_unseparated(w);
        }
        if let Some(s) = p.status {
            sets.push("status = ").push_bind_unseparated(s);
        }
        if let Some(u) = p.url {
            sets.push("url = ").push_bind_unseparated(u);
        }
        if let Some(sa) = p.synced_at {
            sets.push("synced_at = ").push_bind_unseparated(sa);
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
            return Err(Error::NotFound(format!("unknown task: {id}")));
        }
        Ok(())
    }

    /// Soft-delete: stamp `deleted_at`, keep the row so its id stays reserved
    /// (reuse would re-point links). Hidden from [`list`]/[`get`]; a repeat misses.
    pub async fn delete<'e, E>(&self, exec: E, ws_id: &str, id: &str) -> Result<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let res = sqlx::query(
            "UPDATE tasks SET deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
             WHERE workspace_id = ? AND id = ? AND deleted_at IS NULL",
        )
        .bind(ws_id)
        .bind(id)
        .execute(exec)
        .await?;
        if res.rows_affected() == 0 {
            return Err(Error::NotFound(format!("unknown task: {id}")));
        }
        Ok(())
    }
    /// Tasks whose title or description matches `query`. Soft-deleted tasks
    /// never match. Newest first, so a truncating `limit` keeps the freshest.
    pub async fn search<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
        query: &str,
        limit: i64,
    ) -> Result<Vec<TaskMatch>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let pattern = crate::search::like_pattern(query);
        Ok(sqlx::query_as::<_, TaskMatch>(
            "SELECT id, external_id, title, description FROM tasks \
             WHERE workspace_id = ? AND deleted_at IS NULL \
               AND (title LIKE ? ESCAPE '\\' OR description LIKE ? ESCAPE '\\') \
             ORDER BY updated_at DESC LIMIT ?",
        )
        .bind(ws_id)
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
    use crate::workspaces::WorkspaceRepository;
    use serde_json::json;
    use tempfile::tempdir;

    fn record(source_data: &str) -> TaskRecord {
        TaskRecord {
            id: "t1".into(),
            title: "T".into(),
            source: "internal".into(),
            external_id: Some("MOD-001".into()),
            status: Some("planning".into()),
            source_data: source_data.into(),
            url: None,
            approved: Some(true),
            description: "d".into(),
            max_variants: Some(2),
            worktree: None,
            synced_at: None,
            created_at: None,
        }
    }

    #[test]
    fn record_parses_source_data_and_leaves_assembled_fields_empty() {
        let task = Task::from(record(r#"{"k":1}"#));
        assert_eq!(task.source_data, Some(json!({"k": 1})));
        assert!(task.variants.is_empty());
        assert!(task.labels.is_empty());
    }

    #[test]
    fn record_source_data_none_on_parse_failure() {
        assert_eq!(Task::from(record("not json")).source_data, None);
    }

    /// The repository takes the caller's executor, so a caller-owned transaction
    /// that rolls back leaves no task behind — the unit of work is the service's.
    #[tokio::test]
    async fn create_internal_honors_caller_rollback() {
        let dir = tempdir().unwrap();
        let pool = crate::open(&dir.path().join("t.sqlite")).await.unwrap();
        let mut conn = pool.acquire().await.unwrap();
        let ws = WorkspaceRepository::new()
            .create(&mut conn, "Modula", None)
            .await
            .unwrap();
        drop(conn);
        let tasks = TaskRepository::new();

        let mut tx = pool.begin().await.unwrap();
        let (id, external_id) = tasks
            .create_internal(&mut tx, &ws, "t", "{}", None, "", None, None, "2026-06-30")
            .await
            .unwrap();
        assert_eq!(external_id, "MOD-001");
        // Visible inside the open transaction...
        assert!(tasks.get(&mut *tx, &ws, &id).await.is_ok());
        tx.rollback().await.unwrap();

        // ...gone once the caller rolls back.
        assert!(tasks.get(&pool, &ws, &id).await.is_err());
    }
}
