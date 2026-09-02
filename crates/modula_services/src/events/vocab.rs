//! Event payload shaping and the event-type vocabulary. The `type` constants
//! live in [`modula_types::event_types`] (next to the
//! `WorkspaceEvent::from_parts` decoder, so publish and decode can't drift)
//! and are re-exported here so `events::TASK_CREATE`-style references resolve
//! unchanged. Storage (insert/query) lives in [`modula_db::events`]; live
//! broadcast lives in the engine's bus.

use serde_json::{json, Map, Value as Json};

pub use modula_types::event_types::*;

/// Shape an update-event payload: the changed fields plus identity keys (e.g.
/// `task_id`) for routing. Callers skip publishing when `changed` is empty.
pub fn update_event(identity: &[(&str, Json)], changed: Map<String, Json>) -> Json {
    let mut map = changed;
    for (k, v) in identity {
        map.insert((*k).to_string(), v.clone());
    }
    Json::Object(map)
}

/// Canonical `variant.update` payload. The single source of truth for this
/// event's shape, shared by the real publish (variant status update) and the
/// dispatcher's synthetic per-variant fan-out, so the two can't drift.
pub fn variant_update(task_id: &str, variant_id: &str, status: Json) -> Json {
    let mut changed = Map::new();
    changed.insert("status".into(), status);
    update_event(
        &[
            ("task_id", json!(task_id)),
            ("variant_id", json!(variant_id)),
        ],
        changed,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_event_merges_identity_and_changes() {
        let mut changed = Map::new();
        changed.insert("status".into(), json!("accepted"));
        let ev = update_event(
            &[("task_id", json!("T1")), ("variant_id", json!("V1"))],
            changed,
        );
        assert_eq!(ev["task_id"], json!("T1"));
        assert_eq!(ev["variant_id"], json!("V1"));
        assert_eq!(ev["status"], json!("accepted"));
    }

    #[test]
    fn variant_update_has_canonical_shape() {
        let ev = variant_update("T1", "V1", json!("ready_for_workers"));
        assert_eq!(
            ev,
            json!({"task_id": "T1", "variant_id": "V1", "status": "ready_for_workers"})
        );
    }
}
