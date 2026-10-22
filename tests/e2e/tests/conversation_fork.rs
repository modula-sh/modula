//! E2E: cloning a conversation at a message. The clone holds the transcript up
//! to that message and nothing after; its first turn forks the provider's
//! session there when the CLI can, and otherwise starts a session from the
//! transcript.

use anyhow::Result;
use modula_rpc::v1::{
    Conversation, CreateConversationRequest, CreateProviderRequest, ForkConversationRequest,
    GetConversationRequest, SendMessageRequest,
};
use modula_test_support::Harness;
use serde_json::json;
use tonic::Code;

use modula_test_support::fixtures as common;

async fn conversation(h: &Harness, ws: &str, provider_type: &str) -> Result<String> {
    let cfg_dir = h.modula_dir.join(format!("fake-{provider_type}"));
    std::fs::create_dir_all(&cfg_dir)?;
    let provider_id = h
        .providers()
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
        .id;
    Ok(h.conversations()
        .create(CreateConversationRequest {
            workspace_id: ws.to_string(),
            provider_id,
            title: None,
            model: None,
            context: None,
            mode: None,
        })
        .await?
        .into_inner()
        .id)
}

/// Send one message and wait for the turn to end.
async fn send(h: &Harness, ws: &str, conv_id: &str, message: &str) -> Result<()> {
    let mut stream = h
        .conversations()
        .send(SendMessageRequest {
            workspace_id: ws.to_string(),
            conversation_id: conv_id.to_string(),
            message: message.to_string(),
            model: None,
        })
        .await?
        .into_inner();
    while stream.message().await?.is_some() {}
    Ok(())
}

async fn get(h: &Harness, ws: &str, conv_id: &str) -> Result<Conversation> {
    Ok(h.conversations()
        .get(GetConversationRequest {
            workspace_id: ws.to_string(),
            conversation_id: conv_id.to_string(),
            message_limit: None,
        })
        .await?
        .into_inner())
}

async fn fork(h: &Harness, ws: &str, conv_id: &str, through_seq: Option<u32>) -> Result<String> {
    Ok(h.conversations()
        .fork(ForkConversationRequest {
            workspace_id: ws.to_string(),
            conversation_id: conv_id.to_string(),
            through_seq,
        })
        .await?
        .into_inner()
        .id)
}

/// The provider's argv on its most recent launch.
fn last_argv(h: &Harness, ws: &str) -> Vec<String> {
    let log = std::fs::read_to_string(h.workspace_path(ws).join("mock-claude-argv.jsonl"))
        .unwrap_or_default();
    log.lines()
        .last()
        .and_then(|line| serde_json::from_str(line).ok())
        .unwrap_or_default()
}

fn transcript(conv: &Conversation) -> Vec<(&str, &str)> {
    conv.messages
        .iter()
        .map(|m| (m.role.as_str(), m.content.as_str()))
        .collect()
}

#[tokio::test]
async fn claude_clone_forks_the_session_at_the_chosen_reply() -> Result<()> {
    let recipe = json!({ "stream": [
        {"type": "system", "subtype": "init", "session_id": "announced-session"},
        {"type": "stream_event", "event": {"type": "content_block_delta", "delta": {"text": "answer"}}},
        {"type": "assistant", "uuid": "entry-1", "parent_tool_use_id": null,
         "message": {"content": [{"type": "text", "text": "answer"}]}},
        {"type": "result", "subtype": "success"},
    ]})
    .to_string();
    let h = Harness::start_with_env(&[("MODULA_MOCK_RECIPE", &recipe)]).await?;
    let ws = common::fresh_workspace(&h, "demo").await?;
    let source = conversation(&h, &ws, "claude").await?;
    send(&h, &ws, &source, "first").await?;
    send(&h, &ws, &source, "second").await?;
    let source_session = get(&h, &ws, &source).await?.session_id.expect("session");

    // Cut after the first reply: the clone holds one exchange and no session.
    let clone = fork(&h, &ws, &source, Some(1)).await?;
    let detail = get(&h, &ws, &clone).await?;
    assert_eq!(
        transcript(&detail),
        [("user", "first"), ("assistant", "answer")]
    );
    assert_eq!(detail.session_id, None);
    assert_eq!(
        get(&h, &ws, &source).await?.messages.len(),
        4,
        "source untouched"
    );

    // Its first turn resumes the source session into a new one, cut at that reply.
    send(&h, &ws, &clone, "branch").await?;
    let argv = last_argv(&h, &ws);
    let at = argv.iter().position(|a| a == "--resume").expect("--resume");
    assert_eq!(
        argv[at..at + 5],
        [
            "--resume",
            source_session.as_str(),
            "--fork-session",
            "--resume-session-at",
            "entry-1"
        ]
    );
    let detail = get(&h, &ws, &clone).await?;
    assert_eq!(detail.session_id.as_deref(), Some("announced-session"));
    assert_eq!(
        transcript(&detail),
        [
            ("user", "first"),
            ("assistant", "answer"),
            ("user", "branch"),
            ("assistant", "answer"),
        ]
    );

    // From then on it is an ordinary conversation on its own session.
    send(&h, &ws, &clone, "again").await?;
    let argv = last_argv(&h, &ws);
    let at = argv.iter().position(|a| a == "--resume").expect("--resume");
    assert_eq!(argv[at + 1], "announced-session");
    assert!(!argv.contains(&"--fork-session".to_string()));
    Ok(())
}

#[tokio::test]
async fn codex_clone_forks_the_thread_through_the_chosen_turn() -> Result<()> {
    let recipe = json!({ "stream": [
        {"method": "turn/started", "params": {"turn": {"id": "turn-1"}}},
        {"method": "item/agentMessage/delta", "params": {"delta": "answer"}},
        {"method": "turn/completed", "params": {"turn": {"status": "completed"}}},
    ]})
    .to_string();
    let h = Harness::start_with_env(&[("MODULA_MOCK_RECIPE", &recipe)]).await?;
    let ws = common::fresh_workspace(&h, "demo").await?;
    let source = conversation(&h, &ws, "codex").await?;
    send(&h, &ws, &source, "first").await?;

    let clone = fork(&h, &ws, &source, Some(1)).await?;
    send(&h, &ws, &clone, "branch").await?;
    let detail = get(&h, &ws, &clone).await?;
    assert_eq!(
        detail.session_id.as_deref(),
        Some("fork-of-mock-thread-at-turn-1")
    );
    assert_eq!(detail.messages.len(), 4);
    Ok(())
}

/// Gemini's CLI can't fork a session, so the clone starts a new one whose
/// first prompt carries the transcript it was cut from.
#[tokio::test]
async fn gemini_clone_starts_a_session_from_the_transcript() -> Result<()> {
    let recipe = json!({ "stream": [
        {"type": "init", "session_id": "s1", "model": "auto"},
        {"type": "message", "role": "assistant", "content": "the answer", "delta": true},
        {"type": "result", "status": "success", "stats": {}},
    ]})
    .to_string();
    let h = Harness::start_with_env(&[("MODULA_MOCK_RECIPE", &recipe)]).await?;
    let ws = common::fresh_workspace(&h, "demo").await?;
    let source = conversation(&h, &ws, "gemini").await?;
    send(&h, &ws, &source, "first question").await?;
    send(&h, &ws, &source, "second question").await?;

    let clone = fork(&h, &ws, &source, Some(1)).await?;
    send(&h, &ws, &clone, "branch question").await?;

    let argv = last_argv(&h, &ws);
    assert!(!argv.contains(&"--resume".to_string()), "{argv:?}");
    let prompt = argv
        .iter()
        .find_map(|a| a.strip_prefix("--prompt="))
        .expect("prompt");
    assert!(prompt.contains("<conversation_history>"), "{prompt}");
    assert!(prompt.contains("[user]\nfirst question"), "{prompt}");
    assert!(prompt.contains("[assistant]\nthe answer"), "{prompt}");
    assert!(!prompt.contains("second question"), "{prompt}");
    assert!(prompt.ends_with("branch question"), "{prompt}");
    Ok(())
}

#[tokio::test]
async fn clone_keeps_nothing_when_cut_before_the_first_message() -> Result<()> {
    let h = Harness::start().await?;
    let ws = common::fresh_workspace(&h, "demo").await?;
    let source = conversation(&h, &ws, "claude").await?;
    send(&h, &ws, &source, "first").await?;

    let clone = fork(&h, &ws, &source, None).await?;
    let detail = get(&h, &ws, &clone).await?;
    assert!(detail.messages.is_empty());
    assert_eq!(detail.provider_id, get(&h, &ws, &source).await?.provider_id);

    let past_the_end = h
        .conversations()
        .fork(ForkConversationRequest {
            workspace_id: ws.clone(),
            conversation_id: source.clone(),
            through_seq: Some(2),
        })
        .await;
    assert_eq!(past_the_end.unwrap_err().code(), Code::InvalidArgument);
    Ok(())
}
