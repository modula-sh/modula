//! Thread (comment/verdict) business logic. Owns the repositories it needs plus
//! an [`EventSink`]; transport-independent.

use std::sync::Arc;

use serde_json::json;
use sqlx::SqlitePool;

use modula_db::tasks::TaskRepository;
use modula_db::threads::ThreadRepository;
use modula_db::variants::VariantRepository;
use modula_rpc::status::DomainError;
use modula_types::{ThreadBundle, ThreadEntry};

use crate::events;
use crate::events::EventSink;

type Result<T> = std::result::Result<T, DomainError>;

pub const ALLOWED_KINDS: &[&str] = &["comment", "question", "verdict", "rework"];
const VARIANT_VERDICTS: &[&str] = &["ACCEPT", "REQUEST_CHANGES"];
const TASK_VERDICTS: &[&str] = &["APPROVE", "KICK_BACK"];

pub struct AppendInput {
    pub content: String,
    pub variant: Option<String>,
    pub author: String,
    pub kind: String,
    pub round: Option<i64>,
    pub verdict: Option<String>,
    pub affected_variants: Option<Vec<String>>,
}

#[derive(Clone)]
pub struct ThreadService {
    /// Held so future multi-write thread operations can open a transaction; the
    /// current ops are single writes routed through `&self.pool`.
    pool: SqlitePool,
    tasks: TaskRepository,
    variants: VariantRepository,
    threads: ThreadRepository,
    events: Arc<dyn EventSink>,
}

impl ThreadService {
    pub fn new(
        pool: SqlitePool,
        tasks: TaskRepository,
        variants: VariantRepository,
        threads: ThreadRepository,
        events: Arc<dyn EventSink>,
    ) -> Self {
        Self {
            pool,
            tasks,
            variants,
            threads,
            events,
        }
    }

    /// Validate the task exists, then return its full thread as the
    /// `ThreadBundle` domain aggregate (task- and variant-scoped entries,
    /// grouped by variant). The handler converts it to proto with one `.into()`.
    pub async fn list_threads(&self, ws: &str, task_id: &str) -> Result<ThreadBundle> {
        self.tasks.get(&self.pool, ws, task_id).await?;
        self.threads.list_for_task(&self.pool, ws, task_id).await
    }

    pub async fn create(&self, ws: &str, task_id: &str, input: AppendInput) -> Result<ThreadEntry> {
        let content = input.content.trim();
        if content.is_empty() {
            return Err(DomainError::BadRequest("content is required".into()));
        }
        let author = input.author.trim();
        if author.is_empty() {
            return Err(DomainError::BadRequest("author is required".into()));
        }
        if !ALLOWED_KINDS.contains(&input.kind.as_str()) {
            return Err(DomainError::BadRequest(format!(
                "kind must be one of {ALLOWED_KINDS:?}"
            )));
        }

        self.tasks.get(&self.pool, ws, task_id).await?;

        if let Some(v) = input.variant.as_deref() {
            let variants = self.variants.list_for_task(&self.pool, ws, task_id).await?;
            if !variants.iter().any(|row| row.id == v) {
                return Err(DomainError::NotFound(format!(
                    "unknown variant on task {task_id}: {v}"
                )));
            }
        }

        let verdict = if let Some(v) = input.verdict.as_deref() {
            let v = v.trim();
            if v.is_empty() {
                None
            } else {
                if input.kind != "verdict" {
                    return Err(DomainError::BadRequest(
                        "verdict only allowed when kind = 'verdict'".into(),
                    ));
                }
                let allowed: &[&str] = if input.variant.is_some() {
                    VARIANT_VERDICTS
                } else {
                    TASK_VERDICTS
                };
                if !allowed.contains(&v) {
                    return Err(DomainError::BadRequest(format!(
                        "verdict must be one of {allowed:?} for this scope"
                    )));
                }
                Some(v.to_string())
            }
        } else {
            if input.kind == "verdict" {
                return Err(DomainError::BadRequest(
                    "kind=verdict requires a verdict value".into(),
                ));
            }
            None
        };

        let affected_json = if let Some(av) = input.affected_variants.as_ref() {
            if verdict.as_deref() != Some("KICK_BACK") {
                return Err(DomainError::BadRequest(
                    "affected_variants only valid with verdict=KICK_BACK".into(),
                ));
            }
            if av.is_empty() {
                return Err(DomainError::BadRequest(
                    "affected_variants must be non-empty for KICK_BACK".into(),
                ));
            }
            Some(json!(av))
        } else {
            if verdict.as_deref() == Some("KICK_BACK") {
                return Err(DomainError::BadRequest(
                    "KICK_BACK verdict requires affected_variants".into(),
                ));
            }
            None
        };

        let entry = self
            .threads
            .append(
                &self.pool,
                ws,
                task_id,
                input.variant.as_deref(),
                author,
                &input.kind,
                content,
                input.round,
                verdict.as_deref(),
                affected_json.as_ref(),
            )
            .await?;

        let mut event_data = json!({
            "task_id": task_id,
            "entry_id": entry.id,
            "variant_id": input.variant,
            "kind": input.kind,
            "author": author,
        });
        if let Some(v) = &verdict {
            if let Some(map) = event_data.as_object_mut() {
                map.insert("verdict".into(), json!(v));
            }
        }
        self.events
            .publish(ws, events::THREAD_APPEND, event_data)
            .await;

        Ok(entry)
    }

    /// Validates author + ownership + kind=comment before edit/delete. Shared
    /// by `update` and `delete` so the "only the owner can touch it" rule
    /// lives in exactly one place.
    async fn authorize_mutation(
        &self,
        ws: &str,
        task_id: &str,
        entry_id: i64,
        author: &str,
    ) -> Result<(ThreadEntry, Option<String>)> {
        let author = author.trim();
        if author.is_empty() {
            return Err(DomainError::BadRequest("author is required".into()));
        }
        self.tasks.get(&self.pool, ws, task_id).await?;
        let (entry, variant_id) = self
            .threads
            .get(&self.pool, ws, task_id, entry_id)
            .await?
            .ok_or_else(|| DomainError::NotFound(format!("unknown thread entry {entry_id}")))?;
        if entry.author != author {
            return Err(DomainError::Forbidden(
                "only the comment owner can modify it".into(),
            ));
        }
        if entry.kind != "comment" {
            return Err(DomainError::BadRequest(
                "only comments can be edited or deleted".into(),
            ));
        }
        Ok((entry, variant_id))
    }

    pub async fn update(
        &self,
        ws: &str,
        task_id: &str,
        entry_id: i64,
        author: &str,
        content: &str,
    ) -> Result<ThreadEntry> {
        let content = content.trim();
        if content.is_empty() {
            return Err(DomainError::BadRequest("content is required".into()));
        }
        let (entry, variant_id) = self
            .authorize_mutation(ws, task_id, entry_id, author)
            .await?;
        let updated = self
            .threads
            .update_content(&self.pool, ws, task_id, entry_id, content)
            .await?;
        let event_data = json!({
            "task_id": task_id,
            "variant_id": variant_id,
            "kind": entry.kind,
            "author": entry.author,
            "entry_id": entry_id,
        });
        self.events
            .publish(ws, events::THREAD_UPDATE, event_data)
            .await;
        Ok(updated)
    }

    pub async fn delete(&self, ws: &str, task_id: &str, entry_id: i64, author: &str) -> Result<()> {
        let (entry, variant_id) = self
            .authorize_mutation(ws, task_id, entry_id, author)
            .await?;
        self.threads
            .delete(&self.pool, ws, task_id, entry_id)
            .await?;
        let event_data = json!({
            "task_id": task_id,
            "variant_id": variant_id,
            "kind": entry.kind,
            "author": entry.author,
            "entry_id": entry_id,
        });
        self.events
            .publish(ws, events::THREAD_DELETE, event_data)
            .await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::{env, seed_task, Env};

    fn service(env: &Env) -> ThreadService {
        ThreadService::new(
            env.pool.clone(),
            TaskRepository::new(),
            VariantRepository::new(),
            ThreadRepository::new(),
            env.sink.clone(),
        )
    }

    fn append(author: &str, kind: &str, content: &str) -> AppendInput {
        AppendInput {
            content: content.into(),
            variant: None,
            author: author.into(),
            kind: kind.into(),
            round: None,
            verdict: None,
            affected_variants: None,
        }
    }

    #[tokio::test]
    async fn create_comment_appends_and_emits() {
        let env = env().await;
        let task_id = seed_task(&env).await;
        let svc = service(&env);
        let row = svc
            .create(&env.ws, &task_id, append("human", "comment", "hi"))
            .await
            .unwrap();
        assert_eq!(row.content, "hi");
        assert_eq!(env.sink.types(), vec![events::THREAD_APPEND]);
    }

    #[tokio::test]
    async fn create_rejects_empty_author() {
        let env = env().await;
        let task_id = seed_task(&env).await;
        let svc = service(&env);
        for author in ["", "   "] {
            assert!(matches!(
                svc.create(&env.ws, &task_id, append(author, "comment", "hi"))
                    .await,
                Err(DomainError::BadRequest(_))
            ));
        }
        assert!(env.sink.types().is_empty());
    }

    #[tokio::test]
    async fn create_accepts_any_agent_author() {
        let env = env().await;
        let task_id = seed_task(&env).await;
        let svc = service(&env);
        let row = svc
            .create(
                &env.ws,
                &task_id,
                append("my-custom-agent", "comment", "hi"),
            )
            .await
            .unwrap();
        assert_eq!(row.author, "my-custom-agent");
        let (_, ty, data) = env.sink.last().unwrap();
        assert_eq!(ty, events::THREAD_APPEND);
        assert_eq!(data["author"], "my-custom-agent");
    }

    #[tokio::test]
    async fn verdict_kind_requires_a_verdict_value() {
        let env = env().await;
        let task_id = seed_task(&env).await;
        let svc = service(&env);
        assert!(matches!(
            svc.create(
                &env.ws,
                &task_id,
                append("code-reviewer", "verdict", "done")
            )
            .await,
            Err(DomainError::BadRequest(_))
        ));
    }

    #[tokio::test]
    async fn only_owner_can_delete_a_comment() {
        let env = env().await;
        let task_id = seed_task(&env).await;
        let svc = service(&env);
        let row = svc
            .create(&env.ws, &task_id, append("worker", "comment", "mine"))
            .await
            .unwrap();
        let entry_id = row.id;

        assert!(matches!(
            svc.delete(&env.ws, &task_id, entry_id, "human").await,
            Err(DomainError::Forbidden(_))
        ));
        assert!(svc
            .delete(&env.ws, &task_id, entry_id, "worker")
            .await
            .is_ok());
    }
}
