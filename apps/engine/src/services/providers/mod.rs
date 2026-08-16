//! Provider strategy — data/behavior split for the supported provider CLIs.
//!
//! The [`Provider`](modula_types::Provider) domain type is data;
//! [`ProviderRuntime`] is behavior. The factory fns hydrate a provider (or bare
//! type string) into an `Arc<dyn ProviderRuntime>`. Adding a provider type means
//! one new file implementing [`ProviderRuntime`] plus one arm in [`build`].

use std::ffi::OsString;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::process::Command;
use std::sync::Arc;

use serde_json::Value as JsonValue;

mod claude;
mod codex;
mod opencode;
mod service;

use claude::ClaudeRuntime;
use codex::CodexRuntime;
use opencode::OpenCodeRuntime;

pub use service::{CatalogEntry, CreateParams, CreatedProvider, ProviderService, UpdateParams};

/// Boxed future so trait methods can be async while the trait stays
/// dyn-compatible (no `async-trait` dependency).
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub fn expand_tilde(raw: &str) -> PathBuf {
    let raw = raw.trim();
    if let Some(rest) = raw.strip_prefix('~') {
        if rest.is_empty() {
            if let Some(h) = crate::platform::home_dir() {
                return h;
            }
        }
        if let Some(after) = rest.strip_prefix('/') {
            if let Some(h) = crate::platform::home_dir() {
                return h.join(after);
            }
        }
    }
    PathBuf::from(raw)
}

/// Resolve a provider CLI to a concrete executable on `PATH`, honoring the
/// platform extension rule (`.exe`/`.cmd` on Windows). Falls back to the bare
/// name when unresolved so `Command` still reports a clean not-found at spawn.
fn program(name: &str) -> PathBuf {
    crate::platform::which(name).unwrap_or_else(|| PathBuf::from(name))
}

/// One selectable model for a provider type, as served by the catalog API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderModel {
    pub id: String,
    pub label: String,
}

/// Normalized event emitted by `parse_stream_line`. Provider-specific shapes
/// are collapsed to these variants before being forwarded to SSE clients.
pub enum ChatEvent {
    Delta {
        text: String,
    },
    Session {
        id: String,
    },
    ToolUse {
        name: String,
        input: serde_json::Value,
    },
    Done,
    Error {
        message: String,
    },
}

/// Per-provider-type behavior: spawn commands, stream parsing, MCP summary,
/// and model discovery. Implementations carry their instance settings
/// (`config_dir`, `model`); hydrate them via [`ProviderService`]'s
/// `runtime_from_provider` / `runtime_for_type`.
pub trait ProviderRuntime: Send + Sync {
    /// All models this provider can offer. `static_models` is the type's
    /// entry from `provider_catalog.toml`; the default implementation returns
    /// it unchanged, for types with no discovery mechanism. Async for every
    /// type — discovery may shell out (see OpenCode).
    fn models<'a>(
        &'a self,
        static_models: Vec<ProviderModel>,
    ) -> BoxFuture<'a, Vec<ProviderModel>> {
        Box::pin(std::future::ready(static_models))
    }

    /// Build the spawn command. `session_id`, when `Some`, adds the
    /// per-provider resume flag so the CLI continues an existing session.
    fn build_command(&self, prompt: &str, session_id: Option<&str>) -> Command;

    /// Command for the first chat turn when the CLI accepts a caller-chosen
    /// session id, or `None` when it doesn't — callers then fall back to
    /// `build_command` and capture the session id from the stream instead.
    fn build_command_chat_first(&self, _prompt: &str, _preset_session_id: &str) -> Option<Command> {
        None
    }

    /// Command for chat resume turns; defaults to the standard resume command.
    fn build_command_chat_resume(&self, prompt: &str, session_id: &str) -> Command {
        self.build_command(prompt, Some(session_id))
    }

    /// Environment variables for spawned provider processes (config-dir
    /// override plus the model advertised to hooks/sub-tools).
    fn env_vars(&self) -> Vec<(&'static str, OsString)>;

    /// Returns a provider-specific MCP server summary. Shape is the same for
    /// all types: `{ config_exists, projects, needs_auth }`. Each project
    /// entry contains `{ path, mcp_servers, count }`.
    fn mcp_summary(&self) -> JsonValue;

    /// Parse one decoded provider stream-json value into normalized
    /// `ChatEvent`s. Returns an empty vec for values that don't carry
    /// meaningful chat content (setup, telemetry, unknown event types). A
    /// single value may yield multiple events (e.g. an assistant message
    /// with both text and tool_use blocks, or an opencode text event
    /// carrying both session id and delta).
    fn parse_line(&self, v: &JsonValue) -> Vec<ChatEvent>;

    /// Parse one raw line of provider stream output. Non-JSON lines yield no
    /// events.
    fn parse_stream_line(&self, line: &str) -> Vec<ChatEvent> {
        match serde_json::from_str::<JsonValue>(line) {
            Ok(v) => self.parse_line(&v),
            Err(_) => vec![],
        }
    }
}

/// Hydrate a runtime instance for a provider type. The type→struct mapping
/// lives here beside the instance structs; [`ProviderService`] is the public
/// owner of hydration (`runtime_for_type` / `runtime_from_provider` wrap this).
pub(super) fn build(
    provider_type: &str,
    config_dir: PathBuf,
    model: Option<String>,
) -> Option<Arc<dyn ProviderRuntime>> {
    match provider_type {
        "claude" => Some(Arc::new(ClaudeRuntime { config_dir, model })),
        "opencode" => Some(Arc::new(OpenCodeRuntime { config_dir, model })),
        "codex" => Some(Arc::new(CodexRuntime { config_dir, model })),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::error::ApiError;
    use modula_types::Provider;
    use serde_json::Map;

    fn provider(ptype: &str, config_dir: &str) -> Provider {
        Provider {
            id: "p1".to_string(),
            name: "Test".to_string(),
            r#type: ptype.to_string(),
            description: None,
            config_dir: config_dir.to_string(),
            config_dir_exists: false,
            mcp_server_count: 0,
            mcp_endpoints: vec![],
            agents_using: vec![],
            mcp_servers: vec![],
            mcp_summary: Map::new(),
        }
    }

    #[test]
    fn for_type_unknown_returns_none() {
        assert!(ProviderService::runtime_for_type("unknown", "/tmp".into()).is_none());
    }

    #[test]
    fn from_provider_rejects_unknown_type() {
        let Err(err) = ProviderService::runtime_from_provider(&provider("unknown", "/tmp"), None)
        else {
            panic!("expected error");
        };
        assert!(matches!(
            err,
            ApiError::BadRequest(ref m) if m.contains("unsupported provider type")
        ));
    }

    #[test]
    fn from_provider_rejects_missing_config_dir() {
        let Err(err) = ProviderService::runtime_from_provider(
            &provider("claude", "/nonexistent/path/xyz"),
            None,
        ) else {
            panic!("expected error");
        };
        assert!(matches!(
            err,
            ApiError::BadRequest(ref m) if m.contains("config_dir missing")
        ));
    }

    #[test]
    fn from_provider_hydrates_known_type() {
        let tmp = tempfile::tempdir().unwrap();
        let p = provider("claude", tmp.path().to_str().unwrap());
        assert!(ProviderService::runtime_from_provider(&p, Some("opus".to_string())).is_ok());
    }

    #[test]
    fn chat_first_default_returns_none() {
        for ptype in ["codex", "opencode"] {
            let rt = ProviderService::runtime_for_type(ptype, "/tmp".into()).unwrap();
            assert!(rt.build_command_chat_first("hello", "uuid-1").is_none());
        }
    }

    #[tokio::test]
    async fn models_default_returns_static_list() {
        let static_models = vec![ProviderModel {
            id: "opus".to_string(),
            label: "Opus".to_string(),
        }];
        // Claude and Codex use the trait's default (no discovery mechanism).
        for ptype in ["claude", "codex"] {
            let rt = ProviderService::runtime_for_type(ptype, "/tmp".into()).unwrap();
            assert_eq!(rt.models(static_models.clone()).await, static_models);
        }
    }
}
