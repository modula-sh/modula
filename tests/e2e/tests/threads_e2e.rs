//! ThreadService over gRPC IPC: task-scope + variant-scope comments grouped
//! correctly, `NotFound` on unknown variant/task, task delete wipes thread rows.
//! Also covers agent-author entries (researcher questions, code-reviewer/reviewer
//! verdicts) and the validation gates they ride on.

use anyhow::Result;
use modula_rpc::convert::{str_to_kind, str_to_verdict};
use modula_rpc::v1::{
    AppendEntryRequest, DeleteEntryRequest, DeleteTaskRequest, EditEntryRequest, GetThreadsRequest,
    GetThreadsResponse,
};
use modula_test_support::Harness;
use serde_json::{json, Value as Json};
use tonic::Code;

mod common;

/// A task-scope `comment` from `human` — the common case; tests override the
/// fields they exercise.
fn comment(ws: &str, task: &str, content: &str) -> AppendEntryRequest {
    AppendEntryRequest {
        workspace_id: ws.to_string(),
        task_id: task.to_string(),
        content: content.to_string(),
        author: "human".to_string(),
        kind: str_to_kind("comment"),
        variant_id: None,
        round: None,
        verdict: None,
        affected_variants: vec![],
    }
}

async fn get_threads(
    h: &Harness,
    ws: &str,
    task: &str,
) -> Result<GetThreadsResponse, tonic::Status> {
    Ok(h.threads()
        .get_threads(GetThreadsRequest {
            workspace_id: ws.to_string(),
            task_id: task.to_string(),
        })
        .await?
        .into_inner())
}

/// The most recent DB event of the given type (the engine lists newest-first).
async fn latest_event_of(h: &Harness, ws: &str, type_: &str) -> Result<Json> {
    common::list_events(h, ws)
        .await?
        .into_iter()
        .find(|(ty, _)| ty == type_)
        .map(|(_, data)| data)
        .ok_or_else(|| anyhow::anyhow!("no event of type {type_}"))
}

#[tokio::test]
async fn task_and_variant_comments_roundtrip() -> Result<()> {
    let h = Harness::start().await?;
    let ws = common::fresh_workspace(&h, "demo").await?;
    let task_id = common::create_task(&h, &ws, "First").await?;

    // Empty thread → both groups present and empty (no variants yet).
    let t = get_threads(&h, &ws, &task_id).await?;
    assert_eq!(t.task_id, task_id);
    assert!(t.task_thread.is_empty());
    assert!(t.variant_threads.is_empty());

    // Task-scope comment.
    h.threads()
        .append_entry(comment(&ws, &task_id, "hello task"))
        .await?;

    // Unknown variant UUID (no variants seeded yet) → NotFound.
    let mut unknown_variant = comment(&ws, &task_id, "hi");
    unknown_variant.variant_id = Some("00000000-0000-0000-0000-000000000000".into());
    let err = h
        .threads()
        .append_entry(unknown_variant)
        .await
        .expect_err("unknown variant UUID should 404");
    assert_eq!(err.code(), Code::NotFound);

    // Whitespace-only content → InvalidArgument.
    let err = h
        .threads()
        .append_entry(comment(&ws, &task_id, "   "))
        .await
        .expect_err("empty content should be rejected");
    assert_eq!(err.code(), Code::InvalidArgument);

    // Task thread now has one row.
    let t = get_threads(&h, &ws, &task_id).await?;
    assert_eq!(t.task_thread.len(), 1);
    let e = &t.task_thread[0];
    assert_eq!(e.author, "human");
    assert_eq!(e.kind, str_to_kind("comment"));
    assert_eq!(e.content, "hello task");
    assert!(!e.ts.is_empty());

    // Delete the task → thread rows should be gone (404 on read).
    h.tasks()
        .delete(DeleteTaskRequest {
            workspace_id: ws.clone(),
            task_id: task_id.clone(),
        })
        .await?;
    let err = get_threads(&h, &ws, &task_id)
        .await
        .expect_err("task gone → 404 on threads read");
    assert_eq!(err.code(), Code::NotFound);

    Ok(())
}

#[tokio::test]
async fn researcher_question_roundtrips_and_emits_event() -> Result<()> {
    let h = Harness::start().await?;
    let ws = common::fresh_workspace(&h, "demo").await?;
    let id = common::create_task(&h, &ws, "T").await?;

    let mut q = comment(&ws, &id, "what is X?");
    q.author = "researcher".to_string();
    q.kind = str_to_kind("question");
    h.threads().append_entry(q).await?;

    let t = get_threads(&h, &ws, &id).await?;
    assert_eq!(t.task_thread.len(), 1);
    assert_eq!(t.task_thread[0].author, "researcher");
    assert_eq!(t.task_thread[0].kind, str_to_kind("question"));
    assert_eq!(t.task_thread[0].content, "what is X?");

    let ev = latest_event_of(&h, &ws, "thread.append").await?;
    assert_eq!(ev["task_id"], json!(id));
    assert_eq!(ev["author"], json!("researcher"));
    assert_eq!(ev["kind"], json!("question"));
    Ok(())
}

#[tokio::test]
async fn variant_verdict_accept_roundtrips_and_emits_event() -> Result<()> {
    let h = Harness::start().await?;
    let ws = common::fresh_workspace(&h, "demo").await?;
    let id = common::create_task(&h, &ws, "T").await?;
    let variants = common::create_variants(&h, &ws, &id, 1).await?;
    let (v_uuid, _) = &variants[0];

    let mut v = comment(&ws, &id, "looks good");
    v.variant_id = Some(v_uuid.clone());
    v.author = "code-reviewer".to_string();
    v.kind = str_to_kind("verdict");
    v.verdict = str_to_verdict("ACCEPT");
    h.threads().append_entry(v).await?;

    let t = get_threads(&h, &ws, &id).await?;
    let vt = t
        .variant_threads
        .iter()
        .find(|vt| &vt.variant_id == v_uuid)
        .expect("variant thread entry");
    assert_eq!(vt.entries.len(), 1);
    assert_eq!(vt.entries[0].author, "code-reviewer");
    assert_eq!(vt.entries[0].verdict, str_to_verdict("ACCEPT"));

    let ev = latest_event_of(&h, &ws, "thread.append").await?;
    assert_eq!(ev["variant_id"], json!(v_uuid));
    assert_eq!(ev["verdict"], json!("ACCEPT"));
    Ok(())
}

#[tokio::test]
async fn task_verdict_approve_and_kickback() -> Result<()> {
    let h = Harness::start().await?;
    let ws = common::fresh_workspace(&h, "demo").await?;
    let id = common::create_task(&h, &ws, "T").await?;
    let variants = common::create_variants(&h, &ws, &id, 1).await?;
    let (v_uuid, _) = &variants[0];

    // APPROVE at task scope: works.
    let mut approve = comment(&ws, &id, "ship it");
    approve.author = "reviewer".to_string();
    approve.kind = str_to_kind("verdict");
    approve.verdict = str_to_verdict("APPROVE");
    h.threads().append_entry(approve).await?;

    // KICK_BACK with affected_variants: works + event mirrors verdict.
    let mut kickback = comment(&ws, &id, "needs rework");
    kickback.author = "reviewer".to_string();
    kickback.kind = str_to_kind("verdict");
    kickback.verdict = str_to_verdict("KICK_BACK");
    kickback.affected_variants = vec![v_uuid.clone()];
    h.threads().append_entry(kickback).await?;

    let ev = latest_event_of(&h, &ws, "thread.append").await?;
    assert_eq!(ev["verdict"], json!("KICK_BACK"));

    let t = get_threads(&h, &ws, &id).await?;
    assert_eq!(t.task_thread.len(), 2);
    let kb = t
        .task_thread
        .iter()
        .find(|e| e.verdict == str_to_verdict("KICK_BACK"))
        .expect("kickback entry");
    assert_eq!(kb.affected_variants, vec![v_uuid.clone()]);

    // KICK_BACK without affected_variants → InvalidArgument. (At the wire level an
    // absent and an empty `affected_variants` are the same empty repeated field.)
    let mut bad = comment(&ws, &id, "bad");
    bad.author = "reviewer".to_string();
    bad.kind = str_to_kind("verdict");
    bad.verdict = str_to_verdict("KICK_BACK");
    let err = h
        .threads()
        .append_entry(bad)
        .await
        .expect_err("kickback needs affected");
    assert_eq!(err.code(), Code::InvalidArgument);

    // affected_variants with a non-KICK_BACK verdict → InvalidArgument.
    let mut bad = comment(&ws, &id, "bad");
    bad.author = "reviewer".to_string();
    bad.kind = str_to_kind("verdict");
    bad.verdict = str_to_verdict("APPROVE");
    bad.affected_variants = vec![v_uuid.clone()];
    let err = h
        .threads()
        .append_entry(bad)
        .await
        .expect_err("affected only with kickback");
    assert_eq!(err.code(), Code::InvalidArgument);
    Ok(())
}

#[tokio::test]
async fn verdict_scope_and_missing_value_validation() -> Result<()> {
    let h = Harness::start().await?;
    let ws = common::fresh_workspace(&h, "demo").await?;
    let id = common::create_task(&h, &ws, "T").await?;
    let variants = common::create_variants(&h, &ws, &id, 1).await?;
    let (v_uuid, _) = &variants[0];

    // ACCEPT at task scope → InvalidArgument (variant-only verdict).
    let mut bad = comment(&ws, &id, "x");
    bad.author = "code-reviewer".to_string();
    bad.kind = str_to_kind("verdict");
    bad.verdict = str_to_verdict("ACCEPT");
    let err = h
        .threads()
        .append_entry(bad)
        .await
        .expect_err("accept is variant-only");
    assert_eq!(err.code(), Code::InvalidArgument);

    // APPROVE at variant scope → InvalidArgument (task-only verdict).
    let mut bad = comment(&ws, &id, "x");
    bad.variant_id = Some(v_uuid.clone());
    bad.author = "reviewer".to_string();
    bad.kind = str_to_kind("verdict");
    bad.verdict = str_to_verdict("APPROVE");
    let err = h
        .threads()
        .append_entry(bad)
        .await
        .expect_err("approve is task-only");
    assert_eq!(err.code(), Code::InvalidArgument);

    // kind=verdict but no verdict value → InvalidArgument.
    let mut bad = comment(&ws, &id, "x");
    bad.author = "reviewer".to_string();
    bad.kind = str_to_kind("verdict");
    let err = h
        .threads()
        .append_entry(bad)
        .await
        .expect_err("verdict kind needs a value");
    assert_eq!(err.code(), Code::InvalidArgument);

    // Whitespace-only author → InvalidArgument (authors are otherwise open).
    let mut bad = comment(&ws, &id, "x");
    bad.author = "   ".to_string();
    let err = h
        .threads()
        .append_entry(bad)
        .await
        .expect_err("empty author rejected");
    assert_eq!(err.code(), Code::InvalidArgument);
    Ok(())
}

// Authors are an open set: any user-defined agent name round-trips verbatim.
// `pr-creator` is the historical regression — enum-mapped but whitelist-rejected.
#[tokio::test]
async fn custom_agent_author_roundtrips_and_emits_event() -> Result<()> {
    let h = Harness::start().await?;
    let ws = common::fresh_workspace(&h, "demo").await?;
    let id = common::create_task(&h, &ws, "T").await?;

    for author in ["my-custom-agent", "pr-creator"] {
        let mut c = comment(&ws, &id, "hi from a user-defined agent");
        c.author = author.to_string();
        h.threads().append_entry(c).await?;

        let ev = latest_event_of(&h, &ws, "thread.append").await?;
        assert_eq!(ev["author"], json!(author));
    }

    let t = get_threads(&h, &ws, &id).await?;
    assert_eq!(t.task_thread.len(), 2);
    assert_eq!(t.task_thread[0].author, "my-custom-agent");
    assert_eq!(t.task_thread[1].author, "pr-creator");
    Ok(())
}

#[tokio::test]
async fn unknown_task_returns_not_found() -> Result<()> {
    let h = Harness::start().await?;
    let ws = common::fresh_workspace(&h, "demo").await?;

    let err = get_threads(&h, &ws, "00000000-0000-0000-0000-000000000000")
        .await
        .expect_err("unknown task should 404");
    assert_eq!(err.code(), Code::NotFound);
    Ok(())
}

#[tokio::test]
async fn entry_exposes_id() -> Result<()> {
    let h = Harness::start().await?;
    let ws = common::fresh_workspace(&h, "demo").await?;
    let id = common::create_task(&h, &ws, "T").await?;

    let entry = h
        .threads()
        .append_entry(comment(&ws, &id, "hi"))
        .await?
        .into_inner()
        .entry
        .expect("appended entry");
    assert!(entry.id > 0, "entry should carry a positive id");

    // The id is also present when read back through the thread listing.
    let t = get_threads(&h, &ws, &id).await?;
    assert_eq!(t.task_thread[0].id, entry.id);
    Ok(())
}

#[tokio::test]
async fn owner_edits_and_deletes_comment() -> Result<()> {
    let h = Harness::start().await?;
    let ws = common::fresh_workspace(&h, "demo").await?;
    let id = common::create_task(&h, &ws, "T").await?;

    let entry_id = h
        .threads()
        .append_entry(comment(&ws, &id, "first"))
        .await?
        .into_inner()
        .entry
        .expect("entry")
        .id;

    // Owner edits the content.
    h.threads()
        .edit_entry(EditEntryRequest {
            workspace_id: ws.clone(),
            task_id: id.clone(),
            entry_id,
            content: "edited".into(),
            author: "human".to_string(),
        })
        .await?;

    let t = get_threads(&h, &ws, &id).await?;
    assert_eq!(t.task_thread.len(), 1);
    assert_eq!(t.task_thread[0].content, "edited");

    // thread.update event emitted (numbers cross `Struct` as f64).
    let ev = latest_event_of(&h, &ws, "thread.update").await?;
    assert_eq!(ev["entry_id"].as_f64(), Some(entry_id as f64));
    assert_eq!(ev["author"], json!("human"));

    // Owner deletes.
    h.threads()
        .delete_entry(DeleteEntryRequest {
            workspace_id: ws.clone(),
            task_id: id.clone(),
            entry_id,
            author: "human".to_string(),
        })
        .await?;

    let t = get_threads(&h, &ws, &id).await?;
    assert!(t.task_thread.is_empty(), "comment row should be gone");

    let ev = latest_event_of(&h, &ws, "thread.delete").await?;
    assert_eq!(ev["entry_id"].as_f64(), Some(entry_id as f64));
    Ok(())
}

#[tokio::test]
async fn mutation_authorization_and_validation() -> Result<()> {
    let h = Harness::start().await?;
    let ws = common::fresh_workspace(&h, "demo").await?;
    let id = common::create_task(&h, &ws, "T").await?;

    // A human comment and an agent (researcher) question to test the gates.
    let human_id = h
        .threads()
        .append_entry(comment(&ws, &id, "mine"))
        .await?
        .into_inner()
        .entry
        .expect("entry")
        .id;
    let mut q = comment(&ws, &id, "q?");
    q.author = "researcher".to_string();
    q.kind = str_to_kind("question");
    let question_id = h
        .threads()
        .append_entry(q)
        .await?
        .into_inner()
        .entry
        .expect("entry")
        .id;

    // Non-owner editing the human comment → PermissionDenied.
    let err = h
        .threads()
        .edit_entry(EditEntryRequest {
            workspace_id: ws.clone(),
            task_id: id.clone(),
            entry_id: human_id,
            content: "hijack".into(),
            author: "researcher".to_string(),
        })
        .await
        .expect_err("non-owner edit should be forbidden");
    assert_eq!(err.code(), Code::PermissionDenied);

    // Non-owner deleting → PermissionDenied (human cannot delete the AI question).
    let err = h
        .threads()
        .delete_entry(DeleteEntryRequest {
            workspace_id: ws.clone(),
            task_id: id.clone(),
            entry_id: question_id,
            author: "human".to_string(),
        })
        .await
        .expect_err("non-owner delete should be forbidden");
    assert_eq!(err.code(), Code::PermissionDenied);

    // Unknown entry id → NotFound.
    let err = h
        .threads()
        .edit_entry(EditEntryRequest {
            workspace_id: ws.clone(),
            task_id: id.clone(),
            entry_id: 999_999,
            content: "x".into(),
            author: "human".to_string(),
        })
        .await
        .expect_err("unknown id should 404");
    assert_eq!(err.code(), Code::NotFound);

    // Editing a non-comment kind (owner of the question) → InvalidArgument.
    let err = h
        .threads()
        .edit_entry(EditEntryRequest {
            workspace_id: ws.clone(),
            task_id: id.clone(),
            entry_id: question_id,
            content: "x".into(),
            author: "researcher".to_string(),
        })
        .await
        .expect_err("non-comment edit should be rejected");
    assert_eq!(err.code(), Code::InvalidArgument);

    // Empty edit content → InvalidArgument.
    let err = h
        .threads()
        .edit_entry(EditEntryRequest {
            workspace_id: ws.clone(),
            task_id: id.clone(),
            entry_id: human_id,
            content: "   ".into(),
            author: "human".to_string(),
        })
        .await
        .expect_err("empty content should be rejected");
    assert_eq!(err.code(), Code::InvalidArgument);
    Ok(())
}
