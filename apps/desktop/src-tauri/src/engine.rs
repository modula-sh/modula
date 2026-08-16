//! Engine process lifecycle for the desktop shell.
//!
//! The engine binary is bundled beside the app as a Tauri sidecar. On every
//! launch we stop any engine still serving (a prior session's may be an
//! outdated build) and spawn the bundled binary as a detached standalone
//! process (so its agents are never tied to its lifetime). On a full Quit we
//! stop it. Per-OS spawn/kill lives in `modula-platform`; this module is the
//! OS-agnostic orchestration.

use std::path::PathBuf;
use std::time::Duration;

use tauri::async_runtime;

use modula_client::ModulaClient;

use crate::platform;

const READY_TIMEOUT: Duration = Duration::from_secs(15);
const STOP_TIMEOUT: Duration = Duration::from_secs(5);
const POLL_INTERVAL: Duration = Duration::from_millis(200);

/// The bundled engine binary, which Tauri places next to the app executable.
fn engine_binary() -> std::io::Result<PathBuf> {
    let name = if cfg!(windows) {
        "modula.exe"
    } else {
        "modula"
    };
    let dir = std::env::current_exe()?
        .parent()
        .ok_or_else(|| std::io::Error::other("app executable has no parent dir"))?
        .to_path_buf();
    Ok(dir.join(name))
}

/// Make the bundled engine runnable as `modula` from a terminal. Version-gated:
/// a shipped app only rewrites the symlink across upgrades, not on every launch
/// (dev launches relink unconditionally via `modula link-cli` in `scripts/dev.sh`).
/// Best-effort: a failure here never blocks the GUI.
pub fn link_cli() {
    let bin = match engine_binary() {
        Ok(bin) => bin,
        Err(e) => {
            eprintln!("modula: could not resolve engine binary: {e}");
            return;
        }
    };
    match platform::link_for_version(platform::cli_linker(), &bin, env!("CARGO_PKG_VERSION")) {
        Ok(None) | Ok(Some(platform::LinkOutcome::Linked(_))) => {}
        Ok(Some(platform::LinkOutcome::NeedsPath(path))) => {
            if let Some(dir) = path.parent() {
                eprintln!(
                    "modula: CLI linked at {} — add {} to your PATH to use `modula`",
                    path.display(),
                    dir.display()
                );
            }
        }
        Err(e) => eprintln!("modula: could not link the CLI onto PATH: {e}"),
    }
}

/// Whether the engine is serving on the local IPC endpoint, via the shared
/// gRPC client's `HealthService.Check`.
fn is_running(engine: &ModulaClient) -> bool {
    async_runtime::block_on(engine.is_serving())
}

/// Start this launch's bundled engine, stopping any engine already serving so
/// the engine always matches the app's binary.
pub fn ensure_running(engine: &ModulaClient) -> anyhow::Result<()> {
    stop_serving(engine)?;

    let bin = engine_binary()?;
    let mut cmd = std::process::Command::new(&bin);
    cmd.arg("engine");
    platform::process_manager().spawn_standalone(cmd, log_io()?)?;

    let deadline = std::time::Instant::now() + READY_TIMEOUT;
    while std::time::Instant::now() < deadline {
        if is_running(engine) {
            return Ok(());
        }
        std::thread::sleep(POLL_INTERVAL);
    }
    anyhow::bail!("engine did not become healthy within {READY_TIMEOUT:?}");
}

/// Stop the engine on a full Quit. Only acts when an engine is actually serving,
/// and terminates just that process — in-flight agents keep running.
pub fn shutdown(engine: &ModulaClient) {
    if let Err(e) = stop_serving(engine) {
        eprintln!("modula: failed to stop engine: {e}");
    }
}

/// Stop whatever engine is serving: SIGTERM, then SIGKILL if it stays on the
/// socket. The pid comes from the health response (the pidfile can be stale);
/// the pidfile is only a fallback for engines that predate the field.
fn stop_serving(engine: &ModulaClient) -> anyhow::Result<()> {
    if !is_running(engine) {
        return Ok(());
    }
    let pid = async_runtime::block_on(engine.serving_pid())
        .or_else(pidfile_pid)
        .ok_or_else(|| anyhow::anyhow!("an engine is serving but its pid is unknown"))?;

    for escalate in [false, true] {
        platform::process_manager().kill(pid, escalate)?;
        let deadline = std::time::Instant::now() + STOP_TIMEOUT;
        while std::time::Instant::now() < deadline {
            if !is_running(engine) {
                return Ok(());
            }
            std::thread::sleep(POLL_INTERVAL);
        }
    }
    anyhow::bail!("engine (pid {pid}) still serving after SIGKILL");
}

fn pidfile_pid() -> Option<platform::Pid> {
    let path = platform::engine_pid_file()?;
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}

/// Open the engine's log files as the child's stdout/stderr, matching where the
/// engine previously logged under launchd.
fn log_io() -> std::io::Result<platform::SpawnIo> {
    let dir = platform::modula_dir()
        .ok_or_else(|| std::io::Error::other("could not determine modula dir"))?
        .join("logs");
    std::fs::create_dir_all(&dir)?;
    let open = |name: &str| {
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join(name))
    };
    Ok(platform::SpawnIo {
        stdout: open("engine.log")?,
        stderr: open("engine.err.log")?,
    })
}
