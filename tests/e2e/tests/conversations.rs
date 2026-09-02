//! E2E: conversation CRUD plus the `ConversationService.Send` server stream,
//! over gRPC IPC. Replaces the former SSE `/send` path — the stream now carries
//! typed `ConvEvent`s and ends at the terminal `Done`.

use anyhow::Result;
use modula_rpc::v1::{
    conv_event, AttachConversationRequest, CreateConversationRequest, DeleteConversationRequest,
    GetConversationRequest, ListConversationsRequest, SendMessageRequest,
};
use modula_test_support::Harness;
use tonic::Code;

use modula_test_support::fixtures as common;

/// Drain a `Send`/`Attach` stream to its terminal event, returning which event
/// kinds were seen. The stream ends on its own at `Done`/`Error`, so dropping
/// it is unnecessary — but doing so detaches without cancelling the run.
async fn drain(
    mut stream: tonic::Streaming<modula_rpc::v1::ConvEvent>,
) -> Result<(bool, bool, bool)> {
    let (mut delta, mut done, mut error) = (false, false, false);
    while let Some(ev) = stream.message().await? {
        match ev.event {
            Some(conv_event::Event::Delta(_)) => delta = true,
            Some(conv_event::Event::Done(_)) => done = true,
            Some(conv_event::Event::Error(_)) => error = true,
            _ => {}
        }
    }
    Ok((delta, done, error))
}

#[tokio::test]
async fn conversations_crud_and_stream() -> Result<()> {
    let h = Harness::start().await?;
    let ws = common::fresh_workspace(&h, "demo").await?;

    // ProviderService::runtime_from_provider validates config_dir on disk at
    // send time, so point a claude provider at a real dir instead of the
    // default ~/.claude.
    let cfg_dir = h.modula_dir.join("fake-claude");
    std::fs::create_dir_all(&cfg_dir)?;
    let provider_id = common::create_provider(&h, &ws, "Claude", &cfg_dir).await?;

    // 1. Create a conversation.
    let conv_id = h
        .conversations()
        .create(CreateConversationRequest {
            workspace_id: ws.clone(),
            provider_id,
            title: None,
            model: None,
            context: None,
        })
        .await?
        .into_inner()
        .id;
    assert!(!conv_id.is_empty());

    // 2. List → conversation appears.
    let list = h
        .conversations()
        .list(ListConversationsRequest {
            workspace_id: ws.clone(),
        })
        .await?
        .into_inner()
        .conversations;
    assert!(
        list.iter().any(|c| c.id == conv_id),
        "conversation not in list"
    );

    // 3. Get detail → no messages, no session yet.
    let detail = h
        .conversations()
        .get(GetConversationRequest {
            workspace_id: ws.clone(),
            conversation_id: conv_id.clone(),
        })
        .await?
        .into_inner();
    assert_eq!(detail.id, conv_id);
    assert!(detail.messages.is_empty());
    assert!(detail.session_id.is_none());

    // 4. Send → server stream with at least one delta and a terminal done.
    let stream = h
        .conversations()
        .send(SendMessageRequest {
            workspace_id: ws.clone(),
            conversation_id: conv_id.clone(),
            message: "What does this workspace do?".to_string(),
            model: None,
        })
        .await?
        .into_inner();
    let (delta, done, error) = drain(stream).await?;
    assert!(delta, "no delta event");
    assert!(done, "no done event");
    assert!(!error, "unexpected error event");

    // 5. Get → user + assistant messages persisted, session_id captured.
    let detail = h
        .conversations()
        .get(GetConversationRequest {
            workspace_id: ws.clone(),
            conversation_id: conv_id.clone(),
        })
        .await?
        .into_inner();
    assert!(
        detail.messages.len() >= 2,
        "expected at least user + assistant, got {:?}",
        detail.messages
    );
    assert_eq!(detail.messages[0].role, "user");
    assert_eq!(detail.messages[0].content, "What does this workspace do?");
    assert_eq!(detail.messages[1].role, "assistant");
    assert!(
        !detail.messages[1].content.is_empty(),
        "assistant content is empty"
    );
    let session_id = detail.session_id.expect("session_id populated");
    assert!(!session_id.is_empty());

    // 6. Second send → resume path; should pass --resume <session_id> to the CLI.
    let stream = h
        .conversations()
        .send(SendMessageRequest {
            workspace_id: ws.clone(),
            conversation_id: conv_id.clone(),
            message: "Tell me more.".to_string(),
            model: None,
        })
        .await?
        .into_inner();
    let (_, done, _) = drain(stream).await?;
    assert!(done, "no done on second send");

    // mock-claude appends its argv as a JSON line per invocation.
    let argv_log = h.workspace_path(&ws).join("mock-claude-argv.jsonl");
    let log_content = std::fs::read_to_string(&argv_log).unwrap_or_default();
    let invocations: Vec<Vec<String>> = log_content
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    assert!(
        invocations.len() >= 2,
        "expected at least 2 mock-claude invocations, got {}: {log_content}",
        invocations.len()
    );
    let second_argv = &invocations[1];
    assert!(
        second_argv.contains(&"--resume".to_string()),
        "second invocation missing --resume: {second_argv:?}"
    );
    assert!(
        second_argv.contains(&session_id),
        "second invocation missing session_id {session_id}: {second_argv:?}"
    );

    // 7. Conversation still in the snapshot.
    let snap = common::snapshot(&h, &ws).await?;
    let convs = snap["conversations"]
        .as_array()
        .expect("conversations in snapshot");
    assert!(
        convs
            .iter()
            .any(|c| c["id"].as_str() == Some(conv_id.as_str())),
        "conversation not in snapshot"
    );

    // 8. Delete, then Get → NotFound.
    h.conversations()
        .delete(DeleteConversationRequest {
            workspace_id: ws.clone(),
            conversation_id: conv_id.clone(),
        })
        .await?;
    let gone = h
        .conversations()
        .get(GetConversationRequest {
            workspace_id: ws.clone(),
            conversation_id: conv_id.clone(),
        })
        .await;
    assert_eq!(gone.unwrap_err().code(), Code::NotFound);

    Ok(())
}

#[tokio::test]
async fn conversation_empty_message_rejected() -> Result<()> {
    let h = Harness::start().await?;
    let ws = common::fresh_workspace(&h, "demo").await?;

    let cfg_dir = h.modula_dir.join("fake-claude");
    std::fs::create_dir_all(&cfg_dir)?;
    let provider_id = common::create_provider(&h, &ws, "Claude", &cfg_dir).await?;

    let conv_id = h
        .conversations()
        .create(CreateConversationRequest {
            workspace_id: ws.clone(),
            provider_id,
            title: None,
            model: None,
            context: None,
        })
        .await?
        .into_inner()
        .id;

    let err = h
        .conversations()
        .send(SendMessageRequest {
            workspace_id: ws.clone(),
            conversation_id: conv_id,
            message: String::new(),
            model: None,
        })
        .await
        .expect_err("empty message must be rejected");
    assert_eq!(err.code(), Code::InvalidArgument);

    Ok(())
}

/// Detach/reattach (A5): dropping a `Send` stream mid-run must **detach** the
/// client without cancelling the underlying provider run, and a second client
/// must be able to `Attach`, replay the buffered output, and resume to the run's
/// completion. The recipe keeps the provider alive (`sleep_ms`) so the run is
/// still in-flight when the first client detaches.
#[tokio::test]
async fn detach_does_not_kill_run_and_reattach_resumes() -> Result<()> {
    // init (so a session is captured) + one delta, then the provider sleeps with
    // stdout still open — the run stays in-flight, no terminal event yet.
    let recipe = serde_json::json!({
        "stream": [
            {"type": "system", "subtype": "init", "session_id": "detach-session"},
            {"type": "stream_event", "event": {"type": "content_block_delta",
                "delta": {"text": "partial response"}}}
        ],
        "sleep_ms": 3000
    })
    .to_string();
    let h = Harness::start_with_env(&[("MODULA_MOCK_RECIPE", &recipe)]).await?;
    let ws = common::fresh_workspace(&h, "demo").await?;

    let cfg_dir = h.modula_dir.join("fake-claude");
    std::fs::create_dir_all(&cfg_dir)?;
    let provider_id = common::create_provider(&h, &ws, "Claude", &cfg_dir).await?;

    let conv_id = h
        .conversations()
        .create(CreateConversationRequest {
            workspace_id: ws.clone(),
            provider_id,
            title: None,
            model: None,
            context: None,
        })
        .await?
        .into_inner()
        .id;

    // Client A: send, read up to the first delta, then DROP the stream mid-run.
    let mut a = h
        .conversations()
        .send(SendMessageRequest {
            workspace_id: ws.clone(),
            conversation_id: conv_id.clone(),
            message: "Begin.".to_string(),
            model: None,
        })
        .await?
        .into_inner();
    let mut a_saw_delta = false;
    while let Some(ev) = a.message().await? {
        if matches!(ev.event, Some(conv_event::Event::Delta(_))) {
            a_saw_delta = true;
            break;
        }
    }
    assert!(a_saw_delta, "first client never received a live delta");
    drop(a); // detach — must NOT cancel the still-sleeping provider run.

    // Client B: attach to the in-flight run; the replay buffer carries the delta
    // already emitted, then the live Done arrives once the provider exits.
    let b = h
        .conversations()
        .attach(AttachConversationRequest {
            workspace_id: ws.clone(),
            conversation_id: conv_id.clone(),
        })
        .await?
        .into_inner();
    let (delta, done, error) = drain(b).await?;
    assert!(delta, "reattached client did not replay the buffered delta");
    assert!(done, "reattached client did not see the run complete");
    assert!(!error, "run errored — detach must not have killed it");

    // The run reached completion (not cancelled by A's detach): the assistant
    // message was persisted before Done, so it is visible now.
    let detail = h
        .conversations()
        .get(GetConversationRequest {
            workspace_id: ws.clone(),
            conversation_id: conv_id.clone(),
        })
        .await?
        .into_inner();
    assert!(
        detail
            .messages
            .iter()
            .any(|m| m.role == "assistant" && m.content.contains("partial response")),
        "assistant output not persisted — detach cancelled the run: {:?}",
        detail.messages
    );

    Ok(())
}
