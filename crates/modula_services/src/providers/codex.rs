//! Codex provider — `codex` CLI.

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Command;

use modula_db::conversations::SessionAnchor;
use serde_json::{json, Value as JsonValue};

use super::{content_text, program, ChatEvent, ProviderRuntime};

pub struct CodexRuntime {
    pub config_dir: PathBuf,
    pub model: Option<String>,
}

impl ProviderRuntime for CodexRuntime {
    fn build_command(&self, prompt: &str, session_id: Option<&str>) -> Command {
        let mut cmd = Command::new(program("codex"));
        if let Some(sid) = session_id {
            cmd.arg("exec").arg("resume").arg(sid);
        } else {
            cmd.arg("exec");
        }
        cmd.arg("--json")
            .arg("--dangerously-bypass-approvals-and-sandbox")
            .arg("--skip-git-repo-check");
        if let Some(m) = &self.model {
            cmd.arg("--model").arg(m);
        }
        cmd.arg(prompt);
        cmd
    }

    /// `app-server`, not `exec`: the only Codex surface a message can join a running turn through.
    fn build_command_chat(&self, _prompt: &str) -> Command {
        app_server()
    }

    fn build_command_chat_resume(&self, _prompt: &str, _session_id: &str) -> Command {
        app_server()
    }

    fn build_command_chat_fork(&self, _prompt: &str, _from: &SessionAnchor) -> Option<Command> {
        Some(app_server())
    }

    /// Requests on one thread run in order, so the turn needn't wait for the resume reply.
    fn chat_open(
        &self,
        session_id: Option<&str>,
        fork: Option<&SessionAnchor>,
    ) -> Option<Vec<String>> {
        let mut thread = json!({
            "approvalPolicy": "never",
            "sandbox": "danger-full-access",
        });
        if let Some(m) = &self.model {
            thread["model"] = json!(m);
        }
        let open = match (session_id, fork) {
            (Some(id), _) => {
                thread["threadId"] = json!(id);
                rpc("thread/resume", thread)
            }
            // The fork's thread id comes back in the reply to this request.
            (None, Some(from)) => {
                thread["threadId"] = json!(from.session);
                thread["lastTurnId"] = json!(from.anchor);
                thread["excludeTurns"] = json!(true);
                rpc("thread/fork", thread)
            }
            (None, None) => rpc("thread/start", thread),
        };
        Some(vec![
            rpc(
                "initialize",
                json!({ "clientInfo": { "name": "modula", "title": "Modula", "version": env!("CARGO_PKG_VERSION") } }),
            ),
            format!("{}\n", json!({ "method": "initialized" })),
            open,
        ])
    }

    /// `turn/start` over `turn/steer`: it joins a running turn or starts the next one.
    fn chat_input(&self, text: &str, session_id: Option<&str>) -> Option<String> {
        Some(rpc(
            "turn/start",
            json!({
                "threadId": session_id?,
                "input": [{ "type": "text", "text": text }],
            }),
        ))
    }

    fn env_vars(&self) -> Vec<(&'static str, OsString)> {
        let mut vars: Vec<(&'static str, OsString)> =
            vec![("CODEX_HOME", self.config_dir.as_os_str().to_owned())];
        if let Some(m) = &self.model {
            vars.push(("MODULA_PROVIDER_MODEL", OsString::from(m)));
        }
        vars
    }

    fn mcp_summary(&self) -> JsonValue {
        let config_file = self.config_dir.join("config.toml");
        let config_exists = config_file.is_file();
        let raw = std::fs::read_to_string(&config_file).unwrap_or_default();
        let data: toml::Value =
            toml::from_str(&raw).unwrap_or(toml::Value::Table(Default::default()));

        let mut projects: Vec<JsonValue> = Vec::new();
        if let Some(mcp) = data.get("mcp_servers").and_then(|v| v.as_table()) {
            let mut server_names: Vec<&String> = mcp.keys().collect();
            server_names.sort();
            let mut servers: Vec<JsonValue> = Vec::new();
            for name in server_names {
                let Some(cfg) = mcp.get(name).and_then(|v| v.as_table()) else {
                    continue;
                };
                servers.push(serde_json::json!({
                    "name": name,
                    "type": cfg.get("type").and_then(|v| v.as_str()),
                    "url":  cfg.get("url").and_then(|v| v.as_str()),
                    "command": cfg.get("command").and_then(|v| v.as_str()),
                    "needs_auth": false,
                }));
            }
            if !servers.is_empty() {
                let count = servers.len();
                projects.push(serde_json::json!({
                    "path": "codex (global)",
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

    fn parse_line(&self, v: &JsonValue) -> Vec<ChatEvent> {
        if v.get("method").is_some() || v.get("id").is_some() {
            return parse_app_server(v);
        }
        // `codex exec --json`, which agents and one-shot generation run.
        let item = &v["item"];
        match v["type"].as_str().unwrap_or("") {
            "thread.started" => match v["thread_id"].as_str() {
                Some(id) => vec![ChatEvent::Session { id: id.to_string() }],
                None => vec![],
            },
            "item.started" if item["type"].as_str() == Some("command_execution") => {
                vec![ChatEvent::ToolUse {
                    id: item["id"].as_str().unwrap_or("").to_string(),
                    name: "command_execution".to_string(),
                    input: json!({ "command": item["command"] }),
                    parent_id: None,
                }]
            }
            "item.completed" => match item["type"].as_str() {
                Some("command_execution") => vec![ChatEvent::ToolResult {
                    id: item["id"].as_str().unwrap_or("").to_string(),
                    output: item["aggregated_output"].as_str().unwrap_or("").to_string(),
                    is_error: item["status"].as_str() == Some("failed"),
                }],
                Some("agent_message") => match item["text"].as_str() {
                    Some(text) => vec![
                        ChatEvent::Break,
                        ChatEvent::Delta {
                            text: text.to_string(),
                        },
                    ],
                    None => vec![],
                },
                _ => vec![],
            },
            "turn.completed" => vec![ChatEvent::Done],
            _ => vec![],
        }
    }
}

fn app_server() -> Command {
    let mut cmd = Command::new(program("codex"));
    cmd.arg("app-server");
    cmd
}

/// One JSON-RPC request line; only error replies are read.
fn rpc(method: &str, params: JsonValue) -> String {
    let id = uuid::Uuid::new_v4().to_string();
    format!(
        "{}\n",
        json!({ "method": method, "id": id, "params": params })
    )
}

/// A thread item that is a tool call, as `(name, input)`: the item's own type
/// and fields, an MCP or dynamic tool under the name the model called it by.
fn tool_item(item: &JsonValue) -> Option<(String, JsonValue)> {
    let pick = |keys: &[&str]| {
        JsonValue::Object(
            keys.iter()
                .filter(|k| !item[**k].is_null())
                .map(|k| (k.to_string(), item[*k].clone()))
                .collect(),
        )
    };
    let kind = item["type"].as_str()?;
    Some(match kind {
        "commandExecution" => (kind.to_string(), pick(&["command", "cwd"])),
        "fileChange" => (kind.to_string(), pick(&["changes"])),
        "webSearch" => (kind.to_string(), pick(&["query", "action"])),
        "imageView" => (kind.to_string(), pick(&["path"])),
        "mcpToolCall" => (
            format!(
                "{}.{}",
                item["server"].as_str().unwrap_or(""),
                item["tool"].as_str().unwrap_or("")
            ),
            item["arguments"].clone(),
        ),
        "dynamicToolCall" => {
            let tool = item["tool"].as_str().unwrap_or("");
            let name = match item["namespace"].as_str() {
                Some(ns) => format!("{ns}.{tool}"),
                None => tool.to_string(),
            };
            (name, item["arguments"].clone())
        }
        "collabAgentToolCall" => (
            item["tool"].as_str().unwrap_or(kind).to_string(),
            pick(&["prompt", "model", "receiverThreadIds"]),
        ),
        _ => return None,
    })
}

/// What a finished tool item returned, and whether it failed.
fn tool_item_result(item: &JsonValue) -> (String, bool) {
    let failed = matches!(item["status"].as_str(), Some("failed" | "declined"));
    let output = match item["type"].as_str() {
        Some("commandExecution") => item["aggregatedOutput"].as_str().unwrap_or("").to_string(),
        Some("mcpToolCall") => match item["error"]["message"].as_str() {
            Some(message) => message.to_string(),
            None => content_text(&item["result"]["content"]),
        },
        Some("dynamicToolCall") => item["contentItems"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|c| c["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    };
    (output, failed || item["success"].as_bool() == Some(false))
}

fn parse_app_server(v: &JsonValue) -> Vec<ChatEvent> {
    if let Some(message) = v["error"]["message"].as_str() {
        return vec![ChatEvent::Error {
            message: message.to_string(),
        }];
    }
    // The reply to a thread request names the thread; a fork's is new.
    if let Some(id) = v["result"]["thread"]["id"].as_str() {
        return vec![ChatEvent::Session { id: id.to_string() }];
    }
    let p = &v["params"];
    let item = &p["item"];
    let id = || item["id"].as_str().unwrap_or("").to_string();
    let delta = |text: &JsonValue| text.as_str().unwrap_or("").to_string();
    match v["method"].as_str().unwrap_or("") {
        "thread/started" => {
            let mut events = Vec::new();
            if let Some(id) = p["thread"]["id"].as_str() {
                events.push(ChatEvent::Session { id: id.to_string() });
            }
            if let Some(name) = p["thread"]["model"].as_str() {
                events.push(ChatEvent::Model {
                    name: name.to_string(),
                });
            }
            events
        }
        // A fork made with this as its last turn keeps the thread through it.
        "turn/started" => match p["turn"]["id"].as_str() {
            Some(id) => vec![ChatEvent::Anchor { id: id.to_string() }],
            None => vec![],
        },
        "item/agentMessage/delta" => vec![ChatEvent::Delta {
            text: delta(&p["delta"]),
        }],
        "item/reasoning/summaryTextDelta" | "item/reasoning/textDelta" => {
            vec![ChatEvent::Thinking {
                text: delta(&p["delta"]),
            }]
        }
        "item/reasoning/summaryPartAdded" => vec![ChatEvent::Break],
        "item/started" => match item["type"].as_str() {
            Some("agentMessage" | "reasoning") => vec![ChatEvent::Break],
            _ => match tool_item(item) {
                Some((name, input)) => vec![ChatEvent::ToolUse {
                    id: id(),
                    name,
                    input,
                    parent_id: None,
                }],
                None => vec![],
            },
        },
        "item/completed" => match item["type"].as_str() {
            Some("userMessage") => vec![ChatEvent::InputAccepted],
            _ => match tool_item(item) {
                // The finished item carries the call's final input (a patch's
                // diff, say), so it is restated ahead of the result.
                Some((name, input)) => {
                    let (output, is_error) = tool_item_result(item);
                    vec![
                        ChatEvent::ToolUse {
                            id: id(),
                            name,
                            input,
                            parent_id: None,
                        },
                        ChatEvent::ToolResult {
                            id: id(),
                            output,
                            is_error,
                        },
                    ]
                }
                None => vec![],
            },
        },
        // `last` is the model call just made; a turn is the sum of its calls.
        "thread/tokenUsage/updated" => {
            let last = &p["tokenUsage"]["last"];
            vec![ChatEvent::Usage {
                input_tokens: last["inputTokens"].as_u64().unwrap_or(0),
                output_tokens: last["outputTokens"].as_u64().unwrap_or(0),
                cost_usd: None,
            }]
        }
        "turn/completed" if p["turn"]["status"].as_str() == Some("failed") => {
            vec![ChatEvent::Error {
                message: p["turn"]["error"]["message"]
                    .as_str()
                    .unwrap_or("turn failed")
                    .to_string(),
            }]
        }
        "turn/completed" => {
            let mut events = Vec::new();
            if let Some(ms) = p["turn"]["durationMs"].as_u64() {
                events.push(ChatEvent::Duration { ms });
            }
            events.push(ChatEvent::Done);
            events
        }
        "error" if p["willRetry"].as_bool() != Some(true) => vec![ChatEvent::Error {
            message: p["error"]["message"]
                .as_str()
                .unwrap_or("provider error")
                .to_string(),
        }],
        _ => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(cmd: &std::process::Command) -> Vec<&std::ffi::OsStr> {
        cmd.get_args().collect()
    }

    fn codex_rt() -> CodexRuntime {
        CodexRuntime {
            config_dir: "/tmp".into(),
            model: None,
        }
    }

    #[test]
    fn codex_build_command_argv() {
        let cmd = codex_rt().build_command("a very long prompt", None);
        assert_eq!(
            args(&cmd),
            &[
                "exec",
                "--json",
                "--dangerously-bypass-approvals-and-sandbox",
                "--skip-git-repo-check",
                "a very long prompt",
            ]
        );
    }

    #[test]
    fn codex_build_command_resume() {
        let cmd = codex_rt().build_command("hello", Some("thread_abc"));
        let argv = args(&cmd);
        // resume mode: "exec resume <id>" before flags
        assert_eq!(argv[0], std::ffi::OsStr::new("exec"));
        assert_eq!(argv[1], std::ffi::OsStr::new("resume"));
        assert_eq!(argv[2], std::ffi::OsStr::new("thread_abc"));
        assert_eq!(argv.last(), Some(&std::ffi::OsStr::new("hello")));
    }

    #[test]
    fn codex_build_command_with_model() {
        let rt = CodexRuntime {
            config_dir: "/tmp".into(),
            model: Some("o4-mini".to_string()),
        };
        let cmd = rt.build_command("hello", None);
        let argv = args(&cmd);
        assert!(argv.contains(&std::ffi::OsStr::new("--model")));
        assert!(argv.contains(&std::ffi::OsStr::new("o4-mini")));
        assert_eq!(argv.last(), Some(&std::ffi::OsStr::new("hello")));
    }

    #[test]
    fn codex_mcp_summary_missing_dir() {
        let rt = CodexRuntime {
            config_dir: "/nonexistent/path/xyz".into(),
            model: None,
        };
        let s = rt.mcp_summary();
        assert_eq!(s["config_exists"], false);
        assert_eq!(s["projects"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn codex_mcp_summary_parses_mcp_servers() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("config.toml"),
            r#"
[mcp_servers.my-server]
command = "npx"
type = "local"
"#,
        )
        .unwrap();
        let rt = CodexRuntime {
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
        assert_eq!(servers[0]["command"], "npx");
        assert_eq!(servers[0]["type"], "local");
    }

    #[test]
    fn codex_parse_session_event() {
        let line = r#"{"type":"thread.started","thread_id":"thread-abc"}"#;
        let events = codex_rt().parse_stream_line(line);
        assert_eq!(events.len(), 1);
        match &events[0] {
            ChatEvent::Session { id } => assert_eq!(id, "thread-abc"),
            _ => panic!("expected Session"),
        }
    }

    #[test]
    fn codex_parse_item_completed() {
        let line = r#"{"type":"item.completed","item":{"type":"agent_message","text":"hi there"}}"#;
        assert!(matches!(
            &codex_rt().parse_stream_line(line)[..],
            [ChatEvent::Break, ChatEvent::Delta { text }] if text == "hi there"
        ));
    }

    #[test]
    fn codex_parse_turn_completed() {
        let line = r#"{"type":"turn.completed"}"#;
        let events = codex_rt().parse_stream_line(line);
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], ChatEvent::Done));
    }

    #[test]
    fn codex_parse_command_execution_string() {
        // command as a plain string — the verified shape from live `codex --json`.
        let line = r#"{"type":"item.started","item":{"id":"i1","type":"command_execution","command":"ls -la"}}"#;
        assert!(matches!(
            &codex_rt().parse_stream_line(line)[..],
            [ChatEvent::ToolUse { id, name, input, .. }]
                if id == "i1" && name == "command_execution" && input["command"] == "ls -la"
        ));
        let done = r#"{"type":"item.completed","item":{"id":"i1","type":"command_execution","aggregated_output":"a\n","status":"failed"}}"#;
        assert!(matches!(
            &codex_rt().parse_stream_line(done)[..],
            [ChatEvent::ToolResult { id, output, is_error: true }] if id == "i1" && output == "a\n"
        ));
    }

    #[test]
    fn codex_chat_opens_app_server_and_resumes_thread() {
        let rt = CodexRuntime {
            config_dir: "/tmp".into(),
            model: Some("gpt-5".to_string()),
        };
        assert_eq!(
            args(&rt.build_command_chat_resume("hi", "t1")),
            &["app-server"]
        );
        let open = rt.chat_open(Some("t1"), None).unwrap();
        let resume: JsonValue = serde_json::from_str(open[2].trim_end()).unwrap();
        assert_eq!(resume["method"], "thread/resume");
        assert_eq!(resume["params"]["threadId"], "t1");
        assert_eq!(resume["params"]["model"], "gpt-5");
        assert_eq!(resume["params"]["sandbox"], "danger-full-access");
    }

    #[test]
    fn codex_chat_forks_a_thread_through_a_turn() {
        let rt = codex_rt();
        let from = SessionAnchor {
            session: "t1".into(),
            anchor: "turn-a".into(),
        };
        assert_eq!(
            args(&rt.build_command_chat_fork("hi", &from).unwrap()),
            &["app-server"]
        );
        let open = rt.chat_open(None, Some(&from)).unwrap();
        let fork: JsonValue = serde_json::from_str(open[2].trim_end()).unwrap();
        assert_eq!(fork["method"], "thread/fork");
        assert_eq!(fork["params"]["threadId"], "t1");
        assert_eq!(fork["params"]["lastTurnId"], "turn-a");

        // The reply carries the fork's own thread id; each turn reports its id.
        assert!(matches!(
            &rt.parse_stream_line(r#"{"id":"x","result":{"thread":{"id":"t2"},"model":"m"}}"#)[..],
            [ChatEvent::Session { id }] if id == "t2"
        ));
        assert!(matches!(
            &rt.parse_stream_line(r#"{"method":"turn/started","params":{"turn":{"id":"turn-b"}}}"#)[..],
            [ChatEvent::Anchor { id }] if id == "turn-b"
        ));
    }

    #[test]
    fn codex_chat_input_waits_for_the_thread() {
        assert!(codex_rt().chat_input("hi", None).is_none());
        let line = codex_rt().chat_input("hi", Some("t1")).unwrap();
        let v: JsonValue = serde_json::from_str(line.trim_end()).unwrap();
        assert_eq!(v["method"], "turn/start");
        assert_eq!(v["params"]["threadId"], "t1");
        assert_eq!(v["params"]["input"][0]["text"], "hi");
    }

    #[test]
    fn codex_parse_app_server_turn() {
        let rt = codex_rt();
        let parse = |line: &str| rt.parse_stream_line(line);
        assert!(matches!(
            &parse(r#"{"method":"thread/started","params":{"thread":{"id":"t1"}}}"#)[..],
            [ChatEvent::Session { id }] if id == "t1"
        ));
        assert!(matches!(
            &parse(r#"{"method":"item/reasoning/summaryTextDelta","params":{"delta":"hm"}}"#)[..],
            [ChatEvent::Thinking { text }] if text == "hm"
        ));
        assert!(matches!(
            &parse(r#"{"method":"item/agentMessage/delta","params":{"delta":"hi"}}"#)[..],
            [ChatEvent::Delta { text }] if text == "hi"
        ));
        assert!(matches!(
            &parse(r#"{"method":"item/started","params":{"item":{"type":"commandExecution","id":"e1","command":"ls"}}}"#)[..],
            [ChatEvent::ToolUse { id, name, input, .. }]
                if id == "e1" && name == "commandExecution" && input == &json!({"command": "ls"})
        ));
        assert!(matches!(
            &parse(r#"{"method":"item/completed","params":{"item":{"type":"commandExecution","id":"e1","command":"ls","aggregatedOutput":"a.txt\n","status":"completed"}}}"#)[..],
            [ChatEvent::ToolUse { .. }, ChatEvent::ToolResult { id, output, is_error: false }]
                if id == "e1" && output == "a.txt\n"
        ));
        assert!(matches!(
            parse(r#"{"method":"item/completed","params":{"item":{"type":"userMessage"}}}"#)[..],
            [ChatEvent::InputAccepted]
        ));
        assert!(matches!(
            parse(r#"{"method":"turn/completed","params":{"turn":{"status":"completed"}}}"#)[..],
            [ChatEvent::Done]
        ));
        assert!(matches!(
            &parse(r#"{"id":"x","error":{"code":-32600,"message":"thread not found"}}"#)[..],
            [ChatEvent::Error { message }] if message == "thread not found"
        ));
        assert!(
            parse(r#"{"method":"error","params":{"willRetry":true,"error":{"message":"x"}}}"#)
                .is_empty()
        );
        assert!(parse(r#"{"id":"x","result":{}}"#).is_empty());
    }

    /// The other tool items, shaped as the app-server protocol defines them.
    #[test]
    fn codex_parse_app_server_tool_items() {
        let rt = codex_rt();
        let completed = |item: JsonValue| {
            rt.parse_line(&json!({ "method": "item/completed", "params": { "item": item } }))
        };
        assert!(matches!(
            &completed(json!({
                "type": "mcpToolCall", "id": "m1", "server": "blender", "tool": "get_screenshot",
                "arguments": {"window": 1}, "status": "completed",
                "result": {"content": [{"type": "text", "text": "ok"}, {"type": "image"}]},
            }))[..],
            [ChatEvent::ToolUse { name, input, .. }, ChatEvent::ToolResult { output, is_error: false, .. }]
                if name == "blender.get_screenshot" && input == &json!({"window": 1}) && output == "ok\n[image]"
        ));
        assert!(matches!(
            &completed(json!({
                "type": "mcpToolCall", "id": "m2", "server": "s", "tool": "t", "arguments": {},
                "status": "failed", "error": {"message": "boom"},
            }))[..],
            [_, ChatEvent::ToolResult { output, is_error: true, .. }] if output == "boom"
        ));
        let changes =
            json!([{"path": "a.rs", "kind": {"type": "update", "move_path": null}, "diff": "@@"}]);
        assert!(matches!(
            &completed(json!({"type": "fileChange", "id": "f1", "changes": changes, "status": "completed"}))[..],
            [ChatEvent::ToolUse { name, input, .. }, ChatEvent::ToolResult { output, is_error: false, .. }]
                if name == "fileChange" && input["changes"] == changes && output.is_empty()
        ));
        assert!(matches!(
            &completed(json!({"type": "imageView", "id": "v1", "path": "/tmp/a.png"}))[..],
            [ChatEvent::ToolUse { name, input, .. }, _]
                if name == "imageView" && input == &json!({"path": "/tmp/a.png"})
        ));
        assert!(completed(json!({"type": "agentMessage", "id": "a1", "text": "hi"})).is_empty());
    }

    /// A real app-server turn: commentary, four shell commands (one failing),
    /// then the final answer.
    #[test]
    fn codex_parse_recorded_turn() {
        let rt = codex_rt();
        let events: Vec<ChatEvent> = include_str!("fixtures/codex.jsonl")
            .lines()
            .flat_map(|line| rt.parse_stream_line(line))
            .collect();
        assert!(matches!(
            &events[..2],
            [ChatEvent::Session { .. }, ChatEvent::Model { name }] if name == "gpt-6-astra"
        ));
        let results: Vec<(&str, bool)> = events
            .iter()
            .filter_map(|e| match e {
                ChatEvent::ToolResult {
                    output, is_error, ..
                } => Some((output.as_str(), *is_error)),
                _ => None,
            })
            .collect();
        assert_eq!(results.len(), 4);
        assert_eq!(
            results[3],
            ("cat: does-not-exist.txt: No such file or directory\n", true)
        );
        assert!(events.iter().any(|e| matches!(
            e,
            ChatEvent::ToolUse { name, input, .. }
                if name == "commandExecution" && input["command"] == "/bin/zsh -lc 'cat does-not-exist.txt'"
        )));
        let (input, output) = events.iter().fold((0, 0), |(i, o), e| match e {
            ChatEvent::Usage {
                input_tokens,
                output_tokens,
                ..
            } => (i + input_tokens, o + output_tokens),
            _ => (i, o),
        });
        assert_eq!((input, output), (78_873, 242));
        assert!(matches!(
            &events[events.len() - 2..],
            [ChatEvent::Duration { ms: 21_593 }, ChatEvent::Done]
        ));
    }
}
