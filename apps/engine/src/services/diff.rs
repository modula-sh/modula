//! Generic JSON diffing for patch-style update events, so an event carries only
//! the fields that actually changed and edge-triggered rules fire on real
//! transitions. Table-agnostic and DB-unaware: a service diffs the current row
//! against validated incoming fields, then emits (see
//! [`crate::services::events::update_event`]).

use serde_json::{Map, Value as Json};

/// Top-level fields of `incoming` whose value differs from `current` (missing in
/// `current` counts as changed). Non-object inputs yield an empty diff.
/// `incoming` must hold only the fields the request actually sent.
pub fn changed_fields(current: &Json, incoming: &Json) -> Map<String, Json> {
    let cur = current.as_object();
    let Some(inc) = incoming.as_object() else {
        return Map::new();
    };
    inc.iter()
        .filter(|(k, v)| cur.and_then(|c| c.get(k.as_str())) != Some(v))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reports_only_changed_fields() {
        let current = json!({ "title": "a", "approved": true, "max_variants": null });
        // Title changes; approved is re-sent unchanged; max_variants untouched.
        let incoming = json!({ "title": "b", "approved": true });
        let diff = changed_fields(&current, &incoming);
        assert_eq!(diff.len(), 1);
        assert_eq!(diff.get("title"), Some(&json!("b")));
        assert!(!diff.contains_key("approved"));
    }

    #[test]
    fn null_transitions_are_changes() {
        let current = json!({ "approved": null });
        let incoming = json!({ "approved": true });
        assert_eq!(
            changed_fields(&current, &incoming).get("approved"),
            Some(&json!(true))
        );

        let current = json!({ "approved": true });
        let incoming = json!({ "approved": null });
        assert_eq!(
            changed_fields(&current, &incoming).get("approved"),
            Some(&json!(null))
        );
    }

    #[test]
    fn field_missing_from_current_counts_as_changed() {
        let current = json!({ "title": "a" });
        let incoming = json!({ "status": "in_progress" });
        assert_eq!(
            changed_fields(&current, &incoming).get("status"),
            Some(&json!("in_progress"))
        );
    }

    #[test]
    fn identical_payload_yields_empty_diff() {
        let current = json!({ "title": "a", "approved": false });
        let incoming = json!({ "title": "a", "approved": false });
        assert!(changed_fields(&current, &incoming).is_empty());
    }
}
