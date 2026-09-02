//! Provider MCP reconcile API over gRPC IPC: create/update carry an optional
//! `mcp_servers` list that the engine writes onto each provider type's config
//! file via `services/mcp_config`; Get returns the managed list; an omitted
//! field is a no-op; and unrelated keys + command/stdio servers always survive.

use anyhow::Result;
use modula_rpc::v1::{CreateProviderRequest, GetProviderRequest, McpServer, UpdateProviderRequest};
use modula_test_support::Harness;
use serde_json::{json, Value as Json};
use tonic::Code;

use modula_test_support::fixtures as common;

fn mcp(key: &str, url: &str, token: Option<&str>) -> McpServer {
    McpServer {
        key: key.into(),
        url: url.into(),
        auth_token: token.map(|t| t.into()),
    }
}

async fn create_typed(
    h: &Harness,
    ws: &str,
    name: &str,
    ptype: &str,
    dir: &std::path::Path,
    mcp_servers: Vec<McpServer>,
) -> Result<String> {
    Ok(h.providers()
        .create(CreateProviderRequest {
            workspace_id: ws.to_string(),
            name: name.to_string(),
            r#type: ptype.to_string(),
            config_dir: dir.to_string_lossy().to_string(),
            description: None,
            mcp_servers,
        })
        .await?
        .into_inner()
        .id)
}

async fn put_mcp(h: &Harness, ws: &str, id: &str, mcp_servers: Vec<McpServer>) -> Result<()> {
    h.providers()
        .update(UpdateProviderRequest {
            workspace_id: ws.to_string(),
            provider_id: id.to_string(),
            mcp_servers,
            update_mcp_servers: true,
            ..Default::default()
        })
        .await?;
    Ok(())
}

/// The managed `mcp_servers` from Get, sorted by key for stable assertions.
async fn get_mcp(h: &Harness, ws: &str, id: &str) -> Result<Vec<McpServer>> {
    let mut servers = h
        .providers()
        .get(GetProviderRequest {
            workspace_id: ws.to_string(),
            provider_id: id.to_string(),
        })
        .await?
        .into_inner()
        .mcp_servers;
    servers.sort_by(|a, b| a.key.cmp(&b.key));
    Ok(servers)
}

const ATLASSIAN_URL: &str = "https://mcp.atlassian.com/v1/mcp";
const LINEAR_URL: &str = "https://mcp.linear.app/mcp";

#[tokio::test]
async fn claude_mcp_roundtrip() -> Result<()> {
    let h = Harness::start().await?;
    let ws = common::fresh_workspace(&h, "demo").await?;
    let dir = h.modula_dir.join("prov-claude");
    std::fs::create_dir_all(&dir)?;
    let file = dir.join(".claude.json");
    // Seed an unrelated top-level key and a command/stdio server we must not touch.
    std::fs::write(
        &file,
        serde_json::to_string_pretty(&json!({
            "unrelatedKey": "keep-me",
            "mcpServers": {
                "local-tool": { "command": "node", "args": ["x.js"] }
            }
        }))?,
    )?;

    let id = create_typed(
        &h,
        &ws,
        "p-claude",
        "claude",
        &dir,
        vec![mcp("atlassian", ATLASSIAN_URL, Some("tok-1"))],
    )
    .await?;

    // Get surfaces the managed server with its token (Bearer-prefixed on write).
    let servers = get_mcp(&h, &ws, &id).await?;
    assert_eq!(servers.len(), 1);
    assert_eq!(servers[0].key, "atlassian");
    assert_eq!(servers[0].url, ATLASSIAN_URL);
    assert_eq!(servers[0].auth_token.as_deref(), Some("Bearer tok-1"));

    // On disk: documented http shape; unrelated key + command server intact.
    let disk: Json = serde_json::from_str(&std::fs::read_to_string(&file)?)?;
    assert_eq!(disk["unrelatedKey"], "keep-me");
    assert_eq!(disk["mcpServers"]["local-tool"]["command"], "node");
    let atl = &disk["mcpServers"]["atlassian"];
    assert_eq!(atl["type"], "http");
    assert_eq!(atl["url"], ATLASSIAN_URL);
    assert_eq!(atl["headers"]["Authorization"], "Bearer tok-1");

    // Edit token + add a second server.
    put_mcp(
        &h,
        &ws,
        &id,
        vec![
            mcp("atlassian", ATLASSIAN_URL, Some("tok-2")),
            mcp("linear", LINEAR_URL, Some("lin")),
        ],
    )
    .await?;
    let servers = get_mcp(&h, &ws, &id).await?;
    assert_eq!(servers.len(), 2);
    assert_eq!(servers[0].auth_token.as_deref(), Some("Bearer tok-2"));
    assert_eq!(servers[1].key, "linear");

    // Remove linear; command server still present.
    put_mcp(
        &h,
        &ws,
        &id,
        vec![mcp("atlassian", ATLASSIAN_URL, Some("tok-2"))],
    )
    .await?;
    let disk: Json = serde_json::from_str(&std::fs::read_to_string(&file)?)?;
    assert!(disk["mcpServers"].get("linear").is_none());
    assert_eq!(disk["mcpServers"]["local-tool"]["command"], "node");

    // Update without `mcp_servers` (update_mcp_servers=false) leaves the file
    // byte-identical.
    let before = std::fs::read(&file)?;
    h.providers()
        .update(UpdateProviderRequest {
            workspace_id: ws.clone(),
            provider_id: id.clone(),
            name: Some("p-claude-renamed".into()),
            ..Default::default()
        })
        .await?;
    assert_eq!(std::fs::read(&file)?, before);
    Ok(())
}

#[tokio::test]
async fn codex_mcp_roundtrip() -> Result<()> {
    let h = Harness::start().await?;
    let ws = common::fresh_workspace(&h, "demo").await?;
    let dir = h.modula_dir.join("prov-codex");
    std::fs::create_dir_all(&dir)?;
    let file = dir.join("config.toml");
    std::fs::write(
        &file,
        "# top comment\nmodel = \"o1\"\n\n[mcp_servers.local-tool]\ncommand = \"node\"\n",
    )?;

    let id = create_typed(
        &h,
        &ws,
        "p-codex",
        "codex",
        &dir,
        vec![mcp("atlassian", ATLASSIAN_URL, Some("tok-1"))],
    )
    .await?;

    let servers = get_mcp(&h, &ws, &id).await?;
    assert_eq!(servers.len(), 1);
    assert_eq!(servers[0].url, ATLASSIAN_URL);

    let text = std::fs::read_to_string(&file)?;
    assert!(text.contains("# top comment"), "toml comment lost:\n{text}");
    assert!(text.contains("model = \"o1\""));
    assert!(text.contains("[mcp_servers.local-tool]"));
    assert!(text.contains("[mcp_servers.atlassian]"));
    assert!(text.contains(ATLASSIAN_URL));

    // Delete the managed server; local-tool + comment survive.
    put_mcp(&h, &ws, &id, vec![]).await?;
    let text = std::fs::read_to_string(&file)?;
    assert!(!text.contains("[mcp_servers.atlassian]"));
    assert!(text.contains("[mcp_servers.local-tool]"));
    assert!(text.contains("# top comment"));
    Ok(())
}

#[tokio::test]
async fn opencode_mcp_roundtrip() -> Result<()> {
    let h = Harness::start().await?;
    let ws = common::fresh_workspace(&h, "demo").await?;
    let dir = h.modula_dir.join("prov-opencode");
    std::fs::create_dir_all(&dir)?;
    let file = dir.join("opencode.jsonc");
    std::fs::write(
        &file,
        "{\n  \"$schema\": \"https://opencode.ai/config.json\",\n  \"theme\": \"dark\"\n}\n",
    )?;

    let id = create_typed(
        &h,
        &ws,
        "p-opencode",
        "opencode",
        &dir,
        vec![mcp("atlassian", ATLASSIAN_URL, Some("tok-1"))],
    )
    .await?;

    let servers = get_mcp(&h, &ws, &id).await?;
    assert_eq!(servers.len(), 1);
    assert_eq!(servers[0].url, ATLASSIAN_URL);

    let disk: Json = serde_json::from_str(&std::fs::read_to_string(&file)?)?;
    assert_eq!(disk["theme"], "dark");
    let atl = &disk["mcp"]["atlassian"];
    assert_eq!(atl["type"], "remote");
    assert_eq!(atl["url"], ATLASSIAN_URL);

    // Reject a duplicate key.
    let err = h
        .providers()
        .update(UpdateProviderRequest {
            workspace_id: ws.clone(),
            provider_id: id.clone(),
            mcp_servers: vec![
                mcp("dup", ATLASSIAN_URL, None),
                mcp("dup", LINEAR_URL, None),
            ],
            update_mcp_servers: true,
            ..Default::default()
        })
        .await
        .expect_err("duplicate mcp key must be rejected");
    assert_eq!(err.code(), Code::InvalidArgument);
    Ok(())
}
