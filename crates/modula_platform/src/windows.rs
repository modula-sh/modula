//! Windows process backend: the POSIX session/group/signal model has no Windows
//! analogue, so this is a redesign rather than a port.
//!
//! - **Detached spawn** → `CREATE_NO_WINDOW` creation flag + a leaked `Child`
//!   handle (the engine never waits on the agent), so the agent outlives the
//!   engine exactly as `setsid` + `mem::forget` does on unix. `spawn_standalone`
//!   is the same minus the Job Object, so the daemon's children stay untracked.
//! - **Tree kill** → a per-agent **Job Object** named after the pid. Closing the
//!   engine's job handle does *not* kill members (the default limit flags omit
//!   `KILL_ON_JOB_CLOSE`), and a living member keeps the named object alive, so a
//!   restarted engine re-opens the job by name and `TerminateJobObject` still
//!   tears down the whole tree — the `killpg` analogue, restart-safe.
//! - **Single kill** → `TerminateProcess` on just that pid, leaving any children.
//! - **Liveness** → open the process handle and `WaitForSingleObject`; a process
//!   that is gone (or signalled) is dead. No zombies, so no reaping side effect.

use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};

use windows_sys::Win32::Foundation::{CloseHandle, FALSE, WAIT_OBJECT_0};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, OpenJobObjectW, TerminateJobObject,
};
use windows_sys::Win32::System::Threading::{
    OpenProcess, TerminateProcess, WaitForSingleObject, CREATE_NO_WINDOW, PROCESS_SET_QUOTA,
    PROCESS_SYNCHRONIZE, PROCESS_TERMINATE,
};

use super::process::{Pid, ProcessManager, SpawnIo};

/// The single access right `TerminateJobObject` requires. windows-sys 0.59 does
/// not export the job access-right constants, so define the one we need.
const JOB_OBJECT_TERMINATE: u32 = 0x0008;

pub struct WindowsProcessManager;

/// UTF-16, null-terminated Job Object name for an agent pid. The pid is the only
/// state the engine persists, so naming the job after it is what lets a
/// restarted engine re-open the tree for `kill_tree`.
fn job_name(pid: Pid) -> Vec<u16> {
    format!("modula_agent_{pid}\0").encode_utf16().collect()
}

/// Spawn `cmd` detached with no console window, stdin closed and stdout/stderr
/// routed to `io`. Returns the live `Child`; the caller decides whether to bind
/// it to a Job Object before leaking the handle.
fn spawn_no_window(mut cmd: Command, io: SpawnIo) -> std::io::Result<std::process::Child> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::from(io.stdout))
        .stderr(Stdio::from(io.stderr))
        .creation_flags(CREATE_NO_WINDOW);
    cmd.spawn()
}

impl ProcessManager for WindowsProcessManager {
    fn spawn_detached(&self, cmd: Command, io: SpawnIo) -> std::io::Result<Pid> {
        let child = spawn_no_window(cmd, io)?;
        let pid = child.id();

        // Bind the agent to a named Job Object so its whole tree can be killed
        // later — and, via the pid-derived name, after an engine restart.
        // SAFETY: each handle is null-checked, and every successful Open is
        // paired with a Close; Assign borrows the handles, taking neither.
        unsafe {
            let job = CreateJobObjectW(std::ptr::null(), job_name(pid).as_ptr());
            if !job.is_null() {
                let proc = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, FALSE, pid);
                if !proc.is_null() {
                    AssignProcessToJobObject(job, proc);
                    CloseHandle(proc);
                }
                // Drop our handle: the running member keeps the job alive and
                // resolvable by name, and the default flags don't kill on close.
                CloseHandle(job);
            }
        }

        // Leak the handle so nothing waits on the agent and it survives the engine.
        std::mem::forget(child);
        Ok(pid)
    }

    fn spawn_standalone(&self, cmd: Command, io: SpawnIo) -> std::io::Result<Pid> {
        // No Job Object: a later `kill` terminates only this pid, never the
        // daemon's agent children (which carry their own jobs).
        let child = spawn_no_window(cmd, io)?;
        let pid = child.id();
        std::mem::forget(child);
        Ok(pid)
    }

    fn is_alive(&self, pid: Pid) -> bool {
        // SAFETY: handle is null-checked before use and closed on every path.
        unsafe {
            let proc = OpenProcess(PROCESS_SYNCHRONIZE, FALSE, pid);
            if proc.is_null() {
                return false;
            }
            // Signalled (WAIT_OBJECT_0) means the process has exited; WAIT_TIMEOUT
            // (the only other expected result for a 0ms wait) means still running.
            let alive = WaitForSingleObject(proc, 0) != WAIT_OBJECT_0;
            CloseHandle(proc);
            alive
        }
    }

    fn kill(&self, pid: Pid, _escalate: bool) -> std::io::Result<()> {
        // Windows termination is always forceful; `escalate` is a no-op.
        single_process_kill(pid)
    }

    fn kill_tree(&self, pid: Pid, _escalate: bool) -> std::io::Result<()> {
        // Windows termination is always forceful; there is no SIGTERM/SIGKILL
        // distinction, so `escalate` is a no-op.
        // SAFETY: handle is null-checked, and closed when a job was opened.
        unsafe {
            let job = OpenJobObjectW(JOB_OBJECT_TERMINATE, FALSE, job_name(pid).as_ptr());
            if job.is_null() {
                return single_process_kill(pid);
            }
            let ok = TerminateJobObject(job, 1);
            CloseHandle(job);
            if ok == FALSE {
                return Err(std::io::Error::last_os_error());
            }
        }
        Ok(())
    }
}

/// Fallback when no job is resolvable (the member already exited and the named
/// object is gone): terminate the lone pid. A missing process is already dead.
fn single_process_kill(pid: Pid) -> std::io::Result<()> {
    // SAFETY: handle is null-checked and closed on every path.
    unsafe {
        let proc = OpenProcess(PROCESS_TERMINATE, FALSE, pid);
        if proc.is_null() {
            return Ok(());
        }
        let ok = TerminateProcess(proc, 1);
        CloseHandle(proc);
        if ok == FALSE {
            return Err(std::io::Error::last_os_error());
        }
    }
    Ok(())
}
