//! Projects: name and on-disk path.

use async_trait::async_trait;
use sqlx::SqlitePool;

use modula_core::error::ApiResult;
use modula_core::repositories::Repositories;
use modula_types::{SearchHit, SearchKind};

use super::super::SearchSource;

pub(in crate::search) struct Projects {
    pool: SqlitePool,
    projects: modula_db::projects::ProjectRepository,
}

impl Projects {
    pub(in crate::search) fn new(repos: &Repositories) -> Self {
        Self {
            pool: repos.pool.clone(),
            projects: repos.projects.clone(),
        }
    }
}

#[async_trait]
impl SearchSource for Projects {
    fn kind(&self) -> SearchKind {
        SearchKind::Project
    }

    async fn search(&self, ws: &str, query: &str, limit: i64) -> ApiResult<Vec<SearchHit>> {
        Ok(self
            .projects
            .search(&self.pool, ws, query, limit)
            .await?
            .into_iter()
            .filter_map(|p| {
                super::hit(
                    SearchKind::Project,
                    p.id,
                    &p.name,
                    Some(p.path.clone()),
                    query,
                    &[("path", &p.path)],
                )
            })
            .collect())
    }
}
