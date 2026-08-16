//! Dispatcher fires matching agents over gRPC IPC and marks the event processed.

use anyhow::Result;
use modula_rpc::json::struct_to_json;
use modula_rpc::v1::{ListEventsRequest, ListRunsForAgentRequest};
use modula_test_support::Harness;
use serde_json::json;
use std::time::{Duration, Instant};

mod common;

#[tokio::test]
async fn rule_match_triggers_dispatch_and_marks_event_processed() -> Result<()> {
    let h = Harness::start_with_env(&[("MODULA_DISPATCH_INTERVAL_SECS", "1")]).await?;
    let ws = common::fresh_workspace(&h, "demo").await?;

    let provider_dir = h.modula_dir.join("fake-provider");
    std::fs::create_dir_all(&provider_dir)?;
    let p1_id = common::create_provider(&h, &ws, "p1", &provider_dir).await?;

    // mock-claude default recipe just emits init+result and exits — perfect
    // for a "did the dispatcher fire" check.
    let agent_id = common::create_agent(
        &h,
        &ws,
        &p1_id,
        "auto-agent",
        &["event.type == 'task.create'"],
        false,
    )
    .await?;

    // Poke the engine — create task → task.create event → dispatcher fires.
    let task_id = common::create_task(&h, &ws, "fires the agent").await?;

    // Poll runs until the agent's run row shows up (≤ 8s).
    let deadline = Instant::now() + Duration::from_secs(8);
    let runs = loop {
        let runs = h
            .runs()
            .list_for_agent(ListRunsForAgentRequest {
                workspace_id: ws.clone(),
                agent_id: agent_id.clone(),
            })
            .await?
            .into_inner()
            .runs;
        if !runs.is_empty() || Instant::now() >= deadline {
            break runs;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    };
    assert!(!runs.is_empty(), "expected a run row for auto-agent");
    assert_eq!(runs[0].agent_name, "auto-agent");
    assert_eq!(runs[0].agent_id, agent_id);
    assert!(runs[0].event_id.is_some());
    let status = runs[0].status.as_str();
    assert!(
        status == "running" || status == "completed",
        "unexpected run status: {status:?}"
    );

    // Once the dispatcher has run, the matching event should be processed.
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        let events = h
            .events()
            .list(ListEventsRequest {
                workspace_id: ws.clone(),
            })
            .await?
            .into_inner()
            .events;
        if let Some(c) = events.iter().find(|e| e.r#type == "task.create") {
            if c.processed {
                let data = c
                    .data
                    .clone()
                    .map(struct_to_json)
                    .unwrap_or_else(|| json!({}));
                assert_eq!(data["task_id"], json!(task_id));
                return Ok(());
            }
        }
        if Instant::now() >= deadline {
            break;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    panic!("task.create event was never marked processed");
}

#[tokio::test]
async fn agent_with_no_matching_rule_is_skipped() -> Result<()> {
    let h = Harness::start_with_env(&[("MODULA_DISPATCH_INTERVAL_SECS", "1")]).await?;
    let ws = common::fresh_workspace(&h, "demo").await?;
    let provider_dir = h.modula_dir.join("fake-provider");
    std::fs::create_dir_all(&provider_dir)?;
    let p1_id = common::create_provider(&h, &ws, "p1", &provider_dir).await?;

    let agent_id = common::create_agent(
        &h,
        &ws,
        &p1_id,
        "picky",
        &["event.type == 'task.delete'"],
        false,
    )
    .await?;

    // task.create should NOT dispatch this agent.
    common::create_task(&h, &ws, "nope").await?;

    // Wait long enough for ~3 dispatch ticks, then confirm no runs landed.
    tokio::time::sleep(Duration::from_secs(3)).await;
    let runs = h
        .runs()
        .list_for_agent(ListRunsForAgentRequest {
            workspace_id: ws.clone(),
            agent_id: agent_id.clone(),
        })
        .await?
        .into_inner()
        .runs;
    assert_eq!(runs.len(), 0);
    Ok(())
}
