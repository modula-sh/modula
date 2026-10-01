//! Conversations: title and the transcript's message contents.

use async_trait::async_trait;
use sqlx::SqlitePool;

use modula_core::error::ApiResult;
use modula_core::repositories::Repositories;
use modula_types::{SearchHit, SearchKind};

use super::super::SearchSource;

const UNTITLED: &str = "Untitled";

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
                let bodies: Vec<(&str, &str)> = c
                    .content
                    .as_deref()
                    .map(|content| ("transcript", content))
                    .into_iter()
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
