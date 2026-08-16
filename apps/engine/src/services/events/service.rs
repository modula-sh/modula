//! `EventService` — the business layer behind the `EventService` gRPC handler
//! and the engine's only publish path. Every event is persisted to the DB
//! event log (dispatcher rule-matching, backfill) before it is broadcast on
//! the [`Bus`] (live watch streams); an event can never reach a subscriber
//! without its durable row. The raw log path (`list`) returns
//! [`EventRecord`]s; the typed backfill (`list_recent`) decodes them into
//! [`WorkspaceEvent`]s.

use async_trait::async_trait;
use serde_json::Value as Json;

use modula_db::events::{EventRecord, EventRepository};
use modula_db::workspaces::WorkspaceRepository;
use modula_db::Database;
use modula_types::WorkspaceEvent;

use crate::core::error::{ApiError, ApiResult};
use crate::services::events::{Bus, EventSink};

const BACKFILL_LIMIT: i64 = 200;

#[derive(Clone)]
pub struct EventService {
    pool: Database,
    events: EventRepository,
    workspaces: WorkspaceRepository,
    bus: Bus,
}

impl EventService {
    pub fn new(
        pool: Database,
        events: EventRepository,
        workspaces: WorkspaceRepository,
        bus: Bus,
    ) -> Self {
        Self {
            pool,
            events,
            workspaces,
            bus,
        }
    }

    /// The single publish path: validate, persist to the event log, then
    /// broadcast to live watch-stream subscribers. Returns the new row id.
    pub async fn publish(&self, ws: &str, type_: &str, data: Json) -> ApiResult<i64> {
        self.workspaces.get(&self.pool, ws).await?;
        let type_ = type_.trim();
        if type_.is_empty() {
            return Err(ApiError::BadRequest("type is required".into()));
        }
        let id = self.events.create(&self.pool, ws, type_, &data).await?;
        self.bus.broadcast(ws, type_, data).await;
        Ok(id)
    }

    /// Recent events for the workspace, newest first. Existence is validated so
    /// an unknown workspace surfaces a 404 rather than an empty list.
    pub async fn list(&self, ws: &str) -> ApiResult<Vec<EventRecord>> {
        self.workspaces.get(&self.pool, ws).await?;
        Ok(self
            .events
            .list_recent(&self.pool, ws, BACKFILL_LIMIT)
            .await?)
    }

    /// Incremental typed backfill: at most `limit` recent events (defaulting
    /// when non-positive), filtered to ids strictly greater than `after_seq`
    /// and decoded to [`WorkspaceEvent`]s (records whose type has no typed
    /// kind are skipped, matching the watch stream). DB rows have no bus seq,
    /// so the row id is the ordering key.
    pub async fn list_recent(
        &self,
        ws: &str,
        limit: i64,
        after_seq: i64,
    ) -> ApiResult<Vec<WorkspaceEvent>> {
        self.workspaces.get(&self.pool, ws).await?;
        let limit = if limit > 0 { limit } else { BACKFILL_LIMIT };
        let rows = self.events.list_recent(&self.pool, ws, limit).await?;
        Ok(rows
            .into_iter()
            .filter(|r| after_seq == 0 || r.id > after_seq)
            .filter_map(|r| {
                WorkspaceEvent::from_parts(r.id, ws, &r.created_at, &r.type_, &r.data_json())
            })
            .collect())
    }
}

/// Fire-and-forget publish for internal domain events: the same single path,
/// with failures logged instead of bubbled.
#[async_trait]
impl EventSink for EventService {
    async fn publish(&self, ws: &str, type_: &str, data: Json) {
        if let Err(e) = EventService::publish(self, ws, type_, data).await {
            tracing::warn!("[events] publish {type_} for {ws} failed: {e}");
        }
    }
}
