use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use super::{auth_token, reconcile_json, write_atomic, McpConfigStrategy, McpServer};
use crate::core::error::{ApiError, ApiResult};

pub struct OpenCodeStrategy;

/// Prefer an existing `opencode.json`, then an existing `opencode.jsonc`; when
/// neither exists, fresh writes target `opencode.jsonc` (the name the task uses).
fn config_path(config_dir: &Path) -> PathBuf {
    let json = config_dir.join("opencode.json");
    if json.is_file() {
        return json;
    }
    config_dir.join("opencode.jsonc")
}

/// Parse JSONC (comments tolerated) into a serde value.
fn parse(text: &str, path: &Path) -> ApiResult<Value> {
    let parsed: Option<Value> =
        jsonc_parser::parse_to_serde_value(text, &jsonc_parser::ParseOptions::default())
            .map_err(|e| ApiError::BadRequest(format!("{}: {e}", path.display())))?;
    Ok(parsed.unwrap_or_else(|| Value::Object(Map::new())))
}

fn entry(server: &McpServer) -> Value {
    let mut e = Map::new();
    e.insert("type".into(), json!("remote"));
    e.insert("url".into(), json!(server.url));
    e.insert("enabled".into(), json!(true));
    if let Some(token) = auth_token(server) {
        e.insert("headers".into(), json!({ "Authorization": token }));
    }
    Value::Object(e)
}

impl McpConfigStrategy for OpenCodeStrategy {
    fn read(&self, config_dir: &Path) -> ApiResult<Vec<McpServer>> {
        let path = config_path(config_dir);
        if !path.is_file() {
            return Ok(vec![]);
        }
        let root = parse(&std::fs::read_to_string(&path)?, &path)?;
        let Some(servers) = root.get("mcp").and_then(Value::as_object) else {
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
        let path = config_path(config_dir);
        let mut root = match path.is_file() {
            true => parse(&std::fs::read_to_string(&path)?, &path)?,
            false => json!({ "$schema": "https://opencode.ai/config.json" }),
        };
        let obj = root.as_object_mut().ok_or_else(|| {
            ApiError::BadRequest(format!("{}: root is not an object", path.display()))
        })?;
        let servers = obj
            .entry("mcp")
            .or_insert_with(|| Value::Object(Map::new()))
            .as_object_mut()
            .ok_or_else(|| {
                ApiError::BadRequest(format!("{}: mcp is not an object", path.display()))
            })?;
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

    fn read_jsonc(path: &Path) -> Value {
        let parsed: Option<Value> = jsonc_parser::parse_to_serde_value(
            &std::fs::read_to_string(path).unwrap(),
            &jsonc_parser::ParseOptions::default(),
        )
        .unwrap();
        parsed.unwrap()
    }

    #[test]
    fn create_fresh_writes_jsonc_with_schema() {
        let tmp = tempfile::tempdir().unwrap();
        let want = vec![srv(
            "atlassian",
            "https://mcp.atlassian.com/v1/mcp",
            Some("tok"),
        )];
        OpenCodeStrategy.apply(tmp.path(), &want).unwrap();

        let path = tmp.path().join("opencode.jsonc");
        assert!(path.is_file());
        let v = read_jsonc(&path);
        assert_eq!(v["$schema"], "https://opencode.ai/config.json");
        assert_eq!(v["mcp"]["atlassian"]["type"], "remote");
        assert_eq!(v["mcp"]["atlassian"]["enabled"], true);
        // The token is stored with a Bearer prefix.
        assert_eq!(
            v["mcp"]["atlassian"]["headers"]["Authorization"],
            "Bearer tok"
        );
        assert_eq!(
            OpenCodeStrategy.read(tmp.path()).unwrap(),
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
        OpenCodeStrategy
            .apply(
                tmp.path(),
                &[srv("linear", "https://mcp.linear.app/mcp", None)],
            )
            .unwrap();
        let v = read_jsonc(&tmp.path().join("opencode.jsonc"));
        assert!(v["mcp"]["linear"].get("headers").is_none());
    }

    #[test]
    fn prefers_existing_json_and_preserves_data() {
        // Existing opencode.json with an unrelated key + local server. The `//`
        // comment is intentionally dropped on rewrite (serde_json pretty-print);
        // only data and unknown keys are preserved, not hand-written comments.
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("opencode.json"),
            r#"{
  // user theme
  "theme": "dark",
  "mcp": {
    "local": { "type": "local", "command": ["x"] },
    "old": { "type": "remote", "url": "https://old.example", "enabled": true }
  }
}"#,
        )
        .unwrap();

        OpenCodeStrategy
            .apply(
                tmp.path(),
                &[srv("github", "https://api.githubcopilot.com/mcp/", None)],
            )
            .unwrap();

        // Wrote back to the same opencode.json (not a new jsonc).
        let v = read_jsonc(&tmp.path().join("opencode.json"));
        assert_eq!(v["theme"], "dark");
        assert_eq!(v["mcp"]["local"]["type"], "local");
        assert!(v["mcp"].get("old").is_none());
        assert_eq!(
            v["mcp"]["github"]["url"],
            "https://api.githubcopilot.com/mcp/"
        );
        assert_eq!(
            OpenCodeStrategy.read(tmp.path()).unwrap(),
            vec![srv("github", "https://api.githubcopilot.com/mcp/", None)]
        );
    }

    #[test]
    fn delete_clears_all_managed() {
        let tmp = tempfile::tempdir().unwrap();
        OpenCodeStrategy
            .apply(
                tmp.path(),
                &[srv("a", "https://a", None), srv("b", "https://b", None)],
            )
            .unwrap();
        OpenCodeStrategy.apply(tmp.path(), &[]).unwrap();
        assert!(OpenCodeStrategy.read(tmp.path()).unwrap().is_empty());
    }

    #[test]
    fn malformed_file_errors_without_clobbering() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("opencode.jsonc");
        std::fs::write(&path, "{ not : json ,,").unwrap();
        let err = OpenCodeStrategy.apply(tmp.path(), &[]).unwrap_err();
        assert!(matches!(err, ApiError::BadRequest(_)));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ not : json ,,");
    }
}
