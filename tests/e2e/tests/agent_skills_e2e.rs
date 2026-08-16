//! Agent skills catalog + opt-in round-trip over gRPC IPC.

use anyhow::Result;
use modula_rpc::v1::{CreateAgentRequest, GetAgentRequest, ListSkillsRequest, UpdateAgentRequest};
use modula_test_support::Harness;
use tonic::Code;

mod common;

#[tokio::test]
async fn agent_skills_catalog_and_roundtrip() -> Result<()> {
    let h = Harness::start().await?;
    let ws = common::fresh_workspace(&h, "demo").await?;

    // Catalog returns the seeded skills with hidden flags; AgentSkill carries no
    // prompt body, so the prompt can't leak through this surface.
    let skills = h
        .agents()
        .list_skills(ListSkillsRequest {
            workspace_id: ws.clone(),
        })
        .await?
        .into_inner()
        .skills;
    let slugs: Vec<&str> = skills.iter().map(|s| s.slug.as_str()).collect();
    assert!(
        slugs.contains(&"engine-api"),
        "missing engine-api: {slugs:?}"
    );
    assert!(slugs.contains(&"ai-wiki"), "missing ai-wiki: {slugs:?}");
    assert!(
        skills
            .iter()
            .find(|s| s.slug == "engine-api")
            .unwrap()
            .hidden
    );
    assert!(!skills.iter().find(|s| s.slug == "ai-wiki").unwrap().hidden);

    // Create an agent opting into an optional skill — round-trips on Get.
    let provider_dir = h.modula_dir.join("fake-provider");
    std::fs::create_dir_all(&provider_dir)?;
    let provider_id = common::create_provider(&h, &ws, "p1", &provider_dir).await?;

    let agent_id = h
        .agents()
        .create(skilled_agent_req(
            &ws,
            "skilled-agent",
            &provider_id,
            &["ai-wiki"],
        ))
        .await?
        .into_inner()
        .id;

    let detail = h
        .agents()
        .get(GetAgentRequest {
            workspace_id: ws.clone(),
            agent_id: agent_id.clone(),
        })
        .await?
        .into_inner();
    assert_eq!(detail.skills, vec!["ai-wiki".to_string()]);

    // Patching skills round-trips and dedupes.
    h.agents()
        .update(UpdateAgentRequest {
            workspace_id: ws.clone(),
            agent_id: agent_id.clone(),
            skills: vec!["ai-wiki".into(), "ai-wiki".into()],
            update_skills: true,
            ..Default::default()
        })
        .await?;
    let detail = h
        .agents()
        .get(GetAgentRequest {
            workspace_id: ws.clone(),
            agent_id,
        })
        .await?
        .into_inner();
    assert_eq!(detail.skills, vec!["ai-wiki".to_string()]);

    // Unknown skill slug → InvalidArgument.
    let err = h
        .agents()
        .create(skilled_agent_req(
            &ws,
            "bad-agent",
            &provider_id,
            &["does-not-exist"],
        ))
        .await
        .expect_err("unknown skill must be rejected");
    assert_eq!(err.code(), Code::InvalidArgument);

    // Hidden slugs are injected at spawn time and must not be opt-in.
    let err = h
        .agents()
        .create(skilled_agent_req(
            &ws,
            "hidden-agent",
            &provider_id,
            &["engine-api"],
        ))
        .await
        .expect_err("hidden skill must not be opt-in");
    assert_eq!(err.code(), Code::InvalidArgument);

    Ok(())
}

fn skilled_agent_req(
    ws: &str,
    name: &str,
    provider_id: &str,
    skills: &[&str],
) -> CreateAgentRequest {
    CreateAgentRequest {
        workspace_id: ws.to_string(),
        name: name.to_string(),
        description: "test".into(),
        provider_id: provider_id.to_string(),
        model: None,
        manual: true,
        schedule: None,
        rules: vec![],
        args: vec![],
        prompt: "hello".into(),
        spawn_per_variant: false,
        skills: skills.iter().map(|s| s.to_string()).collect(),
    }
}
