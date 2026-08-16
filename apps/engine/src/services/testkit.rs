//! Shared scaffolding for the service unit tests: a recording [`EventSink`] and
//! a temp-SQLite environment seeded with the default `Modula` workspace.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::Value as Json;
use sqlx::SqlitePool;
use tempfile::TempDir;

use modula_db::workspaces::WorkspaceRepository;

use crate::core::paths::Paths;
use crate::services::events::EventSink;

/// An [`EventSink`] that records every published event so a test can assert on
/// what a service emitted — or that it stayed silent.
#[derive(Default)]
pub struct RecordingSink {
    events: Mutex<Vec<(String, String, Json)>>,
}

#[async_trait]
impl EventSink for RecordingSink {
    async fn publish(&self, ws: &str, type_: &str, data: Json) {
        self.events
            .lock()
            .unwrap()
            .push((ws.to_string(), type_.to_string(), data));
    }
}

impl RecordingSink {
    pub fn types(&self) -> Vec<String> {
        self.events
            .lock()
            .unwrap()
            .iter()
            .map(|(_, t, _)| t.clone())
            .collect()
    }

    pub fn last(&self) -> Option<(String, String, Json)> {
        self.events.lock().unwrap().last().cloned()
    }

    pub fn clear(&self) {
        self.events.lock().unwrap().clear();
    }
}

/// A temp-SQLite environment with the default `Modula` workspace created. Holds
/// the [`TempDir`] so the database file outlives the test body.
pub struct Env {
    pub pool: SqlitePool,
    pub ws: String,
    pub sink: Arc<RecordingSink>,
    _dir: TempDir,
}

impl Env {
    /// A [`Paths`] rooted at this env's temp dir, for services that resolve
    /// on-disk workspace directories (e.g. `TaskService::reset`).
    pub fn paths(&self) -> Arc<Paths> {
        Arc::new(Paths {
            modula: self._dir.path().to_path_buf(),
        })
    }
}

pub async fn env() -> Env {
    let dir = tempfile::tempdir().unwrap();
    let pool = modula_db::open(&dir.path().join("t.sqlite")).await.unwrap();
    let mut conn = pool.acquire().await.unwrap();
    let ws = WorkspaceRepository::new()
        .create(&mut conn, "Modula", None)
        .await
        .unwrap();
    drop(conn);
    Env {
        pool,
        ws,
        sink: Arc::new(RecordingSink::default()),
        _dir: dir,
    }
}

/// Seed an internal task and return its id, clearing the create event so the
/// caller's assertions start from a clean sink.
pub async fn seed_task(env: &Env) -> String {
    use crate::services::loop_registry::LoopRegistry;
    use crate::services::scheduler::SchedulerHandle;
    use crate::services::tasks::{CreateInternalInput, TaskService};
    use crate::services::workspaces::WorkspaceService;
    use crate::state::Repositories;
    use modula_db::agent_runs::AgentRunRepository;
    use modula_db::agents::AgentRepository;
    use modula_db::labels::LabelRepository;
    use modula_db::pipeline::PipelineRepository;
    use modula_db::roadmap::RoadmapRepository;
    use modula_db::task_agent_settings::TaskAgentSettingsRepository;
    use modula_db::tasks::TaskRepository;
    use modula_db::threads::ThreadRepository;
    use modula_db::variants::VariantRepository;

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
    let svc = TaskService::new(
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
    );
    let (id, _) = svc
        .create_internal(
            &env.ws,
            CreateInternalInput {
                title: "t".into(),
                description: String::new(),
                source_data: "{}".into(),
                approved: None,
                max_variants: None,
                worktree: None,
            },
        )
        .await
        .unwrap();
    env.sink.clear();
    id
}
