//! Launch agents — pure Rust, no bash. `spawn_tracked` is the single entry
//! point for every trigger (manual, scheduled, event dispatch). For
//! multi-iteration agents (`loop_amount > 1`) a tokio loop controller
//! drives iterations 2..N and finalizes each run; single-iteration runs
//! have no per-run task — the central dispatcher reaper (SIGCHLD-driven,
//! with a periodic safety net for engine-restart leftovers) finalizes them.

use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use chrono::Local;
use serde_json::Value as Json;

use crate::core::error::ApiError;
use crate::platform::{self, SpawnIo};
use crate::services::events::{self, EventSink};
use crate::services::loop_registry::LoopRegistry;
use crate::services::providers::{ProviderRuntime, ProviderService};
use crate::state::Repositories;
use modula_db::agent_runs::{STATUS_COMPLETED, STATUS_FAILED};

/// Owned so the loop controller can reuse them for iterations 2..N.
pub struct SpawnParams {
    pub ws_id: String,
    pub ws_dir: PathBuf,
    /// UUID of the agent being spawned.
    pub agent_id: String,
    /// Human-readable display name (used for log filenames and env vars).
    pub agent_name: String,
    pub arg_map: BTreeMap<String, String>,
    /// Local IPC socket path the spawned agent uses to reach the engine
    /// (injected as `MODULA_ENGINE_SOCKET`, resolved by the `modula` CLI).
    pub engine_socket: String,
}

/// Resolved spawn-time data. Re-read by the loop controller before each
/// iteration so edits applied mid-loop take effect (e.g. model change).
struct ResolvedAgent {
    runtime: Arc<dyn ProviderRuntime>,
    loop_amount: u32,
    prompt: String,
    /// Effective skill prompt bodies (hidden ∪ opted-in), ordered by position.
    skills: Vec<String>,
    /// Human-readable spec folder for this run, relative to cwd (the workspace
    /// dir). `specs/<task-slug>/v<position>` for a variant-scoped agent, or
    /// `specs/<task-slug>` for a task-scoped one. `None` when there's no task.
    spec_dir: Option<String>,
}

/// The `agent_runs` row id and the first iteration's PID.
pub struct TrackedSpawn {
    pub run_id: i64,
    pub pid: u32,
}

const POLL_INTERVAL: Duration = Duration::from_millis(500);

/// Insert the `agent_runs` row, spawn iteration 1, and flip the row to
/// `failed` when the spawn errors — owning the compensation here means no
/// call site can forget it.
pub async fn spawn_tracked(
    repos: &Repositories,
    loops: &LoopRegistry,
    params: SpawnParams,
    event_id: Option<i64>,
    run_data: &Json,
    events: &Arc<dyn EventSink>,
) -> Result<TrackedSpawn, ApiError> {
    let ws_id = params.ws_id.clone();
    let agent_id = params.agent_id.clone();
    let agent_name = params.agent_name.clone();
    let run_id = repos
        .agent_runs
        .create(
            &repos.pool,
            &ws_id,
            &agent_id,
            &agent_name,
            event_id,
            run_data,
        )
        .await?;
    match launch(repos, loops, params, run_id, event_id, run_data, events).await {
        Ok(pid) => {
            events
                .publish(
                    &ws_id,
                    events::RUN_SPAWNED,
                    serde_json::json!({
                        "run_id": run_id,
                        "agent_id": agent_id,
                        "agent_name": agent_name,
                        "pid": pid,
                    }),
                )
                .await;
            Ok(TrackedSpawn { run_id, pid })
        }
        Err(e) => {
            let _ = repos
                .agent_runs
                .set_status(&repos.pool, run_id, STATUS_FAILED)
                .await;
            Err(e)
        }
    }
}

/// Best-effort finalize, shared by the loop controller and the central
/// reaper. `tag` prefixes the warn logs.
pub async fn finalize_run(
    repos: &Repositories,
    tag: &str,
    run_id: i64,
    pid: i64,
    ws_id: &str,
    events: &Arc<dyn EventSink>,
) {
    if let Err(e) = repos
        .agent_runs
        .set_status(&repos.pool, run_id, STATUS_COMPLETED)
        .await
    {
        tracing::warn!("{tag} mark run {run_id} completed: {e}");
    }
    if let Err(e) = repos.agent_processes.delete(&repos.pool, pid).await {
        tracing::warn!("{tag} delete process {pid}: {e}");
    }
    events
        .publish(
            ws_id,
            events::RUN_EXITED,
            serde_json::json!({"run_id": run_id, "pid": pid}),
        )
        .await;
}

async fn launch(
    repos: &Repositories,
    loops: &LoopRegistry,
    params: SpawnParams,
    run_id: i64,
    event_id: Option<i64>,
    run_data: &Json,
    events: &Arc<dyn EventSink>,
) -> Result<u32, ApiError> {
    let resolved = resolve(repos, &params.ws_id, &params.agent_id, &params.arg_map).await?;
    let total = resolved.loop_amount;
    let Spawned { pid, log_name } = spawn_iteration(&params, &resolved, 1, total)?;

    repos
        .agent_runs
        .set_log_path(&repos.pool, run_id, &log_name)
        .await?;
    repos
        .agent_runs
        .set_loop_meta(&repos.pool, run_id, 1, total as i64, run_id)
        .await?;
    repos
        .agent_processes
        .create(
            &repos.pool,
            &params.ws_id,
            &params.agent_id,
            &params.agent_name,
            run_id,
            pid,
        )
        .await?;

    if total > 1 {
        let cancel = Arc::new(AtomicBool::new(false));
        loops.register(pid, cancel.clone());
        spawn_loop_controller(LoopTask {
            registry: loops.clone(),
            repos: repos.clone(),
            cancel,
            params,
            total,
            first_pid: pid,
            first_run_id: run_id,
            event_id,
            data: run_data.to_string(),
            events: events.clone(),
        });
    }
    Ok(pid)
}

/// Look up the agent + its provider and build the ProviderRuntime. The loop
/// amount is a per-task setting: when `task_id` is set and a matching
/// `task_agent_settings` row exists it supplies the count, otherwise 1.
/// The human-readable spec folder for a run, relative to the workspace dir.
/// Variant-scoped → `specs/<task-slug>/v<position>`; task-scoped → `specs/<task-slug>`.
/// Returns `None` when there's no task context, or the lookup fails (best-effort).
async fn compute_spec_dir(
    repos: &Repositories,
    ws_id: &str,
    arg_map: &BTreeMap<String, String>,
) -> Option<String> {
    let task_id = arg_map.get("task-id")?;
    let task = repos.tasks.get(&repos.pool, ws_id, task_id).await.ok()?;
    let slug =
        crate::services::workspaces::task_spec_slug(task.external_id.as_deref(), &task.title);
    let base = format!("specs/{slug}");
    match arg_map.get("variant-id") {
        Some(vid) => {
            let pos = repos
                .variants
                .position_of(&repos.pool, ws_id, task_id, vid)
                .await
                .ok()
                .flatten();
            Some(match pos {
                Some(p) => format!("{base}/v{p}"),
                None => base,
            })
        }
        None => Some(base),
    }
}

async fn resolve(
    repos: &Repositories,
    ws_id: &str,
    agent_id: &str,
    arg_map: &BTreeMap<String, String>,
) -> Result<ResolvedAgent, ApiError> {
    let agent = repos.agents.get(&repos.pool, ws_id, agent_id).await?;
    if agent.provider_id.trim().is_empty() {
        return Err(ApiError::BadRequest(format!(
            "agent {agent_id:?} has no provider_id"
        )));
    }
    let skills = repos
        .agent_skills
        .for_agent(&repos.pool, ws_id, &agent.skills)
        .await?;
    let provider = repos
        .providers
        .get(&repos.pool, ws_id, &agent.provider_id)
        .await?;
    let model = agent
        .model
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let runtime = ProviderService::runtime_from_provider(&provider, model)?;
    let task_id = arg_map.get("task-id").map(String::as_str);
    let loop_amount = match task_id {
        Some(task_id) => repos
            .task_agent_settings
            .get(&repos.pool, ws_id, task_id, agent_id)
            .await?
            .map(|s| s.loop_setting.amount.max(1) as u32)
            .unwrap_or(1),
        None => 1,
    };
    let spec_dir = compute_spec_dir(repos, ws_id, arg_map).await;
    Ok(ResolvedAgent {
        runtime,
        loop_amount,
        prompt: agent.prompt.unwrap_or_default(),
        skills,
        spec_dir,
    })
}

struct Spawned {
    pid: u32,
    log_name: String,
}

/// One provider-CLI invocation. Builds the prompt, log file, and env, then
/// hands the command to `platform::ProcessManager::spawn_detached`, which
/// detaches the child so it survives the engine. `iter` is 1-based; `total`
/// is the configured loop amount (always >= 1). Returns the spawned pid + log
/// filename (basename) so the caller can stamp it onto `agent_runs`.
fn spawn_iteration(
    params: &SpawnParams,
    resolved: &ResolvedAgent,
    iter: u32,
    total: u32,
) -> Result<Spawned, ApiError> {
    let SpawnParams {
        ws_id,
        ws_dir,
        agent_id,
        agent_name,
        arg_map,
        engine_socket,
    } = params;

    let logs_dir = ws_dir.join("logs");
    std::fs::create_dir_all(&logs_dir)?;
    let ts = Local::now().format("%Y%m%d-%H%M%S").to_string();
    let tag = sanitize_tag(&pick_log_tag(arg_map));
    let iter_suffix = if total > 1 {
        format!("-i{iter}")
    } else {
        String::new()
    };
    let log_name = if tag.is_empty() {
        format!("{agent_name}-{ts}{iter_suffix}.log")
    } else {
        format!("{agent_name}-{tag}-{ts}{iter_suffix}.log")
    };
    let log_path = logs_dir.join(&log_name);
    let log_file: File = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&log_path)?;
    let stderr_file = log_file.try_clone()?;

    let prompt = build_prompt(
        ws_id,
        &resolved.prompt,
        &resolved.skills,
        arg_map,
        resolved.spec_dir.as_deref(),
        iter,
        total,
    );

    let mut cmd = resolved.runtime.build_command(&prompt, None);
    cmd.env("MODULA_WORKSPACE", ws_id)
        .env("MODULA_ENGINE_SOCKET", engine_socket)
        .env("MODULA_AGENT_ID", agent_id)
        .env("MODULA_AGENT_NAME", agent_name)
        .env("MODULA_AGENT_EXTRA", build_extra(arg_map))
        .env("MODULA_LOOP_ITER", iter.to_string())
        .env("MODULA_LOOP_TOTAL", total.to_string())
        .current_dir(ws_dir);
    for (k, v) in resolved.runtime.env_vars() {
        cmd.env(k, v);
    }
    cmd.env("MODULA_LOG_TS", &ts);

    // Agents invoke the `modula` CLI by name; the engine binary IS `modula`, so
    // prepend its own directory to the child's PATH to guarantee the subcommand
    // resolves across dev (`target/debug`), release, and installed layouts.
    match std::env::current_exe() {
        Ok(exe) => {
            if let Some(dir) = exe.parent() {
                let mut search = vec![dir.to_path_buf()];
                if let Some(existing) = std::env::var_os("PATH") {
                    search.extend(std::env::split_paths(&existing));
                }
                match std::env::join_paths(search) {
                    Ok(path) => {
                        cmd.env("PATH", path);
                    }
                    Err(e) => tracing::warn!("could not assemble child PATH: {e}"),
                }
            }
        }
        Err(e) => tracing::warn!("current_exe() failed; leaving child PATH untouched: {e}"),
    }

    let io = SpawnIo {
        stdout: log_file,
        stderr: stderr_file,
    };
    let pid = platform::process_manager()
        .spawn_detached(cmd, io)
        .map_err(|e| ApiError::Internal(format!("spawn provider: {e}")))?;
    Ok(Spawned { pid, log_name })
}

struct LoopTask {
    registry: LoopRegistry,
    repos: Repositories,
    cancel: Arc<AtomicBool>,
    params: SpawnParams,
    total: u32,
    first_pid: u32,
    first_run_id: i64,
    event_id: Option<i64>,
    data: String,
    events: Arc<dyn EventSink>,
}

fn spawn_loop_controller(task: LoopTask) {
    tokio::spawn(async move {
        let LoopTask {
            registry,
            repos,
            cancel,
            params,
            total,
            first_pid,
            first_run_id,
            event_id,
            data,
            events,
        } = task;
        let SpawnParams {
            ws_id,
            agent_id,
            agent_name,
            arg_map,
            ..
        } = &params;
        let mut current_pid = first_pid;
        let mut current_run_id = first_run_id;
        for iter in 2..=total {
            wait_for_exit(current_pid).await;
            if cancel.load(Ordering::SeqCst) {
                tracing::info!("[loop] {agent_name} cancelled before iteration {iter}/{total}");
                break;
            }
            let resolved = match resolve(&repos, ws_id, agent_id, arg_map).await {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!("[loop] {agent_name} resolve: {e}");
                    break;
                }
            };
            let next_run_id = match repos
                .agent_runs
                .create_iteration(
                    &repos.pool,
                    ws_id,
                    agent_id,
                    agent_name,
                    event_id,
                    &data,
                    iter as i64,
                    total as i64,
                    first_run_id,
                )
                .await
            {
                Ok(id) => id,
                Err(e) => {
                    tracing::warn!("[loop] {agent_name} insert iter {iter} row: {e}");
                    break;
                }
            };
            let Spawned {
                pid: next_pid,
                log_name,
            } = match spawn_iteration(&params, &resolved, iter, total) {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!("[loop] {agent_name} iteration {iter} spawn: {e}");
                    let _ = repos
                        .agent_runs
                        .set_status(&repos.pool, next_run_id, STATUS_FAILED)
                        .await;
                    break;
                }
            };
            if let Err(e) = repos
                .agent_runs
                .set_log_path(&repos.pool, next_run_id, &log_name)
                .await
            {
                tracing::warn!("[loop] {agent_name} set log_path on run {next_run_id}: {e}");
            }
            if let Err(e) = repos
                .agent_processes
                .create(
                    &repos.pool,
                    ws_id,
                    agent_id,
                    agent_name,
                    next_run_id,
                    next_pid,
                )
                .await
            {
                tracing::warn!("[loop] {agent_name} insert process row: {e}");
            }
            events
                .publish(
                    ws_id,
                    events::RUN_SPAWNED,
                    serde_json::json!({
                        "run_id": next_run_id,
                        "agent_id": agent_id,
                        "agent_name": agent_name,
                        "pid": next_pid,
                        "iter": iter,
                    }),
                )
                .await;
            registry.advance(current_pid, next_pid);
            tracing::info!(
                "[loop] {agent_name} iteration {iter}/{total} \u{2192} run {next_run_id} pid {next_pid}"
            );
            finalize_run(
                &repos,
                "[loop]",
                current_run_id,
                current_pid as i64,
                ws_id,
                &events,
            )
            .await;
            current_pid = next_pid;
            current_run_id = next_run_id;
        }
        wait_for_exit(current_pid).await;
        registry.deregister(current_pid);
        finalize_run(
            &repos,
            "[loop]",
            current_run_id,
            current_pid as i64,
            ws_id,
            &events,
        )
        .await;
        tracing::info!(
            "[loop] {agent_name} group {first_run_id} \u{2192} final run {current_run_id} completed (pid {current_pid})"
        );
    });
}

/// Poll-wait for `pid` to exit via the platform `ProcessManager`, which reaps
/// the child as part of the liveness check on platforms that produce zombies.
async fn wait_for_exit(pid: u32) {
    let pm = platform::process_manager();
    while pm.is_alive(pid) {
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

/// Render the "Inputs for this run:" block appended to the agent prompt.
fn build_extra(args: &BTreeMap<String, String>) -> String {
    if args.is_empty() {
        return String::new();
    }
    let lines = args
        .iter()
        .map(|(k, v)| format!("- {k}: {v}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!("Inputs for this run:\n{lines}")
}

/// Choose the most-identifying arg as the log filename tag.
fn pick_log_tag(args: &BTreeMap<String, String>) -> String {
    for preferred in ["task-id", "task", "branch"] {
        if let Some(v) = args.get(preferred) {
            return v.clone();
        }
    }
    args.values().next().cloned().unwrap_or_default()
}

fn sanitize_tag(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut last_dash = false;
    for c in input.chars() {
        if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
            out.push(c);
            last_dash = c == '-';
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

/// Assemble the spawn prompt: workspace header → skill bodies (framework
/// context, ordered by position) → base prompt body (role context) → run
/// inputs + footer.
fn build_prompt(
    ws_id: &str,
    body: &str,
    skills: &[String],
    args: &BTreeMap<String, String>,
    spec_dir: Option<&str>,
    iter: u32,
    total: u32,
) -> String {
    let loop_line = if total > 1 {
        let finality = if iter == total {
            "This is the FINAL iteration: finish the work and do all end-of-run \
             finalization (status changes, summaries, handoffs) now."
        } else {
            "This is NOT the final iteration: make incremental progress, then exit \
             WITHOUT any end-of-run finalization — no status changes, summaries, or \
             handoffs. A later iteration resumes the work and closes it out."
        };
        format!("\nThis run is iteration {iter} of {total} of a repeat-spawn loop. {finality}")
    } else {
        String::new()
    };
    let header = format!(
        "You are operating in workspace '{ws_id}'. Your cwd is the workspace data \
directory (it holds specs/, logs/, wiki/) — it is NOT a git repository and NOT \
your project's source code, so `git`/`ls` here will look unfamiliar; project \
checkouts live at the paths listed in the engine's /config, and you `cd` into one \
only when a step needs git or code work. Workspace state lives in the engine and \
is reached through the `modula` CLI, which is already on your PATH and \
auto-detects the running engine and current workspace — read and write via it \
(never an HTTP API or curl). The MODULA_* environment variables are already set \
for you. Trust this environment: if one command's output ever looks empty or \
odd, re-run that single command; do not start diagnosing connectivity, sockets, \
or env vars.{loop_line}"
    );
    let skills_len: usize = skills.iter().map(|s| s.len() + 2).sum();
    let mut p = String::with_capacity(header.len() + skills_len + body.len() + 128);
    p.push_str(&header);
    for skill in skills {
        let skill = skill.trim();
        if skill.is_empty() {
            continue;
        }
        p.push_str("\n\n");
        p.push_str(skill);
    }
    p.push_str("\n\n");
    p.push_str(body);
    let extra = build_extra(args);
    if !extra.is_empty() {
        p.push_str("\n\n");
        p.push_str(&extra);
    }
    if let Some(sd) = spec_dir {
        p.push_str("\n\nThis run's spec folder is `");
        p.push_str(sd);
        p.push_str("` (relative to your cwd) — read and write your spec files there.");
    }
    p.push_str("\n\nRun end-to-end and print your end-of-run report.");
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_prompt_orders_skills_before_base_and_footer() {
        let skills = vec![
            "## Skill: Engine API\nhidden one".to_string(),
            "## Skill: AI Wiki\nopted in".to_string(),
        ];
        let out = build_prompt(
            "ws1",
            "# Agent: Worker\nrole body",
            &skills,
            &BTreeMap::new(),
            None,
            1,
            1,
        );

        let header = out.find("You are operating in workspace").unwrap();
        let engine = out.find("## Skill: Engine API").unwrap();
        let wiki = out.find("## Skill: AI Wiki").unwrap();
        let base = out.find("# Agent: Worker").unwrap();
        let footer = out.find("Run end-to-end").unwrap();

        // header < skills (in position order) < base < footer.
        assert!(header < engine && engine < wiki && wiki < base && base < footer);
    }

    #[test]
    fn build_prompt_without_skills_keeps_header_then_base() {
        let out = build_prompt("ws1", "role body", &[], &BTreeMap::new(), None, 1, 1);
        assert!(!out.contains("## Skill"));
        let header = out.find("You are operating").unwrap();
        let base = out.find("role body").unwrap();
        assert!(header < base);
    }

    #[test]
    fn build_prompt_has_every_part_with_inputs_and_loop_line() {
        let skills = vec!["## Skill: Engine API\nhidden one".to_string()];
        let mut args = BTreeMap::new();
        args.insert("task-id".to_string(), "T1".to_string());
        args.insert("variant-id".to_string(), "V1".to_string());

        // Non-final iteration of a multi-iteration loop.
        let out = build_prompt(
            "ws1",
            "# Agent: Worker\nrole body",
            &skills,
            &args,
            None,
            2,
            3,
        );

        let header = out.find("You are operating in workspace").unwrap();
        let loop_line = out.find("iteration 2 of 3").unwrap();
        let skill = out.find("## Skill: Engine API").unwrap();
        let base = out.find("# Agent: Worker").unwrap();
        let inputs = out.find("Inputs for this run:").unwrap();
        let footer = out.find("Run end-to-end").unwrap();

        // Every part is present and in order:
        // header (with loop line) < skills < base < inputs < footer.
        assert!(header < loop_line && loop_line < skill);
        assert!(skill < base && base < inputs && inputs < footer);

        // The loop line marks a non-final iteration, and the inputs are rendered.
        assert!(out.contains("This is NOT the final iteration"));
        assert!(out.contains("- task-id: T1"));
        assert!(out.contains("- variant-id: V1"));
    }
}
