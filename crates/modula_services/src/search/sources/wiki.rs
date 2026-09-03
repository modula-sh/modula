//! AI wiki: page title, workspace-relative path, and markdown contents. Backed
//! by the filesystem rather than a table, so its cost tracks content size.

use async_trait::async_trait;

use modula_core::error::ApiResult;
use modula_types::{SearchHit, SearchKind};

use super::super::SearchSource;
use crate::workspaces::WorkspaceService;

pub(in crate::search) struct Wiki {
    workspaces: WorkspaceService,
}

impl Wiki {
    pub(in crate::search) fn new(workspaces: WorkspaceService) -> Self {
        Self { workspaces }
    }
}

#[async_trait]
impl SearchSource for Wiki {
    fn kind(&self) -> SearchKind {
        SearchKind::Wiki
    }

    async fn search(&self, ws: &str, query: &str, limit: i64) -> ApiResult<Vec<SearchHit>> {
        let root = crate::wiki::wiki_root(&self.workspaces.workspace_dir(ws).await?)?;
        Ok(crate::wiki::search(&root, query, limit as usize)
            .into_iter()
            .filter_map(|m| {
                // The path is the id: it is what the wiki view navigates by.
                let parent = m.path.rsplit_once('/').map(|(dir, _)| dir.to_string());
                super::hit(
                    SearchKind::Wiki,
                    m.path.clone(),
                    &m.title,
                    parent,
                    query,
                    &[("path", &m.path), ("contents", &m.body)],
                )
            })
            .collect())
    }
}
