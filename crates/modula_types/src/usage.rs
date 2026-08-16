use modula_rpc::v1 as pb;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UsageTokens {
    pub input: i64,
    pub output: i64,
    pub cache_creation: i64,
    pub cache_read: i64,
}

impl From<pb::UsageTokens> for UsageTokens {
    fn from(t: pb::UsageTokens) -> Self {
        Self {
            input: t.input,
            output: t.output,
            cache_creation: t.cache_creation,
            cache_read: t.cache_read,
        }
    }
}

impl From<UsageTokens> for pb::UsageTokens {
    fn from(t: UsageTokens) -> Self {
        Self {
            input: t.input,
            output: t.output,
            cache_creation: t.cache_creation,
            cache_read: t.cache_read,
        }
    }
}

/// One finished agent run's usage record (`dto::usage_entry` / frontend
/// `UsageRecord`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UsageEntry {
    pub run_id: i64,
    pub log: String,
    pub agent: String,
    pub mtime: String,
    pub duration_ms: i64,
    pub cost_usd: f64,
    pub tokens: UsageTokens,
}

impl From<pb::UsageEntry> for UsageEntry {
    fn from(u: pb::UsageEntry) -> Self {
        Self {
            run_id: u.run_id,
            log: u.log,
            agent: u.agent,
            mtime: u.mtime,
            duration_ms: u.duration_ms,
            cost_usd: u.cost_usd,
            tokens: u.tokens.unwrap_or_default().into(),
        }
    }
}

impl From<UsageEntry> for pb::UsageEntry {
    fn from(u: UsageEntry) -> Self {
        Self {
            run_id: u.run_id,
            log: u.log,
            agent: u.agent,
            mtime: u.mtime,
            duration_ms: u.duration_ms,
            cost_usd: u.cost_usd,
            tokens: Some(u.tokens.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn entry() -> UsageEntry {
        UsageEntry {
            run_id: 1,
            log: "run.log".into(),
            agent: "worker".into(),
            mtime: "2026-01-01T00:00:00Z".into(),
            duration_ms: 1500,
            cost_usd: 0.25,
            tokens: UsageTokens {
                input: 10,
                output: 20,
                cache_creation: 0,
                cache_read: 5,
            },
        }
    }

    #[test]
    fn round_trip() {
        let d = entry();
        assert_eq!(d, UsageEntry::from(pb::UsageEntry::from(d.clone())));
    }

    #[test]
    fn serde_matches_dto() {
        let want = json!({
            "run_id": 1, "log": "run.log", "agent": "worker", "mtime": "2026-01-01T00:00:00Z",
            "duration_ms": 1500, "cost_usd": 0.25,
            "tokens": {"input": 10, "output": 20, "cache_creation": 0, "cache_read": 5},
        });
        assert_eq!(serde_json::to_value(entry()).unwrap(), want);
    }
}
