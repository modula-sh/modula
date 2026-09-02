//! Central event-driven dispatcher.
//!
//! One tokio task per process. It wakes on three triggers:
//! 1. The event ticker (default 5s) — walks every workspace, dispatches
//!    matching agents for unprocessed events, then reaps exited children.
//! 2. SIGCHLD from the kernel — runs an immediate reap pass so the
//!    dashboard sees `agent_runs.status = completed` within milliseconds
//!    of an agent exiting (no per-run tokio task needed).
//! 3. The reap safety-net ticker (1s) — catches engine-restart leftovers
//!    where the original child has been reparented to init and SIGCHLD
//!    will never fire here.
//! 4. The prune ticker (1h) — drops `events` past the sync feed's retention
//!    window, which is what keeps a backfill scan bounded.
//!
//! Status mutations are owned by agents (via CRUD PUTs), not the dispatcher.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value as Json};
use tokio::task::JoinHandle;

use modula_db::events::EventRecord;
use modula_platform as platform;
use modula_types::{Agent, AgentArgDef};

use crate::events::{self, EventSink};
use crate::loop_registry::LoopRegistry;
use crate::spawn::{self, SpawnParams};
use crate::workspaces::WorkspaceService;
use modula_core::repositories::Repositories;
use modula_db::events::EVENT_RETENTION_DAYS;

pub mod expr;

const DEFAULT_INTERVAL_SECS: u64 = 5;
const REAP_SAFETY_NET_SECS: u64 = 1;
const EVENT_MAX_AGE_SECS: i64 = 24 * 60 * 60;
const EVENT_BATCH_LIMIT: i64 = 100;
const PRUNE_INTERVAL_SECS: u64 = 60 * 60;

/// Owns the dispatcher's shared dependencies so they aren't threaded through
/// every call. Cheap to construct; all fields are cloneable handles.
pub struct Dispatcher {
    repos: Repositories,
    workspaces: WorkspaceService,
    loops: LoopRegistry,
    engine_socket: String,
    events: Arc<dyn EventSink>,
}

impl Dispatcher {
    pub fn new(
        repos: Repositories,
        workspaces: WorkspaceService,
        loops: LoopRegistry,
        engine_socket: String,
        events: Arc<dyn EventSink>,
    ) -> Self {
        Self {
            repos,
            workspaces,
            loops,
            engine_socket,
            events,
        }
    }

    /// Move the dispatcher onto its own tokio task and return its handle.
    pub fn spawn(self) -> JoinHandle<()> {
        let interval = std::env::var("MODULA_DISPATCH_INTERVAL_SECS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .filter(|&n| n > 0)
            .unwrap_or(DEFAULT_INTERVAL_SECS);
        tracing::info!(
            "[dispatcher] tick interval = {interval}s, reap interval = {REAP_SAFETY_NET_SECS}s"
        );
        tokio::spawn(async move {
            let mut event_ticker = tokio::time::interval(Duration::from_secs(interval));
            event_ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            let mut reap_ticker = tokio::time::interval(Duration::from_secs(REAP_SAFETY_NET_SECS));
            reap_ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            // Fast-path reap trigger: fires on each child exit where the OS supports
            // it (unix SIGCHLD), and never fires elsewhere — see `platform::child_exit`.
            let mut prune_ticker = tokio::time::interval(Duration::from_secs(PRUNE_INTERVAL_SECS));
            prune_ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            let mut child_exit = platform::ChildExitWatcher::new();
            loop {
                tokio::select! {
                    _ = event_ticker.tick() => {
                        if let Err(e) = self.event_tick().await {
                            tracing::warn!("[dispatcher] event tick error: {e}");
                        }
                    }
                    _ = reap_ticker.tick() => {
                        self.reap_all().await;
                    }
                    _ = prune_ticker.tick() => {
                        self.prune_events().await;
                    }
                    _ = child_exit.recv() => {
                        self.reap_all().await;
                    }
                }
            }
        })
    }

    async fn event_tick(&self) -> Result<(), modula_core::error::ApiError> {
        let workspaces = self.repos.workspaces.list(&self.repos.pool).await?;
        for ws in workspaces {
            let ws_dir = match self.workspaces.workspace_dir(&ws.id).await {
                Ok(d) => d,
                Err(e) => {
                    tracing::warn!("[dispatcher] ws {} missing on disk: {}", ws.id, e);
                    continue;
                }
            };
            self.drive_events(&ws.id, &ws_dir).await;
        }
        Ok(())
    }

    /// Drop event-log rows past the log's retention window. A consumer
    /// resuming from a pruned cursor has to start over.
    async fn prune_events(&self) {
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(EVENT_RETENTION_DAYS))
            .format("%Y-%m-%dT%H:%M:%S%.3fZ")
            .to_string();
        match self
            .repos
            .events
            .prune_before(&self.repos.pool, &cutoff)
            .await
        {
            Ok(0) => {}
            Ok(n) => tracing::info!("[dispatcher] pruned {n} events older than {cutoff}"),
            Err(e) => tracing::warn!("[dispatcher] prune events: {e}"),
        }
    }

    /// Reap exited children across every workspace. Pids belonging to multi-
    /// iteration loops are skipped — their loop controller drives them.
    async fn reap_all(&self) {
        let workspaces = match self.repos.workspaces.list(&self.repos.pool).await {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("[dispatcher] list workspaces for reap: {e}");
                return;
            }
        };
        for ws in workspaces {
            self.reap_processes(&ws.id).await;
        }
    }

    async fn drive_events(&self, ws_id: &str, ws_dir: &Path) {
        let events = match self
            .repos
            .events
            .list_unprocessed(
                &self.repos.pool,
                ws_id,
                EVENT_MAX_AGE_SECS,
                EVENT_BATCH_LIMIT,
            )
            .await
        {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("[dispatcher] list events for {ws_id}: {e}");
                return;
            }
        };
        if events.is_empty() {
            return;
        }
        let agents = match self.repos.agents.list(&self.repos.pool, ws_id).await {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("[dispatcher] list agents for {ws_id}: {e}");
                return;
            }
        };

        for ev in events {
            let ev_data = ev.data_json();
            let ev_env = json!({
                "event": {
                    "type": ev.type_,
                    "data": ev_data,
                }
            });
            for agent in &agents {
                if matches_agent(agent, &ev_env) {
                    let arg_map = build_arg_map(&agent.args, &ev_data);
                    self.dispatch(self.spawn_params(ws_id, ws_dir, agent, arg_map), ev.id)
                        .await;
                    continue;
                }
                // Fan-out: task-scoped event (e.g. a task.update carrying
                // `pipeline_status`) that didn't match directly but the
                // agent operates per-variant. Re-evaluate against each variant.
                if needs_variant_fanout(agent, &ev_data) {
                    self.fan_out_per_variant(ws_id, ws_dir, agent, &ev, &ev_data)
                        .await;
                }
            }
            if let Err(e) = self
                .repos
                .events
                .mark_processed(&self.repos.pool, ev.id)
                .await
            {
                tracing::warn!("[dispatcher] mark processed {}: {e}", ev.id);
            }
        }
    }

    async fn fan_out_per_variant(
        &self,
        ws_id: &str,
        ws_dir: &Path,
        agent: &Agent,
        ev: &EventRecord,
        ev_data: &Json,
    ) {
        let task_id = match ev_data.get("task_id").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => return,
        };
        // Listing skips variants the agent is already running on, so we don't
        // double-fire on a fresh roadmap event while the first run is alive.
        let variants = match self
            .repos
            .variants
            .list_for_task_idle_for(&self.repos.pool, ws_id, task_id, &agent.id)
            .await
        {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("[dispatcher] fan-out list idle variants {ws_id}/{task_id}: {e}");
                return;
            }
        };
        for v in variants {
            let synth_data = events::variant_update(task_id, &v.id, json!(v.status));
            let synth_env = json!({
                "event": { "type": events::VARIANT_UPDATE, "data": synth_data.clone() }
            });
            if !matches_agent(agent, &synth_env) {
                continue;
            }
            let arg_map = build_arg_map(&agent.args, &synth_data);
            tracing::info!(
                "[dispatcher] fan-out: {agent} for {task_id}/{variant} on event {evid}",
                agent = agent.name,
                variant = v.id,
                evid = ev.id,
            );
            self.dispatch(self.spawn_params(ws_id, ws_dir, agent, arg_map), ev.id)
                .await;
        }
    }

    fn spawn_params(
        &self,
        ws_id: &str,
        ws_dir: &Path,
        agent: &Agent,
        arg_map: BTreeMap<String, String>,
    ) -> SpawnParams {
        SpawnParams {
            ws_id: ws_id.to_string(),
            ws_dir: ws_dir.to_path_buf(),
            agent_id: agent.id.clone(),
            agent_name: agent.name.clone(),
            arg_map,
            engine_socket: self.engine_socket.clone(),
        }
    }

    async fn dispatch(&self, params: SpawnParams, event_id: i64) {
        let run_data = json!({ "args": &params.arg_map });
        let label = format!("{}/{}", params.ws_id, params.agent_name);
        match spawn::spawn_tracked(
            &self.repos,
            &self.loops,
            params,
            Some(event_id),
            &run_data,
            &self.events,
        )
        .await
        {
            Ok(s) => tracing::info!(
                "[dispatcher] dispatched {label} → pid {} (run {}, event {event_id})",
                s.pid,
                s.run_id
            ),
            Err(e) => tracing::warn!("[dispatcher] spawn {label} failed: {e}"),
        }
    }

    async fn reap_processes(&self, ws_id: &str) {
        let rows = match self
            .repos
            .agent_processes
            .list_for_workspace(&self.repos.pool, ws_id)
            .await
        {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("[dispatcher] list processes for {ws_id}: {e}");
                return;
            }
        };
        for row in rows {
            // Multi-iteration loops are driven + finalized by their controller.
            if self.loops.is_registered(row.pid as u32) {
                continue;
            }
            if platform::process_manager().is_alive(row.pid as u32) {
                continue;
            }
            spawn::finalize_run(
                &self.repos,
                "[dispatcher]",
                row.agent_run_id,
                row.pid,
                ws_id,
                &self.events,
            )
            .await;
        }
    }
}

/// True when the agent is flagged `spawn_per_variant` AND the event
/// references a task but no specific variant. The dispatcher will
/// synthesise one `variant.update` event per variant and re-evaluate
/// the agent's rules against each.
fn needs_variant_fanout(agent: &Agent, ev_data: &Json) -> bool {
    if !agent.spawn_per_variant {
        return false;
    }
    let Some(data_map) = ev_data.as_object() else {
        return false;
    };
    data_map.get("task_id").is_some_and(|v| v.is_string()) && !data_map.contains_key("variant_id")
}

fn matches_agent(agent: &Agent, ev_env: &Json) -> bool {
    for src in &agent.rules {
        match expr::parse(src) {
            Ok(e) => {
                if expr::eval(&e, ev_env) {
                    return true;
                }
            }
            Err(err) => {
                tracing::warn!(
                    "[dispatcher] bad rule on agent {}: {err} (src={src:?})",
                    agent.name
                );
            }
        }
    }
    false
}

/// Fill the agent's declared arg flags from `event.data` keys of the same
/// name. Flag names use kebab-case (`--task-id`); event payload keys use
/// snake_case (`task_id`), matching their DB column names. Translate
/// `'-' → '_'` for the lookup.
fn build_arg_map(arg_defs: &[AgentArgDef], ev_data: &Json) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let data_map = ev_data.as_object();
    for def in arg_defs {
        let flag = def.flag.trim_start_matches('-').to_string();
        let lookup_key = flag.replace('-', "_");
        let value = data_map
            .and_then(|m| m.get(&lookup_key))
            .and_then(json_to_arg_value);
        if let Some(v) = value {
            out.insert(flag, v);
        }
    }
    out
}

fn json_to_arg_value(v: &Json) -> Option<String> {
    match v {
        Json::String(s) if !s.is_empty() => Some(s.clone()),
        Json::Number(n) => Some(n.to_string()),
        Json::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}
