//! Conversations: title and the transcript's message contents.

use async_trait::async_trait;
use serde_json::Value;
use sqlx::SqlitePool;

use modula_core::error::ApiResult;
use modula_core::repositories::Repositories;
use modula_types::{SearchHit, SearchKind};

use super::super::SearchSource;

/// Shown when a conversation has no title yet, matching the sidebar.
const UNTITLED: &str = "Untitled";

/// How many rows to ask SQL for per row we can return. This is the one source
/// whose predicate is wider than what it renders, so envelope-only rows would
/// otherwise spend the whole limit and zero the kind out.
const OVERFETCH: i64 = 4;

/// Message texts from the `data` blob, as tolerant of an odd message shape as
/// `modula_db::conversations`' reader of the same column: a message without a
/// string `content` contributes nothing instead of voiding the whole transcript.
fn message_contents(data: &str) -> Vec<String> {
    serde_json::from_str::<Value>(data)
        .ok()
        .as_ref()
        .and_then(|v| v.get("messages"))
        .and_then(Value::as_array)
        .map(|messages| {
            messages
                .iter()
                .filter_map(|m| Some(m.get("content")?.as_str()?.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

pub(in crate::search) struct Conversations {
    pool: SqlitePool,
    conversations: modula_db::conversations::ConversationRepository,
}

impl Conversations {
    pub(in crate::search) fn new(repos: &Repositories) -> Self {
        Self {
            pool: repos.pool.clone(),
            conversations: repos.conversations.clone(),
        }
    }
}

#[async_trait]
impl SearchSource for Conversations {
    fn kind(&self) -> SearchKind {
        SearchKind::Conversation
    }

    async fn search(&self, ws: &str, query: &str, limit: i64) -> ApiResult<Vec<SearchHit>> {
        // The SQL matched the raw JSON, so a row may have hit the envelope (a
        // role, a timestamp, a key name) rather than any message text. Decoding
        // only the matched rows keeps that cheap; `hit` then drops a row whose
        // title and every message content come up empty.
        Ok(self
            .conversations
            .search(&self.pool, ws, query, limit.saturating_mul(OVERFETCH))
            .await?
            .into_iter()
            .filter_map(|c| {
                let contents = message_contents(&c.data);
                let bodies: Vec<(&str, &str)> = contents
                    .iter()
                    .map(|content| ("transcript", content.as_str()))
                    .collect();
                let title = if c.title.trim().is_empty() {
                    UNTITLED
                } else {
                    &c.title
                };
                super::hit(SearchKind::Conversation, c.id, title, None, query, &bodies)
            })
            .take(limit.max(0) as usize)
            .collect())
    }
}
