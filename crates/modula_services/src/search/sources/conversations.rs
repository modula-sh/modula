//! Conversations: title and the transcript's message contents.

use async_trait::async_trait;
use serde_json::Value;
use sqlx::SqlitePool;

use modula_core::error::ApiResult;
use modula_core::repositories::Repositories;
use modula_types::{SearchHit, SearchKind};

use super::super::SearchSource;

const UNTITLED: &str = "Untitled";

/// Message texts from the `data` blob. A malformed message contributes nothing
/// rather than voiding the whole transcript.
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
        // The SQL matched raw JSON, so a row may have hit the envelope (a role,
        // a timestamp) rather than message text; `hit` drops those.
        Ok(self
            .conversations
            .search(
                &self.pool,
                ws,
                query,
                limit.saturating_mul(super::OVERFETCH),
            )
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
