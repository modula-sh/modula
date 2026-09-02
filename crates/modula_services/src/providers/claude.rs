//! Claude provider — `claude` CLI.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value as JsonValue;

use super::{program, ChatEvent, ProviderRuntime};

pub struct ClaudeRuntime {
    pub config_dir: PathBuf,
    pub model: Option<String>,
}

fn base_command() -> Command {
    let mut cmd = Command::new(program("claude"));
    cmd.arg("--permission-mode")
        .arg("bypassPermissions")
        .arg("--output-format")
        .arg("stream-json")
        .arg("--verbose");
    cmd
}

impl ProviderRuntime for ClaudeRuntime {
    fn build_command(&self, prompt: &str, session_id: Option<&str>) -> Command {
        let mut cmd = base_command();
        if let Some(m) = &self.model {
            cmd.arg("--model").arg(m);
        }
        if let Some(sid) = session_id {
            cmd.arg("--resume").arg(sid);
        }
        cmd.arg("-p").arg(prompt);
        cmd
    }

    /// Passes `--session-id <preset_id>` so Claude adopts that uuid as its
    /// session identifier (rather than generating its own). The id is then
    /// returned in every stream event and can be used for `--resume` on
    /// subsequent turns.
    fn build_command_chat_first(&self, prompt: &str, preset_session_id: &str) -> Option<Command> {
        let mut cmd = base_command();
        cmd.arg("--include-partial-messages");
        if let Some(m) = &self.model {
            cmd.arg("--model").arg(m);
        }
        cmd.arg("--session-id").arg(preset_session_id);
        cmd.arg("-p").arg(prompt);
        Some(cmd)
    }

    /// Adds `--include-partial-messages` (consistent with
    /// `build_command_chat_first`) so text always arrives via stream_event
    /// partials and the assistant envelope only needs to be consulted for
    /// tool_use blocks.
    fn build_command_chat_resume(&self, prompt: &str, session_id: &str) -> Command {
        let mut cmd = base_command();
        cmd.arg("--include-partial-messages")
            .arg("--resume")
            .arg(session_id);
        if let Some(m) = &self.model {
            cmd.arg("--model").arg(m);
        }
        cmd.arg("-p").arg(prompt);
        cmd
    }

    fn env_vars(&self) -> Vec<(&'static str, OsString)> {
        let mut vars: Vec<(&'static str, OsString)> =
            vec![("CLAUDE_CONFIG_DIR", self.config_dir.as_os_str().to_owned())];
        if let Some(m) = &self.model {
            vars.push(("MODULA_CLAUDE_MODEL", OsString::from(m)));
        }
        vars
    }

    fn mcp_summary(&self) -> JsonValue {
        claude_mcp_summary(&self.config_dir)
    }

    fn parse_line(&self, v: &JsonValue) -> Vec<ChatEvent> {
        let t = match v["type"].as_str() {
            Some(t) => t,
            None => return vec![],
        };
        match t {
            "system" if v["subtype"].as_str() == Some("init") => match v["session_id"].as_str() {
                Some(id) => vec![ChatEvent::Session { id: id.to_string() }],
                None => vec![],
            },
            "stream_event" => {
                let ev = match v.get("event") {
                    Some(e) => e,
                    None => return vec![],
                };
                if ev["type"].as_str() == Some("content_block_delta") {
                    match ev["delta"]["text"].as_str() {
                        Some(text) => vec![ChatEvent::Delta {
                            text: text.to_string(),
                        }],
                        None => vec![],
                    }
                } else {
                    vec![]
                }
            }
            "assistant" => {
                let content = match v["message"]["content"].as_array() {
                    Some(c) => c,
                    None => return vec![],
                };
                // Both first and resume turns use --include-partial-messages, so
                // text always arrives via stream_event/content_block_delta. The
                // assistant envelope is only needed for tool_use blocks.
                content
                    .iter()
                    .filter(|block| block["type"].as_str() == Some("tool_use"))
                    .map(|block| ChatEvent::ToolUse {
                        name: block["name"].as_str().unwrap_or("").to_string(),
                        input: block["input"].clone(),
                    })
                    .collect()
            }
            "result" => {
                let is_error = v["is_error"].as_bool().unwrap_or(false);
                let subtype = v["subtype"].as_str().unwrap_or("");
                let succeeded = !is_error && matches!(subtype, "success" | "final");
                if succeeded {
                    return vec![ChatEvent::Done];
                }
                let msg = v["result"]
                    .as_str()
                    .or_else(|| v["error"].as_str())
                    .or_else(|| {
                        v["errors"]
                            .as_array()
                            .and_then(|a| a.first().and_then(|e| e.as_str()))
                    })
                    .unwrap_or("provider error")
                    .to_string();
                vec![ChatEvent::Error { message: msg }]
            }
            _ => vec![],
        }
    }
}

fn read_json(path: &Path) -> JsonValue {
    if !path.is_file() {
        return JsonValue::Object(Default::default());
    }
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => return JsonValue::Object(Default::default()),
    };
    serde_json::from_str(&text).unwrap_or(JsonValue::Object(Default::default()))
}

/// Build the sorted `mcp_servers` array for one Claude `mcpServers` object,
/// flagging entries the auth cache says still need login.
fn claude_servers(
    servers_raw: &serde_json::Map<String, JsonValue>,
    auth_keys: &[String],
) -> Vec<JsonValue> {
    let mut keys: Vec<&String> = servers_raw.keys().collect();
    keys.sort();
    keys.into_iter()
        .filter_map(|name| {
            let scfg = servers_raw.get(name)?.as_object()?;
            Some(serde_json::json!({
                "name": name,
                "type": scfg.get("type"),
                "url": scfg.get("url"),
                "command": scfg.get("command"),
                "needs_auth": auth_keys.contains(name),
            }))
        })
        .collect()
}

fn claude_mcp_summary(config_dir: &Path) -> JsonValue {
    let data = read_json(&config_dir.join(".claude.json"));
    let auth_cache = read_json(&config_dir.join("mcp-needs-auth-cache.json"));
    let auth_keys: Vec<String> = auth_cache
        .as_object()
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default();

    let mut projects: Vec<JsonValue> = Vec::new();
    // Top-level `mcpServers` — where this feature writes managed servers.
    if let Some(servers_raw) = data.get("mcpServers").and_then(|v| v.as_object()) {
        let servers = claude_servers(servers_raw, &auth_keys);
        if !servers.is_empty() {
            let count = servers.len();
            projects.push(serde_json::json!({
                "path": "(user)",
                "mcp_servers": servers,
                "count": count,
            }));
        }
    }
    if let Some(raw_projects) = data.get("projects").and_then(|v| v.as_object()) {
        let mut sorted: BTreeMap<String, &JsonValue> = BTreeMap::new();
        for (k, v) in raw_projects {
            sorted.insert(k.clone(), v);
        }
        for (proj_path, pcfg) in sorted {
            let Some(servers_raw) = pcfg.get("mcpServers").and_then(|v| v.as_object()) else {
                continue;
            };
            let servers = claude_servers(servers_raw, &auth_keys);
            if !servers.is_empty() {
                let count = servers.len();
                projects.push(serde_json::json!({
                    "path": proj_path,
                    "mcp_servers": servers,
                    "count": count,
                }));
            }
        }
    }
    serde_json::json!({
        "config_exists": config_dir.join(".claude.json").is_file(),
        "projects": projects,
        "needs_auth": auth_cache,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(cmd: &std::process::Command) -> Vec<&std::ffi::OsStr> {
        cmd.get_args().collect()
    }

    fn claude_rt() -> ClaudeRuntime {
        ClaudeRuntime {
            config_dir: "/tmp".into(),
            model: None,
        }
    }

    #[test]
    fn claude_build_command_argv() {
        let cmd = claude_rt().build_command("a very long prompt with spaces and \"quotes\"", None);
        assert_eq!(
            args(&cmd),
            &[
                "--permission-mode",
                "bypassPermissions",
                "--output-format",
                "stream-json",
                "--verbose",
                "-p",
                "a very long prompt with spaces and \"quotes\"",
            ]
        );
    }

    #[test]
    fn claude_build_command_resume() {
        let cmd = claude_rt().build_command("hello", Some("sess-abc"));
        let argv = args(&cmd);
        assert!(argv.contains(&std::ffi::OsStr::new("--resume")));
        assert!(argv.contains(&std::ffi::OsStr::new("sess-abc")));
        assert_eq!(argv.last(), Some(&std::ffi::OsStr::new("hello")));
    }

    #[test]
    fn claude_build_command_with_model() {
        let rt = ClaudeRuntime {
            config_dir: "/tmp".into(),
            model: Some("claude-3-5-sonnet".to_string()),
        };
        let cmd = rt.build_command("hello", None);
        let argv = args(&cmd);
        assert!(argv.contains(&std::ffi::OsStr::new("--model")));
        assert!(argv.contains(&std::ffi::OsStr::new("claude-3-5-sonnet")));
    }

    #[test]
    fn claude_chat_first_presets_session_id() {
        let cmd = claude_rt()
            .build_command_chat_first("hello", "uuid-1")
            .unwrap();
        let argv = args(&cmd);
        assert!(argv.contains(&std::ffi::OsStr::new("--session-id")));
        assert!(argv.contains(&std::ffi::OsStr::new("uuid-1")));
        assert_eq!(argv.last(), Some(&std::ffi::OsStr::new("hello")));
    }

    #[test]
    fn claude_mcp_summary_missing_dir() {
        let rt = ClaudeRuntime {
            config_dir: "/nonexistent/path/xyz".into(),
            model: None,
        };
        let s = rt.mcp_summary();
        assert_eq!(s["config_exists"], false);
        assert_eq!(s["projects"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn claude_parse_session_event() {
        let line = r#"{"type":"system","subtype":"init","session_id":"sess-123"}"#;
        let events = claude_rt().parse_stream_line(line);
        assert_eq!(events.len(), 1);
        match &events[0] {
            ChatEvent::Session { id } => assert_eq!(id, "sess-123"),
            _ => panic!("expected Session"),
        }
    }

    #[test]
    fn claude_parse_assistant_text_and_tool_use() {
        // With --include-partial-messages on all turns, text arrives via
        // stream_event/content_block_delta. The assistant envelope is only
        // consulted for tool_use blocks — text blocks are skipped to avoid
        // double-emission on first turns.
        let line = r#"{"type":"assistant","message":{"content":[
            {"type":"text","text":"Let me check that."},
            {"type":"tool_use","name":"Bash","input":{"command":"ls -la"}}
        ]}}"#;
        let events = claude_rt().parse_stream_line(line);
        assert_eq!(
            events.len(),
            1,
            "expected 1 event (ToolUse only), got {}",
            events.len()
        );
        match &events[0] {
            ChatEvent::ToolUse { name, input } => {
                assert_eq!(name, "Bash");
                assert_eq!(input["command"], "ls -la");
            }
            _ => panic!("expected ToolUse"),
        }
    }

    #[test]
    fn claude_parse_assistant_delta() {
        // Text from the assistant envelope is skipped; text arrives via stream_event.
        let line =
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"hello world"}]}}"#;
        let events = claude_rt().parse_stream_line(line);
        // No Delta emitted — text will arrive via stream_event/content_block_delta.
        assert_eq!(
            events.len(),
            0,
            "expected 0 events from assistant text-only envelope"
        );
    }

    #[test]
    fn claude_parse_result_success() {
        let line = r#"{"type":"result","subtype":"success","total_cost_usd":0}"#;
        let events = claude_rt().parse_stream_line(line);
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], ChatEvent::Done));
    }

    #[test]
    fn claude_parse_result_auth_error_is_error() {
        let line = r#"{"type":"result","subtype":"success","is_error":true,"result":"Not logged in · Please run /login"}"#;
        let events = claude_rt().parse_stream_line(line);
        assert_eq!(events.len(), 1);
        match &events[0] {
            ChatEvent::Error { message } => {
                assert!(message.contains("Not logged in"));
            }
            _ => panic!("expected Error"),
        }
    }

    #[test]
    fn claude_parse_result_error_during_execution_is_error() {
        let line = r#"{"type":"result","subtype":"error_during_execution","is_error":true,"errors":["No conversation found with session ID: abc"]}"#;
        let events = claude_rt().parse_stream_line(line);
        assert_eq!(events.len(), 1);
        match &events[0] {
            ChatEvent::Error { message } => {
                assert!(message.contains("No conversation found"));
            }
            _ => panic!("expected Error"),
        }
    }

    #[test]
    fn claude_parse_unknown_returns_empty() {
        let line = r#"{"type":"system","subtype":"init"}"#;
        // No session_id → empty vec
        assert!(claude_rt().parse_stream_line(line).is_empty());
    }
}
