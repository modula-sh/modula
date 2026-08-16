use std::collections::HashSet;
use std::path::Path;

use toml_edit::{value, DocumentMut, InlineTable, Item, Table, Value};

use super::{auth_token, write_atomic, McpConfigStrategy, McpServer};
use crate::core::error::{ApiError, ApiResult};

pub struct CodexStrategy;

const FILE: &str = "config.toml";

fn parse(text: &str) -> ApiResult<DocumentMut> {
    text.parse::<DocumentMut>()
        .map_err(|e| ApiError::BadRequest(format!("{FILE}: {e}")))
}

impl McpConfigStrategy for CodexStrategy {
    fn read(&self, config_dir: &Path) -> ApiResult<Vec<McpServer>> {
        let path = config_dir.join(FILE);
        if !path.is_file() {
            return Ok(vec![]);
        }
        let doc = parse(&std::fs::read_to_string(&path)?)?;
        let Some(servers) = doc.get("mcp_servers").and_then(Item::as_table) else {
            return Ok(vec![]);
        };
        Ok(servers
            .iter()
            .filter_map(|(key, item)| {
                let table = item.as_table_like()?;
                let url = table.get("url").and_then(Item::as_str)?;
                let auth_token = table
                    .get("http_headers")
                    .and_then(Item::as_table_like)
                    .and_then(|h| h.get("Authorization"))
                    .and_then(Item::as_str)
                    .map(str::to_string);
                Some(McpServer {
                    key: key.to_string(),
                    url: url.to_string(),
                    auth_token,
                })
            })
            .collect())
    }

    fn apply(&self, config_dir: &Path, desired: &[McpServer]) -> ApiResult<()> {
        let path = config_dir.join(FILE);
        let mut doc = match path.is_file() {
            true => parse(&std::fs::read_to_string(&path)?)?,
            false => DocumentMut::new(),
        };
        if doc.get("mcp_servers").is_none() {
            doc["mcp_servers"] = Item::Table(Table::new());
        }
        let servers = doc["mcp_servers"]
            .as_table_mut()
            .ok_or_else(|| ApiError::BadRequest(format!("{FILE}: mcp_servers is not a table")))?;

        let keep: HashSet<&str> = desired.iter().map(|s| s.key.as_str()).collect();
        let drop: Vec<String> = servers
            .iter()
            .filter(|(key, item)| {
                item.as_table_like().is_some_and(|t| t.contains_key("url")) && !keep.contains(*key)
            })
            .map(|(key, _)| key.to_string())
            .collect();
        for key in drop {
            servers.remove(&key);
        }

        for server in desired {
            let mut table = Table::new();
            table.insert("url", value(server.url.clone()));
            if let Some(token) = auth_token(server) {
                let mut headers = InlineTable::new();
                headers.insert("Authorization", Value::from(token));
                table.insert("http_headers", value(headers));
            }
            servers.insert(&server.key, Item::Table(table));
        }

        write_atomic(&path, &doc.to_string())
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

    fn text(dir: &Path) -> String {
        std::fs::read_to_string(dir.join(FILE)).unwrap()
    }

    #[test]
    fn create_fresh_then_read_round_trips() {
        let tmp = tempfile::tempdir().unwrap();
        let want = vec![srv(
            "atlassian",
            "https://mcp.atlassian.com/v1/mcp",
            Some("tok"),
        )];
        CodexStrategy.apply(tmp.path(), &want).unwrap();

        let body = text(tmp.path());
        assert!(body.contains("[mcp_servers.atlassian]"));
        assert!(body.contains(r#"url = "https://mcp.atlassian.com/v1/mcp""#));
        assert!(body.contains("Authorization"));
        // The token is stored with a Bearer prefix.
        assert!(body.contains(r#""Bearer tok""#));
        assert_eq!(
            CodexStrategy.read(tmp.path()).unwrap(),
            vec![srv(
                "atlassian",
                "https://mcp.atlassian.com/v1/mcp",
                Some("Bearer tok")
            )]
        );
    }

    #[test]
    fn no_token_omits_http_headers() {
        let tmp = tempfile::tempdir().unwrap();
        CodexStrategy
            .apply(
                tmp.path(),
                &[srv("linear", "https://mcp.linear.app/mcp", None)],
            )
            .unwrap();
        assert!(!text(tmp.path()).contains("http_headers"));
    }

    #[test]
    fn preserves_comments_unrelated_keys_and_command_servers() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join(FILE),
            r#"# top comment
model = "o4-mini"

[mcp_servers.local]
command = "npx"

[mcp_servers.old]
url = "https://old.example/mcp"
"#,
        )
        .unwrap();

        CodexStrategy
            .apply(
                tmp.path(),
                &[srv(
                    "github",
                    "https://api.githubcopilot.com/mcp/",
                    Some("g"),
                )],
            )
            .unwrap();

        let body = text(tmp.path());
        assert!(body.contains("# top comment"));
        assert!(body.contains(r#"model = "o4-mini""#));
        assert!(body.contains("[mcp_servers.local]"));
        assert!(body.contains(r#"command = "npx""#));
        assert!(!body.contains("old.example"));
        assert!(body.contains("[mcp_servers.github]"));
        assert_eq!(
            CodexStrategy.read(tmp.path()).unwrap(),
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
        CodexStrategy
            .apply(tmp.path(), &[srv("k", "https://a", Some("t1"))])
            .unwrap();
        CodexStrategy
            .apply(tmp.path(), &[srv("k", "https://b", Some("t2"))])
            .unwrap();
        assert_eq!(
            CodexStrategy.read(tmp.path()).unwrap(),
            vec![srv("k", "https://b", Some("Bearer t2"))]
        );
    }

    #[test]
    fn delete_clears_all_managed() {
        let tmp = tempfile::tempdir().unwrap();
        CodexStrategy
            .apply(
                tmp.path(),
                &[srv("a", "https://a", None), srv("b", "https://b", None)],
            )
            .unwrap();
        CodexStrategy.apply(tmp.path(), &[]).unwrap();
        assert!(CodexStrategy.read(tmp.path()).unwrap().is_empty());
    }

    #[test]
    fn malformed_file_errors_without_clobbering() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join(FILE), "this = = broken").unwrap();
        let err = CodexStrategy.apply(tmp.path(), &[]).unwrap_err();
        assert!(matches!(err, ApiError::BadRequest(_)));
        assert_eq!(text(tmp.path()), "this = = broken");
    }
}
