//! Per-task agent settings. Identity = `(workspace_id, task_id, agent_id)`.
//! Settings are columns (only `loop_amount` today); the spawn path reads
//! `loop_amount` from here, defaulting to 1 when no row exists.
//!
//! Every method takes a caller-provided executor (`&pool` for a standalone
//! statement, or `&mut *tx` to enlist in a service-owned transaction) so the
//! service layer owns the unit of work. The repository is a stateless namespace
//! for the SQL — it never holds the pool.

use modula_types::{AgentLoop, TaskAgentSetting};
use sqlx::{Executor, Sqlite};

use crate::{Error, Result};

/// Raw `task_agent_settings` columns. Private serialization detail: the
/// repository maps it into the [`TaskAgentSetting`] domain type at its boundary,
/// nesting the flat `loop_amount` into the `{type: "fixed", amount}` loop shape.
#[derive(Debug, Clone, sqlx::FromRow)]
struct TaskAgentSettingRecord {
    agent_id: String,
    loop_amount: i64,
}

impl From<TaskAgentSettingRecord> for TaskAgentSetting {
    fn from(r: TaskAgentSettingRecord) -> Self {
        TaskAgentSetting {
            agent_id: r.agent_id,
            loop_setting: AgentLoop {
                kind: "fixed".into(),
                amount: r.loop_amount,
            },
        }
    }
}

#[derive(Clone, Default)]
pub struct TaskAgentSettingsRepository;

impl TaskAgentSettingsRepository {
    pub fn new() -> Self {
        Self
    }

    pub async fn get<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
        task_id: &str,
        agent_id: &str,
    ) -> Result<Option<TaskAgentSetting>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        Ok(sqlx::query_as::<_, TaskAgentSettingRecord>(
            "SELECT agent_id, loop_amount FROM task_agent_settings \
             WHERE workspace_id = ? AND task_id = ? AND agent_id = ?",
        )
        .bind(ws_id)
        .bind(task_id)
        .bind(agent_id)
        .fetch_optional(exec)
        .await?
        .map(TaskAgentSetting::from))
    }

    pub async fn list_for_task<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
        task_id: &str,
    ) -> Result<Vec<TaskAgentSetting>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        Ok(sqlx::query_as::<_, TaskAgentSettingRecord>(
            "SELECT agent_id, loop_amount FROM task_agent_settings \
             WHERE workspace_id = ? AND task_id = ? ORDER BY agent_id",
        )
        .bind(ws_id)
        .bind(task_id)
        .fetch_all(exec)
        .await?
        .into_iter()
        .map(TaskAgentSetting::from)
        .collect())
    }

    pub async fn upsert<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
        task_id: &str,
        agent_id: &str,
        loop_amount: i64,
    ) -> Result<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        sqlx::query(
            "INSERT INTO task_agent_settings (workspace_id, task_id, agent_id, loop_amount) \
             VALUES (?, ?, ?, ?) \
             ON CONFLICT(workspace_id, task_id, agent_id) \
             DO UPDATE SET loop_amount = excluded.loop_amount",
        )
        .bind(ws_id)
        .bind(task_id)
        .bind(agent_id)
        .bind(loop_amount)
        .execute(exec)
        .await?;
        Ok(())
    }

    pub async fn delete<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
        task_id: &str,
        agent_id: &str,
    ) -> Result<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let res = sqlx::query(
            "DELETE FROM task_agent_settings \
             WHERE workspace_id = ? AND task_id = ? AND agent_id = ?",
        )
        .bind(ws_id)
        .bind(task_id)
        .bind(agent_id)
        .execute(exec)
        .await?;
        if res.rows_affected() == 0 {
            return Err(Error::NotFound(format!(
                "no agent settings for agent {agent_id} on task {task_id}"
            )));
        }
        Ok(())
    }
}
