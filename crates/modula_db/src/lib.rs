//! `modula-db` — SQLite repositories. Each `*Repository` is a stateless unit
//! struct owning the SQL for one entity; the caller supplies the executor (a
//! pool ref for single statements, or a `&mut SqliteConnection` for
//! multi-statement writes) and owns the unit of work, so services compose
//! transactions over them. The shared error is
//! [`modula_rpc::status::DomainError`]; gRPC handlers map it to `tonic::Status`
//! at the edge.

use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use sqlx::sqlite::{
    SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions, SqliteSynchronous,
};

pub use modula_rpc::status::DomainError as Error;
pub type Result<T> = std::result::Result<T, Error>;

mod search;
mod slug;

pub mod agent_processes;
pub mod agent_runs;
pub mod agent_skills;
pub mod agents;
pub mod conversations;
pub mod events;
pub mod integrations;
pub mod labels;
pub mod pipeline;
pub mod projects;
pub mod providers;
pub mod roadmap;
pub mod settings;
pub mod task_agent_settings;
pub mod tasks;
pub mod threads;
pub mod variants;
pub mod workspaces;

/// The shared SQLite pool. Repositories are cheap `Clone`s over it.
pub type Database = SqlitePool;

/// Open the pool, run migrations, and backfill developer-owned defaults. One
/// global DB at `<modula>/db.sqlite`.
pub async fn open(path: &Path) -> anyhow::Result<Database> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let opts = SqliteConnectOptions::from_str(&format!("sqlite://{}", path.display()))?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(5));

    // Sqlite serializes writes; one connection matches the WAL one-writer
    // model and avoids spurious SQLITE_BUSY across competing pool conns.
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await?;

    // Plugins add their own migrations to the same `_sqlx_migrations` table, so
    // this migrator must tolerate applied versions it does not own.
    let mut migrator = sqlx::migrate!("./migrations");
    migrator.set_ignore_missing(true);
    migrator.run(&pool).await?;
    let mut tx = pool.begin().await?;
    agent_skills::AgentSkillRepository::new()
        .sync_all(&mut tx)
        .await?;
    agents::AgentRepository::new()
        .sync_missing_defaults(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn open_creates_schema() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.sqlite");
        let pool = open(&path).await.expect("open db");
        let tables: Vec<String> = sqlx::query_scalar(
            "SELECT name FROM sqlite_master \
             WHERE type='table' AND name NOT LIKE 'sqlite_%' AND name <> '_sqlx_migrations' \
             ORDER BY name",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        for expected in [
            "agent_processes",
            "agent_runs",
            "agent_skills",
            "agents",
            "conversations",
            "events",
            "integrations",
            "labels",
            "pipeline_statuses",
            "projects",
            "providers",
            "roadmap_rows",
            "task_agent_settings",
            "task_labels",
            "thread_entries",
            "tasks",
            "variants",
            "workspace_settings",
            "workspaces",
        ] {
            assert!(
                tables.iter().any(|t| t == expected),
                "missing table: {expected} (have {tables:?})"
            );
        }
    }
}
