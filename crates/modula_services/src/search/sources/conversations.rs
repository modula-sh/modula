//! Conversations: title and the transcript's message contents.

use async_trait::async_trait;
use serde::Deserialize;
use sqlx::SqlitePool;

use modula_core::error::ApiResult;
use modula_core::repositories::Repositories;
use modula_types::{SearchHit, SearchKind};

use super::super::SearchSource;

/// Shown when a conversation has no title yet, matching the sidebar.
const UNTITLED: &str = "Untitled";

/// The slice of the `data` blob this source reads.
#[derive(Default, Deserialize)]
struct Transcript {
    #[serde(default)]
    messages: Vec<Message>,
}

#[derive(Deserialize)]
struct Message {
    #[serde(default)]
    content: String,
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
            .search(&self.pool, ws, query, limit)
            .await?
            .into_iter()
            .filter_map(|c| {
                let transcript: Transcript = serde_json::from_str(&c.data).unwrap_or_default();
                let bodies: Vec<(&str, &str)> = transcript
                    .messages
                    .iter()
                    .map(|m| ("transcript", m.content.as_str()))
                    .collect();
                let title = if c.title.trim().is_empty() {
                    UNTITLED
                } else {
                    &c.title
                };
                super::hit(SearchKind::Conversation, c.id, title, None, query, &bodies)
            })
            .collect())
    }
}
