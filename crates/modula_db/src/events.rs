//! Event log. The engine's `EventService::publish` inserts one row per
//! published event; the central dispatcher reads unprocessed rows and matches
//! them against each agent's `rules`.
//!
//! Every method takes a caller-provided executor (`&pool` for a standalone
//! statement, or `&mut *tx` to enlist in a service-owned transaction) so the
//! caller owns the unit of work. The repository is a stateless namespace for
//! the SQL — it never holds the pool.

use serde_json::Value as Json;
use sqlx::{Executor, Sqlite};

use crate::Result;

/// A raw event-log record, not a domain type: the dispatcher rule-matches on
/// the raw `type`/`data` strings (the typed `WorkspaceEvent` decode would lose
/// them) and `processed` is log-cursor bookkeeping with no wire meaning. The
/// typed view is `modula_types::WorkspaceEvent::from_parts`, applied by the
/// engine's `EventService` for the watch/backfill paths.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct EventRecord {
    pub id: i64,
    #[sqlx(rename = "type")]
    pub type_: String,
    pub data: String,
    pub processed: bool,
    pub created_at: String,
}

impl EventRecord {
    /// The payload as JSON, tolerating a corrupt column (`{}`).
    pub fn data_json(&self) -> Json {
        serde_json::from_str(&self.data).unwrap_or(Json::Object(Default::default()))
    }
}

const SELECT_COLS: &str = "id, type, data, processed, created_at";

/// How long the event log is retained, bounding the range any consumer can
/// replay from. The dispatcher prunes to this window.
pub const EVENT_RETENTION_DAYS: i64 = 30;

#[derive(Clone, Default)]
pub struct EventRepository;

impl EventRepository {
    pub fn new() -> Self {
        Self
    }

    pub async fn create<'e, E>(&self, exec: E, ws_id: &str, type_: &str, data: &Json) -> Result<i64>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO events (workspace_id, type, data) VALUES (?, ?, ?) RETURNING id",
        )
        .bind(ws_id)
        .bind(type_)
        .bind(data.to_string())
        .fetch_one(exec)
        .await?;
        Ok(id)
    }

    pub async fn list_recent<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
        limit: i64,
    ) -> Result<Vec<EventRecord>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        Ok(sqlx::query_as::<_, EventRecord>(&format!(
            "SELECT {SELECT_COLS} FROM events WHERE workspace_id = ? \
             ORDER BY id DESC LIMIT ?"
        ))
        .bind(ws_id)
        .bind(limit)
        .fetch_all(exec)
        .await?)
    }

    /// Lowest and highest retained event id for the workspace; `None` when the
    /// log is empty. Bounds the sync feed's resumable range.
    pub async fn id_range<'e, E>(&self, exec: E, ws_id: &str) -> Result<Option<(i64, i64)>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let row: (Option<i64>, Option<i64>) =
            sqlx::query_as("SELECT MIN(id), MAX(id) FROM events WHERE workspace_id = ?")
                .bind(ws_id)
                .fetch_one(exec)
                .await?;
        Ok(row.0.zip(row.1))
    }

    /// Events with `id > after`, oldest first.
    pub async fn list_after<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
        after: i64,
        limit: i64,
    ) -> Result<Vec<EventRecord>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        Ok(sqlx::query_as::<_, EventRecord>(&format!(
            "SELECT {SELECT_COLS} FROM events WHERE workspace_id = ? AND id > ? \
             ORDER BY id ASC LIMIT ?"
        ))
        .bind(ws_id)
        .bind(after)
        .bind(limit)
        .fetch_all(exec)
        .await?)
    }

    /// Unprocessed events, oldest first, within the freshness window.
    pub async fn list_unprocessed<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
        max_age_secs: i64,
        limit: i64,
    ) -> Result<Vec<EventRecord>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        Ok(sqlx::query_as::<_, EventRecord>(&format!(
            "SELECT {SELECT_COLS} FROM events \
             WHERE workspace_id = ? AND processed = 0 \
               AND created_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?) \
             ORDER BY id ASC LIMIT ?"
        ))
        .bind(ws_id)
        .bind(format!("-{max_age_secs} seconds"))
        .bind(limit)
        .fetch_all(exec)
        .await?)
    }

    /// Drop events created before `cutoff` (an RFC3339 string, compared
    /// lexicographically like the rest of the engine's timestamps), across all
    /// workspaces. Returns the number of rows removed.
    pub async fn prune_before<'e, E>(&self, exec: E, cutoff: &str) -> Result<u64>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        Ok(sqlx::query("DELETE FROM events WHERE created_at < ?")
            .bind(cutoff)
            .execute(exec)
            .await?
            .rows_affected())
    }

    pub async fn mark_processed<'e, E>(&self, exec: E, id: i64) -> Result<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        sqlx::query("UPDATE events SET processed = 1 WHERE id = ?")
            .bind(id)
            .execute(exec)
            .await?;
        Ok(())
    }
}
