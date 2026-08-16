//! Unix process backend: `setsid` detachment, `waitpid(WNOHANG)` liveness +
//! zombie reaping, `kill` for a single process, and `killpg` tree termination.
//! Compiles only on unix; macOS and Linux share it. This is a move of the logic
//! previously inline in `services/spawn.rs`, `services/processes.rs`, and
//! `services/dispatcher`.

use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};

use nix::errno::Errno;
use nix::sys::signal::{kill, killpg, Signal};
use nix::sys::wait::{waitpid, WaitPidFlag, WaitStatus};
use nix::unistd::{getpgid, setsid, Pid as NixPid};

use super::process::{Pid, ProcessManager, SpawnIo};

pub struct UnixProcessManager;

/// Spawn `cmd` detached into its own session + process group via `setsid`, with
/// stdin closed and stdout/stderr routed to `io`. Shared by both spawn flavors:
/// the only difference between them is the later kill path (single vs group).
fn spawn_setsid(mut cmd: Command, io: SpawnIo) -> std::io::Result<Pid> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::from(io.stdout))
        .stderr(Stdio::from(io.stderr));
    unsafe {
        cmd.pre_exec(|| setsid().map(|_| ()).map_err(std::io::Error::other));
    }
    let child = cmd.spawn()?;
    let pid = child.id();
    // Leak the handle: nothing should implicitly wait on the child and race the
    // dispatcher's reaper, and we want it to outlive its spawner.
    std::mem::forget(child);
    Ok(pid)
}

fn signal_for(escalate: bool) -> Signal {
    if escalate {
        Signal::SIGKILL
    } else {
        Signal::SIGTERM
    }
}

impl ProcessManager for UnixProcessManager {
    fn spawn_detached(&self, cmd: Command, io: SpawnIo) -> std::io::Result<Pid> {
        spawn_setsid(cmd, io)
    }

    fn spawn_standalone(&self, cmd: Command, io: SpawnIo) -> std::io::Result<Pid> {
        spawn_setsid(cmd, io)
    }

    fn is_alive(&self, pid: Pid) -> bool {
        // `kill(pid, 0)` reports zombies as alive; `waitpid(WNOHANG)` instead
        // distinguishes running from just-exited and reaps the zombie here.
        // `Err` (ECHILD: not our child / already reaped) counts as dead.
        matches!(
            waitpid(NixPid::from_raw(pid as i32), Some(WaitPidFlag::WNOHANG)),
            Ok(WaitStatus::StillAlive)
        )
    }

    fn kill(&self, pid: Pid, escalate: bool) -> std::io::Result<()> {
        // ESRCH (no such process) means it is already gone — treat as success.
        match kill(NixPid::from_raw(pid as i32), signal_for(escalate)) {
            Ok(()) | Err(Errno::ESRCH) => Ok(()),
            Err(e) => Err(std::io::Error::other(e)),
        }
    }

    fn kill_tree(&self, pid: Pid, escalate: bool) -> std::io::Result<()> {
        let nix_pid = NixPid::from_raw(pid as i32);
        let pgid = getpgid(Some(nix_pid)).map_err(std::io::Error::other)?;
        killpg(pgid, signal_for(escalate)).map_err(std::io::Error::other)
    }
}
