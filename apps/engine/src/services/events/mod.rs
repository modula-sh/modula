//! Workspace events: the [`EventSink`] trait, the in-process broadcast
//! [`Bus`], and the [`EventService`] that owns the single publish path
//! (persist to the DB event log, then broadcast). The event-type vocabulary
//! and payload-shaping helpers live in [`vocab`], re-exported here so
//! `crate::services::events::*` references resolve unchanged.

pub mod bus;
pub mod service;
pub mod vocab;
pub use bus::Bus;
pub use service::EventService;
pub use vocab::*;

use async_trait::async_trait;
use serde_json::Value as Json;

/// Fire-and-forget sink for workspace domain events, implemented by
/// [`EventService`] (the only publish path). Services call
/// [`EventSink::publish`] after a successful write; failures are logged, not
/// bubbled — a mutation succeeding without its event leaves the engine
/// consistent.
#[async_trait]
pub trait EventSink: Send + Sync {
    async fn publish(&self, ws: &str, type_: &str, data: Json);
}
