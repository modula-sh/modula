//! Cron scheduler — fires scheduled agents on their `schedule_*` columns.
//!
//! Owns one `JobScheduler` for the engine process. At startup and after every
//! agent CRUD write, `reconfigure` walks the `agents` table for every
//! workspace (rows with `schedule_enabled = 1` + non-empty `schedule_cron`),
//! drops the existing jobs, and re-registers one job per matching agent.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use chrono_tz::Tz;
use parking_lot::Mutex;
use tokio_cron_scheduler::{Job, JobScheduler};
use uuid::Uuid;

use super::events::EventSink;
use super::loop_registry::LoopRegistry;
use super::spawn::{self, SpawnParams};
use crate::core::error::{ApiError, ApiResult};
use crate::state::Repositories;

/// Cheaply-clonable handle into the engine's cron scheduler.
#[derive(Clone)]
pub struct SchedulerHandle {
    scheduler: JobScheduler,
    modula: PathBuf,
    job_ids: Arc<Mutex<Vec<Uuid>>>,
    loops: LoopRegistry,
    engine_socket: String,
    events: Arc<dyn EventSink>,
    repos: Repositories,
}

impl SchedulerHandle {
    /// Build the scheduler and start its tick loop. Caller invokes
    /// `reconfigure` once after construction so existing schedule-enabled
    /// agents pick up.
    pub async fn start(
        modula: PathBuf,
        loops: LoopRegistry,
        engine_socket: String,
        events: Arc<dyn EventSink>,
        repos: Repositories,
    ) -> anyhow::Result<Self> {
        let scheduler = JobScheduler::new()
            .await
            .map_err(|e| anyhow::anyhow!("scheduler init: {e}"))?;
        scheduler
            .start()
            .await
            .map_err(|e| anyhow::anyhow!("scheduler start: {e}"))?;
        Ok(Self {
            scheduler,
            modula,
            job_ids: Arc::new(Mutex::new(Vec::new())),
            loops,
            engine_socket,
            events,
            repos,
        })
    }

    /// Drop every job we previously registered and re-register from the DB.
    /// Called at startup and after every CRUD write that could affect a
    /// schedule.
    pub async fn reconfigure(&self) -> anyhow::Result<()> {
        let old: Vec<Uuid> = std::mem::take(&mut *self.job_ids.lock());
        for id in old {
            if let Err(e) = self.scheduler.remove(&id).await {
                tracing::warn!("[scheduler] remove job {id}: {e}");
            }
        }

        let mut new_ids: Vec<Uuid> = Vec::new();
        // Returns (ws_id, agent_id, agent_name, cron, tz)
        let scheduled = self
            .repos
            .agents
            .scheduled_across_workspaces(&self.repos.pool)
            .await?;
        for (ws_id, agent_id, agent_name, raw_cron, raw_tz) in scheduled {
            let Some(spec) = scheduled_spec(&agent_id, &agent_name, &raw_cron, &raw_tz) else {
                continue;
            };
            match self.add_agent_job(ws_id.clone(), spec).await {
                Ok(id) => new_ids.push(id),
                Err(e) => tracing::warn!("[scheduler] agent reg in {ws_id}: {e}"),
            }
        }
        let registered = new_ids.len();
        *self.job_ids.lock() = new_ids;
        tracing::info!("[scheduler] {registered} job(s) registered");
        Ok(())
    }

    async fn add_agent_job(&self, ws: String, spec: ScheduledSpec) -> anyhow::Result<Uuid> {
        let ScheduledSpec {
            agent_id,
            agent_name,
            cron,
            tz,
        } = spec;
        let this = self.clone();
        let job = Job::new_async_tz(cron.as_str(), tz, move |_uuid, _sched| {
            let this = this.clone();
            let ws = ws.clone();
            let agent_id = agent_id.clone();
            let agent_name = agent_name.clone();
            Box::pin(async move {
                this.fire_agent(&ws, &agent_id, &agent_name).await;
            })
        })
        .map_err(|e| anyhow::anyhow!("build agent job: {e}"))?;
        self.scheduler
            .add(job)
            .await
            .map_err(|e| anyhow::anyhow!("register agent job: {e}"))
    }

    /// Resolve a workspace's on-disk directory from the scheduler's own repos.
    /// `WorkspaceService` re-syncs the scheduler on workspace CRUD, so the
    /// scheduler is a construction-time dependency of that service and can't DI
    /// it back without a cycle — it resolves the directory here instead.
    async fn workspace_dir(&self, ws: &str) -> ApiResult<PathBuf> {
        let slug = self.repos.workspaces.slug_for(&self.repos.pool, ws).await?;
        let dir = self.modula.join(&slug);
        if !dir.is_dir() {
            return Err(ApiError::NotFound(format!("workspace not found: {ws}")));
        }
        Ok(dir)
    }

    /// Fire one scheduled agent (no args). Resolves the provider via DB.
    async fn fire_agent(&self, ws: &str, agent_id: &str, agent_name: &str) {
        let ws_dir = match self.workspace_dir(ws).await {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!("[scheduler] agent {}/{} workspace: {}", ws, agent_name, e);
                return;
            }
        };
        match spawn::spawn_tracked(
            &self.repos,
            &self.loops,
            SpawnParams {
                ws_id: ws.to_string(),
                ws_dir,
                agent_id: agent_id.to_string(),
                agent_name: agent_name.to_string(),
                arg_map: BTreeMap::new(),
                engine_socket: self.engine_socket.clone(),
            },
            None,
            &serde_json::json!({"trigger": "scheduled"}),
            &self.events,
        )
        .await
        {
            Ok(s) => tracing::info!(
                "[scheduler] agent {}/{} → pid {} (run {})",
                ws,
                agent_name,
                s.pid,
                s.run_id
            ),
            Err(e) => {
                tracing::warn!("[scheduler] agent {}/{} spawn: {}", ws, agent_name, e);
            }
        }
    }
}

struct ScheduledSpec {
    agent_id: String,
    agent_name: String,
    cron: String,
    tz: Tz,
}

/// Promote 5-field cron expressions to 6-field (sec/min/hour/day/month/dow)
/// for `tokio-cron-scheduler`.
fn scheduled_spec(
    agent_id: &str,
    agent_name: &str,
    raw_cron: &str,
    raw_tz: &str,
) -> Option<ScheduledSpec> {
    let cron = raw_cron.trim();
    if cron.is_empty() {
        return None;
    }
    let cron = if cron.split_whitespace().count() == 5 {
        format!("0 {cron}")
    } else {
        cron.to_string()
    };
    let tz = raw_tz.parse::<Tz>().unwrap_or(Tz::UTC);
    Some(ScheduledSpec {
        agent_id: agent_id.to_string(),
        agent_name: agent_name.to_string(),
        cron,
        tz,
    })
}
