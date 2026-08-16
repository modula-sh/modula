//! Variant rows. Identity = `(workspace_id, task_id, id)` where all three are UUIDs.
//!
//! Methods take a caller-provided executor (`&pool` standalone, or `&mut *tx` to
//! enlist in a service-owned transaction). The repository holds no pool.

use modula_types::Variant;
use sqlx::{Executor, Sqlite, SqliteConnection};
use uuid::Uuid;

use crate::{Error, Result};

/// Raw `variants` columns. Private serialization detail: the repository maps it
/// 1:1 into the [`Variant`] domain type at its boundary.
#[derive(Debug, Clone, sqlx::FromRow)]
struct VariantRecord {
    id: String,
    /// `None` until promoted (e.g. researcher → `ready_for_workers`); no spawn
    /// rule matches a statusless variant.
    status: Option<String>,
    position: i64,
}

impl From<VariantRecord> for Variant {
    fn from(r: VariantRecord) -> Self {
        Variant {
            id: r.id,
            status: r.status,
            position: r.position,
        }
    }
}

#[derive(Clone, Default)]
pub struct VariantRepository;

impl VariantRepository {
    pub fn new() -> Self {
        Self
    }

    /// The 1-based `position` of one variant, or `None` if it doesn't exist.
    pub async fn position_of<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
        task_id: &str,
        variant_id: &str,
    ) -> Result<Option<i64>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        Ok(sqlx::query_scalar(
            "SELECT position FROM variants WHERE workspace_id = ? AND task_id = ? AND id = ?",
        )
        .bind(ws_id)
        .bind(task_id)
        .bind(variant_id)
        .fetch_optional(exec)
        .await?)
    }

    pub async fn list_for_task<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
        task_id: &str,
    ) -> Result<Vec<Variant>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        Ok(sqlx::query_as::<_, VariantRecord>(
            "SELECT id, status, position FROM variants \
             WHERE workspace_id = ? AND task_id = ? ORDER BY position",
        )
        .bind(ws_id)
        .bind(task_id)
        .fetch_all(exec)
        .await?
        .into_iter()
        .map(Variant::from)
        .collect())
    }

    /// Variants for `task_id` that don't currently have an in-flight spawn of
    /// `agent_id`. Used by the dispatcher's per-variant fan-out. Yields nothing for
    /// a soft-deleted task, so deletion also stops background work on its variants.
    pub async fn list_for_task_idle_for<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
        task_id: &str,
        agent_id: &str,
    ) -> Result<Vec<Variant>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        Ok(sqlx::query_as::<_, VariantRecord>(
            "SELECT v.id, v.status, v.position \
             FROM variants v \
             JOIN tasks t ON t.workspace_id = v.workspace_id AND t.id = v.task_id \
             WHERE v.workspace_id = ? AND v.task_id = ? AND t.deleted_at IS NULL \
               AND NOT EXISTS ( \
                 SELECT 1 FROM agent_processes p \
                 JOIN agent_runs r ON r.id = p.agent_run_id \
                 WHERE p.workspace_id = v.workspace_id \
                   AND p.agent_id = ? \
                   AND json_extract(r.data, '$.args.\"task-id\"')  = v.task_id \
                   AND json_extract(r.data, '$.args.\"variant-id\"') = v.id \
               ) \
             ORDER BY v.position",
        )
        .bind(ws_id)
        .bind(task_id)
        .bind(agent_id)
        .fetch_all(exec)
        .await?
        .into_iter()
        .map(Variant::from)
        .collect())
    }

    pub async fn list_all<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
    ) -> Result<std::collections::BTreeMap<String, Vec<Variant>>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        // Skip variants whose task is soft-deleted (hidden, not destroyed).
        let rows: Vec<(String, String, Option<String>, i64)> = sqlx::query_as(
            "SELECT v.task_id, v.id, v.status, v.position FROM variants v \
             JOIN tasks t ON t.workspace_id = v.workspace_id AND t.id = v.task_id \
             WHERE v.workspace_id = ? AND t.deleted_at IS NULL \
             ORDER BY v.task_id, v.position",
        )
        .bind(ws_id)
        .fetch_all(exec)
        .await?;
        let mut out: std::collections::BTreeMap<String, Vec<Variant>> = Default::default();
        for (task_id, id, status, position) in rows {
            out.entry(task_id).or_default().push(Variant {
                id,
                status,
                position,
            });
        }
        Ok(out)
    }

    /// Register `count` variants for a task, each with an auto-generated UUID and
    /// sequential position. Status starts NULL; promotion happens later via a PUT.
    /// Runs on the caller's connection/transaction so the MAX(position) read and
    /// the inserts share one unit of work. Returns `Vec<(uuid, position)>`.
    pub async fn create_batch(
        &self,
        conn: &mut SqliteConnection,
        ws_id: &str,
        task_id: &str,
        count: u32,
    ) -> Result<Vec<(String, i64)>> {
        let start_pos: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(position), 0) FROM variants \
             WHERE workspace_id = ? AND task_id = ?",
        )
        .bind(ws_id)
        .bind(task_id)
        .fetch_one(&mut *conn)
        .await?;

        let mut created = Vec::with_capacity(count as usize);
        for i in 0..count {
            let id = Uuid::new_v4().to_string();
            let pos = start_pos + i as i64 + 1;
            sqlx::query(
                "INSERT INTO variants (workspace_id, task_id, id, position) \
                 VALUES (?, ?, ?, ?)",
            )
            .bind(ws_id)
            .bind(task_id)
            .bind(&id)
            .bind(pos)
            .execute(&mut *conn)
            .await?;
            created.push((id, pos));
        }
        Ok(created)
    }

    /// One variant by identity, or `None` if it doesn't exist.
    pub async fn get<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
        task_id: &str,
        variant_id: &str,
    ) -> Result<Option<Variant>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        Ok(sqlx::query_as::<_, VariantRecord>(
            "SELECT id, status, position FROM variants \
             WHERE workspace_id = ? AND task_id = ? AND id = ?",
        )
        .bind(ws_id)
        .bind(task_id)
        .bind(variant_id)
        .fetch_optional(exec)
        .await?
        .map(Variant::from))
    }

    pub async fn set_status<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
        task_id: &str,
        variant_id: &str,
        status: &str,
    ) -> Result<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let res = sqlx::query(
            "UPDATE variants SET status = ? \
             WHERE workspace_id = ? AND task_id = ? AND id = ?",
        )
        .bind(status)
        .bind(ws_id)
        .bind(task_id)
        .bind(variant_id)
        .execute(exec)
        .await?;
        if res.rows_affected() == 0 {
            return Err(Error::NotFound(format!(
                "unknown variant on task {task_id}: {variant_id}"
            )));
        }
        Ok(())
    }

    /// Delete every variant of a task. Returns whether any rows were removed.
    pub async fn delete_for_task<'e, E>(&self, exec: E, ws_id: &str, task_id: &str) -> Result<bool>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let res = sqlx::query("DELETE FROM variants WHERE workspace_id = ? AND task_id = ?")
            .bind(ws_id)
            .bind(task_id)
            .execute(exec)
            .await?;
        Ok(res.rows_affected() > 0)
    }
}
