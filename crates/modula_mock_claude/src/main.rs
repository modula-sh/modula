//! mock-claude — deterministic stand-in for the `claude` CLI used in E2E
//! tests. Reads a JSON recipe and:
//!   - Emits each `stream[]` entry as one stream-json line to stdout.
//!   - Optionally appends a line to a workspace-relative file (one mutation
//!     kind: `append_line`, used by the loop tests to count iterations).
//!   - Sleeps `sleep_ms`, then exits with `exit_code`.
//!
//! All paths resolve relative to the current directory, which the engine sets
//! to the workspace dir (`<modula>/<slug>`) for every spawn — the same place a
//! real provider runs. The mock never touches the workspace UUID.
//!
//! Recipe lookup (first match wins):
//!   1. $MODULA_MOCK_RECIPE (literal JSON)
//!   2. ./mock-recipes/<agent>.json — `<agent>` from `$MODULA_AGENT_NAME`.
//!   3. Built-in default (init + result events, no mutations).

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::{env, fs, thread, time::Duration};

use serde::Deserialize;
use serde_json::Value as Json;

#[derive(Debug, Default, Deserialize)]
struct Recipe {
    #[serde(default)]
    stream: Vec<Json>,
    #[serde(default)]
    mutations: Vec<Mutation>,
    #[serde(default)]
    sleep_ms: u64,
    #[serde(default)]
    exit_code: i32,
}

#[derive(Debug, Deserialize)]
struct Mutation {
    /// Workspace-relative path of the target file.
    file: String,
    op: MutationOp,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum MutationOp {
    /// Append a single line (with trailing `\n`) to the file.
    AppendLine { value: String },
}

fn main() -> ExitCode {
    let ws_dir = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    // Record argv for test inspection (each invocation appends one JSON line).
    record_argv(&ws_dir);

    let recipe = load_recipe(&ws_dir);
    emit_stream(&recipe);
    if recipe.sleep_ms > 0 {
        thread::sleep(Duration::from_millis(recipe.sleep_ms));
    }
    for m in &recipe.mutations {
        if let Err(e) = apply_mutation(&ws_dir, m) {
            eprintln!("mock-claude: mutation failed for {}: {}", m.file, e);
        }
    }

    if recipe.exit_code == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(recipe.exit_code as u8)
    }
}

fn load_recipe(ws_dir: &Path) -> Recipe {
    if let Ok(literal) = env::var("MODULA_MOCK_RECIPE") {
        match serde_json::from_str::<Recipe>(&literal) {
            Ok(r) => return r,
            Err(e) => eprintln!("mock-claude: bad MODULA_MOCK_RECIPE: {e}"),
        }
    }
    if let Ok(agent) = env::var("MODULA_AGENT_NAME") {
        let candidate = ws_dir.join("mock-recipes").join(format!("{agent}.json"));
        if let Ok(text) = fs::read_to_string(&candidate) {
            match serde_json::from_str::<Recipe>(&text) {
                Ok(r) => return r,
                Err(e) => eprintln!("mock-claude: bad recipe {}: {}", candidate.display(), e),
            }
        }
    }
    default_recipe()
}

fn default_recipe() -> Recipe {
    // session_id is included so the conversation send path can capture it.
    let init = serde_json::json!({
        "type": "system",
        "subtype": "init",
        "session_id": "mock-session-id",
    });
    // Emit text via stream_event/content_block_delta (matches real Claude with
    // --include-partial-messages, which is used on both first and resume turns).
    let stream_delta = serde_json::json!({
        "type": "stream_event",
        "event": {
            "type": "content_block_delta",
            "delta": {
                "text": "mock response",
            },
        },
    });
    let result = serde_json::json!({
        "type": "result",
        "subtype": "success",
        "total_cost_usd": 0.0,
        "duration_ms": 0,
        "usage": {
            "input_tokens": 0,
            "output_tokens": 0,
            "cache_creation_input_tokens": 0,
            "cache_read_input_tokens": 0,
        },
    });
    Recipe {
        stream: vec![init, stream_delta, result],
        ..Default::default()
    }
}

fn emit_stream(recipe: &Recipe) {
    for event in &recipe.stream {
        println!("{}", serde_json::to_string(event).unwrap_or_default());
    }
}

fn record_argv(ws_dir: &Path) {
    let args: Vec<String> = env::args().collect();
    let path = ws_dir.join("mock-claude-argv.jsonl");
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    use std::io::Write;
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(f, "{}", serde_json::to_string(&args).unwrap_or_default());
    }
}

fn apply_mutation(ws_dir: &Path, m: &Mutation) -> anyhow::Result<()> {
    let abs = ws_dir.join(&m.file);
    match &m.op {
        MutationOp::AppendLine { value } => {
            if let Some(parent) = abs.parent() {
                fs::create_dir_all(parent)?;
            }
            use std::io::Write;
            let mut f = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&abs)?;
            writeln!(f, "{value}")?;
        }
    }
    Ok(())
}
