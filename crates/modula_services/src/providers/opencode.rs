//! OpenCode provider — `opencode` CLI.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value as JsonValue;

use super::{program, BoxFuture, ChatEvent, ProviderModel, ProviderRuntime};

pub struct OpenCodeRuntime {
    pub config_dir: PathBuf,
    pub model: Option<String>,
}

impl ProviderRuntime for OpenCodeRuntime {
    /// Discovers the model list via `opencode models` — opencode's built-in
    /// defaults plus every model from the user's configured providers —
    /// reusing the catalog label when an id is also a static entry. Degrades
    /// to `static_models` when the CLI is missing or fails.
    fn models<'a>(
        &'a self,
        static_models: Vec<ProviderModel>,
    ) -> BoxFuture<'a, Vec<ProviderModel>> {
        Box::pin(async move {
            let Some(ids) = opencode_models().await else {
                return static_models;
            };
            ids.into_iter()
                .map(|id| {
                    let label = static_models
                        .iter()
                        .find(|m| m.id == id)
                        .map(|m| m.label.clone())
                        .unwrap_or_else(|| id.clone());
                    ProviderModel { id, label }
                })
                .collect()
        })
    }

    fn build_command(&self, prompt: &str, session_id: Option<&str>) -> Command {
        let mut cmd = Command::new(program("opencode"));
        cmd.arg("run")
            .arg("--format")
            .arg("json")
            .arg("--dangerously-skip-permissions");
        if let Some(m) = &self.model {
            cmd.arg("--model").arg(m);
        }
        if let Some(sid) = session_id {
            cmd.arg("-s").arg(sid);
        }
        cmd.arg(prompt);
        cmd
    }

    fn env_vars(&self) -> Vec<(&'static str, OsString)> {
        let mut vars: Vec<(&'static str, OsString)> = vec![(
            "OPENCODE_CONFIG_DIR",
            self.config_dir.as_os_str().to_owned(),
        )];
        if let Some(m) = &self.model {
            vars.push(("MODULA_PROVIDER_MODEL", OsString::from(m)));
        }
        vars
    }

    fn mcp_summary(&self) -> JsonValue {
        opencode_mcp_summary(&self.config_dir)
    }

    fn parse_line(&self, v: &JsonValue) -> Vec<ChatEvent> {
        let t = match v["type"].as_str() {
            Some(t) => t,
            None => return vec![],
        };
        match t {
            "text" => {
                let mut events = Vec::new();
                // sessionID rides on every text event; emit once (handler deduplicates via session_captured).
                if let Some(sid) = v["sessionID"].as_str() {
                    events.push(ChatEvent::Session {
                        id: sid.to_string(),
                    });
                }
                if let Some(text) = v["part"]["text"].as_str() {
                    events.push(ChatEvent::Delta {
                        text: text.to_string(),
                    });
                }
                events
            }
            "step_finish" => vec![ChatEvent::Done],
            _ => vec![],
        }
    }
}

/// Run `opencode models` and collect the `provider/model` ids it prints, one
/// per line. `None` when the CLI is missing, fails, times out, or emits
/// nothing parseable.
async fn opencode_models() -> Option<Vec<String>> {
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        tokio::process::Command::new(program("opencode"))
            .arg("models")
            .kill_on_drop(true)
            .output(),
    )
    .await
    .ok()?
    .ok()?;
    if !output.status.success() {
        return None;
    }
    let models = parse_opencode_models(&String::from_utf8_lossy(&output.stdout));
    if models.is_empty() {
        None
    } else {
        Some(models)
    }
}

/// One `provider/model` id per line. Lines with whitespace, without a slash,
/// or that look like JSON are noise (warnings, stream events from a test shim)
/// and are dropped.
fn parse_opencode_models(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .map(str::trim)
        .filter(|l| {
            !l.is_empty()
                && l.contains('/')
                && !l.starts_with('{')
                && !l.contains(char::is_whitespace)
        })
        .map(str::to_string)
        .collect()
}

/// Best-effort JSONC read (comments tolerated) for opencode's `opencode.jsonc`;
/// a missing or unparsable file degrades to an empty object.
fn read_jsonc(path: &Path) -> JsonValue {
    let Ok(text) = std::fs::read_to_string(path) else {
        return JsonValue::Object(Default::default());
    };
    jsonc_parser::parse_to_serde_value(&text, &jsonc_parser::ParseOptions::default())
        .ok()
        .flatten()
        .unwrap_or(JsonValue::Object(Default::default()))
}

fn opencode_mcp_summary(config_dir: &Path) -> JsonValue {
    let json = config_dir.join("opencode.json");
    let config_file = if json.is_file() {
        json
    } else {
        config_dir.join("opencode.jsonc")
    };
    let config_exists = config_file.is_file();
    let data = read_jsonc(&config_file);

    let mut projects: Vec<JsonValue> = Vec::new();
    if let Some(mcp) = data.get("mcp").and_then(|v| v.as_object()) {
        let mut server_keys: Vec<String> = mcp.keys().cloned().collect();
        server_keys.sort();
        let mut servers: Vec<JsonValue> = Vec::new();
        for sname in server_keys {
            let Some(scfg) = mcp.get(&sname).and_then(|v| v.as_object()) else {
                continue;
            };
            servers.push(serde_json::json!({
                "name": sname,
                "type": scfg.get("type"),
                "url": scfg.get("url"),
                "command": scfg.get("command"),
                "needs_auth": false,
            }));
        }
        if !servers.is_empty() {
            let count = servers.len();
            projects.push(serde_json::json!({
                "path": "opencode (global)",
                "mcp_servers": servers,
                "count": count,
            }));
        }
    }
    serde_json::json!({
        "config_exists": config_exists,
        "projects": projects,
        "needs_auth": {},
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(cmd: &std::process::Command) -> Vec<&std::ffi::OsStr> {
        cmd.get_args().collect()
    }

    fn opencode_rt() -> OpenCodeRuntime {
        OpenCodeRuntime {
            config_dir: "/tmp".into(),
            model: None,
        }
    }

    #[test]
    fn opencode_models_parses_ids() {
        let out = "opencode/big-pickle\nopencode/deepseek-v4-flash-free\nollama/gemma4:e2b\n";
        assert_eq!(
            parse_opencode_models(out),
            vec![
                "opencode/big-pickle",
                "opencode/deepseek-v4-flash-free",
                "ollama/gemma4:e2b",
            ]
        );
    }

    #[test]
    fn opencode_models_drops_noise() {
        // Stream-json from the test shim, warnings, bare words, blank lines —
        // none of these are `provider/model` ids.
        let out = concat!(
            "{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"mock-session-id\"}\n",
            "warning: model registry slow/stale\n",
            "plainword\n",
            "\n",
            "  opencode/valid-model  \n",
        );
        assert_eq!(parse_opencode_models(out), vec!["opencode/valid-model"]);
    }

    #[test]
    fn opencode_build_command_argv() {
        let cmd =
            opencode_rt().build_command("a very long prompt with spaces and \"quotes\"", None);
        assert_eq!(
            args(&cmd),
            &[
                "run",
                "--format",
                "json",
                "--dangerously-skip-permissions",
                "a very long prompt with spaces and \"quotes\"",
            ]
        );
    }

    #[test]
    fn opencode_build_command_resume() {
        let cmd = opencode_rt().build_command("hello", Some("ses_abc123"));
        let argv = args(&cmd);
        assert!(argv.contains(&std::ffi::OsStr::new("-s")));
        assert!(argv.contains(&std::ffi::OsStr::new("ses_abc123")));
        assert_eq!(argv.last(), Some(&std::ffi::OsStr::new("hello")));
    }

    #[test]
    fn opencode_build_command_with_model() {
        let rt = OpenCodeRuntime {
            config_dir: "/tmp".into(),
            model: Some("anthropic/claude-3-5-sonnet".to_string()),
        };
        let cmd = rt.build_command("hello", None);
        let argv = args(&cmd);
        assert!(argv.contains(&std::ffi::OsStr::new("--model")));
        assert!(argv.contains(&std::ffi::OsStr::new("anthropic/claude-3-5-sonnet")));
        // prompt must be the last arg
        assert_eq!(argv.last(), Some(&std::ffi::OsStr::new("hello")));
    }

    #[test]
    fn opencode_mcp_summary_missing_dir() {
        let rt = OpenCodeRuntime {
            config_dir: "/nonexistent/path/xyz".into(),
            model: None,
        };
        let s = rt.mcp_summary();
        assert_eq!(s["config_exists"], false);
        assert_eq!(s["projects"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn opencode_mcp_summary_parses_mcp_key() {
        let tmp = tempfile::tempdir().unwrap();
        let config = serde_json::json!({
            "mcp": {
                "my-server": {
                    "type": "local",
                    "command": "npx",
                }
            }
        });
        std::fs::write(
            tmp.path().join("opencode.json"),
            serde_json::to_string(&config).unwrap(),
        )
        .unwrap();
        let rt = OpenCodeRuntime {
            config_dir: tmp.path().into(),
            model: None,
        };
        let s = rt.mcp_summary();
        assert_eq!(s["config_exists"], true);
        let projects = s["projects"].as_array().unwrap();
        assert_eq!(projects.len(), 1);
        let servers = projects[0]["mcp_servers"].as_array().unwrap();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0]["name"], "my-server");
        assert_eq!(servers[0]["type"], "local");
    }

    #[test]
    fn opencode_parse_text_delta_with_session() {
        // text events carry both sessionID and the delta text.
        let line = r#"{"type":"text","sessionID":"ses_abc","part":{"text":"hi"}}"#;
        let events = opencode_rt().parse_stream_line(line);
        assert_eq!(events.len(), 2);
        match &events[0] {
            ChatEvent::Session { id } => assert_eq!(id, "ses_abc"),
            _ => panic!("expected Session first"),
        }
        match &events[1] {
            ChatEvent::Delta { text } => assert_eq!(text, "hi"),
            _ => panic!("expected Delta second"),
        }
    }

    #[test]
    fn opencode_parse_text_delta_no_session() {
        // text event without sessionID (e.g. already captured) → only Delta.
        let line = r#"{"type":"text","part":{"text":"world"}}"#;
        let events = opencode_rt().parse_stream_line(line);
        assert_eq!(events.len(), 1);
        match &events[0] {
            ChatEvent::Delta { text } => assert_eq!(text, "world"),
            _ => panic!("expected Delta"),
        }
    }

    #[test]
    fn opencode_parse_step_finish() {
        let line = r#"{"type":"step_finish"}"#;
        let events = opencode_rt().parse_stream_line(line);
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], ChatEvent::Done));
    }
}
