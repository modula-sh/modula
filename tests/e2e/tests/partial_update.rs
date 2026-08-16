//! Partial-update (PATCH) semantics across the surviving update RPCs (agents,
//! providers, projects) over gRPC IPC. Each test creates a fully-populated
//! resource, sends an update that touches a single field, and asserts the
//! unsent fields were preserved verbatim. The proto's `clear_*` flags cover
//! the "user cleared an optional field" path that REST expressed with `null`.

use anyhow::Result;
use modula_rpc::v1::{
    project_service_client::ProjectServiceClient, AgentArgDef, AgentSchedule, CreateAgentRequest,
    CreateProjectRequest, CreateProviderRequest, GetAgentRequest, GetProjectRequest,
    GetProviderRequest, UpdateAgentRequest, UpdateProjectRequest, UpdateProviderRequest,
};
use modula_test_support::Harness;

mod common;

// ─── agents ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn agent_partial_update_preserves_unsent_fields() -> Result<()> {
    let h = Harness::start().await?;
    let ws = common::fresh_workspace(&h, "demo").await?;
    let provider_dir = h.modula_dir.join("fake-provider");
    std::fs::create_dir_all(&provider_dir)?;
    let p1_id = common::create_provider(&h, &ws, "p1", &provider_dir).await?;

    let agent_id = h
        .agents()
        .create(CreateAgentRequest {
            workspace_id: ws.clone(),
            name: "agent-1".into(),
            description: "original description".into(),
            provider_id: p1_id.clone(),
            model: Some("sonnet".into()),
            manual: true,
            schedule: Some(AgentSchedule {
                cron: "*/5 * * * *".into(),
                timezone: "UTC".into(),
                enabled: false,
            }),
            rules: vec!["event.type == 'task.create'".into()],
            args: vec![AgentArgDef {
                flag: "--task".into(),
                required: true,
                help: None,
            }],
            prompt: "original prompt body".into(),
            spawn_per_variant: false,
            skills: vec![],
        })
        .await?
        .into_inner()
        .id;

    // Update touches only `manual`. Description, model, args, rules must all
    // survive; with `update_rules`/`update_args` unset the lists are preserved,
    // and the prompt (not in the request) is untouched.
    h.agents()
        .update(UpdateAgentRequest {
            workspace_id: ws.clone(),
            agent_id: agent_id.clone(),
            manual: Some(false),
            ..Default::default()
        })
        .await?;

    let agent = h
        .agents()
        .get(GetAgentRequest {
            workspace_id: ws.clone(),
            agent_id: agent_id.clone(),
        })
        .await?
        .into_inner();
    assert!(!agent.manual, "manual should be updated");
    assert_eq!(agent.description, "original description");
    assert_eq!(agent.model.as_deref(), Some("sonnet"));
    assert_eq!(agent.provider_id, p1_id);
    assert_eq!(agent.args.len(), 1);
    assert_eq!(agent.rules, vec!["event.type == 'task.create'".to_string()]);
    assert_eq!(agent.prompt.as_deref(), Some("original prompt body"));
    Ok(())
}

#[tokio::test]
async fn agent_partial_update_clears_model() -> Result<()> {
    let h = Harness::start().await?;
    let ws = common::fresh_workspace(&h, "demo").await?;
    let provider_dir = h.modula_dir.join("fake-provider");
    std::fs::create_dir_all(&provider_dir)?;
    let p1_id = common::create_provider(&h, &ws, "p1", &provider_dir).await?;

    let agent_id = h
        .agents()
        .create(CreateAgentRequest {
            workspace_id: ws.clone(),
            name: "agent-2".into(),
            description: "test".into(),
            provider_id: p1_id,
            model: Some("sonnet".into()),
            manual: true,
            schedule: None,
            rules: vec![],
            args: vec![],
            prompt: "hello".into(),
            spawn_per_variant: false,
            skills: vec![],
        })
        .await?
        .into_inner()
        .id;

    h.agents()
        .update(UpdateAgentRequest {
            workspace_id: ws.clone(),
            agent_id: agent_id.clone(),
            clear_model: true,
            ..Default::default()
        })
        .await?;

    let agent = h
        .agents()
        .get(GetAgentRequest {
            workspace_id: ws.clone(),
            agent_id,
        })
        .await?
        .into_inner();
    assert_eq!(agent.model, None);
    Ok(())
}

// ─── providers ───────────────────────────────────────────────────────────

#[tokio::test]
async fn provider_partial_update_preserves_unsent_fields() -> Result<()> {
    let h = Harness::start().await?;
    let ws = common::fresh_workspace(&h, "demo").await?;
    let config_dir_a = h.modula_dir.join("provider-a");
    std::fs::create_dir_all(&config_dir_a)?;

    let p_id = h
        .providers()
        .create(CreateProviderRequest {
            workspace_id: ws.clone(),
            name: "Provider A".into(),
            r#type: String::new(),
            config_dir: config_dir_a.to_string_lossy().to_string(),
            description: Some("original description".into()),
            mcp_servers: vec![],
        })
        .await?
        .into_inner()
        .id;

    h.providers()
        .update(UpdateProviderRequest {
            workspace_id: ws.clone(),
            provider_id: p_id.clone(),
            name: Some("Provider A (renamed)".into()),
            ..Default::default()
        })
        .await?;

    let detail = h
        .providers()
        .get(GetProviderRequest {
            workspace_id: ws.clone(),
            provider_id: p_id,
        })
        .await?
        .into_inner();
    assert_eq!(detail.name, "Provider A (renamed)");
    assert_eq!(detail.description.as_deref(), Some("original description"));
    assert_eq!(detail.config_dir, config_dir_a.to_string_lossy());
    Ok(())
}

#[tokio::test]
async fn provider_partial_update_clears_description() -> Result<()> {
    let h = Harness::start().await?;
    let ws = common::fresh_workspace(&h, "demo").await?;
    let config_dir_b = h.modula_dir.join("provider-b");
    std::fs::create_dir_all(&config_dir_b)?;

    let p_id = h
        .providers()
        .create(CreateProviderRequest {
            workspace_id: ws.clone(),
            name: "Provider B".into(),
            r#type: String::new(),
            config_dir: config_dir_b.to_string_lossy().to_string(),
            description: Some("kill me".into()),
            mcp_servers: vec![],
        })
        .await?
        .into_inner()
        .id;

    h.providers()
        .update(UpdateProviderRequest {
            workspace_id: ws.clone(),
            provider_id: p_id.clone(),
            clear_description: true,
            ..Default::default()
        })
        .await?;

    let detail = h
        .providers()
        .get(GetProviderRequest {
            workspace_id: ws.clone(),
            provider_id: p_id,
        })
        .await?
        .into_inner();
    assert_eq!(detail.description, None);
    assert_eq!(detail.name, "Provider B");
    Ok(())
}

// ─── projects ────────────────────────────────────────────────────────────

#[tokio::test]
async fn project_partial_update_preserves_unsent_fields() -> Result<()> {
    let h = Harness::start().await?;
    let ws = common::fresh_workspace(&h, "demo").await?;
    let mut projects = ProjectServiceClient::new(h.channel());

    let proj_id = projects
        .create(CreateProjectRequest {
            workspace_id: ws.clone(),
            name: "myrepo".into(),
            path: "/tmp/myrepo".into(),
            base_branch: "main".into(),
        })
        .await?
        .into_inner()
        .id;

    projects
        .update(UpdateProjectRequest {
            workspace_id: ws.clone(),
            project_id: proj_id.clone(),
            base_branch: Some("develop".into()),
            ..Default::default()
        })
        .await?;

    let project = projects
        .get(GetProjectRequest {
            workspace_id: ws.clone(),
            project_id: proj_id,
        })
        .await?
        .into_inner();
    assert_eq!(project.name, "myrepo");
    assert_eq!(project.path, "/tmp/myrepo");
    assert_eq!(project.base_branch, "develop");
    Ok(())
}
