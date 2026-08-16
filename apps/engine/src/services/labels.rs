//! Label CRUD + task association business logic. Owns a [`LabelRepository`];
//! transport-independent. Attaching/detaching a label mutates the task, so
//! those paths publish `task.update`; catalog CRUD carries no domain event.

use std::sync::Arc;

use serde_json::json;
use sqlx::SqlitePool;

use modula_db::labels::LabelRepository;
use modula_rpc::status::DomainError;
use modula_types::Label;

use crate::services::events::{self, EventSink};

type Result<T> = std::result::Result<T, DomainError>;

/// Default label scope when the caller leaves `type` blank.
const DEFAULT_KIND: &str = "task";

#[derive(Clone)]
pub struct LabelService {
    pool: SqlitePool,
    labels: LabelRepository,
    events: Arc<dyn EventSink>,
}

impl LabelService {
    pub fn new(pool: SqlitePool, labels: LabelRepository, events: Arc<dyn EventSink>) -> Self {
        Self {
            pool,
            labels,
            events,
        }
    }

    pub async fn list(&self, ws: &str, kind: &str) -> Result<Vec<Label>> {
        self.labels.list(&self.pool, ws, or_default(kind)).await
    }

    pub async fn create(&self, ws: &str, kind: &str, name: &str) -> Result<String> {
        let mut tx = self.pool.begin().await?;
        let id = self
            .labels
            .get_or_create(&mut tx, ws, or_default(kind), name)
            .await?;
        tx.commit().await?;
        Ok(id)
    }

    pub async fn attach(&self, ws: &str, task_id: &str, label_id: &str) -> Result<()> {
        if label_id.is_empty() {
            return Err(DomainError::BadRequest("label_id is required".into()));
        }
        self.labels
            .attach(&self.pool, ws, task_id, label_id)
            .await?;
        self.publish_label_change(ws, task_id, label_id, "attached")
            .await;
        Ok(())
    }

    pub async fn detach(&self, ws: &str, task_id: &str, label_id: &str) -> Result<()> {
        self.labels
            .detach(&self.pool, ws, task_id, label_id)
            .await?;
        self.publish_label_change(ws, task_id, label_id, "detached")
            .await;
        Ok(())
    }

    async fn publish_label_change(&self, ws: &str, task_id: &str, label_id: &str, action: &str) {
        self.events
            .publish(
                ws,
                events::TASK_UPDATE,
                json!({ "task_id": task_id, "label_id": label_id, "label_action": action }),
            )
            .await;
    }
}

fn or_default(kind: &str) -> &str {
    if kind.is_empty() {
        DEFAULT_KIND
    } else {
        kind
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::testkit::env;

    fn service(env: &crate::services::testkit::Env) -> LabelService {
        LabelService::new(env.pool.clone(), LabelRepository::new(), env.sink.clone())
    }

    #[tokio::test]
    async fn create_defaults_blank_kind_and_is_idempotent() {
        let env = env().await;
        let svc = service(&env);

        let id = svc.create(&env.ws, "", "bug").await.unwrap();
        // Re-creating the same (kind, name) returns the existing row.
        let again = svc.create(&env.ws, "task", "bug").await.unwrap();
        assert_eq!(id, again);

        // The blank kind was stored under the "task" default, so it lists there.
        let listed = svc.list(&env.ws, "").await.unwrap();
        assert!(listed.iter().any(|l| l.id == id && l.name == "bug"));
    }

    #[tokio::test]
    async fn attach_requires_label_id() {
        let env = env().await;
        let svc = service(&env);
        assert!(matches!(
            svc.attach(&env.ws, "task-1", "").await,
            Err(DomainError::BadRequest(_))
        ));
        // Rejected attach publishes nothing.
        assert!(env.sink.last().is_none());
    }
}
