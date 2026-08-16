//! Task business logic. [`TaskService`] owns the repositories it needs plus an
//! [`EventSink`] and holds the transport-independent business operations,
//! including the filesystem-touching task orchestration (`reset` and its
//! artifact wiping). Reads return `modula_types` domain models (`Task` with its
//! variants/labels assembled here), which the handlers and snapshot render.

use std::path::Path;
use std::sync::Arc;

use chrono::Local;
use serde_json::{json, Value as Json};
use sqlx::SqlitePool;

use modula_db::agent_runs::AgentRunRepository;
use modula_db::agents::AgentRepository;
use modula_db::labels::LabelRepository;
use modula_db::pipeline::PipelineRepository;
use modula_db::roadmap::RoadmapRepository;
use modula_db::task_agent_settings::TaskAgentSettingsRepository;
use modula_db::tasks::{TaskPatch, TaskRepository};
use modula_db::threads::ThreadRepository;
use modula_db::variants::VariantRepository;
use modula_rpc::status::DomainError;
use modula_types::{RoadmapEntry, Task, TaskAgentSetting};

use crate::core::error::ApiResult;
use crate::services::diff;
use crate::services::events;
use crate::services::events::EventSink;
use crate::services::workspaces;
use crate::services::workspaces::WorkspaceService;

type Result<T> = std::result::Result<T, DomainError>;

/// Upper bound on a task's per-agent loop amount.
const LOOP_AMOUNT_MAX: i64 = 100;

const VARIANT_STATUSES: &[&str] = &[
    "ready_for_workers",
    "in_progress",
    "ready_for_review",
    "in_review",
    "rework",
    "accepted",
];

pub struct CreateInternalInput {
    pub title: String,
    pub description: String,
    pub source_data: String,
    pub approved: Option<bool>,
    pub max_variants: Option<i64>,
    pub worktree: Option<bool>,
}

pub struct UpsertExternalInput {
    pub title: String,
    pub description: String,
    pub source: String,
    pub external_id: String,
    /// Serialized JSON string; None when the caller did not supply `source_data`.
    pub source_data: Option<String>,
    pub synced_at: String,
    pub approved: Option<bool>,
    pub max_variants: Option<i64>,
    pub worktree: Option<bool>,
    pub status: Option<String>,
    pub url: Option<String>,
}

pub enum UpsertResult {
    Created { id: String },
    Updated { id: String },
    NoChange { id: String },
}

impl UpsertResult {
    pub fn id(&self) -> &str {
        match self {
            UpsertResult::Created { id }
            | UpsertResult::Updated { id }
            | UpsertResult::NoChange { id } => id,
        }
    }

    pub fn verb(&self) -> &'static str {
        match self {
            UpsertResult::Created { .. } => "created",
            UpsertResult::Updated { .. } | UpsertResult::NoChange { .. } => "updated",
        }
    }
}

#[derive(Clone)]
pub struct TaskService {
    /// The service owns the pool so multi-write operations open one transaction
    /// and hand `&mut *tx` to each repository call — the unit of work lives here,
    /// not scattered inside the repositories.
    pool: SqlitePool,
    tasks: TaskRepository,
    variants: VariantRepository,
    roadmap: RoadmapRepository,
    pipeline: PipelineRepository,
    labels: LabelRepository,
    agents: AgentRepository,
    task_agent_settings: TaskAgentSettingsRepository,
    /// Repositories `reset` wipes derived state through.
    threads: ThreadRepository,
    agent_runs: AgentRunRepository,
    /// Peer service `reset` resolves the on-disk spec/logs directory through.
    workspaces: WorkspaceService,
    events: Arc<dyn EventSink>,
}

impl TaskService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        pool: SqlitePool,
        tasks: TaskRepository,
        variants: VariantRepository,
        roadmap: RoadmapRepository,
        pipeline: PipelineRepository,
        labels: LabelRepository,
        agents: AgentRepository,
        task_agent_settings: TaskAgentSettingsRepository,
        threads: ThreadRepository,
        agent_runs: AgentRunRepository,
        workspaces: WorkspaceService,
        events: Arc<dyn EventSink>,
    ) -> Self {
        Self {
            pool,
            tasks,
            variants,
            roadmap,
            pipeline,
            labels,
            agents,
            task_agent_settings,
            threads,
            agent_runs,
            workspaces,
            events,
        }
    }

    /// Reset a task: wipe its derived state (spec folder, thread rows, logs,
    /// roadmap row, variants) and publish `TASK_RESET`. The task row itself is
    /// preserved so a future archive feature can resurrect the ticket.
    pub async fn reset(&self, ws: &str, task_id: &str) -> ApiResult<Json> {
        let ws_dir = self.workspaces.workspace_dir(ws).await?;
        self.tasks.get(&self.pool, ws, task_id).await?;
        let mut summary = self.wipe_task_artifacts(ws, &ws_dir, task_id).await?;
        if let Some(map) = summary.as_object_mut() {
            map.insert("ok".into(), json!(true));
            map.insert("task".into(), json!(task_id));
        }
        self.events
            .publish(ws, events::TASK_RESET, json!({ "task_id": task_id }))
            .await;
        Ok(summary)
    }

    async fn wipe_task_artifacts(
        &self,
        ws_id: &str,
        ws_dir: &Path,
        task_id: &str,
    ) -> ApiResult<Json> {
        let mut files: Vec<String> = Vec::new();
        let mut errors: Vec<String> = Vec::new();

        let task = self.tasks.get(&self.pool, ws_id, task_id).await?;
        let tf = workspaces::task_spec_dir(ws_dir, task.external_id.as_deref(), &task.title);
        if tf.is_dir() {
            match std::fs::remove_dir_all(&tf) {
                Ok(()) => files.push(format!(
                    "removed spec folder {}",
                    tf.file_name().unwrap().to_string_lossy()
                )),
                Err(e) => errors.push(format!("spec folder remove failed: {e}")),
            }
        }

        if self
            .threads
            .delete_for_task(&self.pool, ws_id, task_id)
            .await?
        {
            files.push("removed thread rows".into());
        }

        let logs_dir = ws_dir.join("logs");
        let removed_log_paths = self
            .agent_runs
            .delete_for_task_returning_log_paths(&self.pool, ws_id, task_id)
            .await?;
        let mut removed_logs = 0u32;
        for name in removed_log_paths.iter().flatten() {
            let p = logs_dir.join(name);
            if !p.exists() {
                continue;
            }
            match std::fs::remove_file(&p) {
                Ok(()) => removed_logs += 1,
                Err(e) => errors.push(format!("log remove failed for {name}: {e}")),
            }
        }
        if removed_logs > 0 {
            files.push(format!("removed {removed_logs} log file(s)"));
        }
        if !removed_log_paths.is_empty() {
            files.push(format!(
                "removed {} agent_runs row(s)",
                removed_log_paths.len()
            ));
        }

        if self
            .roadmap
            .delete_for_task(&self.pool, ws_id, task_id)
            .await?
        {
            files.push("removed roadmap row".into());
        }

        if self
            .variants
            .delete_for_task(&self.pool, ws_id, task_id)
            .await?
        {
            files.push("cleared variants".into());
        }

        Ok(json!({
            "git": [],
            "files": files,
            "errors": errors,
        }))
    }

    /// List every task in the workspace with its variants and labels assembled in.
    pub async fn list(&self, ws: &str) -> Result<Vec<Task>> {
        let mut tasks = self.tasks.list(&self.pool, ws).await?;
        let mut variants_by_task = self.variants.list_all(&self.pool, ws).await?;
        let mut labels_by_task = self.labels.list_all_by_task(&self.pool, ws).await?;
        for task in &mut tasks {
            task.variants = variants_by_task.remove(&task.id).unwrap_or_default();
            task.labels = labels_by_task.remove(&task.id).unwrap_or_default();
        }
        Ok(tasks)
    }

    /// The workspace's roadmap rows in pipeline order.
    pub async fn list_roadmap(&self, ws: &str) -> Result<Vec<RoadmapEntry>> {
        self.roadmap.list(&self.pool, ws).await
    }

    /// Per-agent settings for a task. 404s on an unknown task.
    pub async fn list_agent_settings(
        &self,
        ws: &str,
        task_id: &str,
    ) -> Result<Vec<TaskAgentSetting>> {
        self.tasks.get(&self.pool, ws, task_id).await?;
        self.task_agent_settings
            .list_for_task(&self.pool, ws, task_id)
            .await
    }

    /// Upsert a task's per-agent loop amount. Validates the bound and that both
    /// the task and agent exist, so the FK never surfaces as an Internal error.
    pub async fn set_agent_settings(
        &self,
        ws: &str,
        task_id: &str,
        agent_id: &str,
        loop_amount: i64,
    ) -> Result<()> {
        if !(1..=LOOP_AMOUNT_MAX).contains(&loop_amount) {
            return Err(DomainError::BadRequest(format!(
                "loop_amount must be between 1 and {LOOP_AMOUNT_MAX}"
            )));
        }
        self.tasks.get(&self.pool, ws, task_id).await?;
        self.agents.get(&self.pool, ws, agent_id).await?;
        self.task_agent_settings
            .upsert(&self.pool, ws, task_id, agent_id, loop_amount)
            .await?;
        Ok(())
    }

    pub async fn delete_agent_settings(
        &self,
        ws: &str,
        task_id: &str,
        agent_id: &str,
    ) -> Result<()> {
        self.task_agent_settings
            .delete(&self.pool, ws, task_id, agent_id)
            .await?;
        Ok(())
    }

    pub async fn delete(&self, ws: &str, task_id: &str) -> Result<()> {
        self.tasks.get(&self.pool, ws, task_id).await?;
        self.tasks.delete(&self.pool, ws, task_id).await?;
        self.events
            .publish(ws, events::TASK_DELETE, json!({ "task_id": task_id }))
            .await;
        Ok(())
    }

    /// Transition a variant to a new status. No-ops silently when the variant is
    /// already in `new_status` so re-PUTting the current status can't re-fire rules.
    pub async fn update_variant(
        &self,
        ws: &str,
        task_id: &str,
        variant_id: &str,
        new_status: &str,
    ) -> Result<()> {
        if !VARIANT_STATUSES.contains(&new_status) {
            return Err(DomainError::BadRequest(format!(
                "status must be one of {VARIANT_STATUSES:?}"
            )));
        }
        self.tasks.get(&self.pool, ws, task_id).await?;
        let existing = self
            .variants
            .get(&self.pool, ws, task_id, variant_id)
            .await?
            .ok_or_else(|| {
                DomainError::NotFound(format!("unknown variant on task {task_id}: {variant_id}"))
            })?;
        if existing.status.as_deref() == Some(new_status) {
            return Ok(());
        }
        self.variants
            .set_status(&self.pool, ws, task_id, variant_id, new_status)
            .await?;
        let event = events::variant_update(task_id, variant_id, json!(new_status));
        self.events.publish(ws, events::VARIANT_UPDATE, event).await;
        Ok(())
    }

    pub async fn create_variants(
        &self,
        ws: &str,
        task_id: &str,
        count: u32,
    ) -> Result<Vec<(String, i64)>> {
        if count == 0 {
            return Err(DomainError::BadRequest(
                "`count` is required and must be >= 1".into(),
            ));
        }
        if count > 10 {
            return Err(DomainError::BadRequest("`count` must be <= 10".into()));
        }
        // One unit of work: the existence check and the batch insert commit together.
        let mut tx = self.pool.begin().await?;
        self.tasks.get(&mut *tx, ws, task_id).await?;
        let created = self
            .variants
            .create_batch(&mut tx, ws, task_id, count)
            .await?;
        tx.commit().await?;
        Ok(created)
    }

    /// Create an internal task. The workspace must exist (caller validates beforehand).
    pub async fn create_internal(
        &self,
        ws: &str,
        input: CreateInternalInput,
    ) -> Result<(String, String)> {
        let today = Local::now().date_naive().to_string();
        let mut tx = self.pool.begin().await?;
        let (id, external_id) = self
            .tasks
            .create_internal(
                &mut tx,
                ws,
                &input.title,
                &input.source_data,
                input.approved,
                &input.description,
                input.max_variants,
                input.worktree,
                &today,
            )
            .await?;
        tx.commit().await?;
        self.events
            .publish(
                ws,
                events::TASK_CREATE,
                json!({ "task_id": id, "source": "internal", "approved": input.approved }),
            )
            .await;
        Ok((id, external_id))
    }

    /// Create or update an external task. Returns which outcome occurred.
    pub async fn upsert_external(
        &self,
        ws: &str,
        input: UpsertExternalInput,
    ) -> Result<UpsertResult> {
        if input.external_id.trim().is_empty() {
            return Err(DomainError::BadRequest(
                "external task requires `external_id`".into(),
            ));
        }
        if input.source.trim().is_empty() {
            return Err(DomainError::BadRequest(
                "external task requires `source`".into(),
            ));
        }
        let parse = |s: &str| serde_json::from_str::<Json>(s).unwrap_or_else(|_| json!({}));
        let source_data_str = input.source_data.as_deref().unwrap_or("{}");

        match self
            .tasks
            .get_by_external(&self.pool, ws, &input.external_id)
            .await?
        {
            Some(existing) => {
                if !existing.source.eq_ignore_ascii_case(&input.source) {
                    return Err(DomainError::Conflict(format!(
                        "task {} exists with source={:?}; cannot upsert as {:?}",
                        input.external_id, existing.source, input.source
                    )));
                }
                let id = existing.id.clone();
                let patch = TaskPatch {
                    title: Some(input.title.clone()),
                    description: Some(input.description.clone()),
                    source_data: input.source_data.clone(),
                    status: input.status.clone().map(Some),
                    url: input.url.clone().map(Some),
                    synced_at: Some(Some(input.synced_at.clone())),
                    ..TaskPatch::default()
                };

                // Build incoming as the set of fields this call is asserting values for.
                // Fields absent from `input` (None) are excluded from the diff.
                let mut incoming = serde_json::Map::new();
                incoming.insert("title".into(), json!(input.title));
                incoming.insert("description".into(), json!(input.description));
                incoming.insert("synced_at".into(), json!(input.synced_at));
                if input.source_data.is_some() {
                    incoming.insert("source_data".into(), parse(source_data_str));
                }
                if let Some(ref s) = input.status {
                    incoming.insert("status".into(), json!(s));
                }
                if let Some(ref u) = input.url {
                    incoming.insert("url".into(), json!(u));
                }
                let current = json!({
                    "title": existing.title,
                    "description": existing.description,
                    "source_data": existing.source_data.clone().unwrap_or_else(|| json!({})),
                    "status": existing.status,
                    "url": existing.url,
                    "synced_at": existing.synced_at,
                });
                let changed = diff::changed_fields(&current, &Json::Object(incoming));
                if changed.is_empty() {
                    return Ok(UpsertResult::NoChange { id });
                }
                self.tasks.patch(&self.pool, ws, &id, patch).await?;
                let event = events::update_event(&[("task_id", json!(id))], changed);
                self.events.publish(ws, events::TASK_UPDATE, event).await;
                Ok(UpsertResult::Updated { id })
            }
            None => {
                let id = self
                    .tasks
                    .create(
                        &self.pool,
                        ws,
                        &input.title,
                        &input.source,
                        Some(&input.external_id),
                        source_data_str,
                        input.approved,
                        &input.description,
                        input.max_variants,
                        input.worktree,
                        Some(&input.synced_at),
                        input.status.as_deref(),
                        input.url.as_deref(),
                    )
                    .await?;
                self.events
                    .publish(
                        ws,
                        events::TASK_CREATE,
                        json!({ "task_id": id, "source": input.source, "approved": input.approved }),
                    )
                    .await;
                Ok(UpsertResult::Created { id })
            }
        }
    }

    /// Update an existing task. Each `Option` represents whether the caller addressed
    /// that field — `None` = not addressed (skip), `Some(v)` = set to v.
    /// Returns true if anything changed (and an event was emitted), false if no-op.
    #[allow(clippy::too_many_arguments)]
    pub async fn update(
        &self,
        ws: &str,
        task_id: &str,
        title: Option<String>,
        description: Option<String>,
        approved: Option<Option<bool>>,
        max_variants: Option<Option<i64>>,
        worktree: Option<Option<bool>>,
    ) -> Result<bool> {
        let existing = self.tasks.get(&self.pool, ws, task_id).await?;

        let mut patch = TaskPatch::default();
        let mut incoming = serde_json::Map::new();

        if let Some(v) = approved {
            patch.approved = Some(v);
            incoming.insert("approved".into(), json!(v));
        }
        let new_max = if let Some(v) = max_variants {
            patch.max_variants = Some(v);
            incoming.insert("max_variants".into(), json!(v));
            Some(v)
        } else {
            None
        };
        if let Some(v) = worktree {
            patch.worktree = Some(v);
            incoming.insert("worktree".into(), json!(v));
        }
        if let Some(d) = description {
            incoming.insert("description".into(), json!(d));
            patch.description = Some(d);
        }
        if let Some(t) = title {
            if !existing.source.eq_ignore_ascii_case("internal") {
                return Err(DomainError::Forbidden(
                    "title is owned by the source agent on external tasks".into(),
                ));
            }
            if t.is_empty() {
                return Err(DomainError::BadRequest("title cannot be empty".into()));
            }
            incoming.insert("title".into(), json!(t));
            patch.title = Some(t);
        }

        // Enforce: worktree=false implies max_variants=1.
        let resulting_worktree = patch.worktree.unwrap_or(existing.worktree);
        if resulting_worktree == Some(false) {
            if new_max.flatten().is_some_and(|n| n > 1) {
                return Err(DomainError::BadRequest(
                    "worktree=false implies max_variants=1; multiple variants would collide on base_branch".into(),
                ));
            }
            patch.max_variants = Some(Some(1));
            incoming.insert("max_variants".into(), json!(1i64));
        }

        let current = json!({
            "title": existing.title,
            "description": existing.description,
            "approved": existing.approved,
            "max_variants": existing.max_variants,
            "worktree": existing.worktree,
        });
        let changed = diff::changed_fields(&current, &Json::Object(incoming));
        if changed.is_empty() {
            return Ok(false);
        }

        self.tasks.patch(&self.pool, ws, task_id, patch).await?;
        let event = events::update_event(&[("task_id", json!(task_id))], changed);
        self.events.publish(ws, events::TASK_UPDATE, event).await;
        Ok(true)
    }

    /// Advance a task on the roadmap. Returns whether a roadmap row was created
    /// (vs. patched). Rejects an empty or unknown pipeline status, and 404s on an
    /// unknown or soft-deleted task — an archived ticket cannot be advanced.
    pub async fn set_roadmap_status(
        &self,
        ws: &str,
        task_id: &str,
        status: &str,
        depends_on: Option<&Json>,
        notes: Option<&str>,
    ) -> Result<bool> {
        let status = status.trim();
        if status.is_empty() {
            return Err(DomainError::BadRequest("status is required".into()));
        }
        self.tasks.get(&self.pool, ws, task_id).await?;
        let keys: Vec<String> = self
            .pipeline
            .list(&self.pool, ws)
            .await?
            .into_iter()
            .map(|p| p.key)
            .collect();
        if !keys.is_empty() && !keys.contains(&status.to_string()) {
            return Err(DomainError::BadRequest(format!(
                "unknown status: {status:?}"
            )));
        }
        let mut tx = self.pool.begin().await?;
        let created = self
            .roadmap
            .upsert(&mut tx, ws, task_id, status, depends_on, notes)
            .await?;
        tx.commit().await?;
        self.events
            .publish(
                ws,
                events::TASK_UPDATE,
                json!({ "task_id": task_id, "pipeline_status": status }),
            )
            .await;
        Ok(created)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::error::ApiError;
    use crate::services::testkit::{env, Env};
    use modula_db::workspaces::WorkspaceRepository;

    async fn service(env: &Env) -> TaskService {
        use crate::services::loop_registry::LoopRegistry;
        use crate::services::scheduler::SchedulerHandle;
        use crate::state::Repositories;

        let repos = Repositories::new(&env.pool);
        let scheduler = SchedulerHandle::start(
            env.paths().modula.clone(),
            LoopRegistry::default(),
            String::new(),
            env.sink.clone(),
            repos.clone(),
        )
        .await
        .unwrap();
        let workspaces = WorkspaceService::new(
            env.pool.clone(),
            repos.workspaces.clone(),
            env.paths(),
            scheduler,
        );
        TaskService::new(
            env.pool.clone(),
            TaskRepository::new(),
            VariantRepository::new(),
            RoadmapRepository::new(),
            PipelineRepository::new(),
            LabelRepository::new(),
            AgentRepository::new(),
            TaskAgentSettingsRepository::new(),
            ThreadRepository::new(),
            AgentRunRepository::new(),
            workspaces,
            env.sink.clone(),
        )
    }

    fn internal(
        title: &str,
        worktree: Option<bool>,
        max_variants: Option<i64>,
    ) -> CreateInternalInput {
        CreateInternalInput {
            title: title.into(),
            description: String::new(),
            source_data: "{}".into(),
            approved: None,
            max_variants,
            worktree,
        }
    }

    #[tokio::test]
    async fn create_internal_emits_event_and_mints_external_id() {
        let env = env().await;
        let svc = service(&env).await;
        let (id, external_id) = svc
            .create_internal(&env.ws, internal("first", None, None))
            .await
            .unwrap();
        assert_eq!(external_id, "MOD-001");
        assert!(svc.tasks.get(&env.pool, &env.ws, &id).await.is_ok());
        assert_eq!(env.sink.types(), vec![events::TASK_CREATE]);
        let (_, _, data) = env.sink.last().unwrap();
        assert_eq!(data["task_id"], json!(id));
    }

    #[tokio::test]
    async fn update_noop_returns_false_without_event() {
        let env = env().await;
        let svc = service(&env).await;
        let (id, _) = svc
            .create_internal(&env.ws, internal("t", None, None))
            .await
            .unwrap();
        env.sink.clear();
        let changed = svc
            .update(&env.ws, &id, None, None, None, None, None)
            .await
            .unwrap();
        assert!(!changed);
        assert!(env.sink.types().is_empty());
    }

    #[tokio::test]
    async fn update_worktree_false_forces_single_variant() {
        let env = env().await;
        let svc = service(&env).await;
        let (id, _) = svc
            .create_internal(&env.ws, internal("t", Some(true), Some(3)))
            .await
            .unwrap();
        env.sink.clear();

        // Flipping worktree off with an explicit max_variants>1 is rejected.
        let err = svc
            .update(
                &env.ws,
                &id,
                None,
                None,
                None,
                Some(Some(3)),
                Some(Some(false)),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, DomainError::BadRequest(_)));

        // Flipping worktree off alone coerces max_variants down to 1.
        let changed = svc
            .update(&env.ws, &id, None, None, None, None, Some(Some(false)))
            .await
            .unwrap();
        assert!(changed);
        let (_, type_, data) = env.sink.last().unwrap();
        assert_eq!(type_, events::TASK_UPDATE);
        assert_eq!(data["max_variants"], json!(1));
        assert_eq!(data["worktree"], json!(false));
    }

    #[tokio::test]
    async fn create_variants_enforces_count_bounds() {
        let env = env().await;
        let svc = service(&env).await;
        let (id, _) = svc
            .create_internal(&env.ws, internal("t", None, None))
            .await
            .unwrap();
        assert!(svc.create_variants(&env.ws, &id, 0).await.is_err());
        assert!(svc.create_variants(&env.ws, &id, 11).await.is_err());
        let created = svc.create_variants(&env.ws, &id, 2).await.unwrap();
        assert_eq!(created.len(), 2);
    }

    #[tokio::test]
    async fn update_variant_is_idempotent_on_same_status() {
        let env = env().await;
        let svc = service(&env).await;
        let (id, _) = svc
            .create_internal(&env.ws, internal("t", None, None))
            .await
            .unwrap();
        let variants = svc.create_variants(&env.ws, &id, 1).await.unwrap();
        let var_id = &variants[0].0;
        env.sink.clear();

        svc.update_variant(&env.ws, &id, var_id, "in_progress")
            .await
            .unwrap();
        // Re-PUTting the current status is a silent no-op — no second event.
        svc.update_variant(&env.ws, &id, var_id, "in_progress")
            .await
            .unwrap();
        assert_eq!(env.sink.types(), vec![events::VARIANT_UPDATE]);

        assert!(matches!(
            svc.update_variant(&env.ws, &id, var_id, "bogus").await,
            Err(DomainError::BadRequest(_))
        ));
    }

    #[tokio::test]
    async fn set_roadmap_status_rejects_empty() {
        let env = env().await;
        let svc = service(&env).await;
        let (id, _) = svc
            .create_internal(&env.ws, internal("t", None, None))
            .await
            .unwrap();
        assert!(matches!(
            svc.set_roadmap_status(&env.ws, &id, "  ", None, None).await,
            Err(DomainError::BadRequest(_))
        ));
    }

    #[tokio::test]
    async fn list_roadmap_returns_upserted_rows() {
        let env = env().await;
        let svc = service(&env).await;
        let (id, _) = svc
            .create_internal(&env.ws, internal("t", None, None))
            .await
            .unwrap();
        assert!(svc.list_roadmap(&env.ws).await.unwrap().is_empty());
        svc.set_roadmap_status(&env.ws, &id, "planning", None, None)
            .await
            .unwrap();
        let rows = svc.list_roadmap(&env.ws).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].task, id);
        assert_eq!(rows[0].status, "planning");
    }

    #[tokio::test]
    async fn set_agent_settings_validates_bound_and_existence() {
        let env = env().await;
        let svc = service(&env).await;
        let (id, _) = svc
            .create_internal(&env.ws, internal("t", None, None))
            .await
            .unwrap();
        // loop_amount out of range is rejected before any lookup.
        assert!(matches!(
            svc.set_agent_settings(&env.ws, &id, "agent", 0).await,
            Err(DomainError::BadRequest(_))
        ));
        assert!(matches!(
            svc.set_agent_settings(&env.ws, &id, "agent", 101).await,
            Err(DomainError::BadRequest(_))
        ));
        // Unknown task 404s.
        assert!(matches!(
            svc.set_agent_settings(&env.ws, "nope", "agent", 1).await,
            Err(DomainError::NotFound(_))
        ));
        // Unknown agent 404s (task exists).
        assert!(matches!(
            svc.set_agent_settings(&env.ws, &id, "nope", 1).await,
            Err(DomainError::NotFound(_))
        ));
    }

    #[tokio::test]
    async fn reset_clears_variants_preserves_task_and_emits_event() {
        let env = env().await;
        let svc = service(&env).await;
        // reset resolves `<modula>/<slug>` on disk; create it so it isn't a 404.
        let slug = WorkspaceRepository::new()
            .slug_for(&env.pool, &env.ws)
            .await
            .unwrap();
        std::fs::create_dir_all(env.paths().modula.join(&slug)).unwrap();
        let (id, _) = svc
            .create_internal(&env.ws, internal("t", None, None))
            .await
            .unwrap();
        svc.create_variants(&env.ws, &id, 2).await.unwrap();
        env.sink.clear();

        let summary = svc.reset(&env.ws, &id).await.unwrap();
        assert_eq!(summary["ok"], json!(true));
        assert_eq!(summary["task"], json!(id));
        assert_eq!(env.sink.types(), vec![events::TASK_RESET]);

        // The task row survives; its variants are gone.
        assert!(svc.tasks.get(&env.pool, &env.ws, &id).await.is_ok());
        let items = svc.list(&env.ws).await.unwrap();
        let item = items.iter().find(|t| t.id == id).unwrap();
        assert!(item.variants.is_empty());
    }

    #[tokio::test]
    async fn reset_unknown_workspace_dir_is_not_found() {
        let env = env().await;
        let svc = service(&env).await;
        let (id, _) = svc
            .create_internal(&env.ws, internal("t", None, None))
            .await
            .unwrap();
        // The workspace dir was never scaffolded on disk → NotFound.
        assert!(matches!(
            svc.reset(&env.ws, &id).await,
            Err(ApiError::NotFound(_))
        ));
    }
}
