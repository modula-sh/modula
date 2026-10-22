//! E2E: terminal conversations. `mode` is fixed at create, and
//! `TerminalLaunch` describes the TUI to spawn without spawning it.

use anyhow::Result;
use modula_rpc::v1::{
    CreateConversationRequest, CreateProviderRequest, GetConversationRequest, LaunchSpec,
    SendMessageRequest, TerminalLaunchRequest,
};
use modula_test_support::fixtures as common;
use modula_test_support::Harness;
use tonic::Code;

async fn provider(h: &Harness, ws: &str, provider_type: &str) -> Result<String> {
    let cfg_dir = h.modula_dir.join(format!("fake-{provider_type}"));
    std::fs::create_dir_all(&cfg_dir)?;
    Ok(h.providers()
        .create(CreateProviderRequest {
            workspace_id: ws.to_string(),
            name: provider_type.to_string(),
            r#type: provider_type.to_string(),
            config_dir: cfg_dir.to_string_lossy().to_string(),
            description: None,
            mcp_servers: vec![],
        })
        .await?
        .into_inner()
        .id)
}

async fn create(
    h: &Harness,
    ws: &str,
    provider_id: &str,
    mode: Option<&str>,
) -> Result<String, tonic::Status> {
    Ok(h.conversations()
        .create(CreateConversationRequest {
            workspace_id: ws.to_string(),
            provider_id: provider_id.to_string(),
            title: None,
            model: None,
            context: None,
            mode: mode.map(str::to_string),
        })
        .await?
        .into_inner()
        .id)
}

async fn get(h: &Harness, ws: &str, id: &str) -> Result<modula_rpc::v1::Conversation> {
    Ok(h.conversations()
        .get(GetConversationRequest {
            workspace_id: ws.to_string(),
            conversation_id: id.to_string(),
            message_limit: None,
        })
        .await?
        .into_inner())
}

async fn launch(
    h: &Harness,
    ws: &str,
    id: &str,
    prompt: Option<&str>,
) -> Result<LaunchSpec, tonic::Status> {
    Ok(h.conversations()
        .terminal_launch(TerminalLaunchRequest {
            workspace_id: ws.to_string(),
            conversation_id: id.to_string(),
            prompt: prompt.map(str::to_string),
        })
        .await?
        .into_inner())
}

#[tokio::test]
async fn create_stores_mode_and_coerces_unsupported_providers() -> Result<()> {
    let h = Harness::start().await?;
    let ws = common::fresh_workspace(&h, "demo").await?;
    let claude = provider(&h, &ws, "claude").await?;
    let codex = provider(&h, &ws, "codex").await?;

    let default = create(&h, &ws, &claude, None).await?;
    assert_eq!(get(&h, &ws, &default).await?.mode, "chat");

    let term = create(&h, &ws, &claude, Some("terminal")).await?;
    assert_eq!(get(&h, &ws, &term).await?.mode, "terminal");

    let coerced = create(&h, &ws, &codex, Some("terminal")).await?;
    assert_eq!(get(&h, &ws, &coerced).await?.mode, "chat");

    let err = create(&h, &ws, &claude, Some("tty"))
        .await
        .expect_err("unknown mode must be rejected");
    assert_eq!(err.code(), Code::InvalidArgument);

    let snap = common::snapshot(&h, &ws).await?;
    let modes: Vec<_> = snap["conversations"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["id"] == term)
        .map(|c| c["mode"].clone())
        .collect();
    assert_eq!(modes, vec![serde_json::json!("terminal")]);
    Ok(())
}

#[tokio::test]
async fn launch_presets_the_session_then_resumes_it() -> Result<()> {
    let h = Harness::start().await?;
    let ws = common::fresh_workspace(&h, "demo").await?;
    let claude = provider(&h, &ws, "claude").await?;
    let id = create(&h, &ws, &claude, Some("terminal")).await?;

    let first = launch(&h, &ws, &id, Some("Fix the build")).await?;
    let conv = get(&h, &ws, &id).await?;
    let sid = conv.session_id.expect("first launch stores the session id");
    assert_eq!(conv.title, "Fix the build");
    let args = &first.argv[1..];
    let at = args.iter().position(|a| a == "--session-id").unwrap();
    assert_eq!(args[at + 1], sid);
    assert!(args.last().unwrap().ends_with("Fix the build"));
    assert!(!args.iter().any(|a| a == "-p" || a == "stream-json"));
    assert_eq!(first.env["MODULA_WORKSPACE"], ws);
    assert!(first.env.contains_key("CLAUDE_CONFIG_DIR"));
    assert!(first.env.contains_key("PATH"));
    assert!(!first.cwd.is_empty());

    let second = launch(&h, &ws, &id, Some("ignored")).await?;
    let args = &second.argv[1..];
    let at = args.iter().position(|a| a == "--resume").unwrap();
    assert_eq!(args[at + 1], sid);
    assert!(!args.iter().any(|a| a.contains("ignored")));
    assert_eq!(get(&h, &ws, &id).await?.session_id, Some(sid));
    Ok(())
}

#[tokio::test]
async fn chat_and_terminal_reject_each_others_entry_points() -> Result<()> {
    let h = Harness::start().await?;
    let ws = common::fresh_workspace(&h, "demo").await?;
    let claude = provider(&h, &ws, "claude").await?;

    let chat = create(&h, &ws, &claude, None).await?;
    let err = launch(&h, &ws, &chat, None)
        .await
        .expect_err("a chat conversation has no terminal");
    assert_eq!(err.code(), Code::InvalidArgument);

    let term = create(&h, &ws, &claude, Some("terminal")).await?;
    let err = h
        .conversations()
        .send(SendMessageRequest {
            workspace_id: ws.clone(),
            conversation_id: term.clone(),
            message: "hi".into(),
            model: None,
        })
        .await
        .expect_err("a terminal conversation takes no Send");
    assert_eq!(err.code(), Code::InvalidArgument);
    assert_eq!(get(&h, &ws, &term).await?.message_count, 0);
    Ok(())
}
