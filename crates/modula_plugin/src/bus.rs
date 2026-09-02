use std::collections::HashMap;
use std::sync::Arc;

use serde_json::Value as Json;
use tokio::sync::{broadcast, Mutex};

/// A single event on the workspace bus. `seq` is the event's durable
/// `events.id`, so backfill and live subscribers share one number space.
#[derive(Clone, Debug)]
pub struct BusEvent {
    pub seq: i64,
    pub workspace_id: String,
    pub type_: String,
    pub data: Json,
}

/// Per-workspace broadcast bus. Bounded channel; lagged subscribers get
/// `RecvError::Lagged` and are expected to log+skip — durability lives in
/// the DB, not here.
const CHANNEL_CAP: usize = 256;

struct Inner {
    senders: Mutex<HashMap<String, broadcast::Sender<BusEvent>>>,
}

/// Cloneable handle to the workspace event broadcast bus. The engine's single
/// publish path broadcasts here after persisting to the DB, so live
/// subscribers — including plugins — receive a push without polling.
#[derive(Clone)]
pub struct Bus {
    inner: Arc<Inner>,
}

impl Default for Bus {
    fn default() -> Self {
        Self {
            inner: Arc::new(Inner {
                senders: Mutex::new(HashMap::new()),
            }),
        }
    }
}

impl Bus {
    pub fn new() -> Self {
        Self::default()
    }

    /// Broadcast an event to all active subscribers for `ws_id`. Best-effort:
    /// if no subscriber is listening the event is silently dropped (the DB
    /// copy is the durable record).
    pub async fn broadcast(&self, ws_id: &str, seq: i64, type_: &str, data: Json) {
        let event = BusEvent {
            seq,
            workspace_id: ws_id.to_string(),
            type_: type_.to_string(),
            data,
        };
        let guard = self.inner.senders.lock().await;
        if let Some(tx) = guard.get(ws_id) {
            if let Err(e) = tx.send(event) {
                tracing::debug!("[bus] no active receivers for ws {ws_id}: {e}");
            }
        }
    }

    /// Subscribe to events for `ws_id`. Creates the channel on first call for
    /// a workspace; subsequent calls return additional receivers from the same
    /// sender.
    pub async fn subscribe(&self, ws_id: &str) -> broadcast::Receiver<BusEvent> {
        let mut guard = self.inner.senders.lock().await;
        let tx = guard.entry(ws_id.to_string()).or_insert_with(|| {
            let (tx, _) = broadcast::channel(CHANNEL_CAP);
            tx
        });
        tx.subscribe()
    }
}
