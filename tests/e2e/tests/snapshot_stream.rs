//! E2E: the workspace snapshot reflects mutations, over gRPC IPC. Covers both
//! the unary `SnapshotService.Get` and the chunked `SnapshotService.Stream`
//! (the phase-5 size-safe server stream that replaced the SSE state poll).

use anyhow::Result;
use modula_rpc::v1::{CreateTaskRequest, StreamSnapshotRequest};
use modula_test_support::Harness;
use serde_json::Value as Json;

use modula_test_support::fixtures as common;

/// Reassemble the chunked snapshot stream into JSON.
async fn snapshot_stream(h: &Harness, ws: &str) -> Result<Json> {
    let mut stream = h
        .snapshots()
        .stream(StreamSnapshotRequest {
            workspace_id: ws.to_string(),
        })
        .await?
        .into_inner();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.message().await? {
        bytes.extend_from_slice(&chunk.data);
    }
    Ok(serde_json::from_slice(&bytes)?)
}

#[tokio::test]
async fn snapshot_reflects_changes_over_get_and_stream() -> Result<()> {
    let h = Harness::start().await?;
    let ws = common::fresh_workspace(&h, "demo").await?;

    let snap = common::snapshot(&h, &ws).await?;
    assert_eq!(snap["tasks"].as_array().unwrap().len(), 0);
    assert_eq!(snap["roadmap"].as_array().unwrap().len(), 0);

    let task_id = common::create_task(&h, &ws, "Hello").await?;

    // Unary Get now carries the task.
    let mut snap = common::snapshot(&h, &ws).await?;
    let tasks = snap["tasks"].as_array().unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["id"].as_str(), Some(task_id.as_str()));

    // The chunked stream reassembles to the identical document, modulo `ts` —
    // the snapshot's generation time, which advances between the two calls.
    let mut streamed = snapshot_stream(&h, &ws).await?;
    for doc in [&mut snap, &mut streamed] {
        doc.as_object_mut().unwrap().remove("ts");
    }
    assert_eq!(streamed, snap);

    Ok(())
}

/// A snapshot *response* larger than tonic's default 4 MB decode cap must
/// transfer without `ResourceExhausted` — over both the unary `Get` (server
/// raises the encode limit, the harness client the decode limit) and the
/// chunked `Stream` (each chunk stays under the cap). Built from several tasks
/// whose descriptions sum past 4 MB (each individual `Create` request stays
/// under the request decode cap — the requirement is about large responses).
#[tokio::test]
async fn oversized_snapshot_transfers_without_resource_exhausted() -> Result<()> {
    let h = Harness::start().await?;
    let ws = common::fresh_workspace(&h, "demo").await?;

    // 6 × ~1 MB = ~6 MB of description, well past the 4 MB default once the
    // snapshot is assembled.
    let chunk = "x".repeat(1024 * 1024);
    for i in 0..6 {
        h.tasks()
            .create(CreateTaskRequest {
                workspace_id: ws.clone(),
                title: format!("task-{i}"),
                description: Some(chunk.clone()),
                approved: None,
                max_variants: None,
                worktree: None,
                source_data: None,
            })
            .await?;
    }

    // Unary Get: a >4 MB response must not fail with ResourceExhausted.
    let snap = common::snapshot(&h, &ws).await?;
    let tasks = snap["tasks"].as_array().unwrap();
    assert_eq!(tasks.len(), 6);
    let total: usize = tasks
        .iter()
        .filter_map(|t| t["description"].as_str().map(str::len))
        .sum();
    assert!(
        total > 4 * 1024 * 1024,
        "snapshot payload {total} did not exceed the 4 MB default cap"
    );

    // Chunked Stream: the oversized payload reassembles to a valid document
    // carrying all 6 tasks (exact-equality with the unary Get is covered by the
    // small-payload test above; the snapshot's live fields drift between calls).
    let streamed = snapshot_stream(&h, &ws).await?;
    assert_eq!(streamed["tasks"].as_array().unwrap().len(), 6);

    Ok(())
}
