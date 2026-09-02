//! `ProviderService` — provider CRUD, catalog, and runtime hydration in one
//! owner. It holds the pool + `ProviderRepository` (+ paths for the catalog
//! override and MCP config files) and is the single place that turns a provider
//! record into an `Arc<dyn ProviderRuntime>` (the D1 decision). gRPC handlers
//! and in-process runtime callers reach providers through this, never the repo.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;
use serde_json::{json, Map, Value as JsonValue};

use super::{build, expand_tilde, ProviderModel, ProviderRuntime};
use crate::events::{self, EventSink};
use crate::mcp_config::{self, McpServer};
use modula_core::error::{ApiError, ApiResult};
use modula_core::paths::Paths;
use modula_db::providers::ProviderRepository;
use modula_db::Database;
use modula_types::{McpServer as DomainMcpServer, Provider};

const DEFAULT_CATALOG_TOML: &str = include_str!("../../provider_catalog.toml");

#[derive(Serialize)]
pub struct CatalogEntry {
    pub id: String,
    pub models: Vec<ProviderModelEntry>,
}

#[derive(Serialize)]
pub struct ProviderModelEntry {
    pub id: String,
    pub label: String,
}

pub struct CreatedProvider {
    pub id: String,
    pub name: String,
}

pub struct CreateParams {
    pub name: String,
    pub r#type: Option<String>,
    pub config_dir: String,
    pub description: Option<String>,
    pub mcp_servers: Option<Vec<McpServer>>,
}

/// `description: Some(None)` clears the field; `None` leaves it unchanged.
#[derive(Default)]
pub struct UpdateParams {
    pub name: Option<String>,
    pub r#type: Option<String>,
    pub config_dir: Option<String>,
    pub description: Option<Option<String>>,
    pub mcp_servers: Option<Vec<McpServer>>,
}

#[derive(Clone)]
pub struct ProviderService {
    pool: Database,
    providers: ProviderRepository,
    paths: Arc<Paths>,
    events: Arc<dyn EventSink>,
}

impl ProviderService {
    pub fn new(
        pool: Database,
        providers: ProviderRepository,
        paths: Arc<Paths>,
        events: Arc<dyn EventSink>,
    ) -> Self {
        Self {
            pool,
            providers,
            paths,
            events,
        }
    }

    /// Hydrate a runtime for the given type without validating `config_dir` on
    /// disk. Used for display / MCP summary / catalog paths where a missing dir
    /// is not an error. Returns `None` for unknown types.
    pub fn runtime_for_type(
        provider_type: &str,
        config_dir: PathBuf,
    ) -> Option<Arc<dyn ProviderRuntime>> {
        build(provider_type, config_dir, None)
    }

    /// Hydrate a validated runtime from a provider for use at spawn time.
    /// Errors if the type is unsupported or `config_dir` is missing.
    pub fn runtime_from_provider(
        p: &Provider,
        model: Option<String>,
    ) -> ApiResult<Arc<dyn ProviderRuntime>> {
        let config_dir = expand_tilde(&p.config_dir);
        let Some(runtime) = build(&p.r#type, config_dir.clone(), model) else {
            return Err(ApiError::BadRequest(format!(
                "unsupported provider type: {:?}",
                p.r#type
            )));
        };
        if !config_dir.is_dir() {
            return Err(ApiError::BadRequest(format!(
                "provider {:?} config_dir missing: {}",
                p.id,
                config_dir.display()
            )));
        }
        Ok(runtime)
    }

    pub async fn catalog(&self) -> ApiResult<Vec<CatalogEntry>> {
        #[derive(serde::Deserialize)]
        #[serde(untagged)]
        enum RawModel {
            Bare(String),
            Object {
                id: String,
                #[serde(default)]
                label: Option<String>,
            },
        }
        #[derive(serde::Deserialize)]
        struct RawProvider {
            id: String,
            models: Vec<RawModel>,
        }
        #[derive(serde::Deserialize)]
        struct RawCatalog {
            providers: Vec<RawProvider>,
        }

        let override_path = self.paths.modula.join("provider_catalog.toml");
        let toml_str = if override_path.is_file() {
            std::fs::read_to_string(&override_path)
                .map_err(|e| ApiError::Internal(format!("read provider_catalog.toml: {e}")))?
        } else {
            DEFAULT_CATALOG_TOML.to_string()
        };
        let parsed: RawCatalog = toml::from_str(&toml_str)
            .map_err(|e| ApiError::Internal(format!("provider_catalog.toml parse error: {e}")))?;

        let mut out = Vec::with_capacity(parsed.providers.len());
        for p in parsed.providers {
            let static_models: Vec<ProviderModel> = p
                .models
                .into_iter()
                .map(|m| match m {
                    RawModel::Bare(id) => {
                        let label = id.clone();
                        ProviderModel { id, label }
                    }
                    RawModel::Object { id, label } => {
                        let label = label.unwrap_or_else(|| id.clone());
                        ProviderModel { id, label }
                    }
                })
                .collect();
            // Each type's runtime turns its static TOML entry into the final list;
            // an id without a runtime is served as-is.
            let models = match Self::runtime_for_type(&p.id, PathBuf::new()) {
                Some(rt) => rt.models(static_models).await,
                None => static_models,
            };
            out.push(CatalogEntry {
                id: p.id,
                models: models
                    .into_iter()
                    .map(|m| ProviderModelEntry {
                        id: m.id,
                        label: m.label,
                    })
                    .collect(),
            });
        }
        Ok(out)
    }

    /// List providers, each enriched with its on-disk config state and runtime
    /// MCP counts. The managed-server list and schemaless MCP summary are the
    /// `get` (detail) concern, so they stay empty here.
    pub async fn list(&self, ws: &str) -> ApiResult<Vec<Provider>> {
        let providers = self.providers.list(&self.pool, ws).await?;
        let mut out = Vec::with_capacity(providers.len());
        for mut p in providers {
            let cfg_dir = expand_tilde(&p.config_dir);
            let (mcp_server_count, mcp_endpoints) = mcp_counts(&p.r#type, &cfg_dir);
            p.agents_using = self.providers.agents_using(&self.pool, ws, &p.id).await?;
            p.config_dir = cfg_dir.to_string_lossy().into_owned();
            p.config_dir_exists = cfg_dir.is_dir();
            p.mcp_server_count = mcp_server_count;
            p.mcp_endpoints = mcp_endpoints;
            out.push(p);
        }
        Ok(out)
    }

    /// One provider fully enriched: config state, runtime MCP summary
    /// (`config_exists`/`projects`/`needs_auth`), and the managed MCP server list.
    pub async fn get(&self, ws: &str, id: &str) -> ApiResult<Provider> {
        let mut p = self.providers.get(&self.pool, ws, id).await?;
        let cfg_dir = expand_tilde(&p.config_dir);
        let (mcp_server_count, mcp_endpoints) = mcp_counts(&p.r#type, &cfg_dir);
        let mcp_summary = match Self::runtime_for_type(&p.r#type, cfg_dir.clone())
            .map(|rt| rt.mcp_summary())
            .unwrap_or_else(|| json!({ "config_exists": false, "projects": [], "needs_auth": {} }))
        {
            JsonValue::Object(m) => m,
            _ => Map::new(),
        };
        // Managed (url-based) servers for the edit form. Degrade to empty when the
        // type is unknown or the config file is missing so the page still opens.
        let mcp_servers = mcp_config::for_type(&p.r#type)
            .and_then(|s| s.read(&cfg_dir).ok())
            .unwrap_or_default()
            .into_iter()
            .map(|s| DomainMcpServer {
                key: s.key,
                url: s.url,
                auth_token: s.auth_token,
            })
            .collect();
        p.agents_using = self.providers.agents_using(&self.pool, ws, id).await?;
        p.config_dir = cfg_dir.to_string_lossy().into_owned();
        p.config_dir_exists = cfg_dir.is_dir();
        p.mcp_server_count = mcp_server_count;
        p.mcp_endpoints = mcp_endpoints;
        p.mcp_servers = mcp_servers;
        p.mcp_summary = mcp_summary;
        Ok(p)
    }

    pub async fn create(&self, ws: &str, params: CreateParams) -> ApiResult<CreatedProvider> {
        let name = require_nonempty("name", &params.name)?.to_string();
        let ptype = params
            .r#type
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("claude")
            .to_string();
        let dir = require_nonempty("config_dir", &params.config_dir)?.to_string();
        let desc = params
            .description
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let id = self
            .providers
            .create(&self.pool, ws, &name, &ptype, &dir, desc)
            .await?;
        // Before the MCP write: that can fail after the row is committed, and a
        // committed row without its event is invisible to the sync feed.
        self.events
            .publish(ws, events::PROVIDER_CREATE, json!({ "provider_id": id }))
            .await;
        if let Some(servers) = &params.mcp_servers {
            apply_mcp_servers(&ptype, &expand_tilde(&dir), servers)?;
        }
        Ok(CreatedProvider { id, name })
    }

    pub async fn update(&self, ws: &str, id: &str, params: UpdateParams) -> ApiResult<()> {
        let name = params
            .name
            .as_deref()
            .map(|n| require_nonempty("name", n).map(str::to_string))
            .transpose()?;
        let ptype = params
            .r#type
            .as_deref()
            .map(|t| require_nonempty("type", t).map(str::to_string))
            .transpose()?;
        let dir = params
            .config_dir
            .as_deref()
            .map(|d| require_nonempty("config_dir", d).map(str::to_string))
            .transpose()?;
        self.providers
            .patch(
                &self.pool,
                ws,
                id,
                name.as_deref(),
                ptype.as_deref(),
                dir.as_deref(),
                params.description,
            )
            .await?;
        self.events
            .publish(ws, events::PROVIDER_UPDATE, json!({ "provider_id": id }))
            .await;
        if let Some(servers) = &params.mcp_servers {
            // Reconcile against the effective (post-patch) type and config dir.
            let row = self.providers.get(&self.pool, ws, id).await?;
            apply_mcp_servers(&row.r#type, &expand_tilde(&row.config_dir), servers)?;
        }
        Ok(())
    }

    pub async fn delete(&self, ws: &str, id: &str) -> ApiResult<()> {
        let agents_using = self.providers.agents_using(&self.pool, ws, id).await?;
        if !agents_using.is_empty() {
            return Err(ApiError::Conflict(format!(
                "provider {id:?} is in use by {} agent(s): {}",
                agents_using.len(),
                agents_using.join(", ")
            )));
        }
        self.providers.delete(&self.pool, ws, id).await?;
        self.events
            .publish(ws, events::PROVIDER_DELETE, json!({ "provider_id": id }))
            .await;
        Ok(())
    }
}

/// Count managed MCP servers and collect their endpoint URLs from the runtime
/// summary. Returns `(0, [])` when the config dir is missing or the type has no
/// runtime.
fn mcp_counts(ptype: &str, cfg_dir: &Path) -> (u64, Vec<String>) {
    if !cfg_dir.is_dir() {
        return (0, Vec::new());
    }
    let Some(rt) = ProviderService::runtime_for_type(ptype, cfg_dir.to_path_buf()) else {
        return (0, Vec::new());
    };
    let summary = rt.mcp_summary();
    let projects = summary["projects"].as_array().cloned().unwrap_or_default();
    let count = projects
        .iter()
        .filter_map(|v| v.get("count").and_then(|c| c.as_u64()))
        .sum();
    let endpoints = projects
        .iter()
        .filter_map(|p| p.get("mcp_servers").and_then(|v| v.as_array()))
        .flatten()
        .filter_map(|s| s.get("url").and_then(|u| u.as_str()).map(str::to_string))
        .collect();
    (count, endpoints)
}

fn require_nonempty<'a>(field: &str, v: &'a str) -> ApiResult<&'a str> {
    let t = v.trim();
    if t.is_empty() {
        return Err(ApiError::BadRequest(format!("{field} is required")));
    }
    Ok(t)
}

/// Validate the submitted MCP list and reconcile it onto the provider's config
/// file via the type's strategy. Rejects blank keys/urls and duplicate keys; a
/// type without a strategy is a no-op.
fn apply_mcp_servers(ptype: &str, config_dir: &Path, servers: &[McpServer]) -> ApiResult<()> {
    let mut seen: HashSet<&str> = HashSet::new();
    let mut desired = Vec::with_capacity(servers.len());
    for s in servers {
        let key = require_nonempty("mcp key", &s.key)?;
        let url = require_nonempty("mcp url", &s.url)?;
        if !seen.insert(key) {
            return Err(ApiError::BadRequest(format!("duplicate mcp key: {key}")));
        }
        desired.push(McpServer {
            key: key.to_string(),
            url: url.to_string(),
            auth_token: s.auth_token.clone(),
        });
    }
    if let Some(strategy) = mcp_config::for_type(ptype) {
        strategy.apply(config_dir, &desired)?;
    }
    Ok(())
}
