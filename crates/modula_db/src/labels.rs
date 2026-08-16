//! Label rows + task associations. A label's identity is `(workspace_id, id)`;
//! `(workspace_id, type, name)` is unique so creation is get-or-create.
//!
//! Every method takes a caller-provided executor (`&pool` for a standalone
//! statement, or `&mut *tx` to enlist in a service-owned transaction) so the
//! service layer owns the unit of work. The repository is a stateless namespace
//! for the SQL — it never holds the pool.

use std::collections::BTreeMap;

use modula_types::{Label, TaskLabel};
use sqlx::{Executor, Sqlite, SqliteConnection};
use uuid::Uuid;

use crate::{Error, Result};

/// Raw `labels` columns. Private serialization detail: the repository maps it
/// into the [`Label`] domain type at its boundary, dropping the `type` column
/// (the frontend only consumes `{id, name}`).
#[derive(Debug, Clone, sqlx::FromRow)]
struct LabelRecord {
    id: String,
    name: String,
}

impl From<LabelRecord> for Label {
    fn from(r: LabelRecord) -> Self {
        Label {
            id: r.id,
            name: r.name,
        }
    }
}

#[derive(Clone, Default)]
pub struct LabelRepository;

impl LabelRepository {
    pub fn new() -> Self {
        Self
    }

    pub async fn list<'e, E>(&self, exec: E, ws_id: &str, kind: &str) -> Result<Vec<Label>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        Ok(sqlx::query_as::<_, LabelRecord>(
            "SELECT id, name FROM labels \
             WHERE workspace_id = ? AND type = ? ORDER BY name",
        )
        .bind(ws_id)
        .bind(kind)
        .fetch_all(exec)
        .await?
        .into_iter()
        .map(Label::from)
        .collect())
    }

    /// Get-or-create by `(type, name)`: idempotent on the UNIQUE constraint, so the
    /// picker's "create" path is collision-safe. Returns the label id. Multi-statement
    /// (insert + read-back), so it runs on the caller's connection/transaction.
    pub async fn get_or_create(
        &self,
        conn: &mut SqliteConnection,
        ws_id: &str,
        kind: &str,
        name: &str,
    ) -> Result<String> {
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::BadRequest("label name is required".into()));
        }
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT OR IGNORE INTO labels (workspace_id, id, type, name) VALUES (?, ?, ?, ?)",
        )
        .bind(ws_id)
        .bind(&id)
        .bind(kind)
        .bind(name)
        .execute(&mut *conn)
        .await?;
        Ok(sqlx::query_scalar(
            "SELECT id FROM labels WHERE workspace_id = ? AND type = ? AND name = ?",
        )
        .bind(ws_id)
        .bind(kind)
        .bind(name)
        .fetch_one(&mut *conn)
        .await?)
    }

    pub async fn attach<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
        task_id: &str,
        label_id: &str,
    ) -> Result<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        sqlx::query(
            "INSERT OR IGNORE INTO task_labels (workspace_id, task_id, label_id) VALUES (?, ?, ?)",
        )
        .bind(ws_id)
        .bind(task_id)
        .bind(label_id)
        .execute(exec)
        .await
        // A FK failure means the task or label doesn't exist — a 404, not a 500.
        .map_err(|e| match e {
            sqlx::Error::Database(db) if db.is_foreign_key_violation() => {
                Error::NotFound(format!("unknown task {task_id} or label {label_id}"))
            }
            other => other.into(),
        })?;
        Ok(())
    }

    pub async fn detach<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
        task_id: &str,
        label_id: &str,
    ) -> Result<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        sqlx::query(
            "DELETE FROM task_labels WHERE workspace_id = ? AND task_id = ? AND label_id = ?",
        )
        .bind(ws_id)
        .bind(task_id)
        .bind(label_id)
        .execute(exec)
        .await?;
        Ok(())
    }

    /// Every task's labels in one query, grouped by task id. Mirrors
    /// `variants::list_all` — used by the snapshot and task list. Yields the
    /// `{id, name}` [`TaskLabel`] subset the task shape carries.
    pub async fn list_all_by_task<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
    ) -> Result<BTreeMap<String, Vec<TaskLabel>>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let rows: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT tl.task_id, l.id, l.name FROM task_labels tl \
             JOIN labels l ON l.workspace_id = tl.workspace_id AND l.id = tl.label_id \
             WHERE tl.workspace_id = ? ORDER BY l.name",
        )
        .bind(ws_id)
        .fetch_all(exec)
        .await?;
        let mut out: BTreeMap<String, Vec<TaskLabel>> = Default::default();
        for (task_id, id, name) in rows {
            out.entry(task_id).or_default().push(TaskLabel { id, name });
        }
        Ok(out)
    }
}
