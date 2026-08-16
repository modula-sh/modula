//! Roadmap upsert: create-vs-update flag, depends_on / notes persistence,
//! and the unknown-task / unknown-pipeline gates, over gRPC IPC.

use anyhow::Result;
use modula_rpc::v1::{ListRoadmapRequest, SetRoadmapStatusRequest};
use modula_test_support::Harness;
use tonic::Code;

mod common;

#[tokio::test]
async fn roadmap_upsert_inserts_then_updates() -> Result<()> {
    let h = Harness::start().await?;
    let ws = common::fresh_workspace(&h, "demo").await?;
    let id = common::create_task(&h, &ws, "T").await?;
    // Create a second/third task to use as depends_on references.
    let dep1 = common::create_task(&h, &ws, "Dep1").await?;
    let dep2 = common::create_task(&h, &ws, "Dep2").await?;

    let resp = h
        .roadmap()
        .set_status(SetRoadmapStatusRequest {
            workspace_id: ws.clone(),
            task_id: id.clone(),
            status: "planning".into(),
            depends_on: vec![dep1.clone()],
            notes: Some("first".into()),
        })
        .await?
        .into_inner();
    assert!(resp.created, "first set_status should create");

    let row = roadmap_row(&h, &ws, &id).await?;
    assert_eq!(row.status, "planning");
    assert_eq!(row.depends_on, vec![dep1.clone()]);
    assert_eq!(row.notes, "first");

    // The status change emits a task.update carrying the new pipeline status.
    let events = common::list_events(&h, &ws).await?;
    assert!(
        events.iter().any(|(ty, data)| ty == "task.update"
            && data["task_id"] == serde_json::json!(id)
            && data["pipeline_status"] == "planning"),
        "expected a task.update event for the roadmap status change"
    );

    // Re-set → created=false, fields updated.
    let resp = h
        .roadmap()
        .set_status(SetRoadmapStatusRequest {
            workspace_id: ws.clone(),
            task_id: id.clone(),
            status: "ready_for_research".into(),
            depends_on: vec![dep1.clone(), dep2.clone()],
            notes: Some("second".into()),
        })
        .await?
        .into_inner();
    assert!(!resp.created, "second set_status should update, not create");

    let row = roadmap_row(&h, &ws, &id).await?;
    assert_eq!(row.status, "ready_for_research");
    assert_eq!(row.depends_on, vec![dep1, dep2]);
    assert_eq!(row.notes, "second");
    Ok(())
}

#[tokio::test]
async fn roadmap_upsert_validation() -> Result<()> {
    let h = Harness::start().await?;
    let ws = common::fresh_workspace(&h, "demo").await?;
    let id = common::create_task(&h, &ws, "T").await?;

    // Unknown task UUID → NotFound.
    let err = h
        .roadmap()
        .set_status(SetRoadmapStatusRequest {
            workspace_id: ws.clone(),
            task_id: "00000000-0000-0000-0000-000000000000".into(),
            status: "planning".into(),
            depends_on: vec![],
            notes: None,
        })
        .await
        .expect_err("unknown task must 404");
    assert_eq!(err.code(), Code::NotFound);

    // Unknown pipeline key → InvalidArgument.
    let err = h
        .roadmap()
        .set_status(SetRoadmapStatusRequest {
            workspace_id: ws.clone(),
            task_id: id.clone(),
            status: "bogus_status".into(),
            depends_on: vec![],
            notes: None,
        })
        .await
        .expect_err("unknown pipeline key must be rejected");
    assert_eq!(err.code(), Code::InvalidArgument);
    Ok(())
}

/// The roadmap row for one task UUID.
async fn roadmap_row(h: &Harness, ws: &str, task_id: &str) -> Result<modula_rpc::v1::RoadmapEntry> {
    h.roadmap()
        .list(ListRoadmapRequest {
            workspace_id: ws.to_string(),
        })
        .await?
        .into_inner()
        .entries
        .into_iter()
        .find(|r| r.task_id == task_id)
        .ok_or_else(|| anyhow::anyhow!("roadmap row for task {task_id} not found"))
}
