use anyhow::Result;
use modula_rpc::v1::{
    CreateTaskRequest, DeleteTaskRequest, ListTasksRequest, ResetTaskRequest,
    SetRoadmapStatusRequest, UpdateTaskRequest,
};
use modula_test_support::Harness;
use tonic::Code;

use modula_test_support::fixtures as common;

fn create_task_req(ws: &str, title: &str) -> CreateTaskRequest {
    CreateTaskRequest {
        workspace_id: ws.to_string(),
        title: title.to_string(),
        description: None,
        approved: None,
        max_variants: None,
        worktree: None,
        source_data: None,
    }
}

#[tokio::test]
async fn task_crud_and_reset() -> Result<()> {
    let h = Harness::start().await?;
    let ws = common::fresh_workspace(&h, "demo").await?;

    // No tasks at start.
    let tasks = h
        .tasks()
        .list(ListTasksRequest {
            workspace_id: ws.clone(),
        })
        .await?
        .into_inner()
        .tasks;
    assert!(tasks.is_empty());

    // Create an internal task — server mints a UUID + auto display id.
    let created = h
        .tasks()
        .create(create_task_req(&ws, "First"))
        .await?
        .into_inner();
    let id = created.id;
    assert_eq!(id.len(), 36, "expected UUID, got: {id}");
    assert_eq!(created.external_id, "DEM-001");

    // Missing title → InvalidArgument.
    let err = h
        .tasks()
        .create(create_task_req(&ws, ""))
        .await
        .expect_err("missing title must be rejected");
    assert_eq!(err.code(), Code::InvalidArgument);

    // Read back.
    let tasks = h
        .tasks()
        .list(ListTasksRequest {
            workspace_id: ws.clone(),
        })
        .await?
        .into_inner()
        .tasks;
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].id, id);
    assert_eq!(tasks[0].external_id.as_deref(), Some("DEM-001"));
    assert_eq!(tasks[0].source, "internal");
    assert_eq!(tasks[0].approved, None);
    assert!(tasks[0].variants.is_empty());

    // Approve.
    h.tasks()
        .update(UpdateTaskRequest {
            workspace_id: ws.clone(),
            task_id: id.clone(),
            approved: Some(true),
            max_variants: None,
            worktree: None,
            description: None,
            title: None,
        })
        .await?;

    // Contradiction: worktree=false + max_variants > 1 → InvalidArgument.
    let err = h
        .tasks()
        .update(UpdateTaskRequest {
            workspace_id: ws.clone(),
            task_id: id.clone(),
            approved: None,
            max_variants: Some(3),
            worktree: Some(false),
            description: None,
            title: None,
        })
        .await
        .expect_err("worktree=false + max_variants>1 must be rejected");
    assert_eq!(err.code(), Code::InvalidArgument);

    // worktree=false alone coerces max_variants → 1.
    h.tasks()
        .update(UpdateTaskRequest {
            workspace_id: ws.clone(),
            task_id: id.clone(),
            approved: None,
            max_variants: None,
            worktree: Some(false),
            description: None,
            title: None,
        })
        .await?;
    let tasks = h
        .tasks()
        .list(ListTasksRequest {
            workspace_id: ws.clone(),
        })
        .await?
        .into_inner()
        .tasks;
    assert_eq!(tasks[0].worktree, Some(false));
    assert_eq!(tasks[0].max_variants, Some(1));

    // Reset → task row stays, variants cleared.
    h.tasks()
        .reset(ResetTaskRequest {
            workspace_id: ws.clone(),
            task_id: id.clone(),
        })
        .await?;

    // Delete.
    h.tasks()
        .delete(DeleteTaskRequest {
            workspace_id: ws.clone(),
            task_id: id.clone(),
        })
        .await?;
    let tasks = h
        .tasks()
        .list(ListTasksRequest {
            workspace_id: ws.clone(),
        })
        .await?
        .into_inner()
        .tasks;
    assert!(tasks.is_empty());

    // NotFound on unknown task UUID.
    let err = h
        .tasks()
        .update(UpdateTaskRequest {
            workspace_id: ws.clone(),
            task_id: "00000000-0000-0000-0000-000000000000".into(),
            approved: Some(false),
            max_variants: None,
            worktree: None,
            description: None,
            title: None,
        })
        .await
        .expect_err("unknown task must 404");
    assert_eq!(err.code(), Code::NotFound);

    Ok(())
}

/// Deleting a task must never free its display id for reuse — a recycled id
/// would silently re-point links (roadmap deps, branches, threads) at a
/// different task. Soft delete keeps the number retired.
#[tokio::test]
async fn deleted_task_ids_are_never_reused() -> Result<()> {
    let h = Harness::start().await?;
    let ws = common::fresh_workspace(&h, "demo").await?;

    // Create the first task → DEM-001.
    let first = h
        .tasks()
        .create(create_task_req(&ws, "First"))
        .await?
        .into_inner();
    assert_eq!(first.external_id, "DEM-001");

    // Delete it.
    h.tasks()
        .delete(DeleteTaskRequest {
            workspace_id: ws.clone(),
            task_id: first.id.clone(),
        })
        .await?;

    // The next task must be DEM-002, NOT the freed DEM-001.
    let second = h
        .tasks()
        .create(create_task_req(&ws, "Second"))
        .await?
        .into_inner();
    assert_eq!(second.external_id, "DEM-002");

    // Only the live task is listed; the deleted one stays hidden.
    let tasks = h
        .tasks()
        .list(ListTasksRequest {
            workspace_id: ws.clone(),
        })
        .await?
        .into_inner()
        .tasks;
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].external_id.as_deref(), Some("DEM-002"));

    Ok(())
}

/// A soft-deleted task is archived: every task-scoped mutation 404s, so neither
/// a stale client nor an agent can edit or advance it behind the scenes.
#[tokio::test]
async fn deleted_task_rejects_followup_mutations() -> Result<()> {
    let h = Harness::start().await?;
    let ws = common::fresh_workspace(&h, "demo").await?;

    let task = h
        .tasks()
        .create(create_task_req(&ws, "T"))
        .await?
        .into_inner();
    let id = task.id;
    h.tasks()
        .delete(DeleteTaskRequest {
            workspace_id: ws.clone(),
            task_id: id.clone(),
        })
        .await?;

    // Edit the task → NotFound.
    let err = h
        .tasks()
        .update(UpdateTaskRequest {
            workspace_id: ws.clone(),
            task_id: id.clone(),
            approved: Some(true),
            max_variants: None,
            worktree: None,
            description: None,
            title: None,
        })
        .await
        .expect_err("editing a deleted task must 404");
    assert_eq!(err.code(), Code::NotFound);

    // Advance it on the roadmap → NotFound.
    let err = h
        .roadmap()
        .set_status(SetRoadmapStatusRequest {
            workspace_id: ws.clone(),
            task_id: id.clone(),
            status: "planning".into(),
            depends_on: vec![],
            notes: None,
        })
        .await
        .expect_err("advancing a deleted task must 404");
    assert_eq!(err.code(), Code::NotFound);

    Ok(())
}
