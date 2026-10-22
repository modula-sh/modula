//! E2E: what a chat turn leaves in the transcript. Each provider's recorded
//! stream is replayed through the engine; the stored message must hold the
//! text, tool calls and results in the order they happened, and page from the
//! tail.

use anyhow::Result;
use modula_rpc::v1::{
    conv_event, message_part::Part, ChatMessage, CreateConversationRequest, CreateProviderRequest,
    GetConversationRequest, ListMessagesRequest, SendMessageRequest, ToolCall, ToolStatus,
};
use modula_test_support::Harness;
use serde_json::{json, Value};

use modula_test_support::fixtures as common;

/// A recipe that replays a recorded provider stream, one JSON value per line.
fn replay(recorded: &str) -> String {
    let stream: Vec<Value> = recorded
        .lines()
        .map(|line| serde_json::from_str(line).expect("recorded line is JSON"))
        .collect();
    json!({ "stream": stream }).to_string()
}

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
        })
        .await?
        .into_inner()
        .id)
}

/// Send one message and collect the stream's events up to its terminal one.
async fn send(
    h: &Harness,
    ws: &str,
    conv_id: &str,
    message: &str,
) -> Result<Vec<conv_event::Event>> {
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
    let mut events = Vec::new();
    while let Some(ev) = stream.message().await? {
        events.extend(ev.event);
    }
    Ok(events)
}

async fn messages(h: &Harness, ws: &str, conv_id: &str) -> Result<Vec<ChatMessage>> {
    Ok(h.conversations()
        .get(GetConversationRequest {
            workspace_id: ws.to_string(),
            conversation_id: conv_id.to_string(),
            message_limit: None,
        })
        .await?
        .into_inner()
        .messages)
}

fn tools(msg: &ChatMessage) -> Vec<&ToolCall> {
    msg.parts
        .iter()
        .filter_map(|p| match &p.part {
            Some(Part::Tool(t)) => Some(t),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn claude_turn_keeps_its_tools_results_and_stats() -> Result<()> {
    let recipe = replay(include_str!(
        "../../../crates/modula_services/src/providers/fixtures/claude.jsonl"
    ));
    let h = Harness::start_with_env(&[("MODULA_MOCK_RECIPE", &recipe)]).await?;
    let ws = common::fresh_workspace(&h, "demo").await?;
    let conv_id = conversation(&h, &ws, "claude").await?;

    let events = send(&h, &ws, &conv_id, "go").await?;
    let results = events
        .iter()
        .filter(|e| matches!(e, conv_event::Event::ToolResult(_)))
        .count();
    assert_eq!(results, 5, "one live result per tool call");

    let msgs = messages(&h, &ws, &conv_id).await?;
    assert_eq!(msgs.len(), 2);
    let reply = &msgs[1];
    assert_eq!((reply.role.as_str(), reply.seq), ("assistant", 1));

    let calls = tools(reply);
    let names: Vec<&str> = calls.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, ["Read", "Read", "Write", "Bash", "Bash"]);
    assert_eq!(calls[0].output.as_deref(), Some("1\talpha\n2\t"));
    assert_eq!(calls[0].status, ToolStatus::Completed as i32);
    assert_eq!(calls[4].status, ToolStatus::Failed as i32);
    assert!(calls[4]
        .output
        .as_deref()
        .is_some_and(|o| o.contains("No such file or directory")));

    // The reply's text follows the calls it reports on, and is the plain content.
    let Some(Part::Text(text)) = &reply.parts.last().and_then(|p| p.part.clone()) else {
        panic!("expected the reply to end in text: {:?}", reply.parts);
    };
    assert!(text.starts_with("I read `a.txt`"));
    assert_eq!(&reply.content, text);

    let stats = reply.stats.as_ref().expect("turn stats");
    assert_eq!(stats.input_tokens, Some(106_859));
    assert_eq!(stats.output_tokens, Some(538));
    assert!(stats.duration_ms.is_some_and(|ms| ms > 0));
    assert_eq!(stats.model.as_deref(), Some("claude-opus-5-5"));
    Ok(())
}

#[tokio::test]
async fn claude_subagent_calls_sit_under_the_call_that_spawned_them() -> Result<()> {
    let recipe = replay(include_str!(
        "../../../crates/modula_services/src/providers/fixtures/claude_subagent.jsonl"
    ));
    let h = Harness::start_with_env(&[("MODULA_MOCK_RECIPE", &recipe)]).await?;
    let ws = common::fresh_workspace(&h, "demo").await?;
    let conv_id = conversation(&h, &ws, "claude").await?;
    send(&h, &ws, &conv_id, "go").await?;

    let msgs = messages(&h, &ws, &conv_id).await?;
    let calls = tools(&msgs[1]);
    assert_eq!(calls.len(), 3);
    assert_eq!(
        (calls[0].name.as_str(), &calls[0].parent_id),
        ("Agent", &None)
    );
    for call in &calls[1..] {
        assert_eq!(call.name, "Read");
        assert_eq!(call.parent_id.as_ref(), Some(&calls[0].id));
    }
    assert!(msgs[1].content.starts_with("The subagent reported"));
    Ok(())
}

#[tokio::test]
async fn codex_turn_keeps_its_commands_and_their_output() -> Result<()> {
    let recipe = replay(include_str!(
        "../../../crates/modula_services/src/providers/fixtures/codex.jsonl"
    ));
    let h = Harness::start_with_env(&[("MODULA_MOCK_RECIPE", &recipe)]).await?;
    let ws = common::fresh_workspace(&h, "demo").await?;
    let conv_id = conversation(&h, &ws, "codex").await?;
    send(&h, &ws, &conv_id, "go").await?;

    let msgs = messages(&h, &ws, &conv_id).await?;
    let reply = msgs.last().expect("a reply");
    let calls = tools(reply);
    assert_eq!(calls.len(), 4, "each command once, not once per event");
    assert!(calls.iter().all(|t| t.name == "commandExecution"));
    assert_eq!(
        calls[3].output.as_deref(),
        Some("cat: does-not-exist.txt: No such file or directory\n")
    );
    assert_eq!(calls[3].status, ToolStatus::Failed as i32);

    // Commentary, the commands, then the final answer: text on both sides.
    assert!(matches!(
        reply.parts.first().and_then(|p| p.part.as_ref()),
        Some(Part::Text(_))
    ));
    assert!(matches!(
        reply.parts.last().and_then(|p| p.part.as_ref()),
        Some(Part::Text(t)) if t.starts_with("Read `a.txt` and `b.txt`")
    ));
    let stats = reply.stats.as_ref().expect("turn stats");
    assert_eq!(stats.duration_ms, Some(21_593));
    assert_eq!(
        (stats.input_tokens, stats.output_tokens),
        (Some(78_873), Some(242))
    );
    Ok(())
}

#[tokio::test]
async fn gemini_turn_keeps_its_own_tool_names() -> Result<()> {
    let recorded = concat!(
        r#"{"type":"init","session_id":"s1","model":"auto"}"#,
        "\n",
        include_str!("../../../crates/modula_services/src/providers/fixtures/gemini_shell.jsonl"),
        r#"{"type":"result","status":"success","stats":{"input_tokens":9,"output_tokens":4,"duration_ms":1500}}"#,
    );
    let h = Harness::start_with_env(&[("MODULA_MOCK_RECIPE", &replay(recorded))]).await?;
    let ws = common::fresh_workspace(&h, "demo").await?;
    let conv_id = conversation(&h, &ws, "gemini").await?;
    send(&h, &ws, &conv_id, "go").await?;

    let msgs = messages(&h, &ws, &conv_id).await?;
    let calls = tools(&msgs[1]);
    assert_eq!(calls.len(), 2);
    assert!(calls.iter().all(|t| t.name == "run_shell_command"));
    assert!(calls[0]
        .output
        .as_deref()
        .is_some_and(|o| o.ends_with("3 out.txt")));
    assert_eq!(
        msgs[1].stats.as_ref().and_then(|s| s.duration_ms),
        Some(1500)
    );
    Ok(())
}

/// OpenCode reports one `step_finish` per model call, so the text after a tool
/// call belongs to the same turn.
#[tokio::test]
async fn opencode_turn_runs_past_its_first_step() -> Result<()> {
    let recipe = json!({ "stream": [
        {"type": "step_start", "sessionID": "ses_1", "part": {"type": "step-start"}},
        {"type": "tool_use", "sessionID": "ses_1", "part": {"type": "tool", "callID": "c1", "tool": "bash",
            "state": {"status": "completed", "input": {"command": "ls"}, "output": "a.txt\n",
                      "title": "ls", "metadata": {}, "time": {"start": 1, "end": 2}}}},
        {"type": "step_finish", "sessionID": "ses_1", "part": {"type": "step-finish", "reason": "tool-calls",
            "cost": 0.25, "tokens": {"input": 10, "output": 2, "reasoning": 0, "cache": {"read": 0, "write": 0}}}},
        {"type": "step_start", "sessionID": "ses_1", "part": {"type": "step-start"}},
        {"type": "text", "sessionID": "ses_1", "part": {"type": "text", "text": "One file: a.txt."}},
        {"type": "step_finish", "sessionID": "ses_1", "part": {"type": "step-finish", "reason": "stop",
            "cost": 0.5, "tokens": {"input": 20, "output": 6, "reasoning": 0, "cache": {"read": 0, "write": 0}}}},
    ]})
    .to_string();
    let h = Harness::start_with_env(&[("MODULA_MOCK_RECIPE", &recipe)]).await?;
    let ws = common::fresh_workspace(&h, "demo").await?;
    let conv_id = conversation(&h, &ws, "opencode").await?;
    let events = send(&h, &ws, &conv_id, "go").await?;
    assert!(matches!(events.last(), Some(conv_event::Event::Done(_))));

    let msgs = messages(&h, &ws, &conv_id).await?;
    let reply = &msgs[1];
    let calls = tools(reply);
    assert_eq!(calls.len(), 1);
    assert_eq!(
        (calls[0].name.as_str(), calls[0].output.as_deref()),
        ("bash", Some("a.txt\n"))
    );
    assert_eq!(reply.content, "One file: a.txt.");
    let stats = reply.stats.as_ref().expect("turn stats");
    assert_eq!(
        (stats.input_tokens, stats.output_tokens, stats.cost_usd),
        (Some(30), Some(8), Some(0.75))
    );
    Ok(())
}

#[tokio::test]
async fn transcript_pages_from_the_tail() -> Result<()> {
    let h = Harness::start().await?;
    let ws = common::fresh_workspace(&h, "demo").await?;
    let conv_id = conversation(&h, &ws, "claude").await?;
    for i in 0..3 {
        send(&h, &ws, &conv_id, &format!("message {i}")).await?;
    }

    // Three turns: six messages, a user and an assistant one each.
    let tail = h
        .conversations()
        .get(GetConversationRequest {
            workspace_id: ws.clone(),
            conversation_id: conv_id.clone(),
            message_limit: Some(2),
        })
        .await?
        .into_inner();
    assert_eq!(tail.message_count, 6);
    let seqs: Vec<u32> = tail.messages.iter().map(|m| m.seq).collect();
    assert_eq!(seqs, [4, 5]);
    assert_eq!(tail.messages[0].content, "message 2");

    let older = h
        .conversations()
        .list_messages(ListMessagesRequest {
            workspace_id: ws.clone(),
            conversation_id: conv_id.clone(),
            before_seq: 4,
            limit: 3,
        })
        .await?
        .into_inner()
        .messages;
    let seqs: Vec<u32> = older.iter().map(|m| m.seq).collect();
    assert_eq!(seqs, [1, 2, 3]);
    Ok(())
}
