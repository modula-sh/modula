//! Track live spawned agent PIDs. The dispatcher's reaper walks this
//! table (woken by SIGCHLD or the 1s safety-net tick), checks each pid via
//! `platform::ProcessManager::is_alive`, and flips the matching `agent_runs`
//! row to `completed` once the process has exited.

//! Every method takes a caller-provided executor (`&pool` for a standalone
//! statement, or `&mut *tx` to enlist in a service-owned transaction) so the
//! service layer owns the unit of work. The repository is a stateless namespace
//! for the SQL — it never holds the pool.

use sqlx::{Executor, Sqlite};

use crate::Result;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AgentProcessRow {
    pub pid: i64,
    pub workspace_id: String,
    pub agent_id: String,
    pub agent_name: String,
    pub agent_run_id: i64,
}

/// One live-process row joined with its `agent_runs` row. An internal query
/// projection (no wire type): `ProcessesService` assembles it into the
/// `RunningAgent` domain type / dashboard JSON, extracting `task`/`variant`/
/// `spec`/`branch` from the run's `data` column.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RunningAgentRecord {
    pub pid: i64,
    pub agent_id: String,
    pub agent_name: String,
    pub agent_run_id: i64,
    pub data: String,
    pub started_at: Option<String>,
    pub run_created_at: String,
}

#[derive(Clone, Default)]
pub struct AgentProcessRepository;

impl AgentProcessRepository {
    pub fn new() -> Self {
        Self
    }

    pub async fn create<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
        agent_id: &str,
        agent_name: &str,
        agent_run_id: i64,
        pid: u32,
    ) -> Result<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        sqlx::query(
            "INSERT INTO agent_processes (pid, workspace_id, agent_id, agent_name, agent_run_id) \
             VALUES (?, ?, ?, ?, ?) \
             ON CONFLICT(pid) DO UPDATE SET \
                workspace_id = excluded.workspace_id, \
                agent_id = excluded.agent_id, \
                agent_name = excluded.agent_name, \
                agent_run_id = excluded.agent_run_id",
        )
        .bind(pid as i64)
        .bind(ws_id)
        .bind(agent_id)
        .bind(agent_name)
        .bind(agent_run_id)
        .execute(exec)
        .await?;
        Ok(())
    }

    pub async fn list_for_workspace<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
    ) -> Result<Vec<AgentProcessRow>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        Ok(sqlx::query_as::<_, AgentProcessRow>(
            "SELECT pid, workspace_id, agent_id, agent_name, agent_run_id \
             FROM agent_processes WHERE workspace_id = ?",
        )
        .bind(ws_id)
        .fetch_all(exec)
        .await?)
    }

    pub async fn list_running_for_workspace<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
    ) -> Result<Vec<RunningAgentRecord>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        Ok(sqlx::query_as::<_, RunningAgentRecord>(
            "SELECT p.pid, p.agent_id, p.agent_name, p.agent_run_id, \
                    r.data AS data, r.started_at AS started_at, \
                    r.created_at AS run_created_at \
             FROM agent_processes p \
             JOIN agent_runs r ON r.id = p.agent_run_id \
             WHERE p.workspace_id = ? \
             ORDER BY r.created_at ASC",
        )
        .bind(ws_id)
        .fetch_all(exec)
        .await?)
    }

    pub async fn exists<'e, E>(&self, exec: E, ws_id: &str, pid: i64) -> Result<bool>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let n: i64 = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM agent_processes WHERE workspace_id = ? AND pid = ?)",
        )
        .bind(ws_id)
        .bind(pid)
        .fetch_one(exec)
        .await?;
        Ok(n != 0)
    }

    pub async fn delete<'e, E>(&self, exec: E, pid: i64) -> Result<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        sqlx::query("DELETE FROM agent_processes WHERE pid = ?")
            .bind(pid)
            .execute(exec)
            .await?;
        Ok(())
    }
}
