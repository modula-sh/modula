//! Claude provider — `claude` CLI.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use modula_db::conversations::SessionAnchor;
use serde_json::Value as JsonValue;

use super::{content_text, program, ChatEvent, ProviderRuntime, TermSession};

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

/// User messages arrive on stdin; `--replay-user-messages` acknowledges each as
/// it is taken in. Partial messages carry the text, so the envelope is tool_use only.
fn chat_command() -> Command {
    let mut cmd = base_command();
    cmd.arg("--include-partial-messages")
        .arg("--input-format")
        .arg("stream-json")
        .arg("--replay-user-messages")
        .arg("-p");
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
    fn build_command_chat_first(&self, _prompt: &str, preset_session_id: &str) -> Option<Command> {
        let mut cmd = chat_command();
        if let Some(m) = &self.model {
            cmd.arg("--model").arg(m);
        }
        cmd.arg("--session-id").arg(preset_session_id);
        Some(cmd)
    }

    fn build_command_chat_resume(&self, _prompt: &str, session_id: &str) -> Command {
        let mut cmd = chat_command();
        cmd.arg("--resume").arg(session_id);
        if let Some(m) = &self.model {
            cmd.arg("--model").arg(m);
        }
        cmd
    }

    /// `--fork-session` copies the session under a new id; `--resume-session-at`
    /// keeps only its entries through the anchor.
    fn build_command_chat_fork(&self, _prompt: &str, from: &SessionAnchor) -> Option<Command> {
        let mut cmd = chat_command();
        cmd.arg("--resume")
            .arg(&from.session)
            .arg("--fork-session")
            .arg("--resume-session-at")
            .arg(&from.anchor);
        if let Some(m) = &self.model {
            cmd.arg("--model").arg(m);
        }
        Some(cmd)
    }

    fn terminal_launch(&self, prompt: Option<&str>, session: TermSession) -> Option<Command> {
        let mut cmd = Command::new(program("claude"));
        cmd.arg("--permission-mode").arg("bypassPermissions");
        if let Some(m) = &self.model {
            cmd.arg("--model").arg(m);
        }
        match session {
            TermSession::New(id) => cmd.arg("--session-id").arg(id),
            TermSession::Resume(id) => cmd.arg("--resume").arg(id),
        };
        if let Some(p) = prompt {
            cmd.arg("--").arg(p);
        }
        Some(cmd)
    }

    fn chat_open(
        &self,
        _session_id: Option<&str>,
        _fork: Option<&SessionAnchor>,
    ) -> Option<Vec<String>> {
        Some(Vec::new())
    }

    fn chat_input(&self, text: &str, _session_id: Option<&str>) -> Option<String> {
        let line = serde_json::json!({
            "type": "user",
            "message": { "role": "user", "content": text },
        });
        Some(format!("{line}\n"))
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
        // On everything a sub-agent produced: the id of the call that spawned it.
        let parent = v["parent_tool_use_id"].as_str();
        let blocks = || v["message"]["content"].as_array().into_iter().flatten();
        // Each message of the main agent is an entry in the session's chain,
        // and `uuid` is the id `--resume-session-at` cuts at.
        let anchor = || {
            v["uuid"]
                .as_str()
                .filter(|_| parent.is_none())
                .map(|id| ChatEvent::Anchor { id: id.to_string() })
        };
        match v["type"].as_str().unwrap_or("") {
            "system" if v["subtype"].as_str() == Some("init") => {
                let mut events = Vec::new();
                if let Some(id) = v["session_id"].as_str() {
                    events.push(ChatEvent::Session { id: id.to_string() });
                }
                if let Some(name) = v["model"].as_str() {
                    events.push(ChatEvent::Model {
                        name: name.to_string(),
                    });
                }
                events
            }
            "user" if v["isReplay"].as_bool() == Some(true) => vec![ChatEvent::InputAccepted],
            "user" => blocks()
                .filter(|block| block["type"].as_str() == Some("tool_result"))
                .map(|block| ChatEvent::ToolResult {
                    id: block["tool_use_id"].as_str().unwrap_or("").to_string(),
                    output: content_text(&block["content"]),
                    is_error: block["is_error"].as_bool().unwrap_or(false),
                })
                .chain(anchor())
                .collect(),
            // A sub-agent's own text is its working-out; its report comes back
            // as the result of the call that spawned it.
            "stream_event" if parent.is_none() => stream_event(&v["event"]),
            // Chat turns use --include-partial-messages, so text and thinking
            // arrive via stream_event; the envelope is only read for tool_use.
            "assistant" => blocks()
                .filter(|block| block["type"].as_str() == Some("tool_use"))
                .map(|block| ChatEvent::ToolUse {
                    id: block["id"].as_str().unwrap_or("").to_string(),
                    name: block["name"].as_str().unwrap_or("").to_string(),
                    input: block["input"].clone(),
                    parent_id: parent.map(str::to_string),
                })
                .chain(anchor())
                .collect(),
            "result" => {
                let mut events = Vec::new();
                let usage = &v["usage"];
                if usage.is_object() {
                    let n = |k: &str| usage[k].as_u64().unwrap_or(0);
                    events.push(ChatEvent::Usage {
                        input_tokens: n("input_tokens")
                            + n("cache_creation_input_tokens")
                            + n("cache_read_input_tokens"),
                        output_tokens: n("output_tokens"),
                        cost_usd: None,
                    });
                }
                // `total_cost_usd` is the whole session's running total, resumed
                // turns included, so it says nothing about this turn.
                if let Some(ms) = v["duration_ms"].as_u64() {
                    events.push(ChatEvent::Duration { ms });
                }
                let is_error = v["is_error"].as_bool().unwrap_or(false);
                let subtype = v["subtype"].as_str().unwrap_or("");
                if !is_error && matches!(subtype, "success" | "final") {
                    events.push(ChatEvent::Done);
                    return events;
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
                events.push(ChatEvent::Error { message: msg });
                events
            }
            _ => vec![],
        }
    }
}

/// One partial-message event of the main agent.
fn stream_event(ev: &JsonValue) -> Vec<ChatEvent> {
    match ev["type"].as_str() {
        Some("content_block_start")
            if matches!(
                ev["content_block"]["type"].as_str(),
                Some("text" | "thinking")
            ) =>
        {
            vec![ChatEvent::Break]
        }
        Some("content_block_delta") => {
            let delta = &ev["delta"];
            if let Some(text) = delta["text"].as_str() {
                vec![ChatEvent::Delta {
                    text: text.to_string(),
                }]
            } else {
                // Empty when the CLI streams only a token count for the block.
                match delta["thinking"].as_str() {
                    Some(text) if !text.is_empty() => vec![ChatEvent::Thinking {
                        text: text.to_string(),
                    }],
                    _ => vec![],
                }
            }
        }
        _ => vec![],
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
        assert!(argv.contains(&std::ffi::OsStr::new("stream-json")));
        assert!(!argv.contains(&std::ffi::OsStr::new("hello")));
    }

    #[test]
    fn claude_chat_forks_a_session_at_an_entry() {
        let from = SessionAnchor {
            session: "sess-1".into(),
            anchor: "entry-9".into(),
        };
        let cmd = claude_rt().build_command_chat_fork("hello", &from).unwrap();
        let argv: Vec<&str> = args(&cmd).iter().filter_map(|a| a.to_str()).collect();
        let at = argv.iter().position(|a| *a == "--resume").unwrap();
        assert_eq!(
            argv[at..at + 5],
            [
                "--resume",
                "sess-1",
                "--fork-session",
                "--resume-session-at",
                "entry-9"
            ]
        );
        // The fork takes its own id, so none is preset.
        assert!(!argv.contains(&"--session-id"));
    }

    /// Every chain entry of the main agent is somewhere a fork can cut; a
    /// sub-agent's entries are not in that chain.
    #[test]
    fn claude_parse_anchors() {
        let rt = claude_rt();
        let text = r#"{"type":"assistant","uuid":"u1","parent_tool_use_id":null,"message":{"content":[{"type":"text","text":"hi"}]}}"#;
        assert!(matches!(
            &rt.parse_stream_line(text)[..],
            [ChatEvent::Anchor { id }] if id == "u1"
        ));
        let result = r#"{"type":"user","uuid":"u2","parent_tool_use_id":null,"message":{"content":[{"type":"tool_result","tool_use_id":"t1","content":"ok"}]}}"#;
        assert!(matches!(
            &rt.parse_stream_line(result)[..],
            [ChatEvent::ToolResult { .. }, ChatEvent::Anchor { id }] if id == "u2"
        ));
        let sub = r#"{"type":"assistant","uuid":"u3","parent_tool_use_id":"t1","message":{"content":[{"type":"text","text":"hi"}]}}"#;
        assert!(rt.parse_stream_line(sub).is_empty());
    }

    #[test]
    fn claude_terminal_launch_new_with_prompt() {
        let rt = ClaudeRuntime {
            config_dir: "/tmp".into(),
            model: Some("opus".to_string()),
        };
        let cmd = rt
            .terminal_launch(Some("-x first"), TermSession::New("uuid-1"))
            .unwrap();
        assert_eq!(
            args(&cmd),
            &[
                "--permission-mode",
                "bypassPermissions",
                "--model",
                "opus",
                "--session-id",
                "uuid-1",
                "--",
                "-x first",
            ]
        );
    }

    #[test]
    fn claude_terminal_launch_resume_without_prompt() {
        let cmd = claude_rt()
            .terminal_launch(None, TermSession::Resume("sess-abc"))
            .unwrap();
        assert_eq!(
            args(&cmd),
            &[
                "--permission-mode",
                "bypassPermissions",
                "--resume",
                "sess-abc"
            ]
        );
    }

    #[test]
    fn claude_chat_input_is_one_user_line() {
        let line = claude_rt().chat_input("hi \"there\"", None).unwrap();
        assert!(line.ends_with('\n'));
        let v: JsonValue = serde_json::from_str(line.trim_end()).unwrap();
        assert_eq!(v["type"], "user");
        assert_eq!(v["message"]["content"], "hi \"there\"");
    }

    #[test]
    fn claude_parse_user_lines() {
        let replay = r#"{"type":"user","message":{"role":"user","content":"hi"},"isReplay":true}"#;
        assert!(matches!(
            claude_rt().parse_stream_line(replay)[..],
            [ChatEvent::InputAccepted]
        ));
        let tool_result = r#"{"type":"user","message":{"role":"user","content":[
            {"type":"tool_result","tool_use_id":"t1","content":[{"type":"text","text":"a"},{"type":"image"}],"is_error":true}
        ]}}"#;
        assert!(matches!(
            &claude_rt().parse_stream_line(tool_result)[..],
            [ChatEvent::ToolResult { id, output, is_error: true }] if id == "t1" && output == "a\n[image]"
        ));
    }

    fn fixture(text: &str) -> Vec<ChatEvent> {
        text.lines()
            .flat_map(|line| claude_rt().parse_stream_line(line))
            .collect()
    }

    /// A real chat turn: two reads, a write, a shell command and a failing one.
    #[test]
    fn claude_parse_recorded_turn() {
        let events = fixture(include_str!("fixtures/claude.jsonl"));
        let tools: Vec<&str> = events
            .iter()
            .filter_map(|e| match e {
                ChatEvent::ToolUse {
                    name,
                    parent_id: None,
                    ..
                } => Some(name.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(tools, ["Read", "Read", "Write", "Bash", "Bash"]);

        let results: Vec<(&str, bool)> = events
            .iter()
            .filter_map(|e| match e {
                ChatEvent::ToolResult {
                    output, is_error, ..
                } => Some((output.as_str(), *is_error)),
                _ => None,
            })
            .collect();
        assert_eq!(results.len(), 5);
        assert_eq!(results[0], ("1\talpha\n2\t", false));
        assert!(results[4].1 && results[4].0.contains("No such file or directory"));

        let text: String = events
            .iter()
            .filter_map(|e| match e {
                ChatEvent::Delta { text } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert!(text.starts_with("I read `a.txt`") && text.ends_with("directory\"."));

        assert!(events.iter().any(|e| matches!(
            e,
            ChatEvent::Usage {
                input_tokens: 106_859,
                output_tokens: 538,
                cost_usd: None
            }
        )));
        assert!(events
            .iter()
            .any(|e| matches!(e, ChatEvent::Duration { ms } if *ms > 0)));
        assert!(events
            .iter()
            .any(|e| matches!(e, ChatEvent::Model { name } if name == "claude-opus-5-5")));
        assert!(matches!(events.last(), Some(ChatEvent::Done)));
    }

    /// A real turn that delegates: the sub-agent's calls name the `Agent` call
    /// as parent, and its text never reaches the reply.
    #[test]
    fn claude_parse_recorded_subagent_turn() {
        let events = fixture(include_str!("fixtures/claude_subagent.jsonl"));
        let calls: Vec<(&str, &str, Option<&str>)> = events
            .iter()
            .filter_map(|e| match e {
                ChatEvent::ToolUse {
                    id,
                    name,
                    parent_id,
                    ..
                } => Some((id.as_str(), name.as_str(), parent_id.as_deref())),
                _ => None,
            })
            .collect();
        let agent = calls[0].0;
        assert_eq!(calls[0].1, "Agent");
        assert_eq!(calls[0].2, None);
        assert_eq!(calls.len(), 3);
        for (_, name, parent) in &calls[1..] {
            assert_eq!((*name, *parent), ("Read", Some(agent)));
        }
        // The CLI streamed the thinking block as a token count, with no text.
        assert!(!events
            .iter()
            .any(|e| matches!(e, ChatEvent::Thinking { .. })));
        let text: String = events
            .iter()
            .filter_map(|e| match e {
                ChatEvent::Delta { text } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert!(text.starts_with("The subagent reported"));
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
        let line = r#"{"type":"system","subtype":"init","session_id":"s","model":"opus"}"#;
        assert!(matches!(
            &claude_rt().parse_stream_line(line)[..],
            [ChatEvent::Session { .. }, ChatEvent::Model { name }] if name == "opus"
        ));
    }

    #[test]
    fn claude_parse_assistant_text_and_tool_use() {
        // With --include-partial-messages on all turns, text arrives via
        // stream_event/content_block_delta. The assistant envelope is only
        // consulted for tool_use blocks — text blocks are skipped to avoid
        // double-emission on first turns.
        let line = r#"{"type":"assistant","message":{"content":[
            {"type":"text","text":"Let me check that."},
            {"type":"tool_use","id":"toolu_1","name":"Bash","input":{"command":"ls -la"}}
        ]}}"#;
        let events = claude_rt().parse_stream_line(line);
        assert_eq!(
            events.len(),
            1,
            "expected 1 event (ToolUse only), got {}",
            events.len()
        );
        match &events[0] {
            ChatEvent::ToolUse {
                id,
                name,
                input,
                parent_id,
            } => {
                assert_eq!((id.as_str(), name.as_str()), ("toolu_1", "Bash"));
                assert_eq!(input["command"], "ls -la");
                assert!(parent_id.is_none());
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
        assert!(matches!(events[..], [ChatEvent::Done]));
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
