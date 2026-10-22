//! Gemini provider — `gemini` CLI.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value as JsonValue};

use super::{program, read_jsonc, ChatEvent, ProviderRuntime, TermSession};

/// `config_dir` is the `.gemini` dir itself; the CLI finds it as `$GEMINI_CLI_HOME/.gemini`.
pub struct GeminiRuntime {
    pub config_dir: PathBuf,
    pub model: Option<String>,
}

impl GeminiRuntime {
    fn command(&self, prompt: &str, session_flag: Option<(&str, &str)>) -> Command {
        let mut cmd = Command::new(program("gemini"));
        // `--skip-trust`: an untrusted cwd exits 55 and silently drops `--yolo`.
        cmd.arg("-o")
            .arg("stream-json")
            .arg("--skip-trust")
            .arg("--yolo");
        if let Some(m) = &self.model {
            cmd.arg("-m").arg(m);
        }
        if let Some((flag, id)) = session_flag {
            cmd.arg(flag).arg(id);
        }
        // One token, so a prompt starting with `-` isn't parsed as a flag.
        cmd.arg(format!("--prompt={prompt}"));
        cmd
    }
}

impl ProviderRuntime for GeminiRuntime {
    fn build_command(&self, prompt: &str, session_id: Option<&str>) -> Command {
        self.command(prompt, session_id.map(|id| ("--resume", id)))
    }

    fn build_command_chat_first(&self, prompt: &str, preset_session_id: &str) -> Option<Command> {
        Some(self.command(prompt, Some(("--session-id", preset_session_id))))
    }

    fn terminal_launch(&self, prompt: Option<&str>, session: TermSession) -> Option<Command> {
        let mut cmd = Command::new(program("gemini"));
        cmd.arg("--skip-trust").arg("--yolo");
        if let Some(m) = &self.model {
            cmd.arg("-m").arg(m);
        }
        match session {
            TermSession::New(id) => cmd.arg("--session-id").arg(id),
            TermSession::Resume(id) => cmd.arg("--resume").arg(id),
        };
        if let Some(p) = prompt {
            cmd.arg(format!("--prompt-interactive={p}"));
        }
        Some(cmd)
    }

    fn env_vars(&self) -> Vec<(&'static str, OsString)> {
        let mut vars: Vec<(&'static str, OsString)> = Vec::new();
        if let Some(home) = self.config_dir.parent() {
            vars.push(("GEMINI_CLI_HOME", home.as_os_str().to_owned()));
        }
        if let Some(m) = &self.model {
            vars.push(("MODULA_PROVIDER_MODEL", OsString::from(m)));
        }
        vars
    }

    fn mcp_summary(&self) -> JsonValue {
        gemini_mcp_summary(&self.config_dir)
    }

    fn parse_line(&self, v: &JsonValue) -> Vec<ChatEvent> {
        let text = |v: &JsonValue| v.as_str().unwrap_or("").to_string();
        match v["type"].as_str().unwrap_or("") {
            "init" => {
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
            "message" if v["role"].as_str() == Some("assistant") => match v["content"].as_str() {
                Some(text) => vec![ChatEvent::Delta {
                    text: text.to_string(),
                }],
                None => vec![],
            },
            "tool_use" => vec![ChatEvent::ToolUse {
                id: text(&v["tool_id"]),
                name: text(&v["tool_name"]),
                input: v["parameters"].clone(),
                parent_id: None,
            }],
            "tool_result" => {
                let is_error = v["status"].as_str() == Some("error");
                let output = match v["output"].as_str() {
                    Some(output) => output.to_string(),
                    None => text(&v["error"]["message"]),
                };
                vec![ChatEvent::ToolResult {
                    id: text(&v["tool_id"]),
                    output,
                    is_error,
                }]
            }
            "result" => {
                let mut events = Vec::new();
                let stats = &v["stats"];
                if stats.is_object() {
                    events.push(ChatEvent::Usage {
                        input_tokens: stats["input_tokens"].as_u64().unwrap_or(0),
                        output_tokens: stats["output_tokens"].as_u64().unwrap_or(0),
                        cost_usd: None,
                    });
                    // Error results report a zero duration.
                    if let Some(ms) = stats["duration_ms"].as_u64().filter(|ms| *ms > 0) {
                        events.push(ChatEvent::Duration { ms });
                    }
                }
                events.push(if v["status"].as_str() == Some("success") {
                    ChatEvent::Done
                } else {
                    ChatEvent::Error {
                        message: v["error"]["message"]
                            .as_str()
                            .unwrap_or("gemini run failed")
                            .to_string(),
                    }
                });
                events
            }
            _ => vec![],
        }
    }
}

fn gemini_mcp_summary(config_dir: &Path) -> JsonValue {
    let config_file = config_dir.join("settings.json");
    let config_exists = config_file.is_file();
    let data = read_jsonc(&config_file);

    let mut projects: Vec<JsonValue> = Vec::new();
    if let Some(mcp) = data.get("mcpServers").and_then(|v| v.as_object()) {
        let mut names: Vec<&String> = mcp.keys().collect();
        names.sort();
        let mut servers: Vec<JsonValue> = Vec::new();
        for name in names {
            let Some(cfg) = mcp.get(name).and_then(|v| v.as_object()) else {
                continue;
            };
            // Gemini's precedence: `httpUrl` > `url` > `command`.
            let (kind, url) = if let Some(u) = cfg.get("httpUrl") {
                (Some("http"), Some(u))
            } else if let Some(u) = cfg.get("url") {
                (Some("sse"), Some(u))
            } else if cfg.contains_key("command") {
                (Some("stdio"), None)
            } else {
                (None, None)
            };
            servers.push(json!({
                "name": name,
                "type": kind,
                "url": url,
                "command": cfg.get("command"),
                "needs_auth": false,
            }));
        }
        if !servers.is_empty() {
            let count = servers.len();
            projects.push(json!({
                "path": "gemini (global)",
                "mcp_servers": servers,
                "count": count,
            }));
        }
    }
    json!({
        "config_exists": config_exists,
        "projects": projects,
        "needs_auth": {},
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(cmd: &Command) -> Vec<&std::ffi::OsStr> {
        cmd.get_args().collect()
    }

    fn gemini_rt() -> GeminiRuntime {
        GeminiRuntime {
            config_dir: "/home/u/.gemini".into(),
            model: None,
        }
    }

    #[test]
    fn gemini_build_command_argv() {
        let cmd = gemini_rt().build_command("- list", None);
        assert_eq!(
            args(&cmd),
            &[
                "-o",
                "stream-json",
                "--skip-trust",
                "--yolo",
                "--prompt=- list"
            ]
        );
    }

    #[test]
    fn gemini_terminal_launch_new_with_prompt() {
        let rt = GeminiRuntime {
            config_dir: "/home/u/.gemini".into(),
            model: Some("gemini-2.5-pro".to_string()),
        };
        let cmd = rt
            .terminal_launch(Some("- list"), TermSession::New("uuid-1"))
            .unwrap();
        assert_eq!(
            args(&cmd),
            &[
                "--skip-trust",
                "--yolo",
                "-m",
                "gemini-2.5-pro",
                "--session-id",
                "uuid-1",
                "--prompt-interactive=- list"
            ]
        );
    }

    #[test]
    fn gemini_terminal_launch_resume_without_prompt() {
        let cmd = gemini_rt()
            .terminal_launch(None, TermSession::Resume("sess-1"))
            .unwrap();
        assert_eq!(
            args(&cmd),
            &["--skip-trust", "--yolo", "--resume", "sess-1"]
        );
    }

    #[test]
    fn gemini_build_command_resume_and_model() {
        let rt = GeminiRuntime {
            config_dir: "/home/u/.gemini".into(),
            model: Some("flash".to_string()),
        };
        let cmd = rt.build_command("hello", Some("sid-1"));
        assert_eq!(
            args(&cmd)[4..],
            ["-m", "flash", "--resume", "sid-1", "--prompt=hello"]
        );
    }

    #[test]
    fn gemini_chat_first_presets_session_id() {
        let cmd = gemini_rt()
            .build_command_chat_first("hello", "uuid-1")
            .unwrap();
        assert_eq!(
            args(&cmd)[4..],
            ["--session-id", "uuid-1", "--prompt=hello"]
        );
    }

    #[test]
    fn gemini_env_vars() {
        let rt = GeminiRuntime {
            config_dir: "/home/u/.gemini".into(),
            model: Some("pro".to_string()),
        };
        let vars = rt.env_vars();
        assert_eq!(
            vars,
            vec![
                ("GEMINI_CLI_HOME", OsString::from("/home/u")),
                ("MODULA_PROVIDER_MODEL", OsString::from("pro")),
            ]
        );
    }

    #[test]
    fn gemini_parse_events() {
        let rt = gemini_rt();
        let parse = |line: &str| rt.parse_stream_line(line);
        assert!(matches!(
            &parse(r#"{"type":"init","session_id":"s1","model":"auto"}"#)[..],
            [ChatEvent::Session { id }, ChatEvent::Model { name }] if id == "s1" && name == "auto"
        ));
        assert!(parse(r#"{"type":"message","role":"user","content":"hi"}"#).is_empty());
        assert!(matches!(
            &parse(r#"{"type":"message","role":"assistant","content":"Red","delta":true}"#)[..],
            [ChatEvent::Delta { text }] if text == "Red"
        ));
        // The tool's own name and parameters, untouched.
        assert!(matches!(
            &parse(r#"{"type":"tool_use","tool_name":"replace","tool_id":"t1","parameters":{"file_path":"a.txt","old_string":"hi","new_string":"bye"}}"#)[..],
            [ChatEvent::ToolUse { id, name, input, parent_id: None }]
                if id == "t1" && name == "replace"
                    && input == &json!({"file_path":"a.txt","old_string":"hi","new_string":"bye"})
        ));
        assert!(matches!(
            &parse(r#"{"type":"tool_result","tool_id":"t1","status":"success"}"#)[..],
            [ChatEvent::ToolResult { id, output, is_error: false }] if id == "t1" && output.is_empty()
        ));
        assert!(matches!(
            &parse(r#"{"type":"tool_result","tool_id":"t1","status":"error","error":{"type":"x","message":"denied"}}"#)[..],
            [ChatEvent::ToolResult { output, is_error: true, .. }] if output == "denied"
        ));
        assert!(parse(r#"{"type":"error","severity":"warning","message":"retrying"}"#).is_empty());
        assert!(matches!(
            parse(
                r#"{"type":"result","status":"success","stats":{"input_tokens":7,"output_tokens":3,"duration_ms":1200}}"#
            )[..],
            [
                ChatEvent::Usage {
                    input_tokens: 7,
                    output_tokens: 3,
                    cost_usd: None
                },
                ChatEvent::Duration { ms: 1200 },
                ChatEvent::Done
            ]
        ));
        assert!(matches!(
            &parse(r#"{"type":"result","status":"error","error":{"type":"x","message":"quota"}}"#)[..],
            [ChatEvent::Error { message }] if message == "quota"
        ));
        assert!(matches!(
            &parse(r#"{"type":"result","status":"error"}"#)[..],
            [ChatEvent::Error { message }] if message == "gemini run failed"
        ));
    }

    /// Real runs: reads and a write that end in a quota error, then the shell
    /// commands of a second one.
    #[test]
    fn gemini_parse_recorded_turns() {
        let rt = gemini_rt();
        let parse_all = |text: &str| -> Vec<ChatEvent> {
            text.lines()
                .flat_map(|line| rt.parse_stream_line(line))
                .collect()
        };
        let events = parse_all(include_str!("fixtures/gemini.jsonl"));
        let tools: Vec<&str> = events
            .iter()
            .filter_map(|e| match e {
                ChatEvent::ToolUse { name, .. } => Some(name.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(
            tools,
            ["update_topic", "read_file", "read_file", "write_file"]
        );
        assert!(events.iter().any(|e| matches!(
            e,
            ChatEvent::ToolUse { name, input, .. }
                if name == "write_file" && input["content"] == "alpha\nbeta\ndone\n"
        )));
        assert!(matches!(
            events.last(),
            Some(ChatEvent::Error { message }) if message.contains("exhausted your daily quota")
        ));
        assert!(events.iter().any(|e| matches!(
            e,
            ChatEvent::Usage {
                input_tokens: 23_353,
                output_tokens: 140,
                ..
            }
        )));

        let events = parse_all(include_str!("fixtures/gemini_shell.jsonl"));
        assert!(matches!(
            &events[3],
            ChatEvent::ToolUse { name, input, .. }
                if name == "run_shell_command" && input["command"] == "ls -la && wc -l out.txt"
        ));
        assert!(matches!(
            &events[4],
            ChatEvent::ToolResult { output, is_error: false, .. } if output.ends_with("3 out.txt")
        ));
    }

    #[test]
    fn gemini_mcp_summary_missing_dir() {
        let rt = GeminiRuntime {
            config_dir: "/nonexistent/path/xyz".into(),
            model: None,
        };
        let s = rt.mcp_summary();
        assert_eq!(s["config_exists"], false);
        assert_eq!(s["projects"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn gemini_mcp_summary_parses_mcp_servers() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("settings.json"),
            r#"{
              // comments are allowed
              "mcpServers": {
                "modula": { "httpUrl": "http://127.0.0.1:1/mcp" },
                "legacy": { "url": "http://127.0.0.1:2/sse" },
                "local": { "command": "npx", "args": ["x"] }
              }
            }"#,
        )
        .unwrap();
        let rt = GeminiRuntime {
            config_dir: tmp.path().into(),
            model: None,
        };
        let s = rt.mcp_summary();
        assert_eq!(s["config_exists"], true);
        let project = &s["projects"][0];
        assert_eq!(project["path"], "gemini (global)");
        assert_eq!(project["count"], 3);
        let servers = project["mcp_servers"].as_array().unwrap();
        assert_eq!(servers[0]["name"], "legacy");
        assert_eq!(servers[0]["type"], "sse");
        assert_eq!(servers[1]["name"], "local");
        assert_eq!(servers[1]["type"], "stdio");
        assert_eq!(servers[1]["command"], "npx");
        assert_eq!(servers[2]["name"], "modula");
        assert_eq!(servers[2]["type"], "http");
        assert_eq!(servers[2]["url"], "http://127.0.0.1:1/mcp");
    }
}
