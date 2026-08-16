//! Provider + agent CRUD and referential integrity over gRPC IPC: create,
//! list, the in-use delete guard, and the agent config/detail round-trip.

use anyhow::Result;
use modula_rpc::v1::{
    CreateAgentRequest, DeleteAgentRequest, DeleteProviderRequest, GetAgentConfigRequest,
    GetAgentRequest, ListProvidersRequest,
};
use modula_test_support::Harness;
use tonic::Code;

mod common;

#[tokio::test]
async fn provider_agent_crud() -> Result<()> {
    let h = Harness::start().await?;
    let ws = common::fresh_workspace(&h, "demo").await?;

    let provider_dir = h.modula_dir.join("fake-provider");
    std::fs::create_dir_all(&provider_dir)?;

    // Create provider → returns UUID.
    let p1_id = common::create_provider(&h, &ws, "p1", &provider_dir).await?;
    assert_eq!(p1_id.len(), 36, "expected UUID provider id");

    // Listing surfaces it (alongside the default `claude` provider seeded on
    // workspace create).
    let names: Vec<String> = h
        .providers()
        .list(ListProvidersRequest {
            workspace_id: ws.clone(),
        })
        .await?
        .into_inner()
        .providers
        .into_iter()
        .map(|p| p.name)
        .collect();
    assert!(
        names.contains(&"p1".to_string()),
        "p1 missing from {names:?}"
    );

    // Agent referencing a missing provider UUID → InvalidArgument.
    let err = h
        .agents()
        .create(agent_req(
            &ws,
            "test-agent",
            "00000000-0000-0000-0000-000000000000",
            &[],
        ))
        .await
        .expect_err("missing provider must be rejected");
    assert_eq!(err.code(), Code::InvalidArgument);

    // Create agent → returns UUID.
    let agent_id = h
        .agents()
        .create(agent_req(&ws, "test-agent", &p1_id, &[]))
        .await?
        .into_inner()
        .id;
    assert_eq!(agent_id.len(), 36, "expected UUID agent id");

    // Provider in use → can't delete (Conflict → AlreadyExists).
    let err = h
        .providers()
        .delete(DeleteProviderRequest {
            workspace_id: ws.clone(),
            provider_id: p1_id.clone(),
        })
        .await
        .expect_err("in-use provider must not delete");
    assert_eq!(err.code(), Code::AlreadyExists);

    // Delete agent first, then provider.
    h.agents()
        .delete(DeleteAgentRequest {
            workspace_id: ws.clone(),
            agent_id,
        })
        .await?;
    h.providers()
        .delete(DeleteProviderRequest {
            workspace_id: ws.clone(),
            provider_id: p1_id,
        })
        .await?;

    // Recreate provider + agent (with rules) and confirm config surfaces it.
    let p1_id2 = common::create_provider(&h, &ws, "p1", &provider_dir).await?;
    let agent2_id = h
        .agents()
        .create(agent_req(
            &ws,
            "test-agent",
            &p1_id2,
            &["event.type == 'task.create'"],
        ))
        .await?
        .into_inner()
        .id;

    let cfg_names: Vec<String> = h
        .agents()
        .get_config(GetAgentConfigRequest {
            workspace_id: ws.clone(),
        })
        .await?
        .into_inner()
        .agents
        .into_iter()
        .map(|a| a.name)
        .collect();
    assert!(cfg_names.contains(&"test-agent".to_string()));

    // The agent round-trips its rules array when fetched by UUID.
    let detail = h
        .agents()
        .get(GetAgentRequest {
            workspace_id: ws.clone(),
            agent_id: agent2_id,
        })
        .await?
        .into_inner();
    assert_eq!(
        detail.rules,
        vec!["event.type == 'task.create'".to_string()]
    );

    Ok(())
}

fn agent_req(ws: &str, name: &str, provider_id: &str, rules: &[&str]) -> CreateAgentRequest {
    CreateAgentRequest {
        workspace_id: ws.to_string(),
        name: name.to_string(),
        description: "test".into(),
        provider_id: provider_id.to_string(),
        model: None,
        manual: true,
        schedule: None,
        rules: rules.iter().map(|r| r.to_string()).collect(),
        args: vec![],
        prompt: "hello".into(),
        spawn_per_variant: false,
        skills: vec![],
    }
}
