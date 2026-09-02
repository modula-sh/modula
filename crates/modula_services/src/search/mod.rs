//! Workspace-wide search.
//!
//! [`SearchService`] fans a query out over one [`SearchSource`] per entity.
//! A source owns which of its own content types it looks at, how it ranks them
//! and how a match becomes a [`SearchHit`] — so supporting a new entity means
//! adding one source and nothing else.

mod excerpt;
mod sources;

use std::sync::Arc;

use async_trait::async_trait;
use futures::future::join_all;

use modula_core::error::ApiResult;
use modula_core::repositories::Repositories;
use modula_types::{SearchHit, SearchKind};

use crate::workspaces::WorkspaceService;

pub(crate) use excerpt::contains;

/// Per-kind result cap. The modal groups by kind, so a global cap would let one
/// noisy kind starve the rest.
const DEFAULT_LIMIT: u32 = 5;
const MAX_LIMIT: u32 = 25;

#[async_trait]
pub trait SearchSource: Send + Sync {
    fn kind(&self) -> SearchKind;
    async fn search(&self, ws: &str, query: &str, limit: i64) -> ApiResult<Vec<SearchHit>>;
}

#[derive(Clone)]
pub struct SearchService {
    workspaces: WorkspaceService,
    sources: Arc<Vec<Arc<dyn SearchSource>>>,
}

impl SearchService {
    /// Registration order is response order: tasks and conversations lead
    /// because they are what the search placeholder names.
    pub fn new(workspaces: WorkspaceService, repos: &Repositories) -> Self {
        let sources: Vec<Arc<dyn SearchSource>> = vec![
            Arc::new(sources::tasks::Tasks::new(repos)),
            Arc::new(sources::conversations::Conversations::new(repos)),
            Arc::new(sources::agents::Agents::new(repos)),
            Arc::new(sources::projects::Projects::new(repos)),
            Arc::new(sources::providers::Providers::new(repos)),
            Arc::new(sources::wiki::Wiki::new(workspaces.clone())),
        ];
        Self {
            workspaces,
            sources: Arc::new(sources),
        }
    }

    /// `kinds` empty means every kind; unrecognised entries are ignored so a
    /// client asking for a kind this engine lacks degrades to fewer results
    /// rather than an error. `limit` is per kind, 0 meaning the default.
    pub async fn search(
        &self,
        ws: &str,
        query: &str,
        kinds: &[String],
        limit: u32,
    ) -> ApiResult<Vec<SearchHit>> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(Vec::new());
        }
        // Reject unknown workspaces with 404 before any fan-out.
        self.workspaces.workspace_dir(ws).await?;

        let wanted: Option<Vec<SearchKind>> =
            (!kinds.is_empty()).then(|| kinds.iter().filter_map(|k| k.parse().ok()).collect());
        let limit = i64::from(if limit == 0 {
            DEFAULT_LIMIT
        } else {
            limit.min(MAX_LIMIT)
        });

        let results = join_all(
            self.sources
                .iter()
                .filter(|s| wanted.as_ref().is_none_or(|w| w.contains(&s.kind())))
                .map(|s| async move { (s.kind(), s.search(ws, query, limit).await) }),
        )
        .await;

        // A search is a read: one broken source contributes nothing rather than
        // blanking every other kind.
        let mut hits = Vec::new();
        for (kind, result) in results {
            match result {
                Ok(found) => hits.extend(found),
                Err(err) => {
                    tracing::warn!(kind = kind.as_str(), %err, "search source failed")
                }
            }
        }
        Ok(hits)
    }
}

#[cfg(test)]
mod tests;
