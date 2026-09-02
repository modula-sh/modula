//! External-source task upsert over gRPC IPC: `TaskService.Upsert` creates then
//! updates by (external_id, source), emits task.create / task.update, rejects a
//! source mismatch with AlreadyExists and a missing external_id with
//! InvalidArgument, while internal `Create` still mints a UUID.

use anyhow::Result;
use modula_rpc::json::{json_to_struct, struct_to_json};
use modula_rpc::v1::{ListTasksRequest, UpsertTaskRequest};
use modula_test_support::Harness;
use serde_json::json;
use tonic::Code;

use modula_test_support::fixtures as common;

fn upsert_req(ws: &str, external_id: &str, source: &str, title: &str) -> UpsertTaskRequest {
    UpsertTaskRequest {
        workspace_id: ws.to_string(),
        external_id: external_id.to_string(),
        source: source.to_string(),
        title: title.to_string(),
        description: None,
        source_data: None,
        status: None,
        url: None,
        synced_at: None,
        approved: None,
        max_variants: None,
        worktree: None,
    }
}

#[tokio::test]
async fn jira_upsert_creates_then_updates() -> Result<()> {
    let h = Harness::start().await?;
    let ws = common::fresh_workspace(&h, "demo").await?;

    let created = h
        .tasks()
        .upsert(UpsertTaskRequest {
            description: Some("old body".into()),
            source_data: json_to_struct(json!({"issue_type": "Story"})),
            status: Some("Open".into()),
            url: Some("https://example.atlassian.net/browse/ENG-1".into()),
            ..upsert_req(&ws, "ENG-1", "jira", "first cut")
        })
        .await?
        .into_inner();
    let task_uuid = created.id;
    assert_eq!(task_uuid.len(), 36, "expected UUID id");
    assert_eq!(created.external_id, "ENG-1");
    assert_eq!(created.upserted, "created");

    let task = common::get_task(&h, &ws, &task_uuid).await?;
    assert_eq!(task.external_id.as_deref(), Some("ENG-1"));
    assert_eq!(task.source, "jira");
    assert_eq!(task.title, "first cut");
    assert_eq!(task.approved, None);

    let events = common::list_events(&h, &ws).await?;
    assert!(
        events.iter().any(|(t, d)| t == "task.create"
            && d["task_id"] == json!(task_uuid)
            && d["source"] == "jira"),
        "missing task.create event"
    );

    // Re-upsert → updates mirrorable fields, preserves approved, same UUID.
    let updated = h
        .tasks()
        .upsert(UpsertTaskRequest {
            description: Some("new body".into()),
            source_data: json_to_struct(json!({"issue_type": "Task"})),
            status: Some("In Progress".into()),
            url: Some("https://example.atlassian.net/browse/ENG-1".into()),
            ..upsert_req(&ws, "ENG-1", "jira", "renamed")
        })
        .await?
        .into_inner();
    assert_eq!(updated.upserted, "updated");
    assert_eq!(updated.id, task_uuid);
    assert_eq!(updated.external_id, "ENG-1");

    let task = common::get_task(&h, &ws, &task_uuid).await?;
    assert_eq!(task.title, "renamed");
    assert_eq!(task.description, "new body");
    let source_data = struct_to_json(task.source_data.expect("source_data"));
    assert_eq!(source_data["issue_type"], json!("Task"));
    assert_eq!(task.status.as_deref(), Some("In Progress"));
    assert_eq!(task.approved, None);

    let events = common::list_events(&h, &ws).await?;
    let upd = events
        .iter()
        .find(|(t, d)| t == "task.update" && d["task_id"] == json!(task_uuid))
        .expect("task.update");
    assert_eq!(upd.1["title"], json!("renamed"));
    assert_eq!(upd.1["status"], json!("In Progress"));
    Ok(())
}

#[tokio::test]
async fn external_source_validation() -> Result<()> {
    let h = Harness::start().await?;
    let ws = common::fresh_workspace(&h, "demo").await?;

    // Create a jira task.
    h.tasks()
        .upsert(upsert_req(&ws, "ENG-1", "jira", "x"))
        .await?;

    // Source mismatch on existing external_id → AlreadyExists.
    let err = h
        .tasks()
        .upsert(upsert_req(&ws, "ENG-1", "linear", "x"))
        .await
        .expect_err("source mismatch must be rejected");
    assert_eq!(err.code(), Code::AlreadyExists);

    // External upsert without external_id → InvalidArgument.
    let err = h
        .tasks()
        .upsert(upsert_req(&ws, "", "jira", "x"))
        .await
        .expect_err("missing external_id must be rejected");
    assert_eq!(err.code(), Code::InvalidArgument);

    // Internal create with no id still works and returns a UUID.
    let id = common::create_task(&h, &ws, "x").await?;
    assert_eq!(id.len(), 36, "expected UUID, got: {id}");

    let count = h
        .tasks()
        .list(ListTasksRequest {
            workspace_id: ws.clone(),
        })
        .await?
        .into_inner()
        .tasks
        .len();
    assert_eq!(count, 2, "jira upsert + internal create");
    Ok(())
}
