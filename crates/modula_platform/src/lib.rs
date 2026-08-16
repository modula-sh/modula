//! Platform strategy boundary, shared by the engine and the desktop shell. Every
//! OS-specific decision lives behind a narrow trait here; callers depend only on
//! the traits, never on `nix`/`launchctl`/`$HOME` or `#[cfg]`. The `#[cfg]` lives
//! only in this file: it selects the per-OS impl, and each impl file compiles
//! solely on its target.

pub mod child_exit;
pub mod cli_linker;
pub mod ctrl_handler;
pub mod env;
pub mod ipc_security;
pub mod paths;
pub mod pipe_security;
pub mod process;
pub mod service;
pub mod which;

pub use child_exit::ChildExitWatcher;
pub use cli_linker::{link_for_version, CliLinker, LinkOutcome};
pub use env::enrich_path_from_user_env;
pub use paths::{engine_pid_file, engine_socket_path, modula_dir};
pub use process::{Pid, ProcessManager, SpawnIo};
pub use service::ServiceManager;
pub use which::{is_on_path, which};

use std::path::PathBuf;

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
mod service_linux;
#[cfg(target_os = "macos")]
mod service_macos;
#[cfg(windows)]
mod service_windows;

/// The user's home directory (`$HOME` on unix, `%USERPROFILE%` on Windows),
/// resolved by `dirs` so callers never branch on the OS env-var name.
pub fn home_dir() -> Option<PathBuf> {
    dirs::home_dir()
}

/// The platform's null device — the bit bucket passed to tools like
/// `git diff --no-index`. `/dev/null` on unix, `NUL` on Windows.
pub const NULL_DEVICE: &str = if cfg!(windows) { "NUL" } else { "/dev/null" };

/// The process-lifecycle backend for this target. Returned as a `'static`
/// reference so every caller shares one zero-sized strategy object.
#[cfg(unix)]
pub fn process_manager() -> &'static dyn ProcessManager {
    static MANAGER: unix::UnixProcessManager = unix::UnixProcessManager;
    &MANAGER
}

#[cfg(windows)]
pub fn process_manager() -> &'static dyn ProcessManager {
    static MANAGER: windows::WindowsProcessManager = windows::WindowsProcessManager;
    &MANAGER
}

/// The autostart/service backend for this target. macOS launchd, Linux systemd
/// user units, and a Windows registry Run key are three distinct supervisors, so
/// — unlike [`process_manager`] — this selects per `target_os`, not per `unix`.
#[cfg(target_os = "macos")]
pub fn service_manager() -> &'static dyn ServiceManager {
    static MANAGER: service_macos::LaunchdServiceManager = service_macos::LaunchdServiceManager;
    &MANAGER
}

#[cfg(target_os = "linux")]
pub fn service_manager() -> &'static dyn ServiceManager {
    static MANAGER: service_linux::SystemdServiceManager = service_linux::SystemdServiceManager;
    &MANAGER
}

#[cfg(windows)]
pub fn service_manager() -> &'static dyn ServiceManager {
    static MANAGER: service_windows::RegistryServiceManager =
        service_windows::RegistryServiceManager;
    &MANAGER
}

/// The CLI-on-PATH backend for this target. macOS and Linux share a symlink
/// strategy, so — like [`process_manager`] — this selects per `unix`/`windows`.
#[cfg(unix)]
pub fn cli_linker() -> &'static dyn CliLinker {
    static LINKER: cli_linker::UnixCliLinker = cli_linker::UnixCliLinker;
    &LINKER
}

#[cfg(windows)]
pub fn cli_linker() -> &'static dyn CliLinker {
    static LINKER: cli_linker::WindowsCliLinker = cli_linker::WindowsCliLinker;
    &LINKER
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::fs::File;
    use std::process::Command;

    fn null_io() -> SpawnIo {
        let stdout = File::options().write(true).open("/dev/null").unwrap();
        let stderr = stdout.try_clone().unwrap();
        SpawnIo { stdout, stderr }
    }

    #[test]
    fn spawn_detached_reports_liveness_then_death() {
        let pm = process_manager();
        let mut cmd = Command::new("sleep");
        cmd.arg("0.3");
        let pid = pm.spawn_detached(cmd, null_io()).unwrap();
        assert!(
            pm.is_alive(pid),
            "child should be alive immediately after spawn"
        );
        // Poll until it exits (well past the 0.3s sleep), then assert death.
        for _ in 0..40 {
            if !pm.is_alive(pid) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        panic!("child {pid} never observed as exited");
    }

    #[test]
    fn kill_tree_terminates_the_process_group() {
        let pm = process_manager();
        let mut cmd = Command::new("sleep");
        cmd.arg("30");
        let pid = pm.spawn_detached(cmd, null_io()).unwrap();
        assert!(pm.is_alive(pid), "child should be alive before kill_tree");
        // Exercises getpgid + killpg (the must-not-change group-kill path):
        // the detached child is its own session/group leader, so the group is
        // exactly its subtree.
        pm.kill_tree(pid, true).unwrap();
        for _ in 0..40 {
            if !pm.is_alive(pid) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        panic!("child {pid} survived kill_tree");
    }

    #[test]
    fn standalone_spawn_is_killed_by_pid() {
        let pm = process_manager();
        let mut cmd = Command::new("sleep");
        cmd.arg("30");
        let pid = pm.spawn_standalone(cmd, null_io()).unwrap();
        assert!(pm.is_alive(pid), "child should be alive before kill");
        pm.kill(pid, true).unwrap();
        for _ in 0..40 {
            if !pm.is_alive(pid) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        panic!("child {pid} survived kill");
    }

    #[test]
    fn kill_of_a_dead_pid_is_ok() {
        let pm = process_manager();
        let cmd = Command::new("true");
        let pid = pm.spawn_standalone(cmd, null_io()).unwrap();
        // Let it exit, reap it, then a second kill must still succeed.
        for _ in 0..40 {
            if !pm.is_alive(pid) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        pm.kill(pid, false)
            .expect("killing an already-dead pid is a no-op");
    }
}
