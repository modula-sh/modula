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
/// The host-wide channel carries every workspace's events.
const ALL_CHANNEL_CAP: usize = 1024;

struct Inner {
    senders: Mutex<HashMap<String, broadcast::Sender<BusEvent>>>,
    all: broadcast::Sender<BusEvent>,
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
                all: broadcast::channel(ALL_CHANNEL_CAP).0,
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
        let _ = self.inner.all.send(event.clone());
        let guard = self.inner.senders.lock().await;
        if let Some(tx) = guard.get(ws_id) {
            if let Err(e) = tx.send(event) {
                tracing::debug!("[bus] no active receivers for ws {ws_id}: {e}");
            }
        }
    }

    /// Subscribe to every workspace's events.
    pub fn subscribe_all(&self) -> broadcast::Receiver<BusEvent> {
        self.inner.all.subscribe()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn subscribe_all_sees_every_workspace() {
        let bus = Bus::new();
        let mut all = bus.subscribe_all();
        let mut b = bus.subscribe("b").await;
        bus.broadcast("a", 1, "task.update", Json::Null).await;
        bus.broadcast("b", 2, "task.update", Json::Null).await;
        assert_eq!(all.recv().await.unwrap().workspace_id, "a");
        assert_eq!(all.recv().await.unwrap().workspace_id, "b");
        assert_eq!(b.recv().await.unwrap().seq, 2);
    }
}
