//! CLI-on-PATH strategy: make the bundled `modula` binary runnable from a
//! terminal, current-user and admin-free. macOS/Linux symlink it into a PATH
//! directory; Windows drops a `.cmd` shim into the per-user `WindowsApps` folder
//! (on PATH by default). Idempotent — the desktop re-runs it on every launch so
//! it self-heals when the app moves or updates.

use std::path::{Path, PathBuf};

/// Where the CLI was made available, and whether that location is already on the
/// user's PATH (so the caller can advise adding it when it isn't).
pub enum LinkOutcome {
    /// Linked into a directory already on PATH; nothing more for the user to do.
    Linked(PathBuf),
    /// Linked into a directory that may not be on PATH yet.
    NeedsPath(PathBuf),
}

pub trait CliLinker: Send + Sync {
    /// Ensure `target` (the bundled engine binary) is invokable as `modula` on
    /// the user's PATH. Idempotent.
    fn ensure_linked(&self, target: &Path) -> std::io::Result<LinkOutcome>;
}

/// Where the version of the currently-linked CLI is recorded, gating the
/// production relink in [`link_for_version`].
fn version_marker() -> Option<PathBuf> {
    crate::modula_dir().map(|dir| dir.join("cli-linked-version"))
}

/// Version-gated link, for a shipped app: (re)link `target` as `modula` only
/// when `version` differs from the version recorded on the last successful link
/// (or nothing is recorded yet), then record `version`. Returns `Ok(None)` when
/// the recorded version already matched and the link was left untouched.
///
/// A shipped app calls this on every launch but only rewrites the symlink across
/// upgrades. Dev rebuilds keep the same crate version, so the dev script links
/// unconditionally via [`CliLinker::ensure_linked`] instead.
pub fn link_for_version(
    linker: &dyn CliLinker,
    target: &Path,
    version: &str,
) -> std::io::Result<Option<LinkOutcome>> {
    let marker =
        version_marker().ok_or_else(|| std::io::Error::other("could not determine modula dir"))?;
    if let Ok(recorded) = std::fs::read_to_string(&marker) {
        if recorded.trim() == version {
            return Ok(None);
        }
    }
    let outcome = linker.ensure_linked(target)?;
    if let Some(parent) = marker.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&marker, version)?;
    Ok(Some(outcome))
}

#[cfg(unix)]
pub use unix_impl::UnixCliLinker;
#[cfg(windows)]
pub use windows_impl::WindowsCliLinker;

#[cfg(unix)]
mod unix_impl {
    use super::*;
    use std::os::unix::fs::symlink;

    const CLI_NAME: &str = "modula";

    pub struct UnixCliLinker;

    impl CliLinker for UnixCliLinker {
        fn ensure_linked(&self, target: &Path) -> std::io::Result<LinkOutcome> {
            // Prefer /usr/local/bin (on PATH by default); fall back to a per-user
            // dir when it isn't writable, which never needs admin.
            let usr_local = Path::new("/usr/local/bin");
            if usr_local.is_dir() {
                let link = usr_local.join(CLI_NAME);
                if force_symlink(target, &link).is_ok() {
                    return Ok(LinkOutcome::Linked(link));
                }
            }

            let dir = crate::home_dir()
                .ok_or_else(|| std::io::Error::other("could not determine home directory"))?
                .join(".local")
                .join("bin");
            let link = dir.join(CLI_NAME);
            force_symlink(target, &link)?;
            if dir_on_path(&dir) {
                Ok(LinkOutcome::Linked(link))
            } else {
                Ok(LinkOutcome::NeedsPath(link))
            }
        }
    }

    /// Create `link` → `target`, replacing any existing entry so re-runs are
    /// idempotent and a moved app's stale symlink is corrected.
    fn force_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
        if let Some(parent) = link.parent() {
            std::fs::create_dir_all(parent)?;
        }
        match std::fs::symlink_metadata(link) {
            Ok(_) => std::fs::remove_file(link)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        symlink(target, link)
    }

    fn dir_on_path(dir: &Path) -> bool {
        std::env::var_os("PATH")
            .map(|path| std::env::split_paths(&path).any(|p| p == dir))
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Records how many times it was asked to link, so a test can prove the
    /// version gate skips the real link work.
    struct CountingLinker {
        calls: AtomicUsize,
    }

    impl CliLinker for CountingLinker {
        fn ensure_linked(&self, target: &Path) -> std::io::Result<LinkOutcome> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(LinkOutcome::Linked(target.to_path_buf()))
        }
    }

    #[test]
    fn link_for_version_relinks_only_when_the_version_changes() {
        let tmp = std::env::temp_dir().join(format!("modula-link-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::env::set_var("MODULA_DIR", &tmp);

        let linker = CountingLinker {
            calls: AtomicUsize::new(0),
        };
        let target = Path::new("/some/modula");
        let calls = || linker.calls.load(Ordering::SeqCst);

        // First link for a version records it and does the work.
        assert!(link_for_version(&linker, target, "1.0.0")
            .unwrap()
            .is_some());
        assert_eq!(calls(), 1);

        // Same version on the next launch is a no-op — no relink.
        assert!(link_for_version(&linker, target, "1.0.0")
            .unwrap()
            .is_none());
        assert_eq!(calls(), 1);

        // A new version (an upgrade) relinks and re-records.
        assert!(link_for_version(&linker, target, "1.1.0")
            .unwrap()
            .is_some());
        assert_eq!(calls(), 2);

        std::env::remove_var("MODULA_DIR");
        let _ = std::fs::remove_dir_all(&tmp);
    }
}

#[cfg(windows)]
mod windows_impl {
    use super::*;

    pub struct WindowsCliLinker;

    impl CliLinker for WindowsCliLinker {
        fn ensure_linked(&self, target: &Path) -> std::io::Result<LinkOutcome> {
            // %LOCALAPPDATA%\Microsoft\WindowsApps is on the per-user PATH by
            // default, so a shim there needs no registry edit or admin. A `.cmd`
            // (on PATHEXT) forwards to the bundled exe by absolute path.
            let local = std::env::var_os("LOCALAPPDATA")
                .ok_or_else(|| std::io::Error::other("LOCALAPPDATA is not set"))?;
            let dir = Path::new(&local).join("Microsoft").join("WindowsApps");
            std::fs::create_dir_all(&dir)?;
            let shim = dir.join("modula.cmd");
            std::fs::write(&shim, format!("@\"{}\" %*\r\n", target.display()))?;
            Ok(LinkOutcome::Linked(shim))
        }
    }
}
