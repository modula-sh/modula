//! Workspaces: name and description. The one source that looks outside the
//! workspace being searched: a hit is another workspace to switch to.

use async_trait::async_trait;

use modula_core::error::ApiResult;
use modula_types::{SearchHit, SearchKind};

use super::super::SearchSource;
use crate::workspaces::WorkspaceService;

pub(in crate::search) struct Workspaces {
    workspaces: WorkspaceService,
}

impl Workspaces {
    pub(in crate::search) fn new(workspaces: WorkspaceService) -> Self {
        Self { workspaces }
    }
}

#[async_trait]
impl SearchSource for Workspaces {
    fn kind(&self) -> SearchKind {
        SearchKind::Workspace
    }

    async fn search(&self, ws: &str, query: &str, limit: i64) -> ApiResult<Vec<SearchHit>> {
        Ok(self
            .workspaces
            .list()
            .await?
            .into_iter()
            .filter(|w| w.id != ws)
            .filter_map(|w| {
                let description = w.description.as_deref().unwrap_or("");
                super::hit(
                    SearchKind::Workspace,
                    w.id.clone(),
                    &w.name,
                    None,
                    query,
                    &[("description", description)],
                )
            })
            .take(limit.max(0) as usize)
            .collect())
    }
}
