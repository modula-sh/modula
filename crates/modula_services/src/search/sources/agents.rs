//! Agents: name, description, and the prompt/instructions.

use async_trait::async_trait;
use sqlx::SqlitePool;

use modula_core::error::ApiResult;
use modula_core::repositories::Repositories;
use modula_types::{SearchHit, SearchKind};

use super::super::SearchSource;

pub(in crate::search) struct Agents {
    pool: SqlitePool,
    agents: modula_db::agents::AgentRepository,
}

impl Agents {
    pub(in crate::search) fn new(repos: &Repositories) -> Self {
        Self {
            pool: repos.pool.clone(),
            agents: repos.agents.clone(),
        }
    }
}

#[async_trait]
impl SearchSource for Agents {
    fn kind(&self) -> SearchKind {
        SearchKind::Agent
    }

    async fn search(&self, ws: &str, query: &str, limit: i64) -> ApiResult<Vec<SearchHit>> {
        Ok(self
            .agents
            .search(&self.pool, ws, query, limit)
            .await?
            .into_iter()
            .filter_map(|a| {
                super::hit(
                    SearchKind::Agent,
                    a.id,
                    &a.name,
                    None,
                    query,
                    &[("description", &a.description), ("prompt", &a.prompt)],
                )
            })
            .collect())
    }
}
