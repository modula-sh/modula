//! Executable resolver — the single source of truth for how a bare command
//! name maps to a file on `PATH`. Windows honors `PATHEXT` (`.exe`/`.cmd`/…);
//! unix matches the bare name. Backs both tool detection (`services::tools`)
//! and command construction (`services::providers`) so the extension rule is
//! never duplicated in a caller.

use std::path::{Path, PathBuf};

/// Resolve `name` against `PATH`, returning the first matching file. A `name`
/// that already contains a path separator is treated as a literal path.
pub fn which(name: &str) -> Option<PathBuf> {
    if name.contains(['/', '\\']) {
        let p = PathBuf::from(name);
        return p.is_file().then_some(p);
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .flat_map(|dir| candidates(&dir, name))
        .find(|c| c.is_file())
}

/// Whether `name` resolves to an executable on `PATH`.
pub fn is_on_path(name: &str) -> bool {
    which(name).is_some()
}

#[cfg(windows)]
fn candidates(dir: &Path, name: &str) -> Vec<PathBuf> {
    // A name that already carries an extension is trusted as-is; otherwise it
    // is expanded against PATHEXT so `npm`→`npm.cmd`, `claude`→`claude.exe`.
    if Path::new(name).extension().is_some() {
        return vec![dir.join(name)];
    }
    let pathext = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into());
    pathext
        .split(';')
        .filter(|ext| !ext.is_empty())
        .map(|ext| dir.join(format!("{name}{ext}")))
        .collect()
}

#[cfg(not(windows))]
fn candidates(dir: &Path, name: &str) -> Vec<PathBuf> {
    vec![dir.join(name)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_known_and_rejects_unknown() {
        #[cfg(unix)]
        assert!(which("sh").is_some(), "sh should resolve on unix PATH");
        assert!(which("definitely-not-a-real-binary-xyz").is_none());
        assert!(!is_on_path("definitely-not-a-real-binary-xyz"));
    }

    #[test]
    fn names_with_a_separator_are_treated_as_literal_paths() {
        // A name carrying a separator bypasses PATH entirely: it must resolve to
        // an existing file as-is, and a non-existent one must not be invented.
        let real = std::env::current_exe().unwrap();
        assert!(real.to_string_lossy().contains(std::path::MAIN_SEPARATOR));
        assert_eq!(
            which(real.to_str().unwrap()).as_deref(),
            Some(real.as_path())
        );

        let missing = std::env::temp_dir().join("modula-no-such-tool-xyz");
        assert!(which(missing.to_str().unwrap()).is_none());
    }

    #[cfg(windows)]
    #[test]
    fn pathext_expands_bare_name() {
        let dir = std::env::temp_dir().join(format!("modula-which-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("widget.cmd");
        std::fs::write(&exe, "echo hi").unwrap();
        let saved = std::env::var_os("PATH");
        std::env::set_var("PATH", &dir);
        std::env::set_var("PATHEXT", ".EXE;.CMD");
        assert_eq!(which("widget"), Some(exe));
        if let Some(p) = saved {
            std::env::set_var("PATH", p);
        }
        std::fs::remove_dir_all(&dir).ok();
    }
}
