//! Variant create + status update behaviour, including event emission and the
//! `action` body shape for accept/rework, over gRPC IPC.

use anyhow::Result;
use modula_rpc::v1::{CreateVariantsRequest, UpdateVariantRequest};
use modula_test_support::Harness;
use serde_json::json;
use tonic::Code;

mod common;

fn create_variants_req(ws: &str, task: &str, count: u32) -> CreateVariantsRequest {
    CreateVariantsRequest {
        workspace_id: ws.to_string(),
        task_id: task.to_string(),
        count,
    }
}

#[tokio::test]
async fn create_variants_registers_without_status_or_events() -> Result<()> {
    let h = Harness::start().await?;
    let ws = common::fresh_workspace(&h, "demo").await?;
    let task_id = common::create_task(&h, &ws, "T").await?;

    let created = h
        .variants()
        .create(create_variants_req(&ws, &task_id, 2))
        .await?
        .into_inner()
        .created;
    assert_eq!(created.len(), 2);

    // Each created entry has a UUID id and a position.
    for entry in &created {
        assert_eq!(
            entry.id.len(),
            36,
            "expected UUID variant id, got: {}",
            entry.id
        );
    }

    // Registration leaves variants statusless so no worker spawns yet.
    let task = common::get_task(&h, &ws, &task_id).await?;
    assert_eq!(task.variants.len(), 2);
    assert!(task.variants.iter().all(|v| v.status.is_none()));

    // And it emits no variant.update events.
    let evs = common::list_events(&h, &ws).await?;
    assert!(evs.iter().all(|(ty, _)| ty != "variant.update"));
    Ok(())
}

#[tokio::test]
async fn create_variants_rejects_bad_count() -> Result<()> {
    let h = Harness::start().await?;
    let ws = common::fresh_workspace(&h, "demo").await?;
    let task_id = common::create_task(&h, &ws, "T").await?;

    for count in [0u32, 11] {
        let err = h
            .variants()
            .create(create_variants_req(&ws, &task_id, count))
            .await
            .expect_err("bad count must be rejected");
        assert_eq!(err.code(), Code::InvalidArgument, "count={count}");
    }

    Ok(())
}

#[tokio::test]
async fn update_variant_status_and_action() -> Result<()> {
    let h = Harness::start().await?;
    let ws = common::fresh_workspace(&h, "demo").await?;
    let task_id = common::create_task(&h, &ws, "T").await?;

    let variants = common::create_variants(&h, &ws, &task_id, 1).await?;
    let (variant_id, _pos) = &variants[0];

    // Status update emits variant.update.
    h.variants()
        .update(UpdateVariantRequest {
            workspace_id: ws.clone(),
            task_id: task_id.clone(),
            variant_id: variant_id.clone(),
            status: Some("ready_for_review".into()),
            action: None,
        })
        .await?;

    let evs = common::list_events(&h, &ws).await?;
    let ready = evs
        .iter()
        .find(|(ty, data)| {
            ty == "variant.update"
                && data["status"] == json!("ready_for_review")
                && data["variant_id"] == json!(variant_id)
        })
        .expect("ready_for_review event");
    assert_eq!(ready.1["task_id"], json!(task_id));

    // Unknown status → InvalidArgument.
    let err = h
        .variants()
        .update(UpdateVariantRequest {
            workspace_id: ws.clone(),
            task_id: task_id.clone(),
            variant_id: variant_id.clone(),
            status: Some("nonsense".into()),
            action: None,
        })
        .await
        .expect_err("unknown status must be rejected");
    assert_eq!(err.code(), Code::InvalidArgument);

    // Neither status nor action → InvalidArgument.
    let err = h
        .variants()
        .update(UpdateVariantRequest {
            workspace_id: ws.clone(),
            task_id: task_id.clone(),
            variant_id: variant_id.clone(),
            status: None,
            action: None,
        })
        .await
        .expect_err("empty update must be rejected");
    assert_eq!(err.code(), Code::InvalidArgument);

    // The action shape resolves to a status.
    let resp = h
        .variants()
        .update(UpdateVariantRequest {
            workspace_id: ws.clone(),
            task_id: task_id.clone(),
            variant_id: variant_id.clone(),
            status: None,
            action: Some("accept".into()),
        })
        .await?
        .into_inner();
    assert_eq!(resp.status, "accepted");
    Ok(())
}

// Re-PUTting the status a variant already holds is a no-op: it must not emit a
// second `variant.update`, or an edge-triggered rule would spawn a duplicate
// worker. The request still succeeds (idempotent).
#[tokio::test]
async fn redundant_variant_status_emits_no_event() -> Result<()> {
    let h = Harness::start().await?;
    let ws = common::fresh_workspace(&h, "demo").await?;
    let task_id = common::create_task(&h, &ws, "T").await?;
    let variants = common::create_variants(&h, &ws, &task_id, 1).await?;
    let (variant_id, _pos) = &variants[0];

    let set_status = |status: &'static str| {
        let h = &h;
        let (ws, task_id, variant_id) = (ws.clone(), task_id.clone(), variant_id.clone());
        async move {
            h.variants()
                .update(UpdateVariantRequest {
                    workspace_id: ws,
                    task_id,
                    variant_id,
                    status: Some(status.to_string()),
                    action: None,
                })
                .await
        }
    };
    let count_updates = || {
        let h = &h;
        let ws = ws.clone();
        async move {
            let evs = common::list_events(h, &ws).await?;
            Ok::<usize, anyhow::Error>(evs.iter().filter(|(ty, _)| ty == "variant.update").count())
        }
    };

    // First transition emits one event.
    set_status("ready_for_workers").await?;
    assert_eq!(count_updates().await?, 1);

    // Re-setting the same status succeeds but emits nothing new.
    set_status("ready_for_workers").await?;
    assert_eq!(count_updates().await?, 1, "no-op status must not re-emit");

    // A genuine transition emits again.
    set_status("in_progress").await?;
    assert_eq!(count_updates().await?, 2);

    Ok(())
}
