//! Detects whether the system CLIs modula recommends are installed by scanning
//! the engine process `PATH`. Presentation (labels, logos, install links) lives
//! in the frontend; this module owns only the canonical id list + installed bool.
//!
//! The `PATH` lookup itself (including the Windows `.exe`/`.cmd` rule) lives in
//! [`crate::platform::which`] so detection and command construction share one rule.

use crate::platform;

pub const TOOLS: [&str; 4] = ["gh", "claude", "codex", "opencode"];

pub fn detect() -> Vec<(&'static str, bool)> {
    TOOLS
        .iter()
        .map(|&id| (id, platform::is_on_path(id)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_known_and_unknown_binaries() {
        #[cfg(unix)]
        assert!(platform::is_on_path("sh"), "sh should be on PATH");
        assert!(!platform::is_on_path("definitely-not-a-real-binary-xyz"));
    }
}
