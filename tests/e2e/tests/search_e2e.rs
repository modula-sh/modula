//! Workspace-wide search over gRPC IPC: the kinds that match, excerpt
//! highlighting, kind filtering, and that a deleted task drops out.

use anyhow::Result;
use modula_rpc::v1::{
    AppendEntryRequest, CreateConversationRequest, CreateProjectRequest, DeleteTaskRequest,
    SearchHit, SearchRequest, ThreadKind,
};
use modula_test_support::fixtures as common;
use modula_test_support::Harness;

/// A word none of the seeded defaults contain, so every match is one we made.
const NEEDLE: &str = "quixotic";

async fn search(h: &Harness, ws: &str, query: &str, kinds: &[&str]) -> Result<Vec<SearchHit>> {
    Ok(h.search()
        .search(SearchRequest {
            workspace_id: ws.to_string(),
            query: query.to_string(),
            kinds: kinds.iter().map(|k| k.to_string()).collect(),
            limit: 0,
        })
        .await?
        .into_inner()
        .hits)
}

fn of_kind<'a>(hits: &'a [SearchHit], kind: &str) -> Vec<&'a SearchHit> {
    hits.iter().filter(|h| h.kind == kind).collect()
}

#[tokio::test]
async fn search_spans_every_kind_and_respects_deletion() -> Result<()> {
    let h = Harness::start().await?;
    let ws = common::fresh_workspace(&h, "demo").await?;

    // A task matching on its title, and one matching only through a comment.
    let titled = common::create_task(&h, &ws, &format!("A {NEEDLE} task")).await?;
    let commented = common::create_task(&h, &ws, "Ordinary task").await?;
    h.threads()
        .append_entry(AppendEntryRequest {
            workspace_id: ws.clone(),
            task_id: commented.clone(),
            content: format!("the fix here is {NEEDLE} indeed"),
            author: "human".into(),
            kind: ThreadKind::Comment as i32,
            variant_id: None,
            round: None,
            verdict: None,
            affected_variants: vec![],
        })
        .await?;

    let provider =
        common::create_provider(&h, &ws, NEEDLE, h.workspace_path(&ws).as_path()).await?;
    common::create_agent(&h, &ws, &provider, &format!("{NEEDLE}-agent"), &[], true).await?;
    h.projects()
        .create(CreateProjectRequest {
            workspace_id: ws.clone(),
            name: format!("{NEEDLE}-project"),
            path: "/tmp/nowhere".into(),
            base_branch: "main".into(),
        })
        .await?;
    h.conversations()
        .create(CreateConversationRequest {
            workspace_id: ws.clone(),
            provider_id: provider.clone(),
            title: Some(format!("{NEEDLE} chat")),
            model: None,
            context: None,
        })
        .await?;

    let hits = search(&h, &ws, NEEDLE, &[]).await?;

    // A title match is its own row 1, so it carries no excerpt.
    let tasks = of_kind(&hits, "task");
    let title_hit = tasks
        .iter()
        .find(|h| h.id == titled)
        .expect("title match missing");
    assert_eq!(title_hit.field, "title");
    assert!(title_hit.excerpt.is_empty());

    // A comment has no view of its own, so it surfaces as its owning task with
    // the matching run highlighted.
    let comment_hit = tasks
        .iter()
        .find(|h| h.id == commented)
        .expect("comment match missing");
    assert_eq!(comment_hit.field, "comment");
    assert_eq!(comment_hit.title, "Ordinary task");
    assert_eq!(
        comment_hit
            .excerpt
            .iter()
            .filter(|s| s.is_match)
            .map(|s| s.text.as_str())
            .collect::<Vec<_>>(),
        [NEEDLE]
    );

    for kind in ["agent", "project", "provider", "conversation"] {
        assert_eq!(of_kind(&hits, kind).len(), 1, "{kind}: {hits:#?}");
    }

    // `kinds` narrows the fan-out.
    let agents = search(&h, &ws, NEEDLE, &["agent"]).await?;
    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0].kind, "agent");

    // An empty query is not an error — it is simply no results.
    assert!(search(&h, &ws, "   ", &[]).await?.is_empty());

    // A deleted task drops out, and so does the comment that pointed at it.
    h.tasks()
        .delete(DeleteTaskRequest {
            workspace_id: ws.clone(),
            task_id: commented.clone(),
        })
        .await?;
    let after = search(&h, &ws, NEEDLE, &["task"]).await?;
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].id, titled);

    Ok(())
}
