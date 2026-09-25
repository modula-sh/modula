//! Gemini provider — `gemini` CLI.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value as JsonValue};

use super::{program, read_jsonc, ChatEvent, ProviderRuntime};

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
        match v["type"].as_str().unwrap_or("") {
            "init" => match v["session_id"].as_str() {
                Some(id) => vec![ChatEvent::Session { id: id.to_string() }],
                None => vec![],
            },
            "message" if v["role"].as_str() == Some("assistant") => match v["content"].as_str() {
                Some(text) => vec![ChatEvent::Delta {
                    text: text.to_string(),
                }],
                None => vec![],
            },
            "tool_use" => {
                let params = &v["parameters"];
                let (name, input) = match v["tool_name"].as_str().unwrap_or("") {
                    "run_shell_command" => ("Bash", json!({ "command": params["command"] })),
                    "write_file" | "replace" => {
                        ("Edit", json!({ "file_path": params["file_path"] }))
                    }
                    other => (other, params.clone()),
                };
                vec![ChatEvent::ToolUse {
                    name: name.to_string(),
                    input,
                }]
            }
            "tool_result" => vec![ChatEvent::ToolResult],
            "result" if v["status"].as_str() == Some("success") => vec![ChatEvent::Done],
            "result" => vec![ChatEvent::Error {
                message: v["error"]["message"]
                    .as_str()
                    .unwrap_or("gemini run failed")
                    .to_string(),
            }],
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
            [ChatEvent::Session { id }] if id == "s1"
        ));
        assert!(parse(r#"{"type":"message","role":"user","content":"hi"}"#).is_empty());
        assert!(matches!(
            &parse(r#"{"type":"message","role":"assistant","content":"Red","delta":true}"#)[..],
            [ChatEvent::Delta { text }] if text == "Red"
        ));
        assert!(matches!(
            &parse(r#"{"type":"tool_use","tool_name":"run_shell_command","tool_id":"t","parameters":{"command":"echo hi","description":"x"}}"#)[..],
            [ChatEvent::ToolUse { name, input }] if name == "Bash" && input == &json!({"command":"echo hi"})
        ));
        assert!(matches!(
            &parse(r#"{"type":"tool_use","tool_name":"replace","tool_id":"t","parameters":{"file_path":"a.txt","old_string":"hi","new_string":"bye"}}"#)[..],
            [ChatEvent::ToolUse { name, input }] if name == "Edit" && input == &json!({"file_path":"a.txt"})
        ));
        assert!(matches!(
            &parse(r#"{"type":"tool_use","tool_name":"glob","tool_id":"t","parameters":{"pattern":"*.rs"}}"#)[..],
            [ChatEvent::ToolUse { name, input }] if name == "glob" && input["pattern"] == "*.rs"
        ));
        assert!(matches!(
            parse(r#"{"type":"tool_result","tool_id":"t","status":"success"}"#)[..],
            [ChatEvent::ToolResult]
        ));
        assert!(parse(r#"{"type":"error","severity":"warning","message":"retrying"}"#).is_empty());
        assert!(matches!(
            parse(r#"{"type":"result","status":"success","stats":{}}"#)[..],
            [ChatEvent::Done]
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
