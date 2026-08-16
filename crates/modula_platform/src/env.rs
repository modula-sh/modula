//! PATH recovery for a process started outside a terminal.
//!
//! A GUI/launchd-launched process inherits a minimal or stale `PATH`, so user-
//! installed tools (Homebrew, npm globals, `~/.local/bin`) are invisible to
//! `which`. Merging in the `PATH` the OS builds for a fresh session restores
//! them: the login shell's on unix, the registry environment on Windows.

#[cfg(windows)]
use windows_sys::Win32::Foundation::ERROR_SUCCESS;
#[cfg(windows)]
use windows_sys::Win32::System::Registry::{
    RegGetValueW, HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ,
};

/// The platform's `PATH` entry separator.
const SEP: &str = if cfg!(windows) { ";" } else { ":" };

/// Merge the user's real `PATH` into this process's `PATH`. No-op when it can't
/// be resolved, leaving `PATH` as-is.
pub fn enrich_path_from_user_env() {
    let Some(recovered) = user_path() else {
        return;
    };
    let current = std::env::var("PATH").unwrap_or_default();
    std::env::set_var("PATH", merge(&recovered, &current));
}

/// Concatenate the recovered `PATH` with the process's, dropping empties and
/// duplicates. Dirs the launcher prepended beyond what the recovered `PATH`
/// knows (test shims, wrapper dirs) keep the lead — recovery adds the user's
/// tools, it must not override a deliberate lookup order. After that, the
/// recovered ordering wins.
fn merge(recovered: &str, current: &str) -> String {
    let recovered_dirs: std::collections::HashSet<&str> =
        recovered.split(SEP).filter(|p| !p.is_empty()).collect();
    let prepended = current
        .split(SEP)
        .filter(|p| !p.is_empty())
        .take_while(|p| !recovered_dirs.contains(p));
    let mut seen = std::collections::HashSet::new();
    prepended
        .chain(recovered.split(SEP))
        .chain(current.split(SEP))
        .filter(|p| !p.is_empty() && seen.insert(*p))
        .collect::<Vec<_>>()
        .join(SEP)
}

/// Ask the user's login+interactive shell for its `PATH`. Runs on a worker
/// thread with a timeout so a slow or input-waiting rc file can't hang startup.
#[cfg(unix)]
fn user_path() -> Option<String> {
    use std::process::{Command, Stdio};
    use std::time::Duration;

    const MARK: &str = "__MODULA_PATH__";
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
    // `-l` loads profile files (Homebrew), `-i` loads rc files (nvm/asdf).
    // Markers bracket the value so rc banners printed before it are ignored.
    let script = format!("printf '{MARK}%s{MARK}' \"$PATH\"");

    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let out = Command::new(&shell)
            .args(["-ilc", &script])
            // Probe from the system default: rc files prepend to whatever
            // they inherit, so probing with our own PATH would echo it back
            // and defeat `merge`'s prepended-prefix rule.
            .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output();
        let _ = tx.send(out);
    });

    let output = rx.recv_timeout(Duration::from_secs(5)).ok()?.ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let start = stdout.find(MARK)? + MARK.len();
    let end = stdout[start..].find(MARK)? + start;
    let path = &stdout[start..end];
    (!path.is_empty()).then(|| path.to_string())
}

/// The `PATH` Windows composes for a new session, machine entries then user —
/// read from the registry rather than the environment because a GUI process
/// inherits the block Explorer captured at logon, which never picks up entries
/// an installer added since.
#[cfg(windows)]
fn user_path() -> Option<String> {
    const MACHINE_ENV: &str = r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment";
    const USER_ENV: &str = "Environment";

    let parts: Vec<String> = [
        registry_value(HKEY_LOCAL_MACHINE, MACHINE_ENV, "Path"),
        registry_value(HKEY_CURRENT_USER, USER_ENV, "Path"),
    ]
    .into_iter()
    .flatten()
    .filter(|p| !p.is_empty())
    .collect();
    (!parts.is_empty()).then(|| parts.join(SEP))
}

/// Read a string value from the registry, expanding any `%VAR%` references.
#[cfg(windows)]
fn registry_value(hive: HKEY, subkey: &str, name: &str) -> Option<String> {
    use std::os::windows::ffi::OsStrExt;

    let wide = |s: &str| -> Vec<u16> {
        std::ffi::OsStr::new(s)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    };
    let subkey = wide(subkey);
    let name = wide(name);

    // Without RRF_NOEXPAND, RegGetValueW expands REG_EXPAND_SZ itself.
    // SAFETY: both name buffers are live and null-terminated. The first call
    // passes no output buffer and only sizes the value; the second writes at
    // most `bytes`, which is the true capacity of `buf`.
    let mut bytes = 0u32;
    let status = unsafe {
        RegGetValueW(
            hive,
            subkey.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut bytes,
        )
    };
    if status != ERROR_SUCCESS || bytes == 0 {
        return None;
    }

    let mut buf = vec![0u16; bytes as usize / 2 + 1];
    let mut bytes = (buf.len() * 2) as u32;
    let status = unsafe {
        RegGetValueW(
            hive,
            subkey.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            buf.as_mut_ptr().cast(),
            &mut bytes,
        )
    };
    if status != ERROR_SUCCESS {
        return None;
    }
    let text = String::from_utf16_lossy(&buf[..bytes as usize / 2]);
    Some(text.trim_end_matches('\0').to_string())
}

#[cfg(not(any(unix, windows)))]
fn user_path() -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::{merge, SEP};

    fn path(dirs: &[&str]) -> String {
        dirs.join(SEP)
    }

    #[test]
    fn merge_dedupes_and_keeps_recovered_order() {
        assert_eq!(
            merge(&path(&["/a", "/b"]), &path(&["/b", "/c"])),
            path(&["/a", "/b", "/c"])
        );
        assert_eq!(merge(&format!("/a{SEP}{SEP}/a"), &format!("/a{SEP}")), "/a");
    }

    #[test]
    fn merge_keeps_launcher_prepended_dirs_first() {
        assert_eq!(
            merge(&path(&["/a", "/b"]), &path(&["/shim", "/a", "/b"])),
            path(&["/shim", "/a", "/b"])
        );
        // Only the prefix is promoted; later novel dirs keep their trailing position.
        assert_eq!(
            merge(&path(&["/a", "/b"]), &path(&["/shim", "/a", "/late"])),
            path(&["/shim", "/a", "/b", "/late"])
        );
    }

    #[cfg(windows)]
    #[test]
    fn user_path_reads_the_registry() {
        let path = super::user_path().expect("registry PATH should resolve");
        assert!(
            path.to_lowercase().contains(r"\windows"),
            "machine PATH always carries a Windows dir, got {path}"
        );
    }
}
