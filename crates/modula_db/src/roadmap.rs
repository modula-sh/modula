//! Roadmap rows. Identity = `(workspace_id, task_id)`. `depends_on` is
//! stored as a JSON-encoded array of task ids.
//!
//! Every method takes a caller-provided executor (`&pool` for a standalone
//! statement, or `&mut *tx` to enlist in a service-owned transaction) so the
//! service layer owns the unit of work. The repository is a stateless namespace
//! for the SQL — it never holds the pool.

use modula_types::RoadmapEntry;
use serde_json::Value as Json;
use sqlx::{Executor, QueryBuilder, Sqlite, SqliteConnection};

use crate::{Error, Result};

/// Raw `roadmap_rows` columns. Private serialization detail: the repository maps
/// it into the [`RoadmapEntry`] domain type at its boundary — the JSON-string
/// `depends_on` column is decoded to a `Vec<String>`.
#[derive(Debug, Clone, sqlx::FromRow)]
struct RoadmapRecord {
    task_id: String,
    status: String,
    depends_on: String,
    notes: String,
    position: i64,
}

impl From<RoadmapRecord> for RoadmapEntry {
    fn from(r: RoadmapRecord) -> Self {
        RoadmapEntry {
            task: r.task_id,
            status: r.status,
            depends_on: serde_json::from_str(&r.depends_on).unwrap_or_default(),
            notes: r.notes,
            position: r.position,
        }
    }
}

#[derive(Clone, Default)]
pub struct RoadmapRepository;

impl RoadmapRepository {
    pub fn new() -> Self {
        Self
    }

    pub async fn list<'e, E>(&self, exec: E, ws_id: &str) -> Result<Vec<RoadmapEntry>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        // Skip rows whose task is soft-deleted (hidden, not destroyed).
        Ok(sqlx::query_as::<_, RoadmapRecord>(
            "SELECT rr.task_id, rr.status, rr.depends_on, rr.notes, rr.position \
         FROM roadmap_rows rr \
         JOIN tasks t ON t.workspace_id = rr.workspace_id AND t.id = rr.task_id \
         WHERE rr.workspace_id = ? AND t.deleted_at IS NULL ORDER BY rr.position",
        )
        .bind(ws_id)
        .fetch_all(exec)
        .await?
        .into_iter()
        .map(RoadmapEntry::from)
        .collect())
    }

    pub async fn set_status<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
        task_id: &str,
        status: &str,
    ) -> Result<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let res = sqlx::query(
            "UPDATE roadmap_rows SET status = ? \
         WHERE workspace_id = ? AND task_id = ?",
        )
        .bind(status)
        .bind(ws_id)
        .bind(task_id)
        .execute(exec)
        .await?;
        if res.rows_affected() == 0 {
            return Err(Error::NotFound(format!("no roadmap row for {task_id}")));
        }
        Ok(())
    }

    /// Insert a roadmap row at the end of the workspace's roadmap, or update its
    /// status + optional depends_on / notes if it already exists. Returns true
    /// when the row was newly created. Multi-statement (existence check + append
    /// position), so it runs on the caller's connection/transaction as one unit.
    pub async fn upsert(
        &self,
        conn: &mut SqliteConnection,
        ws_id: &str,
        task_id: &str,
        status: &str,
        depends_on: Option<&Json>,
        notes: Option<&str>,
    ) -> Result<bool> {
        let existing: Option<i64> =
            sqlx::query_scalar("SELECT 1 FROM roadmap_rows WHERE workspace_id = ? AND task_id = ?")
                .bind(ws_id)
                .bind(task_id)
                .fetch_optional(&mut *conn)
                .await?;

        if existing.is_some() {
            let mut qb: QueryBuilder<Sqlite> =
                QueryBuilder::new("UPDATE roadmap_rows SET status = ");
            qb.push_bind(status);
            if let Some(dep) = depends_on {
                qb.push(", depends_on = ")
                    .push_bind(serde_json::to_string(dep).unwrap_or_else(|_| "[]".into()));
            }
            if let Some(n) = notes {
                qb.push(", notes = ").push_bind(n);
            }
            qb.push(" WHERE workspace_id = ")
                .push_bind(ws_id)
                .push(" AND task_id = ")
                .push_bind(task_id);
            qb.build().execute(&mut *conn).await?;
            return Ok(false);
        }

        let next_pos: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(position), 0) + 1 FROM roadmap_rows WHERE workspace_id = ?",
        )
        .bind(ws_id)
        .fetch_one(&mut *conn)
        .await?;
        let dep_json = depends_on
            .map(|d| serde_json::to_string(d).unwrap_or_else(|_| "[]".into()))
            .unwrap_or_else(|| "[]".into());
        let notes_str = notes.unwrap_or("").to_string();

        sqlx::query(
            "INSERT INTO roadmap_rows (workspace_id, task_id, status, depends_on, notes, position) \
         VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(ws_id)
        .bind(task_id)
        .bind(status)
        .bind(dep_json)
        .bind(notes_str)
        .bind(next_pos)
        .execute(&mut *conn)
        .await
        // A foreign-key failure here means the task was deleted between checks — a
        // 404, not the generic 500 the blanket conversion would produce.
        .map_err(|e| match e {
            sqlx::Error::Database(db) if db.is_foreign_key_violation() => {
                Error::NotFound(format!("task {task_id} does not exist"))
            }
            other => other.into(),
        })?;
        Ok(true)
    }

    pub async fn delete_for_task<'e, E>(&self, exec: E, ws_id: &str, task_id: &str) -> Result<bool>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let res = sqlx::query("DELETE FROM roadmap_rows WHERE workspace_id = ? AND task_id = ?")
            .bind(ws_id)
            .bind(task_id)
            .execute(exec)
            .await?;
        Ok(res.rows_affected() > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_decodes_depends_on_and_renames_task() {
        let entry = RoadmapEntry::from(RoadmapRecord {
            task_id: "t1".into(),
            status: "planning".into(),
            depends_on: r#"["t0","t2"]"#.into(),
            notes: "n".into(),
            position: 3,
        });
        assert_eq!(entry.task, "t1");
        assert_eq!(entry.depends_on, vec!["t0", "t2"]);
        assert_eq!(entry.position, 3);
    }

    #[test]
    fn record_depends_on_defaults_empty_on_bad_json() {
        let entry = RoadmapEntry::from(RoadmapRecord {
            task_id: "t1".into(),
            status: "planning".into(),
            depends_on: "not json".into(),
            notes: String::new(),
            position: 0,
        });
        assert!(entry.depends_on.is_empty());
    }
}
