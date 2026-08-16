//! End-to-end coverage for the `modula` CRUD CLI — the surface every spawned
//! agent uses instead of curl. Each case drives the *compiled binary* as a
//! subprocess (the same one the harness runs as the engine) with
//! `MODULA_ENGINE_SOCKET` / `MODULA_WORKSPACE` set, then asserts on the
//! formatted plain-text output. The point is to prove the write commands take
//! effect over the gRPC IPC transport and that every command prints labelled
//! text, never raw JSON.

use std::process::Output;

use anyhow::Result;
use modula_test_support::{modula_bin, Harness};

mod common;

/// Run `modula <args…>` against the harness engine + workspace, capturing the
/// output. Mirrors how `spawn.rs` invokes the CLI: env-resolved IPC socket +
/// ws, no flags.
fn run_cli(h: &Harness, ws: &str, args: &[&str]) -> Output {
    std::process::Command::new(modula_bin())
        .args(args)
        .env("MODULA_ENGINE_SOCKET", &h.socket)
        .env("MODULA_WORKSPACE", ws)
        .output()
        .expect("spawn modula CLI")
}

/// Run a command expected to succeed; return its stdout. Panics with the
/// captured stderr on a non-zero exit so failures are legible.
fn ok_stdout(h: &Harness, ws: &str, args: &[&str]) -> String {
    let out = run_cli(h, ws, args);
    assert!(
        out.status.success(),
        "`modula {}` failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).expect("utf8 stdout")
}

/// Formatted text never opens with a JSON delimiter — the cardinal rule of the
/// CLI is "plain text, never JSON".
fn assert_not_json(label: &str, s: &str) {
    let first = s.trim_start().chars().next();
    assert!(
        !matches!(first, Some('{') | Some('[')),
        "{label} output looks like raw JSON: {s:?}"
    );
}

/// Pull the task UUID out of `created task: <uuid> (<external>)`.
fn task_id_from_create(stdout: &str) -> String {
    stdout
        .strip_prefix("created task: ")
        .and_then(|rest| rest.split(" (").next())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| panic!("unexpected create output: {stdout:?}"))
}

/// Collect variant UUIDs from the `created variant: <uuid> (position N)` lines.
fn variant_ids_from_create(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .filter_map(|l| l.strip_prefix("created variant: "))
        .filter_map(|rest| rest.split(" (").next())
        .map(|s| s.trim().to_string())
        .collect()
}

/// First pipeline key from `config get` output. The pipeline block lists
/// `  <key> — <label> [<tone>]…`; read a real key instead of hardcoding one.
fn first_pipeline_key(config_out: &str) -> String {
    let mut in_pipeline = false;
    for line in config_out.lines() {
        if line.starts_with("pipeline:") {
            in_pipeline = true;
            continue;
        }
        if in_pipeline {
            if let Some(rest) = line.strip_prefix("  ") {
                if let Some(key) = rest.split(" — ").next() {
                    return key.trim().to_string();
                }
            }
        }
    }
    panic!("no pipeline key in config output: {config_out:?}");
}

#[tokio::test]
async fn cli_crud_lifecycle() -> Result<()> {
    let h = Harness::start().await?;
    let ws = common::fresh_workspace(&h, "cli").await?;

    // config get — labelled sections, a real pipeline key, plain text.
    let config_out = ok_stdout(&h, &ws, &["config", "get"]);
    assert_not_json("config get", &config_out);
    assert!(config_out.contains("pipeline:"), "config: {config_out}");
    assert!(config_out.contains("projects:"), "config: {config_out}");
    assert!(config_out.contains("limits:"), "config: {config_out}");
    let pipeline_key = first_pipeline_key(&config_out);

    // task create → list/get reflect the new task.
    let create_out = ok_stdout(&h, &ws, &["task", "create", r#"{"title":"CLI smoke"}"#]);
    assert_not_json("task create", &create_out);
    let task_id = task_id_from_create(&create_out);

    let list_out = ok_stdout(&h, &ws, &["task", "list"]);
    assert_not_json("task list", &list_out);
    assert!(list_out.contains("title: CLI smoke"), "list: {list_out}");
    assert!(list_out.contains(&task_id), "list missing id: {list_out}");
    assert!(list_out.contains("pipeline_status:"), "list: {list_out}");

    let get_out = ok_stdout(&h, &ws, &["task", "get", &task_id]);
    assert_not_json("task get", &get_out);
    assert!(
        get_out.contains(&format!("id: {task_id}")),
        "get: {get_out}"
    );
    assert!(get_out.contains("title: CLI smoke"), "get: {get_out}");

    // variant create → task get shows the two new variants (write took effect).
    let vcreate_out = ok_stdout(&h, &ws, &["variant", "create", &task_id, r#"{"count":2}"#]);
    assert_not_json("variant create", &vcreate_out);
    let variant_ids = variant_ids_from_create(&vcreate_out);
    assert_eq!(variant_ids.len(), 2, "expected 2 variants: {vcreate_out}");

    let get_out = ok_stdout(&h, &ws, &["task", "get", &task_id]);
    assert!(get_out.contains("position: 1"), "get: {get_out}");
    assert!(get_out.contains("position: 2"), "get: {get_out}");
    assert!(get_out.contains(&variant_ids[0]), "get: {get_out}");

    // variant patch via raw status, then read it back.
    let patch_out = ok_stdout(
        &h,
        &ws,
        &[
            "variant",
            "patch",
            &variant_ids[0],
            r#"{"status":"in_progress"}"#,
        ],
    );
    assert!(
        patch_out.contains("in_progress"),
        "variant patch: {patch_out}"
    );
    let vget_out = ok_stdout(&h, &ws, &["variant", "get", &variant_ids[0]]);
    assert_not_json("variant get", &vget_out);
    assert!(
        vget_out.contains(&format!("variant: {}", variant_ids[0])),
        "variant get: {vget_out}"
    );
    assert!(
        vget_out.contains("status: in_progress"),
        "variant get: {vget_out}"
    );

    // variant patch via the code-reviewer `action` shape → "accepted".
    ok_stdout(
        &h,
        &ws,
        &[
            "variant",
            "patch",
            &variant_ids[1],
            r#"{"status":"ready_for_review"}"#,
        ],
    );
    let accept_out = ok_stdout(
        &h,
        &ws,
        &[
            "variant",
            "patch",
            &variant_ids[1],
            r#"{"action":"accept"}"#,
        ],
    );
    assert!(accept_out.contains("accepted"), "accept: {accept_out}");

    // comment create → list shows the entry.
    let ccreate_out = ok_stdout(
        &h,
        &ws,
        &[
            "comment",
            "create",
            &task_id,
            r#"{"author":"worker","kind":"comment","content":"hello from cli"}"#,
        ],
    );
    assert!(
        ccreate_out.contains("worker"),
        "comment create: {ccreate_out}"
    );

    let clist_out = ok_stdout(&h, &ws, &["comment", "list", &task_id]);
    assert_not_json("comment list", &clist_out);
    assert!(
        clist_out.contains("author: worker"),
        "comment list: {clist_out}"
    );
    assert!(
        clist_out.contains("content: hello from cli"),
        "comment list: {clist_out}"
    );

    // task patch (roadmap routing) advances the pipeline status.
    let tpatch_out = ok_stdout(
        &h,
        &ws,
        &[
            "task",
            "patch",
            &task_id,
            &format!(r#"{{"status":"{pipeline_key}"}}"#),
        ],
    );
    assert!(
        tpatch_out.contains(&pipeline_key),
        "task patch: {tpatch_out}"
    );
    let get_out = ok_stdout(&h, &ws, &["task", "get", &task_id]);
    assert!(
        get_out.contains(&format!("pipeline_status: {pipeline_key}")),
        "task get after roadmap patch: {get_out}"
    );

    Ok(())
}

#[tokio::test]
async fn cli_error_paths_exit_nonzero() -> Result<()> {
    let h = Harness::start().await?;
    let ws = common::fresh_workspace(&h, "cli-err").await?;

    // Unknown variant id — the CLI can't resolve an owning task.
    let out = run_cli(
        &h,
        &ws,
        &["variant", "get", "00000000-0000-0000-0000-000000000000"],
    );
    assert!(!out.status.success(), "bogus variant get should fail");
    assert!(
        !out.stderr.is_empty(),
        "expected a one-line stderr message, got none"
    );
    assert!(out.stdout.is_empty(), "error path must not print stdout");

    // Malformed JSON body is rejected before any RPC call.
    let out = run_cli(&h, &ws, &["task", "create", "not json"]);
    assert!(!out.status.success(), "malformed body should fail");
    assert!(!out.stderr.is_empty(), "expected stderr for bad JSON");

    // A semantically invalid request (missing title) surfaces the server error.
    let out = run_cli(&h, &ws, &["task", "create", "{}"]);
    assert!(
        !out.status.success(),
        "task create with no title should fail"
    );
    assert!(
        !out.stderr.is_empty(),
        "expected stderr for empty task body"
    );

    // An unresolvable --workspace (neither id nor slug) fails before any work.
    let out = run_cli(&h, &ws, &["task", "list", "--ws", "no-such-workspace"]);
    assert!(!out.status.success(), "bogus --ws should fail");
    assert!(!out.stderr.is_empty(), "expected stderr for bad --ws");
    assert!(out.stdout.is_empty(), "error path must not print stdout");

    Ok(())
}

/// `workspace` commands (global, not ws-scoped) and the `--workspace` override
/// resolving an id or a slug.
#[tokio::test]
async fn cli_workspace_commands_and_ws_override() -> Result<()> {
    let h = Harness::start().await?;
    let ws1 = common::fresh_workspace(&h, "cli-one").await?;
    let ws2 = common::fresh_workspace(&h, "cli-two").await?;

    // workspace list — both workspaces, with their derived slugs, plain text.
    let list_out = ok_stdout(&h, &ws1, &["workspace", "list"]);
    assert_not_json("workspace list", &list_out);
    assert!(list_out.contains(&ws1), "list missing ws1: {list_out}");
    assert!(list_out.contains(&ws2), "list missing ws2: {list_out}");
    assert!(list_out.contains("slug: cli-one"), "list: {list_out}");
    assert!(list_out.contains("slug: cli-two"), "list: {list_out}");

    // workspace get by slug resolves to the right id.
    let get_out = ok_stdout(&h, &ws1, &["workspace", "get", "cli-two"]);
    assert_not_json("workspace get", &get_out);
    assert!(get_out.contains(&format!("id: {ws2}")), "get: {get_out}");
    assert!(get_out.contains("slug: cli-two"), "get: {get_out}");

    // --workspace overrides $MODULA_WORKSPACE: env points at ws1, the flag (a
    // slug) targets ws2, so the new task lands in ws2 and not in ws1.
    let create_out = ok_stdout(
        &h,
        &ws1,
        &["task", "create", r#"{"title":"in two"}"#, "--ws", "cli-two"],
    );
    let task_id = task_id_from_create(&create_out);

    let in_two = ok_stdout(&h, &ws1, &["task", "list", "--workspace", &ws2]);
    assert!(in_two.contains(&task_id), "task missing from ws2: {in_two}");

    let in_one = ok_stdout(&h, &ws1, &["task", "list"]);
    assert!(!in_one.contains(&task_id), "task leaked into ws1: {in_one}");

    Ok(())
}
