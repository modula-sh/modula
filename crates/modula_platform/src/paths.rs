//! Well-known on-disk locations shared by the engine and the desktop shell, so
//! both agree on where workspace state and runtime metadata live.

use std::path::PathBuf;

/// The workspace root: `$MODULA_DIR` if set, else `~/.modula`. `None` only when
/// the home directory can't be resolved and no override was given.
pub fn modula_dir() -> Option<PathBuf> {
    match std::env::var_os("MODULA_DIR") {
        Some(dir) => Some(PathBuf::from(dir)),
        None => crate::home_dir().map(|home| home.join(".modula")),
    }
}

/// The engine's pidfile. The engine writes its own pid here on startup; the
/// desktop reads it to stop the engine on quit.
pub fn engine_pid_file() -> Option<PathBuf> {
    modula_dir().map(|dir| dir.join("engine.pid"))
}

/// The engine's local-IPC endpoint path.
///
/// Unix: `~/.modula/engine.sock` — short enough to stay under `sun_path` limits.
/// Windows: `\\.\pipe\modula-engine-<user-hash>`.
pub fn engine_socket_path() -> Option<PathBuf> {
    #[cfg(unix)]
    {
        modula_dir().map(|dir| dir.join("engine.sock"))
    }
    #[cfg(windows)]
    {
        use std::hash::{Hash, Hasher};
        let user = std::env::var("USERNAME").unwrap_or_else(|_| "default".into());
        let mut h = std::collections::hash_map::DefaultHasher::new();
        user.hash(&mut h);
        let hash = h.finish();
        Some(PathBuf::from(format!(
            r"\\.\pipe\modula-engine-{hash:016x}"
        )))
    }
}
