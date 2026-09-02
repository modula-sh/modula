//! E2E: the net-new live broadcast streams (phase-5), over gRPC IPC.
//!
//! `EventService.Watch` and `RunService.WatchStatus` replace the old 2s SSE
//! snapshot poll — a subscribed client must receive typed events *pushed* by
//! the service-layer bus the instant a mutation happens, with no polling. These
//! are the streams the whole migration's liveness story hinges on, so they get
//! explicit coverage here. A negative check also proves the engine serves over a
//! local IPC socket rather than loopback TCP (the former REST surface is gone).

use std::time::Duration;

use anyhow::Result;
use modula_rpc::v1::{
    workspace_event, RunPhase, TriggerAgentRequest, WatchEventsRequest, WatchRunStatusRequest,
};
use modula_test_support::Harness;
use tokio::time::timeout;

use modula_test_support::fixtures as common;

/// `EventService.Watch` pushes a typed event the moment a mutation lands on
/// another client — without the subscriber polling.
#[tokio::test]
async fn event_watch_delivers_live_task_created() -> Result<()> {
    let h = Harness::start().await?;
    let ws = common::fresh_workspace(&h, "demo").await?;

    // Subscribe BEFORE mutating: Watch is a live broadcast, not a replay, so the
    // subscription must exist before the event is published.
    let mut stream = h
        .events()
        .watch(WatchEventsRequest {
            workspace_id: ws.clone(),
            after_seq: 0,
        })
        .await?
        .into_inner();

    // Mutate on a separate client — this must push a typed event to the watcher.
    let task_id = common::create_task(&h, &ws, "watched").await?;

    let found = timeout(Duration::from_secs(5), async {
        while let Some(ev) = stream.message().await? {
            assert_eq!(ev.workspace_id, ws, "event from a foreign workspace");
            if let Some(workspace_event::Event::TaskCreated(tc)) = ev.event {
                if tc.task_id == task_id {
                    return Ok::<bool, anyhow::Error>(true);
                }
            }
        }
        Ok(false)
    })
    .await
    .map_err(|_| anyhow::anyhow!("watch stream timed out — event was never pushed"))??;

    assert!(
        found,
        "TaskCreated for {task_id} never arrived on the watch stream"
    );
    Ok(())
}

/// `RunService.WatchStatus` streams a run's lifecycle (spawned → exited) as the
/// service-layer bus publishes it — the live agent-status channel.
#[tokio::test]
async fn run_status_watch_tracks_manual_agent_lifecycle() -> Result<()> {
    let h = Harness::start().await?;
    let ws = common::fresh_workspace(&h, "demo").await?;

    let provider_dir = h.modula_dir.join("fake-provider");
    std::fs::create_dir_all(&provider_dir)?;
    let provider_id = common::create_provider(&h, &ws, "p1", &provider_dir).await?;
    // Manual agent on the default mock recipe: spawns then exits promptly.
    let agent_id = common::create_agent(&h, &ws, &provider_id, "watched-agent", &[], true).await?;

    // Subscribe before triggering so the spawn isn't missed.
    let mut stream = h
        .runs()
        .watch_status(WatchRunStatusRequest {
            workspace_id: ws.clone(),
            agent_id: None,
        })
        .await?
        .into_inner();

    h.agents()
        .trigger(TriggerAgentRequest {
            workspace_id: ws.clone(),
            agent_id: agent_id.clone(),
            args: None,
        })
        .await?;

    // Expect a Spawned carrying our agent, then an Exited closing the run out.
    let (mut spawned, mut exited) = (false, false);
    timeout(Duration::from_secs(15), async {
        while let Some(st) = stream.message().await? {
            if st.phase == RunPhase::Spawned as i32 && st.agent_id == agent_id {
                spawned = true;
            }
            // Exit events carry no agent id (the run is already torn down).
            if st.phase == RunPhase::Exited as i32 {
                exited = true;
            }
            if spawned && exited {
                break;
            }
        }
        Ok::<(), anyhow::Error>(())
    })
    .await
    .map_err(|_| {
        anyhow::anyhow!("run-status watch timed out (spawned={spawned}, exited={exited})")
    })??;

    assert!(
        spawned,
        "no Spawned status for our agent on the watch stream"
    );
    assert!(exited, "no Exited status on the watch stream");
    Ok(())
}

/// Negative: the engine serves over a local IPC socket, not loopback TCP. The
/// endpoint it actually bound is a Unix domain socket on disk — there is no TCP
/// surface, and the harness reaches it only through the IPC channel. (The
/// absence of any HTTP/REST stack is additionally enforced at compile time:
/// `axum`/`tower-http`/`reqwest` are no longer engine dependencies.)
#[cfg(unix)]
#[tokio::test]
async fn engine_serves_over_ipc_socket_not_tcp() -> Result<()> {
    use std::os::unix::fs::FileTypeExt;

    let h = Harness::start().await?;

    let meta = std::fs::metadata(&h.socket)
        .map_err(|e| anyhow::anyhow!("engine IPC socket {:?} missing: {e}", h.socket))?;
    assert!(
        meta.file_type().is_socket(),
        "engine endpoint {:?} is not a Unix domain socket",
        h.socket
    );

    // The IPC channel reaches a serving engine; there is no TCP host:port in play.
    let resp = h
        .health()
        .check(modula_rpc::v1::HealthCheckRequest {
            service: String::new(),
        })
        .await?
        .into_inner();
    assert_eq!(resp.status, modula_rpc::v1::HealthStatus::Serving as i32);
    Ok(())
}

/// Signal-driven shutdown removes the IPC socket. The desktop stops the engine
/// by pid, so a real `SIGTERM` (not the harness `Drop`'s SIGKILL) must run the
/// engine's cleanup path and unlink the socket — otherwise every Quit leaves a
/// stale endpoint. A following start would then be clean.
#[cfg(unix)]
#[tokio::test]
async fn sigterm_removes_the_ipc_socket() -> Result<()> {
    let h = Harness::start().await?;
    assert!(h.socket.exists(), "socket not bound while serving");

    // A real SIGTERM, so the engine runs its signal-driven cleanup (Drop SIGKILLs,
    // which would skip it).
    let status = std::process::Command::new("kill")
        .arg("-TERM")
        .arg(h.engine_pid().to_string())
        .status()?;
    assert!(status.success(), "kill -TERM failed");

    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while h.socket.exists() {
        if std::time::Instant::now() >= deadline {
            anyhow::bail!(
                "socket {:?} still present after SIGTERM — cleanup did not run",
                h.socket
            );
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    Ok(())
}
