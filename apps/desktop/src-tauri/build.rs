use std::path::Path;

fn main() {
    bake_update_settings();
    tauri_build::build();
}

/// Bake the update knobs from `app-settings.toml` into the binary as `env!`
/// constants read by `src/update.rs` — the Rust-native replacement for the old
/// Vite `define`, so the updater config never touches the JS toolchain.
fn bake_update_settings() {
    // Workspace root: src-tauri -> desktop -> apps -> root.
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../app-settings.toml");
    println!("cargo:rerun-if-changed={}", path.display());

    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let parsed: toml::Value = text.parse().expect("parse app-settings.toml");
    let update = parsed
        .get("update")
        .expect("app-settings.toml: missing [update] table");

    let interval = update
        .get("check_interval_secs")
        .and_then(toml::Value::as_integer)
        .expect("app-settings.toml: [update].check_interval_secs must be an integer");
    let repo = update
        .get("repo")
        .and_then(toml::Value::as_str)
        .expect("app-settings.toml: [update].repo must be a string");

    println!("cargo:rustc-env=MODULA_UPDATE_INTERVAL_SECS={interval}");
    println!("cargo:rustc-env=MODULA_UPDATE_REPO={repo}");
}
