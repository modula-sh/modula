//! Codex provider — `codex` CLI.

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Command;

use serde_json::Value as JsonValue;

use super::{program, ChatEvent, ProviderRuntime};

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
        let t = match v["type"].as_str() {
            Some(t) => t,
            None => return vec![],
        };
        match t {
            "thread.started" => match v["thread_id"].as_str() {
                Some(id) => vec![ChatEvent::Session { id: id.to_string() }],
                None => vec![],
            },
            "item.started" if v["item"]["type"].as_str() == Some("command_execution") => {
                // `command` may be a string or an array of argv tokens.
                let cmd_val = &v["item"]["command"];
                let command = if let Some(s) = cmd_val.as_str() {
                    s.to_string()
                } else if let Some(arr) = cmd_val.as_array() {
                    arr.iter()
                        .filter_map(|a| a.as_str())
                        .collect::<Vec<_>>()
                        .join(" ")
                } else {
                    String::new()
                };
                vec![ChatEvent::ToolUse {
                    name: "Bash".to_string(),
                    input: serde_json::json!({"command": command}),
                }]
            }
            "item.completed" if v["item"]["type"].as_str() == Some("agent_message") => {
                match v["item"]["text"].as_str() {
                    Some(text) => vec![ChatEvent::Delta {
                        text: text.to_string(),
                    }],
                    None => vec![],
                }
            }
            "turn.completed" => vec![ChatEvent::Done],
            _ => vec![],
        }
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
        let events = codex_rt().parse_stream_line(line);
        assert_eq!(events.len(), 1);
        match &events[0] {
            ChatEvent::Delta { text } => assert_eq!(text, "hi there"),
            _ => panic!("expected Delta"),
        }
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
        let line =
            r#"{"type":"item.started","item":{"type":"command_execution","command":"ls -la"}}"#;
        let events = codex_rt().parse_stream_line(line);
        assert_eq!(events.len(), 1);
        match &events[0] {
            ChatEvent::ToolUse { name, input } => {
                assert_eq!(name, "Bash");
                assert_eq!(input["command"], "ls -la");
            }
            _ => panic!("expected ToolUse"),
        }
    }

    #[test]
    fn codex_parse_command_execution_array() {
        // command as an argv token array — defensive handling for possible future shape.
        let line =
            r#"{"type":"item.started","item":{"type":"command_execution","command":["ls","-la"]}}"#;
        let events = codex_rt().parse_stream_line(line);
        assert_eq!(events.len(), 1);
        match &events[0] {
            ChatEvent::ToolUse { name, input } => {
                assert_eq!(name, "Bash");
                assert_eq!(input["command"], "ls -la");
            }
            _ => panic!("expected ToolUse"),
        }
    }
}
