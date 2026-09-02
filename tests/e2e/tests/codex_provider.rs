//! E2E: codex-typed provider spawns correctly and the run reaches completed,
//! over gRPC IPC.

use anyhow::Result;
use modula_rpc::v1::{
    CreateAgentRequest, CreateProviderRequest, GetProviderRequest, ListRunsForAgentRequest,
    TriggerAgentRequest,
};
use modula_test_support::Harness;
use std::time::{Duration, Instant};

use modula_test_support::fixtures as common;

#[tokio::test]
async fn codex_provider_agent_completes() -> Result<()> {
    let h = Harness::start().await?;
    let ws = common::fresh_workspace(&h, "demo").await?;

    let cfg_dir = h.modula_dir.join("fake-codex");
    std::fs::create_dir_all(&cfg_dir)?;
    let cfg_str = cfg_dir.to_string_lossy().to_string();

    // Create a codex-typed provider.
    let cx_id = h
        .providers()
        .create(CreateProviderRequest {
            workspace_id: ws.clone(),
            name: "Codex".into(),
            r#type: "codex".into(),
            config_dir: cfg_str.clone(),
            description: None,
            mcp_servers: vec![],
        })
        .await?
        .into_inner()
        .id;

    // Round-trip — type and config_dir survive.
    let detail = h
        .providers()
        .get(GetProviderRequest {
            workspace_id: ws.clone(),
            provider_id: cx_id.clone(),
        })
        .await?
        .into_inner();
    assert_eq!(detail.r#type, "codex");
    assert_eq!(detail.config_dir, cfg_str);

    // Create a manual agent using the codex provider.
    let agent_id = h
        .agents()
        .create(CreateAgentRequest {
            workspace_id: ws.clone(),
            name: "codex-agent".into(),
            description: "codex test agent".into(),
            provider_id: cx_id,
            model: None,
            manual: true,
            schedule: None,
            rules: vec![],
            args: vec![],
            prompt: "hello from codex".into(),
            spawn_per_variant: false,
            skills: vec![],
        })
        .await?
        .into_inner()
        .id;

    // mock-claude keys on MODULA_AGENT_NAME, not argv, so it runs identically
    // whether invoked as "claude", "opencode", or "codex" — no custom recipe.

    // Trigger a manual run.
    h.agents()
        .trigger(TriggerAgentRequest {
            workspace_id: ws.clone(),
            agent_id: agent_id.clone(),
            args: None,
        })
        .await?;

    // Poll until the run reaches completed (≤ 10s).
    let deadline = Instant::now() + Duration::from_secs(10);
    let final_run = loop {
        let runs = h
            .runs()
            .list_for_agent(ListRunsForAgentRequest {
                workspace_id: ws.clone(),
                agent_id: agent_id.clone(),
            })
            .await?
            .into_inner()
            .runs;
        if let Some(run) = runs.first() {
            if run.status == "completed" || run.status == "failed" {
                break Some(run.clone());
            }
        }
        if Instant::now() >= deadline {
            break None;
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
    };
    let final_run = final_run.expect("run did not finish in time");
    assert_eq!(final_run.status, "completed", "run did not complete");

    // Confirm a log file was written.
    let log_path = final_run.log_path.expect("log_path");
    let log_file = h.workspace_path(&ws).join("logs").join(&log_path);
    assert!(
        log_file.exists(),
        "log file missing: {}",
        log_file.display()
    );

    Ok(())
}
