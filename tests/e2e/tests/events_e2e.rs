//! DB event emission over gRPC IPC: CRUD emits typed events, `task.update`
//! carries only changed fields, and manual emission via `EventService.Create`.

use anyhow::Result;
use modula_rpc::json::json_to_struct;
use modula_rpc::v1::{CreateEventRequest, DeleteTaskRequest, UpdateTaskRequest};
use modula_test_support::Harness;
use serde_json::json;
use tonic::Code;

use modula_test_support::fixtures as common;

#[tokio::test]
async fn crud_emits_events() -> Result<()> {
    let h = Harness::start().await?;
    let ws = common::fresh_workspace(&h, "demo").await?;

    // No events at start.
    assert!(common::list_events(&h, &ws).await?.is_empty());

    // Create → task.create.
    let id = common::create_task(&h, &ws, "first").await?;

    // Update → task.update.
    h.tasks()
        .update(UpdateTaskRequest {
            workspace_id: ws.clone(),
            task_id: id.clone(),
            approved: Some(true),
            ..Default::default()
        })
        .await?;

    // Delete → task.delete.
    h.tasks()
        .delete(DeleteTaskRequest {
            workspace_id: ws.clone(),
            task_id: id.clone(),
        })
        .await?;

    let events = common::list_events(&h, &ws).await?;
    let types: Vec<&str> = events.iter().map(|(ty, _)| ty.as_str()).collect();
    assert!(types.contains(&"task.create"));
    assert!(types.contains(&"task.update"));
    assert!(types.contains(&"task.delete"));

    let (_, create) = events.iter().find(|(ty, _)| ty == "task.create").unwrap();
    assert_eq!(create["task_id"], json!(id));
    assert_eq!(create["source"], json!("internal"));

    let (_, update) = events.iter().find(|(ty, _)| ty == "task.update").unwrap();
    assert_eq!(update["task_id"], json!(id));
    assert_eq!(update["approved"], json!(true));

    let (_, del) = events.iter().find(|(ty, _)| ty == "task.delete").unwrap();
    assert_eq!(del["task_id"], json!(id));

    Ok(())
}

// A `task.update` event must carry only the fields that actually changed, even
// when the client sends the whole editable form (as the desktop debounce save
// does). Otherwise editing the title of an already-approved task re-emits
// `approved`, spuriously firing edge-triggered agent rules.
#[tokio::test]
async fn update_event_only_carries_changed_fields() -> Result<()> {
    let h = Harness::start().await?;
    let ws = common::fresh_workspace(&h, "demo").await?;
    let id = common::create_task(&h, &ws, "first").await?;

    // Approve the task (the transition rules care about).
    h.tasks()
        .update(UpdateTaskRequest {
            workspace_id: ws.clone(),
            task_id: id.clone(),
            approved: Some(true),
            ..Default::default()
        })
        .await?;

    // Edit only the title, but re-send the whole form (approved unchanged) —
    // exactly what the debounce save does.
    let whole_form = || UpdateTaskRequest {
        workspace_id: ws.clone(),
        task_id: id.clone(),
        title: Some("renamed".into()),
        description: Some(String::new()),
        approved: Some(true),
        max_variants: None,
        worktree: None,
    };
    h.tasks().update(whole_form()).await?;

    let events = common::list_events(&h, &ws).await?;
    let updates: Vec<_> = events
        .iter()
        .filter(|(ty, _)| ty == "task.update")
        .collect();
    assert_eq!(updates.len(), 2, "expected exactly two task.update events");

    // The approval transition is emitted as its own event carrying `approved`.
    let (_, approval) = updates
        .iter()
        .find(|(_, data)| data.get("approved").is_some())
        .expect("an update event carrying `approved`");
    assert_eq!(approval["approved"], json!(true));

    // The title edit must carry `title` and NOT `approved`, since `approved`
    // did not change — exactly one such event.
    let (_, title_edit) = updates
        .iter()
        .find(|(_, data)| data.get("title").is_some())
        .expect("an update event carrying `title`");
    assert_eq!(title_edit["title"], json!("renamed"));
    assert!(
        title_edit.get("approved").is_none(),
        "title-only edit must not re-emit unchanged `approved`: {title_edit}"
    );

    // A fully redundant save (nothing changed) emits no event at all.
    h.tasks().update(whole_form()).await?;

    let updates = common::list_events(&h, &ws)
        .await?
        .into_iter()
        .filter(|(ty, _)| ty == "task.update")
        .count();
    assert_eq!(updates, 2, "no-op save must not emit a task.update event");

    Ok(())
}

#[tokio::test]
async fn manual_event_emission() -> Result<()> {
    let h = Harness::start().await?;
    let ws = common::fresh_workspace(&h, "demo").await?;

    // Empty type → InvalidArgument.
    let err = h
        .events()
        .create(CreateEventRequest {
            workspace_id: ws.clone(),
            r#type: String::new(),
            data: json_to_struct(json!({})),
        })
        .await
        .expect_err("empty event type must be rejected");
    assert_eq!(err.code(), Code::InvalidArgument);

    h.events()
        .create(CreateEventRequest {
            workspace_id: ws.clone(),
            r#type: "custom.kind".into(),
            data: json_to_struct(json!({"x": 1})),
        })
        .await?;

    let events = common::list_events(&h, &ws).await?;
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].0, "custom.kind");
    // Numbers round-trip through `google.protobuf.Struct` as f64.
    assert_eq!(events[0].1["x"].as_f64(), Some(1.0));
    Ok(())
}
