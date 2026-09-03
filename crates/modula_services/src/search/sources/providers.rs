//! Providers: name and type.

use async_trait::async_trait;
use sqlx::SqlitePool;

use modula_core::error::ApiResult;
use modula_core::repositories::Repositories;
use modula_types::{SearchHit, SearchKind};

use super::super::SearchSource;

pub(in crate::search) struct Providers {
    pool: SqlitePool,
    providers: modula_db::providers::ProviderRepository,
}

impl Providers {
    pub(in crate::search) fn new(repos: &Repositories) -> Self {
        Self {
            pool: repos.pool.clone(),
            providers: repos.providers.clone(),
        }
    }
}

#[async_trait]
impl SearchSource for Providers {
    fn kind(&self) -> SearchKind {
        SearchKind::Provider
    }

    async fn search(&self, ws: &str, query: &str, limit: i64) -> ApiResult<Vec<SearchHit>> {
        Ok(self
            .providers
            .search(&self.pool, ws, query, limit)
            .await?
            .into_iter()
            .filter_map(|p| {
                super::hit(
                    SearchKind::Provider,
                    p.id,
                    &p.name,
                    Some(p.r#type.clone()),
                    query,
                    &[("type", &p.r#type)],
                )
            })
            .collect())
    }
}
