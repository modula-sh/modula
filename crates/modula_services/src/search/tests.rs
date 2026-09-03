use std::path::PathBuf;

use serde_json::json;

use modula_core::error::ApiError;
use modula_db::conversations::ConversationCreate;

use super::*;
use crate::loop_registry::LoopRegistry;
use crate::scheduler::SchedulerHandle;
use crate::testkit::{env, Env};

/// A word no seeded default row contains, so a match is always the test's own.
const NEEDLE: &str = "quixotic";

struct Fixture {
    svc: SearchService,
    repos: Repositories,
    ws_dir: PathBuf,
    env: Env,
}

impl Fixture {
    async fn search(&self, query: &str) -> Vec<SearchHit> {
        self.svc.search(&self.env.ws, query, &[], 0).await.unwrap()
    }

    fn of_kind(hits: &[SearchHit], kind: SearchKind) -> Vec<&SearchHit> {
        hits.iter().filter(|h| h.kind == kind.as_str()).collect()
    }
}

async fn fixture() -> Fixture {
    let env = env().await;
    let repos = Repositories::new(&env.pool);
    let slug = repos.workspaces.slug_for(&env.pool, &env.ws).await.unwrap();
    // `workspace_dir` 404s without the directory, and the wiki source needs it.
    let ws_dir = env.paths().modula.join(slug);
    std::fs::create_dir_all(ws_dir.join("wiki")).unwrap();
    let scheduler = SchedulerHandle::start(
        env.paths().modula.clone(),
        LoopRegistry::default(),
        String::new(),
        env.sink.clone(),
        repos.clone(),
    )
    .await
    .unwrap();
    let workspaces = WorkspaceService::new(
        env.pool.clone(),
        repos.workspaces.clone(),
        env.paths(),
        scheduler,
    );
    Fixture {
        svc: SearchService::new(workspaces, &repos),
        repos,
        ws_dir,
        env,
    }
}

async fn seed_task(f: &Fixture, title: &str, description: &str) -> String {
    let mut conn = f.env.pool.acquire().await.unwrap();
    f.repos
        .tasks
        .create_internal(
            &mut conn,
            &f.env.ws,
            title,
            "{}",
            None,
            description,
            None,
            None,
            "",
        )
        .await
        .unwrap()
        .0
}

async fn seed_conversation(f: &Fixture, title: &str, messages: &[&str]) -> String {
    let provider = f
        .repos
        .providers
        .list(&f.env.pool, &f.env.ws)
        .await
        .unwrap()[0]
        .id
        .clone();
    let id = uuid::Uuid::new_v4().to_string();
    f.repos
        .conversations
        .create(
            &f.env.pool,
            &f.env.ws,
            &ConversationCreate {
                id: id.clone(),
                title: Some(title.to_string()),
                provider_id: provider,
                model: None,
                context: json!({}),
            },
        )
        .await
        .unwrap();
    for m in messages {
        f.repos
            .conversations
            .append_message(&f.env.pool, &f.env.ws, &id, "user", m, &[])
            .await
            .unwrap();
    }
    id
}

/// A conversation stamped older than every other row. Inserted directly because
/// the `updated_at` trigger rewrites any UPDATE back to now.
async fn seed_oldest_conversation(f: &Fixture, title: &str, message: &str) -> String {
    let provider = f
        .repos
        .providers
        .list(&f.env.pool, &f.env.ws)
        .await
        .unwrap()[0]
        .id
        .clone();
    let id = uuid::Uuid::new_v4().to_string();
    let data = json!({ "messages": [{ "role": "user", "content": message }] });
    sqlx::query(
        "INSERT INTO conversations (workspace_id, id, title, provider_id, data, updated_at) \
         VALUES (?, ?, ?, ?, ?, '2000-01-01T00:00:00.000Z')",
    )
    .bind(&f.env.ws)
    .bind(&id)
    .bind(title)
    .bind(provider)
    .bind(data.to_string())
    .execute(&f.env.pool)
    .await
    .unwrap();
    id
}

#[tokio::test]
async fn task_title_match_carries_no_excerpt_and_a_body_match_does() {
    let f = fixture().await;
    let titled = seed_task(&f, &format!("A {NEEDLE} title"), "nothing here").await;
    let described = seed_task(&f, "Plain title", &format!("buried {NEEDLE} deep")).await;

    let hits = f.search(NEEDLE).await;
    let tasks = Fixture::of_kind(&hits, SearchKind::Task);
    assert_eq!(tasks.len(), 2, "{hits:#?}");

    let title_hit = tasks.iter().find(|h| h.id == titled).unwrap();
    assert_eq!(title_hit.field, "title");
    assert!(title_hit.excerpt.is_empty());
    assert!(title_hit.subtitle.is_some(), "external id is the subtitle");

    let body_hit = tasks.iter().find(|h| h.id == described).unwrap();
    assert_eq!(body_hit.field, "description");
    assert_eq!(
        body_hit
            .excerpt
            .iter()
            .filter(|s| s.is_match)
            .map(|s| s.text.as_str())
            .collect::<Vec<_>>(),
        [NEEDLE]
    );
}

#[tokio::test]
async fn a_comment_surfaces_its_task_once_and_never_a_deleted_one() {
    let f = fixture().await;
    let live = seed_task(&f, "Live task", "").await;
    let dead = seed_task(&f, "Dead task", "").await;
    for task in [&live, &dead] {
        for n in 0..2 {
            f.repos
                .threads
                .append(
                    &f.env.pool,
                    &f.env.ws,
                    task,
                    None,
                    "worker",
                    "comment",
                    &format!("round {n} was {NEEDLE}"),
                    None,
                    None,
                    None,
                )
                .await
                .unwrap();
        }
    }
    f.repos
        .tasks
        .delete(&f.env.pool, &f.env.ws, &dead)
        .await
        .unwrap();

    let hits = f.search(NEEDLE).await;
    let tasks = Fixture::of_kind(&hits, SearchKind::Task);
    // Two comments on the live task, but a task is one row; the deleted task's
    // entries outlive it and must still be filtered out.
    assert_eq!(tasks.len(), 1, "{hits:#?}");
    assert_eq!(tasks[0].id, live);
    assert_eq!(tasks[0].field, "comment");
    assert!(!tasks[0].excerpt.is_empty());
}

#[tokio::test]
async fn a_title_or_description_match_outranks_the_same_task_s_comment() {
    let f = fixture().await;
    let id = seed_task(&f, &format!("{NEEDLE} in the title"), "").await;
    f.repos
        .threads
        .append(
            &f.env.pool,
            &f.env.ws,
            &id,
            None,
            "worker",
            "comment",
            &format!("also {NEEDLE} here"),
            None,
            None,
            None,
        )
        .await
        .unwrap();

    let hits = f.search(NEEDLE).await;
    let tasks = Fixture::of_kind(&hits, SearchKind::Task);
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].field, "title");
}

#[tokio::test]
async fn a_conversation_matching_only_json_envelope_text_is_dropped() {
    let f = fixture().await;
    let real = seed_conversation(&f, "Chat", &[&format!("we discussed {NEEDLE}")]).await;
    // "user" is the role of every seeded message, so the SQL LIKE over the raw
    // blob matches this row even though no message text contains it.
    seed_conversation(&f, "Chat", &["nothing relevant"]).await;

    let hits = f.search("user").await;
    assert!(
        Fixture::of_kind(&hits, SearchKind::Conversation).is_empty(),
        "{hits:#?}"
    );

    let hits = f.search(NEEDLE).await;
    let convs = Fixture::of_kind(&hits, SearchKind::Conversation);
    assert_eq!(convs.len(), 1);
    assert_eq!(convs[0].id, real);
    assert_eq!(convs[0].field, "transcript");
    assert!(!convs[0].excerpt.is_empty());
}

#[tokio::test]
async fn envelope_only_conversations_do_not_crowd_out_a_real_match() {
    let f = fixture().await;
    let real = seed_oldest_conversation(&f, "Chat", "the user asked about it").await;
    // Every seeded message carries the role "user", so each of these matches the
    // SQL LIKE over the raw blob and is then dropped — enough to fill the default
    // per-kind limit ahead of the older row that really matches.
    for _ in 0..=DEFAULT_LIMIT {
        seed_conversation(&f, "Chat", &["nothing relevant"]).await;
    }

    let hits = f.search("user").await;
    let convs = Fixture::of_kind(&hits, SearchKind::Conversation);
    assert_eq!(convs.len(), 1, "{hits:#?}");
    assert_eq!(convs[0].id, real);
}

#[tokio::test]
async fn agents_projects_and_providers_match_their_own_content_types() {
    let f = fixture().await;
    let provider = f
        .repos
        .providers
        .list(&f.env.pool, &f.env.ws)
        .await
        .unwrap()[0]
        .id
        .clone();
    f.repos
        .agents
        .create(
            &f.env.pool,
            &f.env.ws,
            "scribe",
            "",
            &provider,
            None,
            None,
            None,
            false,
            true,
            &json!([]),
            &json!([]),
            &format!("You are {NEEDLE}."),
            false,
            &json!([]),
        )
        .await
        .unwrap();
    f.repos
        .projects
        .create(
            &f.env.pool,
            &f.env.ws,
            "app",
            &format!("/tmp/{NEEDLE}"),
            "main",
        )
        .await
        .unwrap();
    f.repos
        .providers
        .create(&f.env.pool, &f.env.ws, NEEDLE, "codex", "~/.codex", None)
        .await
        .unwrap();

    let hits = f.search(NEEDLE).await;

    let agents = Fixture::of_kind(&hits, SearchKind::Agent);
    assert_eq!(agents.len(), 1, "{hits:#?}");
    assert_eq!(
        (agents[0].title.as_str(), agents[0].field.as_str()),
        ("scribe", "prompt")
    );

    let projects = Fixture::of_kind(&hits, SearchKind::Project);
    assert_eq!(projects.len(), 1, "{hits:#?}");
    assert_eq!(
        (projects[0].title.as_str(), projects[0].field.as_str()),
        ("app", "path")
    );

    let providers = Fixture::of_kind(&hits, SearchKind::Provider);
    assert_eq!(providers.len(), 1, "{hits:#?}");
    assert_eq!(providers[0].field, "title");
    assert_eq!(providers[0].subtitle.as_deref(), Some("codex"));
}

#[tokio::test]
async fn wiki_pages_match_by_heading_by_path_and_by_contents() {
    let f = fixture().await;
    let wiki = f.ws_dir.join("wiki");
    std::fs::create_dir_all(wiki.join("Modula")).unwrap();
    std::fs::write(
        wiki.join("headed.md"),
        format!("# The {NEEDLE} page\n\nbody"),
    )
    .unwrap();
    std::fs::write(
        wiki.join("Modula/plain.md"),
        format!("# Plain\n\nsomething {NEEDLE} in the body"),
    )
    .unwrap();
    std::fs::write(
        wiki.join(format!("Modula/{NEEDLE}-notes.md")),
        "# Notes\n\nneither heading nor body says it",
    )
    .unwrap();

    let hits = f.search(NEEDLE).await;
    let pages = Fixture::of_kind(&hits, SearchKind::Wiki);
    assert_eq!(pages.len(), 3, "{hits:#?}");

    let headed = pages.iter().find(|h| h.id == "headed.md").unwrap();
    assert_eq!(headed.title, format!("The {NEEDLE} page"));
    assert_eq!(headed.field, "title");
    assert!(headed.excerpt.is_empty());
    assert_eq!(
        headed.subtitle, None,
        "a top-level page has no parent folder"
    );

    let nested = pages.iter().find(|h| h.id == "Modula/plain.md").unwrap();
    assert_eq!(nested.title, "Plain");
    assert_eq!(nested.field, "contents");
    assert_eq!(nested.subtitle.as_deref(), Some("Modula"));

    let by_path = pages
        .iter()
        .find(|h| h.id == format!("Modula/{NEEDLE}-notes.md"))
        .unwrap();
    assert_eq!(by_path.title, "Notes");
    assert_eq!(by_path.field, "path");
}

#[tokio::test]
async fn a_blank_query_short_circuits_before_resolving_the_workspace() {
    let f = fixture().await;
    for query in ["", "   ", "\n\t"] {
        assert!(f
            .svc
            .search("no-such-workspace", query, &[], 0)
            .await
            .unwrap()
            .is_empty());
    }
}

#[tokio::test]
async fn an_unknown_workspace_is_a_not_found() {
    let f = fixture().await;
    assert!(matches!(
        f.svc.search("no-such-workspace", NEEDLE, &[], 0).await,
        Err(ApiError::NotFound(_))
    ));
}

#[tokio::test]
async fn kinds_filters_and_unknown_kinds_narrow_rather_than_error() {
    let f = fixture().await;
    seed_task(&f, &format!("{NEEDLE} task"), "").await;
    f.repos
        .projects
        .create(&f.env.pool, &f.env.ws, NEEDLE, "/tmp/p", "main")
        .await
        .unwrap();

    let only_tasks = f
        .svc
        .search(&f.env.ws, NEEDLE, &["task".to_string()], 0)
        .await
        .unwrap();
    assert_eq!(only_tasks.len(), 1);
    assert_eq!(only_tasks[0].kind, "task");

    // An unrecognised kind is skipped, not rejected.
    let mixed = f
        .svc
        .search(
            &f.env.ws,
            NEEDLE,
            &["project".to_string(), "spaceship".to_string()],
            0,
        )
        .await
        .unwrap();
    assert_eq!(mixed.len(), 1);
    assert_eq!(mixed[0].kind, "project");

    // Asking only for kinds this engine lacks yields nothing at all.
    assert!(f
        .svc
        .search(&f.env.ws, NEEDLE, &["spaceship".to_string()], 0)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn the_limit_is_per_kind_and_clamped() {
    let f = fixture().await;
    for n in 0..7 {
        seed_task(&f, &format!("{NEEDLE} task {n}"), "").await;
        f.repos
            .projects
            .create(
                &f.env.pool,
                &f.env.ws,
                &format!("{NEEDLE}-{n}"),
                "/tmp/p",
                "main",
            )
            .await
            .unwrap();
    }

    let defaulted = f.svc.search(&f.env.ws, NEEDLE, &[], 0).await.unwrap();
    assert_eq!(Fixture::of_kind(&defaulted, SearchKind::Task).len(), 5);
    assert_eq!(Fixture::of_kind(&defaulted, SearchKind::Project).len(), 5);

    let two = f.svc.search(&f.env.ws, NEEDLE, &[], 2).await.unwrap();
    assert_eq!(Fixture::of_kind(&two, SearchKind::Task).len(), 2);
    assert_eq!(Fixture::of_kind(&two, SearchKind::Project).len(), 2);

    // Over the ceiling, the clamp — not the request — decides.
    let clamped = f.svc.search(&f.env.ws, NEEDLE, &[], 1000).await.unwrap();
    assert_eq!(Fixture::of_kind(&clamped, SearchKind::Task).len(), 7);
}

#[tokio::test]
async fn a_failing_source_does_not_sink_the_others() {
    struct Boom;

    #[async_trait]
    impl SearchSource for Boom {
        fn kind(&self) -> SearchKind {
            SearchKind::Agent
        }
        async fn search(&self, _: &str, _: &str, _: i64) -> ApiResult<Vec<SearchHit>> {
            Err(ApiError::Internal("boom".into()))
        }
    }

    let f = fixture().await;
    seed_task(&f, &format!("{NEEDLE} task"), "").await;
    let svc = SearchService {
        workspaces: f.svc.workspaces.clone(),
        sources: Arc::new(vec![
            Arc::new(Boom),
            Arc::new(super::sources::tasks::Tasks::new(&f.repos)),
        ]),
    };

    let hits = svc.search(&f.env.ws, NEEDLE, &[], 0).await.unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].kind, "task");
}
