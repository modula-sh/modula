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

use crate::events::{Bus, EventSink, PendingEvent};
use modula_core::error::{ApiError, ApiResult};

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
        self.bus.broadcast(ws, id, type_, data).await;
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

    /// The split path. No workspace or type validation: the caller is already
    /// inside a validated transaction on this workspace.
    async fn record(
        &self,
        conn: &mut sqlx::SqliteConnection,
        ws: &str,
        type_: &str,
        data: Json,
    ) -> Option<PendingEvent> {
        match self.events.create(&mut *conn, ws, type_, &data).await {
            Ok(seq) => Some(PendingEvent {
                seq,
                type_: type_.to_string(),
                data,
            }),
            Err(e) => {
                tracing::warn!("[events] record {type_} for {ws} failed: {e}");
                None
            }
        }
    }

    async fn emit(&self, ws: &str, pending: Option<PendingEvent>) {
        if let Some(p) = pending {
            self.bus.broadcast(ws, p.seq, &p.type_, p.data).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::env;
    use serde_json::json;

    /// `record` writes on the caller's connection, so an event exists only if
    /// the write it describes committed — and nothing reaches the bus before.
    #[tokio::test]
    async fn a_recorded_event_lives_and_dies_with_its_transaction() {
        let env = env().await;
        let svc = EventService::new(
            env.pool.clone(),
            EventRepository::new(),
            WorkspaceRepository::new(),
            Bus::new(),
        );
        let mut rx = svc.bus.subscribe(&env.ws).await;

        let mut tx = env.pool.begin().await.unwrap();
        let pending = svc
            .record(&mut tx, &env.ws, "task.update", json!({ "task_id": "T1" }))
            .await;
        assert!(pending.is_some());
        tx.rollback().await.unwrap();
        assert!(svc.list(&env.ws).await.unwrap().is_empty());
        assert!(rx.try_recv().is_err());

        let mut tx = env.pool.begin().await.unwrap();
        let pending = svc
            .record(&mut tx, &env.ws, "task.update", json!({ "task_id": "T2" }))
            .await;
        tx.commit().await.unwrap();
        svc.emit(&env.ws, pending).await;
        let rows = svc.list(&env.ws).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rx.try_recv().unwrap().seq, rows[0].id);
    }
}
