//! Ralph-loop e2e: CRUD round-trip + execution over the gRPC IPC transport. The
//! loop count lives on a per-task `task_agent_settings` row (TaskService
//! agent-settings RPCs), reached by `spawn::resolve` via the trigger's `task-id`
//! arg. The execution test relies on the mock-claude `append_line` mutation:
//! each iteration appends a line to a counter file, so after `amount` iterations
//! the file has `amount` lines.

use std::time::{Duration, Instant};

use anyhow::Result;
use modula_rpc::json::json_to_struct;
use modula_rpc::v1::{
    AgentArgDef, CreateAgentRequest, DeleteAgentSettingsRequest, KillAgentRequest,
    ListAgentSettingsRequest, SetAgentSettingsRequest, TriggerAgentRequest,
};
use modula_test_support::Harness;
use serde_json::json;
use tonic::Code;

use modula_test_support::fixtures as common;

/// Returns (ws_uuid, provider_uuid).
async fn workspace_with_provider(h: &Harness) -> Result<(String, String)> {
    let ws = common::fresh_workspace(h, "demo").await?;
    let provider_dir = h.modula_dir.join("fake-provider");
    std::fs::create_dir_all(&provider_dir)?;
    let p1_id = common::create_provider(h, &ws, "p1", &provider_dir).await?;
    Ok((ws, p1_id))
}

/// Create a manual `looper` agent declaring a `--task-id` arg (so a trigger
/// body's `task-id` lands in the arg map, which `resolve` reads). Returns the
/// agent UUID.
async fn create_looper(h: &Harness, ws: &str, provider_id: &str) -> Result<String> {
    let resp = h
        .agents()
        .create(CreateAgentRequest {
            workspace_id: ws.to_string(),
            name: "looper".to_string(),
            description: "looping agent".to_string(),
            provider_id: provider_id.to_string(),
            model: None,
            manual: true,
            schedule: None,
            rules: vec![],
            args: vec![AgentArgDef {
                flag: "--task-id".to_string(),
                required: true,
                help: None,
            }],
            prompt: "loop me".to_string(),
            spawn_per_variant: false,
            skills: vec![],
        })
        .await?
        .into_inner();
    Ok(resp.id)
}

#[tokio::test]
async fn loop_crud_roundtrip() -> Result<()> {
    let h = Harness::start().await?;
    let (ws, p1_id) = workspace_with_provider(&h).await?;
    let task = common::create_task(&h, &ws, "looped task").await?;
    let agent_id = create_looper(&h, &ws, &p1_id).await?;

    // Create the setting with amount 3.
    let setting = h
        .tasks()
        .set_agent_settings(SetAgentSettingsRequest {
            workspace_id: ws.clone(),
            task_id: task.clone(),
            agent_id: agent_id.clone(),
            loop_amount: 3,
        })
        .await?
        .into_inner();
    assert_eq!(setting.agent_id, agent_id);
    assert_eq!(setting.loop_amount, 3);

    // List reflects it.
    let list = h
        .tasks()
        .list_agent_settings(ListAgentSettingsRequest {
            workspace_id: ws.clone(),
            task_id: task.clone(),
        })
        .await?
        .into_inner()
        .settings;
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].agent_id, agent_id);
    assert_eq!(list[0].loop_amount, 3);

    // Update amount to 5.
    h.tasks()
        .set_agent_settings(SetAgentSettingsRequest {
            workspace_id: ws.clone(),
            task_id: task.clone(),
            agent_id: agent_id.clone(),
            loop_amount: 5,
        })
        .await?;
    let list = h
        .tasks()
        .list_agent_settings(ListAgentSettingsRequest {
            workspace_id: ws.clone(),
            task_id: task.clone(),
        })
        .await?
        .into_inner()
        .settings;
    assert_eq!(list[0].loop_amount, 5);

    // Delete → no longer listed.
    h.tasks()
        .delete_agent_settings(DeleteAgentSettingsRequest {
            workspace_id: ws.clone(),
            task_id: task.clone(),
            agent_id: agent_id.clone(),
        })
        .await?;
    let list = h
        .tasks()
        .list_agent_settings(ListAgentSettingsRequest {
            workspace_id: ws.clone(),
            task_id: task.clone(),
        })
        .await?
        .into_inner()
        .settings;
    assert!(list.is_empty(), "setting should be gone: {list:?}");

    // Deleting again is NotFound (the row is gone).
    let err = h
        .tasks()
        .delete_agent_settings(DeleteAgentSettingsRequest {
            workspace_id: ws.clone(),
            task_id: task.clone(),
            agent_id: agent_id.clone(),
        })
        .await
        .expect_err("second delete should fail");
    assert_eq!(
        err.code(),
        Code::NotFound,
        "second delete should be NotFound"
    );
    Ok(())
}

#[tokio::test]
async fn loop_validation_rejects_bad_values() -> Result<()> {
    let h = Harness::start().await?;
    let (ws, p1_id) = workspace_with_provider(&h).await?;
    let task = common::create_task(&h, &ws, "looped task").await?;
    let agent_id = create_looper(&h, &ws, &p1_id).await?;

    // Out-of-range amounts are InvalidArgument. (The old REST `type: varies`
    // case no longer exists — the proto carries only a fixed `loop_amount`.)
    for bad in [0, 101] {
        let err = h
            .tasks()
            .set_agent_settings(SetAgentSettingsRequest {
                workspace_id: ws.clone(),
                task_id: task.clone(),
                agent_id: agent_id.clone(),
                loop_amount: bad,
            })
            .await
            .expect_err("bad loop amount should fail");
        assert_eq!(
            err.code(),
            Code::InvalidArgument,
            "bad loop {bad} should be InvalidArgument"
        );
    }

    // Unknown agent → NotFound, not an Internal error from the FK.
    let err = h
        .tasks()
        .set_agent_settings(SetAgentSettingsRequest {
            workspace_id: ws.clone(),
            task_id: task.clone(),
            agent_id: "does-not-exist".to_string(),
            loop_amount: 2,
        })
        .await
        .expect_err("unknown agent should fail");
    assert_eq!(
        err.code(),
        Code::NotFound,
        "unknown agent should be NotFound"
    );
    Ok(())
}

#[tokio::test]
async fn loop_executes_n_iterations() -> Result<()> {
    let h = Harness::start().await?;
    let (ws, p1_id) = workspace_with_provider(&h).await?;
    let task = common::create_task(&h, &ws, "looped task").await?;
    let agent_id = create_looper(&h, &ws, &p1_id).await?;

    // Mock recipe: every claude invocation appends a line to data/loop-count.txt.
    // After N iterations the file has N lines — the single observable proof
    // that the loop ran the right number of times.
    h.write_recipe(
        &ws,
        "looper",
        &json!({
            "stream": [],
            "mutations": [
                {
                    "file": "data/loop-count.txt",
                    "op": {"kind": "append_line", "value": "x"},
                }
            ],
            "sleep_ms": 100,
        }),
    )?;

    // The loop count is a per-task setting now.
    h.tasks()
        .set_agent_settings(SetAgentSettingsRequest {
            workspace_id: ws.clone(),
            task_id: task.clone(),
            agent_id: agent_id.clone(),
            loop_amount: 3,
        })
        .await?;

    let count_file = h.workspace_path(&ws).join("data").join("loop-count.txt");
    std::fs::create_dir_all(count_file.parent().unwrap())?;

    // Fire the agent with the task-id so resolve finds the setting; returns
    // immediately with iteration 1's PID — the loop controller drives the rest.
    h.agents()
        .trigger(TriggerAgentRequest {
            workspace_id: ws.clone(),
            agent_id: agent_id.clone(),
            args: json_to_struct(json!({ "task-id": task })),
        })
        .await?;

    // Poll the counter file until it has 3 entries (or timeout). Each
    // iteration sleeps 100ms inside the mock; with 500ms loop-controller poll
    // interval, three iterations take roughly 100ms + 500ms*2 + 100ms.
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut last_count = 0usize;
    while Instant::now() < deadline {
        if let Ok(text) = std::fs::read_to_string(&count_file) {
            let count = text.lines().filter(|l| !l.is_empty()).count();
            last_count = count;
            if count >= 3 {
                break;
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(last_count, 3, "expected 3 iterations, saw {last_count}");

    // One log file per iteration → at least 3 logs for `looper`.
    let logs_dir = h.workspace_path(&ws).join("logs");
    let looper_logs: Vec<_> = std::fs::read_dir(&logs_dir)?
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("looper-"))
        .collect();
    assert!(
        looper_logs.len() >= 3,
        "expected ≥3 looper-*.log files, saw {}",
        looper_logs.len()
    );
    Ok(())
}

#[tokio::test]
async fn loop_kill_stops_subsequent_iterations() -> Result<()> {
    let h = Harness::start().await?;
    let (ws, p1_id) = workspace_with_provider(&h).await?;
    let task = common::create_task(&h, &ws, "looped task").await?;
    let agent_id = create_looper(&h, &ws, &p1_id).await?;

    // Each iteration sleeps 2s before mutating — kills landing within that
    // window prevent the mutation AND prevent the loop controller from
    // spawning iter 2 (cancel flag is set before the signal lands).
    h.write_recipe(
        &ws,
        "looper",
        &json!({
            "stream": [],
            "mutations": [
                {
                    "file": "data/loop-count.txt",
                    "op": {"kind": "append_line", "value": "x"},
                }
            ],
            "sleep_ms": 2000,
        }),
    )?;

    h.tasks()
        .set_agent_settings(SetAgentSettingsRequest {
            workspace_id: ws.clone(),
            task_id: task.clone(),
            agent_id: agent_id.clone(),
            loop_amount: 5,
        })
        .await?;

    let count_file = h.workspace_path(&ws).join("data").join("loop-count.txt");
    std::fs::create_dir_all(count_file.parent().unwrap())?;

    let trigger = h
        .agents()
        .trigger(TriggerAgentRequest {
            workspace_id: ws.clone(),
            agent_id: agent_id.clone(),
            args: json_to_struct(json!({ "task-id": task })),
        })
        .await?
        .into_inner();
    let pid = trigger.pid;

    // Tiny retry: the spawn → agent_processes insert may not have committed
    // by the time the trigger response races back to us (Kill 404s until then).
    let kill = {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match h
                .agents()
                .kill(KillAgentRequest {
                    workspace_id: ws.clone(),
                    pid,
                    escalate: false,
                })
                .await
            {
                Ok(resp) => break resp.into_inner(),
                Err(e) if Instant::now() < deadline => {
                    assert_eq!(e.code(), Code::NotFound, "unexpected kill error: {e}");
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                Err(e) => anyhow::bail!("kill never succeeded: {e}"),
            }
        }
    };
    assert!(
        kill.loop_cancelled,
        "expected loop_cancelled: true (response: {kill:?})"
    );

    // Wait long enough that iter 2 would have spawned + mutated had the
    // cancel flag not stopped it (controller polls at 500ms; iteration's
    // own sleep is 2s).
    tokio::time::sleep(Duration::from_secs(3)).await;

    let text = std::fs::read_to_string(&count_file).unwrap_or_default();
    let count = text.lines().filter(|l| !l.is_empty()).count();
    assert!(
        count <= 1,
        "expected at most 1 iteration's mutation after kill, saw {count}"
    );
    Ok(())
}
