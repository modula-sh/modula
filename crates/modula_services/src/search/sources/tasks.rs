//! Tasks: title, description, and the task's thread comments.

use async_trait::async_trait;
use sqlx::SqlitePool;

use modula_core::error::ApiResult;
use modula_core::repositories::Repositories;
use modula_types::{SearchHit, SearchKind};

use super::super::excerpt::{excerpt, RADIUS};
use super::super::SearchSource;

pub(in crate::search) struct Tasks {
    pool: SqlitePool,
    tasks: modula_db::tasks::TaskRepository,
    threads: modula_db::threads::ThreadRepository,
}

impl Tasks {
    pub(in crate::search) fn new(repos: &Repositories) -> Self {
        Self {
            pool: repos.pool.clone(),
            tasks: repos.tasks.clone(),
            threads: repos.threads.clone(),
        }
    }
}

#[async_trait]
impl SearchSource for Tasks {
    fn kind(&self) -> SearchKind {
        SearchKind::Task
    }

    async fn search(&self, ws: &str, query: &str, limit: i64) -> ApiResult<Vec<SearchHit>> {
        let mut hits: Vec<SearchHit> = self
            .tasks
            .search(&self.pool, ws, query, limit)
            .await?
            .into_iter()
            .filter_map(|t| {
                super::hit(
                    SearchKind::Task,
                    t.id,
                    &t.title,
                    t.external_id,
                    query,
                    &[("description", &t.description)],
                )
            })
            .collect();

        // A comment has no view of its own, so it surfaces as its owning task,
        // and only if that task has not already matched. Over-fetch because a
        // busy thread on an already-matched task would otherwise spend the limit.
        for c in self
            .threads
            .search(
                &self.pool,
                ws,
                query,
                limit.saturating_mul(super::OVERFETCH),
            )
            .await?
        {
            if hits.len() as i64 >= limit {
                break;
            }
            if hits.iter().any(|h| h.id == c.task_id) {
                continue;
            }
            if let Some(spans) = excerpt(&c.content, query, RADIUS) {
                hits.push(SearchHit {
                    kind: SearchKind::Task.as_str().to_string(),
                    id: c.task_id,
                    title: c.task_title,
                    subtitle: c.task_external_id,
                    field: "comment".to_string(),
                    excerpt: spans,
                });
            }
        }
        hits.truncate(limit as usize);
        Ok(hits)
    }
}
