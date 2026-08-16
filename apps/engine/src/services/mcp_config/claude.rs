use std::path::Path;

use serde_json::{json, Map, Value};

use super::{auth_token, reconcile_json, write_atomic, McpConfigStrategy, McpServer};
use crate::core::error::{ApiError, ApiResult};

pub struct ClaudeStrategy;

const FILE: &str = ".claude.json";

fn parse(text: &str) -> ApiResult<Value> {
    serde_json::from_str(text).map_err(|e| ApiError::BadRequest(format!("{FILE}: {e}")))
}

fn entry(server: &McpServer) -> Value {
    let mut e = Map::new();
    e.insert("type".into(), json!("http"));
    e.insert("url".into(), json!(server.url));
    if let Some(token) = auth_token(server) {
        e.insert("headers".into(), json!({ "Authorization": token }));
    }
    Value::Object(e)
}

impl McpConfigStrategy for ClaudeStrategy {
    fn read(&self, config_dir: &Path) -> ApiResult<Vec<McpServer>> {
        let path = config_dir.join(FILE);
        if !path.is_file() {
            return Ok(vec![]);
        }
        let root = parse(&std::fs::read_to_string(&path)?)?;
        let Some(servers) = root.get("mcpServers").and_then(Value::as_object) else {
            return Ok(vec![]);
        };
        Ok(servers
            .iter()
            .filter_map(|(key, cfg)| {
                let url = cfg.get("url").and_then(Value::as_str)?;
                let auth_token = cfg
                    .get("headers")
                    .and_then(|h| h.get("Authorization"))
                    .and_then(Value::as_str)
                    .map(str::to_string);
                Some(McpServer {
                    key: key.clone(),
                    url: url.to_string(),
                    auth_token,
                })
            })
            .collect())
    }

    fn apply(&self, config_dir: &Path, desired: &[McpServer]) -> ApiResult<()> {
        let path = config_dir.join(FILE);
        let mut root = match path.is_file() {
            true => parse(&std::fs::read_to_string(&path)?)?,
            false => Value::Object(Map::new()),
        };
        let obj = root
            .as_object_mut()
            .ok_or_else(|| ApiError::BadRequest(format!("{FILE}: root is not an object")))?;
        let servers = obj
            .entry("mcpServers")
            .or_insert_with(|| Value::Object(Map::new()))
            .as_object_mut()
            .ok_or_else(|| ApiError::BadRequest(format!("{FILE}: mcpServers is not an object")))?;
        reconcile_json(servers, desired, entry);
        write_atomic(&path, &serde_json::to_string_pretty(&root)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn srv(key: &str, url: &str, token: Option<&str>) -> McpServer {
        McpServer {
            key: key.into(),
            url: url.into(),
            auth_token: token.map(str::to_string),
        }
    }

    fn read_file(dir: &Path) -> Value {
        serde_json::from_str(&std::fs::read_to_string(dir.join(FILE)).unwrap()).unwrap()
    }

    #[test]
    fn create_fresh_then_read_round_trips() {
        let tmp = tempfile::tempdir().unwrap();
        let want = vec![srv(
            "atlassian",
            "https://mcp.atlassian.com/v1/mcp",
            Some("tok"),
        )];
        ClaudeStrategy.apply(tmp.path(), &want).unwrap();

        let v = read_file(tmp.path());
        assert_eq!(v["mcpServers"]["atlassian"]["type"], "http");
        assert_eq!(
            v["mcpServers"]["atlassian"]["url"],
            "https://mcp.atlassian.com/v1/mcp"
        );
        // The token is stored with a Bearer prefix.
        assert_eq!(
            v["mcpServers"]["atlassian"]["headers"]["Authorization"],
            "Bearer tok"
        );
        assert_eq!(
            ClaudeStrategy.read(tmp.path()).unwrap(),
            vec![srv(
                "atlassian",
                "https://mcp.atlassian.com/v1/mcp",
                Some("Bearer tok")
            )]
        );
    }

    #[test]
    fn no_token_omits_headers() {
        let tmp = tempfile::tempdir().unwrap();
        ClaudeStrategy
            .apply(
                tmp.path(),
                &[srv("linear", "https://mcp.linear.app/mcp", None)],
            )
            .unwrap();
        let v = read_file(tmp.path());
        assert!(v["mcpServers"]["linear"].get("headers").is_none());
    }

    #[test]
    fn preserves_unrelated_keys_and_command_servers() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join(FILE),
            serde_json::to_string(&json!({
                "numStartups": 7,
                "projects": { "/x": { "foo": 1 } },
                "mcpServers": {
                    "local-tool": { "command": "npx", "args": ["x"] },
                    "old": { "type": "http", "url": "https://old.example/mcp" }
                }
            }))
            .unwrap(),
        )
        .unwrap();

        ClaudeStrategy
            .apply(
                tmp.path(),
                &[srv(
                    "github",
                    "https://api.githubcopilot.com/mcp/",
                    Some("g"),
                )],
            )
            .unwrap();

        let v = read_file(tmp.path());
        assert_eq!(v["numStartups"], 7);
        assert_eq!(v["projects"]["/x"]["foo"], 1);
        // command server survives; managed url server 'old' (absent from desired) is gone.
        assert_eq!(v["mcpServers"]["local-tool"]["command"], "npx");
        assert!(v["mcpServers"].get("old").is_none());
        assert_eq!(
            v["mcpServers"]["github"]["url"],
            "https://api.githubcopilot.com/mcp/"
        );
        // read returns only the managed (url-based) entry.
        assert_eq!(
            ClaudeStrategy.read(tmp.path()).unwrap(),
            vec![srv(
                "github",
                "https://api.githubcopilot.com/mcp/",
                Some("Bearer g")
            )]
        );
    }

    #[test]
    fn edit_url_and_token() {
        let tmp = tempfile::tempdir().unwrap();
        ClaudeStrategy
            .apply(tmp.path(), &[srv("k", "https://a", Some("t1"))])
            .unwrap();
        ClaudeStrategy
            .apply(tmp.path(), &[srv("k", "https://b", Some("t2"))])
            .unwrap();
        let v = read_file(tmp.path());
        assert_eq!(v["mcpServers"]["k"]["url"], "https://b");
        assert_eq!(
            v["mcpServers"]["k"]["headers"]["Authorization"],
            "Bearer t2"
        );
    }

    #[test]
    fn delete_clears_all_managed() {
        let tmp = tempfile::tempdir().unwrap();
        ClaudeStrategy
            .apply(
                tmp.path(),
                &[srv("a", "https://a", None), srv("b", "https://b", None)],
            )
            .unwrap();
        ClaudeStrategy.apply(tmp.path(), &[]).unwrap();
        assert!(ClaudeStrategy.read(tmp.path()).unwrap().is_empty());
    }

    #[test]
    fn malformed_file_errors_without_clobbering() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join(FILE), "{ not json").unwrap();
        let err = ClaudeStrategy.apply(tmp.path(), &[]).unwrap_err();
        assert!(matches!(err, ApiError::BadRequest(_)));
        assert_eq!(
            std::fs::read_to_string(tmp.path().join(FILE)).unwrap(),
            "{ not json"
        );
    }

    #[test]
    fn missing_dir_reads_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("nope");
        assert!(ClaudeStrategy.read(&missing).unwrap().is_empty());
    }
}
