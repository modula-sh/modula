//! Process-lifecycle strategy. Every OS-specific decision about spawning,
//! liveness, and killing a process lives behind `ProcessManager`; callers
//! depend only on this trait. The concrete impl is selected at the `platform`
//! module boundary — never with `#[cfg]` in callers.

use std::fs::File;
use std::process::Command;

/// Spawned-agent OS pid. Agents are detached children that outlive the engine,
/// so this is just the raw OS identifier the engine stores in `agent_processes`.
pub type Pid = u32;

/// Stdout/stderr destinations for a detached spawn. The caller opens the log
/// file (and decides its name), then hands the two inheritable handles here;
/// the impl owns wiring them onto the child as inherited stdio.
pub struct SpawnIo {
    pub stdout: File,
    pub stderr: File,
}

/// OS process lifecycle. Spawn (as a tree or standalone), observe liveness, and
/// terminate (a tree or a single process). Autostart/service install lives
/// behind a separate [`super::ServiceManager`].
pub trait ProcessManager: Send + Sync {
    /// Spawn `cmd` as a detached child that survives engine restart, tracked as
    /// a killable tree (process group / Job Object), with stdin closed and
    /// stdout/stderr routed to `io`. Returns the child's pid; the `Child` handle
    /// is intentionally leaked so no implicit wait races the dispatcher's reaper.
    fn spawn_detached(&self, cmd: Command, io: SpawnIo) -> std::io::Result<Pid>;

    /// Spawn `cmd` as a detached single process — like `spawn_detached` but
    /// without tree tracking, so its own children are never caught by a later
    /// `kill`. Used for the engine daemon, whose agents must outlive it.
    fn spawn_standalone(&self, cmd: Command, io: SpawnIo) -> std::io::Result<Pid>;

    /// Whether `pid` is still running. On platforms that produce zombies this
    /// also reaps an exited child in the same call, so a `false` result is final.
    fn is_alive(&self, pid: Pid) -> bool;

    /// Terminate a single process by pid (not its descendants). `escalate`
    /// requests a forceful kill (SIGKILL-equivalent) over a graceful one
    /// (SIGTERM-equivalent). A pid that is already gone is treated as success.
    fn kill(&self, pid: Pid, escalate: bool) -> std::io::Result<()>;

    /// Terminate the process and its descendants. `escalate` requests a forceful
    /// kill (SIGKILL-equivalent) instead of a graceful one (SIGTERM-equivalent).
    fn kill_tree(&self, pid: Pid, escalate: bool) -> std::io::Result<()>;
}
