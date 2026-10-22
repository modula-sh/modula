//! Variant plan documents over gRPC IPC: set/get, validation errors,
//! `plan.update` events, progress on task reads, and deletion on task reset.

use std::time::Duration;

use anyhow::Result;
use modula_rpc::v1::{
    patch_plan_request::Op, CreateProjectRequest, GetPlanRequest, ListRunsForAgentRequest,
    PatchPlanRequest, PlanSetTaskStatus, ResetTaskRequest, SetPlanRequest,
};
use modula_test_support::Harness;
use serde_json::{json, Value};
use tonic::Code;

use modula_test_support::fixtures as common;

struct Variant {
    ws: String,
    task: String,
    id: String,
}

async fn seed(h: &Harness) -> Result<Variant> {
    let ws = common::fresh_workspace(h, "plans").await?;
    h.projects()
        .create(CreateProjectRequest {
            workspace_id: ws.clone(),
            name: "Modula".into(),
            path: "/tmp/modula".into(),
            base_branch: "main".into(),
        })
        .await?;
    let task = common::create_task(h, &ws, "Plan me").await?;
    let id = common::create_variants(h, &ws, &task, 1).await?.remove(0).0;
    Ok(Variant { ws, task, id })
}

fn doc() -> String {
    json!({
        "version": 1, "title": "MOD-1 Plan me", "problem": "p", "approach": "a",
        "projects": [{"name": "Modula", "changes": [{"path": "src/lib.rs", "change": "edit"}]}],
        "phases": [
            {"title": "Engine", "scope": "s", "approach": "a",
             "tasks": [{"id": "x", "title": "Types"}, {"title": "Service"}]},
            {"title": "CLI", "scope": "s", "approach": "a", "tasks": [{"title": "Commands"}]}
        ],
        "risks": "r", "test_plan": "t"
    })
    .to_string()
}

fn set_req(v: &Variant, plan: String) -> SetPlanRequest {
    SetPlanRequest {
        workspace_id: v.ws.clone(),
        task_id: v.task.clone(),
        variant_id: v.id.clone(),
        plan,
    }
}

fn get_req(v: &Variant) -> GetPlanRequest {
    GetPlanRequest {
        workspace_id: v.ws.clone(),
        task_id: v.task.clone(),
        variant_id: v.id.clone(),
    }
}

fn status_req(v: &Variant, task: &str, status: &str) -> PatchPlanRequest {
    PatchPlanRequest {
        workspace_id: v.ws.clone(),
        task_id: v.task.clone(),
        variant_id: v.id.clone(),
        op: Some(Op::SetTaskStatus(PlanSetTaskStatus {
            task: task.into(),
            status: status.into(),
            note: None,
        })),
    }
}

#[tokio::test]
async fn set_get_round_trip_assigns_ids() -> Result<()> {
    let h = Harness::start().await?;
    let v = seed(&h).await?;

    let progress = h
        .plans()
        .set(set_req(&v, doc()))
        .await?
        .into_inner()
        .progress
        .expect("progress");
    assert_eq!(progress.tasks_total, 3);
    assert_eq!(progress.current_task_id.as_deref(), Some("1.1"));

    let plan: Value = serde_json::from_str(&h.plans().get(get_req(&v)).await?.into_inner().plan)?;
    assert_eq!(plan["phases"][0]["tasks"][0]["id"], "1.1");
    assert_eq!(plan["phases"][1]["id"], "2");
    assert_eq!(plan["phases"][1]["tasks"][0]["id"], "2.1");
    assert_eq!(plan["phases"][1]["tasks"][0]["status"], "pending");
    Ok(())
}

#[tokio::test]
async fn bad_writes_map_to_not_found_and_invalid_argument() -> Result<()> {
    let h = Harness::start().await?;
    let v = seed(&h).await?;

    let err = h.plans().get(get_req(&v)).await.expect_err("no plan yet");
    assert_eq!(err.code(), Code::NotFound);

    let unknown = Variant {
        id: "00000000-0000-0000-0000-000000000000".into(),
        ws: v.ws.clone(),
        task: v.task.clone(),
    };
    let err = h
        .plans()
        .set(set_req(&unknown, doc()))
        .await
        .expect_err("unknown variant");
    assert_eq!(err.code(), Code::NotFound);

    let typo = doc().replace("\"scope\"", "\"scop\"");
    let err = h
        .plans()
        .set(set_req(&v, typo))
        .await
        .expect_err("unknown field");
    assert_eq!(err.code(), Code::InvalidArgument);
    assert!(
        err.message().contains("unknown field `scop`"),
        "{}",
        err.message()
    );

    h.plans().set(set_req(&v, doc())).await?;
    let err = h
        .plans()
        .patch(status_req(&v, "1.1", "done"))
        .await
        .expect_err("done before in_progress");
    assert_eq!(err.code(), Code::InvalidArgument);
    assert!(
        err.message().contains("mark it in_progress before done"),
        "{}",
        err.message()
    );
    Ok(())
}

#[tokio::test]
async fn writes_emit_plan_update_and_progress_reaches_task_reads() -> Result<()> {
    let h = Harness::start_with_env(&[("MODULA_DISPATCH_INTERVAL_SECS", "1")]).await?;
    let v = seed(&h).await?;
    let provider_dir = h.modula_dir.join("fake-provider");
    std::fs::create_dir_all(&provider_dir)?;
    let provider = common::create_provider(&h, &v.ws, "p1", &provider_dir).await?;
    let agent = common::create_agent(
        &h,
        &v.ws,
        &provider,
        "variant-watcher",
        &["event.type == 'variant.update'"],
        false,
    )
    .await?;

    h.plans().set(set_req(&v, doc())).await?;
    let resp = h
        .plans()
        .patch(status_req(&v, "1.1", "in_progress"))
        .await?
        .into_inner();
    assert_eq!(resp.id, "1.1");

    let plan_events: Vec<Value> = common::list_events(&h, &v.ws)
        .await?
        .into_iter()
        .filter(|(ty, _)| ty == "plan.update")
        .map(|(_, data)| data)
        .collect();
    assert_eq!(plan_events.len(), 2, "one per write: {plan_events:?}");
    assert!(plan_events
        .iter()
        .all(|d| *d == json!({"task_id": v.task, "variant_id": v.id})));

    let task = common::get_task(&h, &v.ws, &v.task).await?;
    let progress = task.variants[0].plan_progress.clone().expect("progress");
    assert_eq!(progress.tasks_total, 3);
    assert_eq!(progress.current_phase_title.as_deref(), Some("Engine"));
    assert_eq!(progress.current_task_title.as_deref(), Some("Types"));

    let snapshot = common::snapshot(&h, &v.ws).await?;
    let snap_variant = &snapshot["tasks"][0]["variants"][0];
    assert_eq!(snap_variant["plan_progress"]["current_task_id"], "1.1");

    // A `variant.update` rule never fires on plan writes.
    tokio::time::sleep(Duration::from_secs(3)).await;
    let runs = h
        .runs()
        .list_for_agent(ListRunsForAgentRequest {
            workspace_id: v.ws.clone(),
            agent_id: agent,
        })
        .await?
        .into_inner()
        .runs;
    assert!(runs.is_empty(), "plan writes dispatched {runs:?}");
    Ok(())
}

#[tokio::test]
async fn task_reset_deletes_the_plan() -> Result<()> {
    let h = Harness::start().await?;
    let v = seed(&h).await?;
    h.plans().set(set_req(&v, doc())).await?;

    h.tasks()
        .reset(ResetTaskRequest {
            workspace_id: v.ws.clone(),
            task_id: v.task.clone(),
        })
        .await?;

    let err = h
        .plans()
        .get(get_req(&v))
        .await
        .expect_err("plan gone after reset");
    assert_eq!(err.code(), Code::NotFound);
    Ok(())
}
