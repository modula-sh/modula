//! One row per agent dispatch. The dispatcher inserts a row right before
//! spawning the agent; the reap pass updates `status` + `finished_at`.
//!
//! Every method takes a caller-provided executor (`&pool` for a standalone
//! statement, or `&mut *tx` to enlist in a service-owned transaction) so the
//! caller owns the unit of work. The repository is a stateless namespace for
//! the SQL — it never holds the pool.

use serde_json::Value as Json;
use sqlx::{Executor, Sqlite};

use modula_types::AgentRun;

use crate::Result;

pub const STATUS_RUNNING: &str = "running";
pub const STATUS_COMPLETED: &str = "completed";
pub const STATUS_FAILED: &str = "failed";

#[derive(Debug, Clone, sqlx::FromRow)]
struct AgentRunRecord {
    id: i64,
    agent_id: String,
    agent_name: String,
    event_id: Option<i64>,
    status: String,
    attempts: i64,
    data: String,
    started_at: Option<String>,
    finished_at: Option<String>,
    created_at: String,
    log_path: Option<String>,
    loop_iter: i64,
    loop_total: i64,
    loop_group_id: Option<i64>,
}

impl From<AgentRunRecord> for AgentRun {
    fn from(r: AgentRunRecord) -> Self {
        let data: Json = serde_json::from_str(&r.data).unwrap_or(Json::Object(Default::default()));
        // task/variant are surfaced flat from the run's args, tolerating either
        // the hyphenated CLI key or the short alias.
        let pick = |a: &str, b: &str| {
            data.pointer(a)
                .or_else(|| data.pointer(b))
                .and_then(|v| v.as_str())
                .map(str::to_string)
        };
        let task = pick("/args/task-id", "/args/task");
        let variant = pick("/args/variant-id", "/args/variant");
        Self {
            id: r.id,
            agent_id: r.agent_id,
            agent_name: r.agent_name,
            event_id: r.event_id,
            status: r.status,
            attempts: r.attempts,
            data,
            task,
            variant,
            started_at: r.started_at,
            finished_at: r.finished_at,
            created_at: r.created_at,
            log_path: r.log_path,
            loop_iter: r.loop_iter,
            loop_total: r.loop_total,
            loop_group_id: r.loop_group_id,
        }
    }
}

const SELECT_COLS: &str = "id, agent_id, agent_name, event_id, status, attempts, data, \
    started_at, finished_at, created_at, log_path, \
    loop_iter, loop_total, loop_group_id";

#[derive(Clone, Default)]
pub struct AgentRunRepository;

impl AgentRunRepository {
    pub fn new() -> Self {
        Self
    }

    pub async fn create<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
        agent_id: &str,
        agent_name: &str,
        event_id: Option<i64>,
        data: &Json,
    ) -> Result<i64>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO agent_runs \
               (workspace_id, agent_id, agent_name, event_id, status, data, started_at) \
             VALUES (?, ?, ?, ?, ?, ?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) RETURNING id",
        )
        .bind(ws_id)
        .bind(agent_id)
        .bind(agent_name)
        .bind(event_id)
        .bind(STATUS_RUNNING)
        .bind(data.to_string())
        .fetch_one(exec)
        .await?;
        Ok(id)
    }

    pub async fn set_status<'e, E>(&self, exec: E, id: i64, status: &str) -> Result<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        sqlx::query(
            "UPDATE agent_runs SET status = ?, \
                finished_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
             WHERE id = ?",
        )
        .bind(status)
        .bind(id)
        .execute(exec)
        .await?;
        Ok(())
    }

    pub async fn set_log_path<'e, E>(&self, exec: E, id: i64, log_path: &str) -> Result<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        sqlx::query("UPDATE agent_runs SET log_path = ? WHERE id = ?")
            .bind(log_path)
            .bind(id)
            .execute(exec)
            .await?;
        Ok(())
    }

    pub async fn set_loop_meta<'e, E>(
        &self,
        exec: E,
        id: i64,
        loop_iter: i64,
        loop_total: i64,
        loop_group_id: i64,
    ) -> Result<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        sqlx::query(
            "UPDATE agent_runs \
                SET loop_iter = ?, loop_total = ?, loop_group_id = ? \
              WHERE id = ?",
        )
        .bind(loop_iter)
        .bind(loop_total)
        .bind(loop_group_id)
        .bind(id)
        .execute(exec)
        .await?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create_iteration<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
        agent_id: &str,
        agent_name: &str,
        event_id: Option<i64>,
        data: &str,
        loop_iter: i64,
        loop_total: i64,
        loop_group_id: i64,
    ) -> Result<i64>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO agent_runs \
                (workspace_id, agent_id, agent_name, event_id, status, data, started_at, \
                 loop_iter, loop_total, loop_group_id) \
             VALUES (?, ?, ?, ?, ?, ?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), ?, ?, ?) \
             RETURNING id",
        )
        .bind(ws_id)
        .bind(agent_id)
        .bind(agent_name)
        .bind(event_id)
        .bind(STATUS_RUNNING)
        .bind(data)
        .bind(loop_iter)
        .bind(loop_total)
        .bind(loop_group_id)
        .fetch_one(exec)
        .await?;
        Ok(id)
    }

    pub async fn get<'e, E>(&self, exec: E, id: i64) -> Result<Option<AgentRun>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        Ok(sqlx::query_as::<_, AgentRunRecord>(&format!(
            "SELECT {SELECT_COLS} FROM agent_runs WHERE id = ?"
        ))
        .bind(id)
        .fetch_optional(exec)
        .await?
        .map(AgentRun::from))
    }

    pub async fn list_recent<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
        limit: i64,
    ) -> Result<Vec<AgentRun>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        // Hide runs of a soft-deleted task (its task-id lives in the run's args
        // JSON); runs with no task-id or a still-live task pass through.
        let rows = sqlx::query_as::<_, AgentRunRecord>(&format!(
            "SELECT {SELECT_COLS} FROM agent_runs ar \
         WHERE ar.workspace_id = ? \
           AND NOT EXISTS ( \
             SELECT 1 FROM tasks t \
             WHERE t.workspace_id = ar.workspace_id \
               AND t.id = json_extract(ar.data, '$.args.\"task-id\"') \
               AND t.deleted_at IS NOT NULL ) \
         ORDER BY ar.id DESC LIMIT ?"
        ))
        .bind(ws_id)
        .bind(limit)
        .fetch_all(exec)
        .await?;
        Ok(rows.into_iter().map(AgentRun::from).collect())
    }

    pub async fn list_for_agent<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
        agent_id: &str,
        limit: i64,
    ) -> Result<Vec<AgentRun>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        // Hide runs of a soft-deleted task (its task-id lives in the run's args
        // JSON); runs with no task-id or a still-live task pass through.
        let rows = sqlx::query_as::<_, AgentRunRecord>(&format!(
            "SELECT {SELECT_COLS} FROM agent_runs ar \
         WHERE ar.workspace_id = ? AND ar.agent_id = ? \
           AND NOT EXISTS ( \
             SELECT 1 FROM tasks t \
             WHERE t.workspace_id = ar.workspace_id \
               AND t.id = json_extract(ar.data, '$.args.\"task-id\"') \
               AND t.deleted_at IS NOT NULL ) \
         ORDER BY ar.id DESC LIMIT ?"
        ))
        .bind(ws_id)
        .bind(agent_id)
        .bind(limit)
        .fetch_all(exec)
        .await?;
        Ok(rows.into_iter().map(AgentRun::from).collect())
    }

    /// Delete runs referencing `task_id` (via `data.args."task-id"`);
    /// returns `log_path` of each deleted row for cleanup.
    pub async fn delete_for_task_returning_log_paths<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
        task_id: &str,
    ) -> Result<Vec<Option<String>>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let rows: Vec<(Option<String>,)> = sqlx::query_as(
            "DELETE FROM agent_runs \
             WHERE workspace_id = ? \
               AND json_extract(data, '$.args.\"task-id\"') = ? \
             RETURNING log_path",
        )
        .bind(ws_id)
        .bind(task_id)
        .fetch_all(exec)
        .await?;
        Ok(rows.into_iter().map(|(p,)| p).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(data: &str) -> AgentRunRecord {
        AgentRunRecord {
            id: 1,
            agent_id: "a1".into(),
            agent_name: "worker".into(),
            event_id: Some(5),
            status: "running".into(),
            attempts: 1,
            data: data.into(),
            started_at: None,
            finished_at: None,
            created_at: "2026-01-01T00:00:00Z".into(),
            log_path: None,
            loop_iter: 0,
            loop_total: 1,
            loop_group_id: None,
        }
    }

    #[test]
    fn extracts_task_and_variant_from_hyphenated_args() {
        let run = AgentRun::from(record(r#"{"args":{"task-id":"t1","variant-id":"v1"}}"#));
        assert_eq!(run.task.as_deref(), Some("t1"));
        assert_eq!(run.variant.as_deref(), Some("v1"));
    }

    #[test]
    fn extracts_task_and_variant_from_alias_args() {
        let run = AgentRun::from(record(r#"{"args":{"task":"t2","variant":"v2"}}"#));
        assert_eq!(run.task.as_deref(), Some("t2"));
        assert_eq!(run.variant.as_deref(), Some("v2"));
    }

    #[test]
    fn missing_args_yield_none_and_default_data() {
        let run = AgentRun::from(record("not json"));
        assert_eq!(run.task, None);
        assert_eq!(run.variant, None);
        assert_eq!(run.data, Json::Object(Default::default()));
    }
}
