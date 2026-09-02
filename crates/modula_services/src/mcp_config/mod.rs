//! Strategy-pattern service for editing each provider type's MCP config file.
//! One strategy per provider type reconciles the HTTP/remote MCP servers we
//! manage while leaving every unrelated key — and any command/stdio server the
//! user configured by hand — untouched.

use std::collections::HashSet;
use std::path::Path;

use serde_json::{Map, Value};

use modula_core::error::ApiResult;

mod claude;
mod codex;
mod opencode;

/// A managed HTTP MCP server entry. `auth_token`, when present and non-blank,
/// is written as the `Authorization` header value, normalized to a `Bearer`
/// credential (see [`auth_token`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpServer {
    pub key: String,
    pub url: String,
    pub auth_token: Option<String>,
}

pub trait McpConfigStrategy {
    /// The managed (url-based) MCP servers in the provider config. Absence (no
    /// dir or file) yields an empty list; a present-but-malformed file is an
    /// error so we never reconcile against garbage.
    fn read(&self, config_dir: &Path) -> ApiResult<Vec<McpServer>>;

    /// Reconcile the config file to `desired`: upsert each entry, delete
    /// url-based entries whose key is absent, never touch command/stdio entries
    /// or any unrelated key.
    fn apply(&self, config_dir: &Path, desired: &[McpServer]) -> ApiResult<()>;
}

pub fn for_type(provider_type: &str) -> Option<Box<dyn McpConfigStrategy>> {
    match provider_type {
        "claude" => Some(Box::new(claude::ClaudeStrategy)),
        "codex" => Some(Box::new(codex::CodexStrategy)),
        "opencode" => Some(Box::new(opencode::OpenCodeStrategy)),
        _ => None,
    }
}

/// The `Authorization` header value when the token is non-blank, else `None` so
/// callers omit the header entirely. MCP servers like Linear expect
/// `Authorization: Bearer <token>`, so we prepend `Bearer ` when it's missing.
fn auth_token(server: &McpServer) -> Option<String> {
    let token = server.auth_token.as_deref()?.trim();
    if token.is_empty() {
        return None;
    }
    // Case-insensitive: don't double-prefix "bearer tok" into "Bearer bearer tok".
    if token
        .get(..7)
        .is_some_and(|p| p.eq_ignore_ascii_case("Bearer "))
    {
        Some(token.to_string())
    } else {
        Some(format!("Bearer {token}"))
    }
}

/// Reconcile a serde_json `mcp`/`mcpServers` map (claude + opencode share this
/// shape; codex uses toml_edit). Managed entries are those carrying a `url`;
/// the user's command/stdio entries have none and are left alone.
fn reconcile_json(
    servers: &mut Map<String, Value>,
    desired: &[McpServer],
    make_entry: impl Fn(&McpServer) -> Value,
) {
    let keep: HashSet<&str> = desired.iter().map(|s| s.key.as_str()).collect();
    servers.retain(|key, cfg| cfg.get("url").is_none() || keep.contains(key.as_str()));
    for server in desired {
        servers.insert(server.key.clone(), make_entry(server));
    }
}

/// Atomic write: write a sibling temp file then rename over the target so a
/// crash mid-write can't leave a half-written config.
fn write_atomic(path: &Path, contents: &str) -> ApiResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = std::path::PathBuf::from(tmp);
    std::fs::write(&tmp, contents)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn srv(key: &str, url: &str, token: Option<&str>) -> McpServer {
        McpServer {
            key: key.to_string(),
            url: url.to_string(),
            auth_token: token.map(str::to_string),
        }
    }

    #[test]
    fn for_type_dispatch() {
        assert!(for_type("claude").is_some());
        assert!(for_type("codex").is_some());
        assert!(for_type("opencode").is_some());
        assert!(for_type("unknown").is_none());
    }

    #[test]
    fn auth_token_blank_is_none() {
        assert_eq!(auth_token(&srv("k", "u", None)), None);
        assert_eq!(auth_token(&srv("k", "u", Some("   "))), None);
    }

    #[test]
    fn auth_token_adds_bearer_prefix() {
        assert_eq!(
            auth_token(&srv("k", "u", Some("lin_api_123"))).as_deref(),
            Some("Bearer lin_api_123")
        );
        // An existing Bearer prefix is left as-is, not doubled — any case.
        assert_eq!(
            auth_token(&srv("k", "u", Some("Bearer lin_api_123"))).as_deref(),
            Some("Bearer lin_api_123")
        );
        assert_eq!(
            auth_token(&srv("k", "u", Some("bearer lin_api_123"))).as_deref(),
            Some("bearer lin_api_123")
        );
    }

    // Per-strategy round-trip tests live in claude.rs / codex.rs / opencode.rs.
}
