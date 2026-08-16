use std::collections::BTreeMap;

use modula_rpc::convert::{kind_to_str, str_to_kind, str_to_verdict, verdict_to_str};
use modula_rpc::v1 as pb;
use serde::{Deserialize, Serialize};

/// One thread entry. `author`/`kind`/`verdict` are the wire strings the engine
/// stores and agents pass on the CLI. Entries carry no task/variant ids: the
/// enclosing bundle groups them by variant, so they would be redundant here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThreadEntry {
    pub id: i64,
    pub ts: String,
    pub author: String,
    pub kind: String,
    pub round: Option<i64>,
    pub content: String,
    pub verdict: Option<String>,
    pub affected_variants: Vec<String>,
}

impl From<pb::ThreadEntry> for ThreadEntry {
    fn from(e: pb::ThreadEntry) -> Self {
        Self {
            id: e.id,
            ts: e.ts,
            author: e.author,
            kind: kind_to_str(e.kind).to_string(),
            round: e.round,
            content: e.content,
            verdict: e.verdict.and_then(verdict_to_str).map(str::to_string),
            affected_variants: e.affected_variants,
        }
    }
}

impl From<ThreadEntry> for pb::ThreadEntry {
    fn from(e: ThreadEntry) -> Self {
        Self {
            id: e.id,
            ts: e.ts,
            author: e.author,
            kind: str_to_kind(&e.kind),
            round: e.round,
            content: e.content,
            verdict: e.verdict.as_deref().and_then(str_to_verdict),
            affected_variants: e.affected_variants,
        }
    }
}

/// A task's full thread: the task-scoped entries plus each variant's entries
/// keyed by variant id (`dto::threads` / frontend `ThreadsResponse`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThreadBundle {
    pub task: String,
    pub task_thread: Vec<ThreadEntry>,
    pub variant_threads: BTreeMap<String, Vec<ThreadEntry>>,
}

impl From<pb::GetThreadsResponse> for ThreadBundle {
    fn from(r: pb::GetThreadsResponse) -> Self {
        Self {
            task: r.task_id,
            task_thread: r.task_thread.into_iter().map(ThreadEntry::from).collect(),
            variant_threads: r
                .variant_threads
                .into_iter()
                .map(|vt| {
                    (
                        vt.variant_id,
                        vt.entries.into_iter().map(ThreadEntry::from).collect(),
                    )
                })
                .collect(),
        }
    }
}

impl From<ThreadBundle> for pb::GetThreadsResponse {
    fn from(b: ThreadBundle) -> Self {
        Self {
            task_id: b.task,
            task_thread: b
                .task_thread
                .into_iter()
                .map(pb::ThreadEntry::from)
                .collect(),
            variant_threads: b
                .variant_threads
                .into_iter()
                .map(|(variant_id, entries)| pb::VariantThread {
                    variant_id,
                    entries: entries.into_iter().map(pb::ThreadEntry::from).collect(),
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn entry() -> ThreadEntry {
        ThreadEntry {
            id: 7,
            ts: "2026-01-01T00:00:00Z".into(),
            author: "my-custom-agent".into(),
            kind: "verdict".into(),
            round: Some(2),
            content: "looks good".into(),
            verdict: Some("ACCEPT".into()),
            affected_variants: vec![],
        }
    }

    fn bundle() -> ThreadBundle {
        ThreadBundle {
            task: "t1".into(),
            task_thread: vec![ThreadEntry {
                id: 1,
                ts: "2026-01-01T00:00:00Z".into(),
                author: "human".into(),
                kind: "comment".into(),
                round: None,
                content: "hi".into(),
                verdict: None,
                affected_variants: vec![],
            }],
            variant_threads: BTreeMap::from([("v1".to_string(), vec![entry()])]),
        }
    }

    #[test]
    fn entry_round_trip() {
        let d = entry();
        assert_eq!(d, ThreadEntry::from(pb::ThreadEntry::from(d.clone())));
    }

    #[test]
    fn bundle_round_trip() {
        let d = bundle();
        assert_eq!(
            d,
            ThreadBundle::from(pb::GetThreadsResponse::from(d.clone()))
        );
    }

    // Locks the JSON the frontend (`ThreadEntry` / `ThreadsResponse`) consumes
    // today via `dto::thread_entry` / `dto::threads`.
    #[test]
    fn bundle_serde_matches_dto() {
        let got = serde_json::to_value(bundle()).unwrap();
        let want = json!({
            "task": "t1",
            "task_thread": [{
                "id": 1, "ts": "2026-01-01T00:00:00Z", "author": "human",
                "kind": "comment", "round": null, "content": "hi",
                "verdict": null, "affected_variants": [],
            }],
            "variant_threads": {
                "v1": [{
                    "id": 7, "ts": "2026-01-01T00:00:00Z", "author": "my-custom-agent",
                    "kind": "verdict", "round": 2, "content": "looks good",
                    "verdict": "ACCEPT", "affected_variants": [],
                }],
            },
        });
        assert_eq!(got, want);
    }
}
