use modula_rpc::json::{json_to_struct, struct_to_json};
use modula_rpc::v1 as pb;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpServer {
    pub key: String,
    pub url: String,
    pub auth_token: Option<String>,
}

impl From<pb::McpServer> for McpServer {
    fn from(m: pb::McpServer) -> Self {
        Self {
            key: m.key,
            url: m.url,
            auth_token: m.auth_token,
        }
    }
}

impl From<McpServer> for pb::McpServer {
    fn from(m: McpServer) -> Self {
        Self {
            key: m.key,
            url: m.url,
            auth_token: m.auth_token,
        }
    }
}

/// A provider (`dto::provider`). On `Get` the schemaless `mcp_summary`
/// (`config_exists` / `projects` / `needs_auth`) is flattened into the object
/// the frontend `ProviderDetail` expects; on `List` it is empty.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Provider {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub r#type: String,
    pub description: Option<String>,
    pub config_dir: String,
    pub config_dir_exists: bool,
    pub mcp_server_count: u64,
    pub mcp_endpoints: Vec<String>,
    pub agents_using: Vec<String>,
    pub mcp_servers: Vec<McpServer>,
    #[serde(flatten)]
    pub mcp_summary: Map<String, Value>,
}

impl From<pb::Provider> for Provider {
    fn from(p: pb::Provider) -> Self {
        let mcp_summary = match p.mcp_summary.map(struct_to_json) {
            Some(Value::Object(m)) => m,
            _ => Map::new(),
        };
        Self {
            id: p.id,
            name: p.name,
            r#type: p.r#type,
            description: p.description,
            config_dir: p.config_dir,
            config_dir_exists: p.config_dir_exists,
            mcp_server_count: p.mcp_server_count,
            mcp_endpoints: p.mcp_endpoints,
            agents_using: p.agents_using,
            mcp_servers: p.mcp_servers.into_iter().map(McpServer::from).collect(),
            mcp_summary,
        }
    }
}

impl From<Provider> for pb::Provider {
    fn from(p: Provider) -> Self {
        let mcp_summary = if p.mcp_summary.is_empty() {
            None
        } else {
            json_to_struct(Value::Object(p.mcp_summary))
        };
        Self {
            id: p.id,
            name: p.name,
            r#type: p.r#type,
            description: p.description,
            config_dir: p.config_dir,
            config_dir_exists: p.config_dir_exists,
            mcp_server_count: p.mcp_server_count,
            mcp_endpoints: p.mcp_endpoints,
            agents_using: p.agents_using,
            mcp_servers: p.mcp_servers.into_iter().map(pb::McpServer::from).collect(),
            mcp_summary,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CatalogModel {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CatalogProvider {
    pub id: String,
    pub models: Vec<CatalogModel>,
}

impl From<pb::CatalogProvider> for CatalogProvider {
    fn from(c: pb::CatalogProvider) -> Self {
        Self {
            id: c.id,
            models: c
                .models
                .into_iter()
                .map(|m| CatalogModel {
                    id: m.id,
                    label: m.label,
                })
                .collect(),
        }
    }
}

impl From<CatalogProvider> for pb::CatalogProvider {
    fn from(c: CatalogProvider) -> Self {
        Self {
            id: c.id,
            models: c
                .models
                .into_iter()
                .map(|m| pb::CatalogModel {
                    id: m.id,
                    label: m.label,
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // List shape: no MCP summary, no managed servers.
    fn summary() -> Provider {
        Provider {
            id: "p1".into(),
            name: "Claude".into(),
            r#type: "claude".into(),
            description: None,
            config_dir: "/c".into(),
            config_dir_exists: true,
            mcp_server_count: 0,
            mcp_endpoints: vec![],
            agents_using: vec!["worker".into()],
            mcp_servers: vec![],
            mcp_summary: Map::new(),
        }
    }

    // Get shape: managed servers + flattened schemaless summary.
    fn detail() -> Provider {
        let mut p = summary();
        p.mcp_server_count = 1;
        p.mcp_servers = vec![McpServer {
            key: "k".into(),
            url: "https://x".into(),
            auth_token: None,
        }];
        p.mcp_summary = json!({"config_exists": true, "needs_auth": {"k": true}})
            .as_object()
            .unwrap()
            .clone();
        p
    }

    #[test]
    fn round_trip() {
        let d = detail();
        assert_eq!(d, Provider::from(pb::Provider::from(d.clone())));
    }

    #[test]
    fn summary_serde_matches_dto() {
        let want = json!({
            "id": "p1", "name": "Claude", "type": "claude", "description": null,
            "config_dir": "/c", "config_dir_exists": true, "mcp_server_count": 0,
            "mcp_endpoints": [], "agents_using": ["worker"], "mcp_servers": [],
        });
        assert_eq!(serde_json::to_value(summary()).unwrap(), want);
    }

    #[test]
    fn detail_serde_flattens_summary() {
        let got = serde_json::to_value(detail()).unwrap();
        let want = json!({
            "id": "p1", "name": "Claude", "type": "claude", "description": null,
            "config_dir": "/c", "config_dir_exists": true, "mcp_server_count": 1,
            "mcp_endpoints": [], "agents_using": ["worker"],
            "mcp_servers": [{"key": "k", "url": "https://x", "auth_token": null}],
            "config_exists": true, "needs_auth": {"k": true},
        });
        assert_eq!(got, want);
    }

    #[test]
    fn catalog_round_trip() {
        let d = CatalogProvider {
            id: "claude".into(),
            models: vec![CatalogModel {
                id: "opus".into(),
                label: "Opus".into(),
            }],
        };
        assert_eq!(
            d,
            CatalogProvider::from(pb::CatalogProvider::from(d.clone()))
        );
    }
}
