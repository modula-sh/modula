//! Label create (get-or-create), list, and task attach/detach over gRPC IPC,
//! plus the labels surfaced on the task payload.

use anyhow::Result;
use modula_rpc::v1::{
    AttachLabelRequest, CreateLabelRequest, DetachLabelRequest, ListLabelsRequest, ListTasksRequest,
};
use modula_test_support::Harness;
use tonic::Code;

use modula_test_support::fixtures as common;

#[tokio::test]
async fn label_lifecycle_create_attach_detach() -> Result<()> {
    let h = Harness::start().await?;
    let ws = common::fresh_workspace(&h, "demo").await?;
    let task = common::create_task(&h, &ws, "T").await?;

    // No labels at start.
    let labels = h
        .labels()
        .list(ListLabelsRequest {
            workspace_id: ws.clone(),
            r#type: String::new(),
        })
        .await?
        .into_inner()
        .labels;
    assert!(labels.is_empty());

    // Create a label — server mints a UUID.
    let label_id = h
        .labels()
        .create(CreateLabelRequest {
            workspace_id: ws.clone(),
            name: "backend".into(),
            r#type: String::new(),
        })
        .await?
        .into_inner()
        .id;
    assert_eq!(label_id.len(), 36, "expected UUID, got: {label_id}");

    // Create is get-or-create: the same name returns the same id.
    let again = h
        .labels()
        .create(CreateLabelRequest {
            workspace_id: ws.clone(),
            name: "backend".into(),
            r#type: String::new(),
        })
        .await?
        .into_inner()
        .id;
    assert_eq!(again, label_id);

    // Blank name → InvalidArgument.
    let err = h
        .labels()
        .create(CreateLabelRequest {
            workspace_id: ws.clone(),
            name: "  ".into(),
            r#type: String::new(),
        })
        .await
        .expect_err("blank name must be rejected");
    assert_eq!(err.code(), Code::InvalidArgument);

    // List returns the single label.
    let labels = h
        .labels()
        .list(ListLabelsRequest {
            workspace_id: ws.clone(),
            r#type: String::new(),
        })
        .await?
        .into_inner()
        .labels;
    assert_eq!(labels.len(), 1);
    assert_eq!(labels[0].name, "backend");

    // Attach to the task.
    h.labels()
        .attach_to_task(AttachLabelRequest {
            workspace_id: ws.clone(),
            task_id: task.clone(),
            label_id: label_id.clone(),
        })
        .await?;

    // The task payload now carries the label.
    let tasks = h
        .tasks()
        .list(ListTasksRequest {
            workspace_id: ws.clone(),
        })
        .await?
        .into_inner()
        .tasks;
    assert_eq!(tasks[0].labels.len(), 1);
    assert_eq!(tasks[0].labels[0].id, label_id);
    assert_eq!(tasks[0].labels[0].name, "backend");

    // Attaching an unknown label → NotFound.
    let err = h
        .labels()
        .attach_to_task(AttachLabelRequest {
            workspace_id: ws.clone(),
            task_id: task.clone(),
            label_id: "00000000-0000-0000-0000-000000000000".into(),
        })
        .await
        .expect_err("unknown label must 404");
    assert_eq!(err.code(), Code::NotFound);

    // Attaching with no label_id → InvalidArgument.
    let err = h
        .labels()
        .attach_to_task(AttachLabelRequest {
            workspace_id: ws.clone(),
            task_id: task.clone(),
            label_id: String::new(),
        })
        .await
        .expect_err("empty label_id must be rejected");
    assert_eq!(err.code(), Code::InvalidArgument);

    // Detach.
    h.labels()
        .detach_from_task(DetachLabelRequest {
            workspace_id: ws.clone(),
            task_id: task.clone(),
            label_id: label_id.clone(),
        })
        .await?;

    // Label is gone from the task, but the label itself still exists in the pool.
    let tasks = h
        .tasks()
        .list(ListTasksRequest {
            workspace_id: ws.clone(),
        })
        .await?
        .into_inner()
        .tasks;
    assert!(tasks[0].labels.is_empty());
    let labels = h
        .labels()
        .list(ListLabelsRequest {
            workspace_id: ws.clone(),
            r#type: String::new(),
        })
        .await?
        .into_inner()
        .labels;
    assert_eq!(labels.len(), 1);

    Ok(())
}
