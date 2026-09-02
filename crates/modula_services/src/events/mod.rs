//! Workspace events: the [`EventSink`] trait, the re-exported in-process
//! broadcast [`Bus`], and the [`EventService`] that owns the single publish path
//! (persist to the DB event log, then broadcast). The event-type vocabulary
//! and payload-shaping helpers live in [`vocab`], re-exported here so
//! `crate::events::*` references resolve unchanged.

pub mod service;
pub mod vocab;
/// In `modula-plugin` so plugins subscribe to the same stream; re-exported
/// here as the engine's own publish path.
pub use modula_plugin::{Bus, BusEvent};
pub use service::EventService;
pub use vocab::*;

use async_trait::async_trait;
use serde_json::Value as Json;
use sqlx::SqliteConnection;

/// An event already persisted inside the caller's transaction, waiting to be
/// broadcast once that transaction commits. `seq` is the event-log row id.
pub struct PendingEvent {
    pub seq: i64,
    pub type_: String,
    pub data: Json,
}

/// Fire-and-forget sink for workspace domain events, implemented by
/// [`EventService`] (the only publish path). Services call
/// [`EventSink::publish`] after a successful write; failures are logged, not
/// bubbled — a mutation succeeding without its event leaves the engine
/// consistent.
#[async_trait]
pub trait EventSink: Send + Sync {
    async fn publish(&self, ws: &str, type_: &str, data: Json);

    /// Persist an event on a connection the caller already owns, so the event
    /// row commits with the write it describes. The caller broadcasts it with
    /// [`EventSink::emit`] after committing; nothing is on the bus until then.
    async fn record(
        &self,
        conn: &mut SqliteConnection,
        ws: &str,
        type_: &str,
        data: Json,
    ) -> Option<PendingEvent>;

    /// Broadcast a committed [`PendingEvent`]. `None` (the record failed) is a
    /// no-op, so callers need no branch.
    async fn emit(&self, ws: &str, pending: Option<PendingEvent>);
}
