//! Host-global overview: every workspace's active tasks with their stage, live
//! agents and plan progress. Read-only, like `SearchService`: no transaction,
//! no writes, no events.

use std::collections::HashMap;

use futures::future::join_all;

use modula_core::error::ApiResult;
use modula_core::repositories::Repositories;
use modula_db::roadmap::RoadmapRepository;
use modula_db::tasks::TaskRepository;
use modula_db::variant_plans::VariantPlanRepository;
use modula_db::variants::VariantRepository;
use modula_db::Database;
use modula_types::{
    OverviewAgent, OverviewTask, OverviewVariant, OverviewWorkspace, Plan, Workspace,
};

use crate::config::ConfigService;
use crate::processes::ProcessesService;
use crate::workspaces::WorkspaceService;

const RECENT_TASKS: usize = 5;

#[derive(Clone)]
pub struct OverviewService {
    pool: Database,
    tasks: TaskRepository,
    roadmap: RoadmapRepository,
    variants: VariantRepository,
    variant_plans: VariantPlanRepository,
    workspaces: WorkspaceService,
    config: ConfigService,
    processes: ProcessesService,
}

impl OverviewService {
    pub fn new(
        workspaces: WorkspaceService,
        config: ConfigService,
        processes: ProcessesService,
        repos: &Repositories,
    ) -> Self {
        Self {
            pool: repos.pool.clone(),
            tasks: repos.tasks.clone(),
            roadmap: repos.roadmap.clone(),
            variants: repos.variants.clone(),
            variant_plans: repos.variant_plans.clone(),
            workspaces,
            config,
            processes,
        }
    }

    /// Workspaces without an active task are left out.
    pub async fn get(&self) -> ApiResult<Vec<OverviewWorkspace>> {
        let workspaces = self.workspaces.list().await?;
        let results = join_all(workspaces.iter().map(|w| self.workspace(w))).await;
        // One broken workspace must not blank the rest.
        let mut out = Vec::new();
        for (w, result) in workspaces.iter().zip(results) {
            match result {
                Ok(o) if !o.tasks.is_empty() => out.push(o),
                Ok(_) => {}
                Err(err) => tracing::warn!(workspace = %w.id, %err, "overview workspace failed"),
            }
        }
        Ok(out)
    }

    /// Active = approved with a non-terminal roadmap status. Tasks waiting on a
    /// human come first; otherwise roadmap order.
    async fn workspace(&self, w: &Workspace) -> ApiResult<OverviewWorkspace> {
        let config = self.config.get(&w.id).await?;
        let roadmap = self.roadmap.list(&self.pool, &w.id).await?;
        let mut tasks: HashMap<String, _> = self
            .tasks
            .list(&self.pool, &w.id)
            .await?
            .into_iter()
            .map(|t| (t.id.clone(), t))
            .collect();
        let mut variants = self.variants.list_all(&self.pool, &w.id).await?;
        let mut plans = HashMap::new();
        for (_, variant_id, text) in self
            .variant_plans
            .list_for_workspace(&self.pool, &w.id)
            .await?
        {
            match Plan::parse(&text) {
                Ok(plan) => {
                    plans.insert(variant_id, plan);
                }
                Err(e) => tracing::warn!("plan for variant {variant_id} does not parse: {e}"),
            }
        }
        let running = self.processes.running_agents(&w.id).await;

        let mut out = Vec::new();
        for entry in roadmap {
            let status = config.pipeline.iter().find(|s| s.key == entry.status);
            if status.is_some_and(|s| s.terminal) {
                continue;
            }
            let Some(task) = tasks.remove(&entry.task) else {
                continue;
            };
            if task.approved != Some(true) {
                continue;
            }
            let agents = running
                .iter()
                .filter(|a| a.task.as_deref() == Some(task.id.as_str()))
                .map(|a| {
                    let provider = config
                        .agents
                        .iter()
                        .find(|c| c.id == a.agent_id)
                        .and_then(|c| config.providers.iter().find(|p| p.id == c.provider_id));
                    OverviewAgent {
                        name: a.name.clone(),
                        provider_type: provider.map(|p| p.r#type.clone()),
                        provider_name: provider.map(|p| p.name.clone()),
                    }
                })
                .collect();
            let variants = variants
                .remove(&task.id)
                .unwrap_or_default()
                .into_iter()
                .map(|v| {
                    let plan = plans.get(&v.id);
                    OverviewVariant {
                        progress: plan.map(Plan::progress),
                        recent_tasks: plan
                            .map(|p| p.recent_tasks(RECENT_TASKS).into_iter().cloned().collect())
                            .unwrap_or_default(),
                        id: v.id,
                        position: v.position,
                        status: v.status,
                    }
                })
                .collect();
            out.push(OverviewTask {
                needs_human: status.is_some_and(|s| s.awaits_human || s.error),
                id: task.id,
                external_id: task.external_id,
                title: task.title,
                status: entry.status,
                notes: entry.notes,
                agents,
                variants,
            });
        }
        out.sort_by_key(|t| !t.needs_human);
        Ok(OverviewWorkspace {
            id: w.id.clone(),
            name: w.name.clone(),
            pipeline: config.pipeline,
            tasks: out,
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::loop_registry::LoopRegistry;
    use crate::scheduler::SchedulerHandle;
    use crate::testkit::env;
    use modula_db::workspaces::WorkspaceRepository;
    use modula_types::PlanTaskStatus;

    struct Seed<'a> {
        repos: &'a Repositories,
        ws: &'a str,
    }

    impl Seed<'_> {
        async fn task(&self, title: &str, approved: bool, status: &str) -> String {
            let mut conn = self.repos.pool.acquire().await.unwrap();
            let (id, _) = self
                .repos
                .tasks
                .create_internal(
                    &mut conn,
                    self.ws,
                    title,
                    "{}",
                    Some(approved),
                    "",
                    None,
                    None,
                    "",
                )
                .await
                .unwrap();
            self.repos
                .roadmap
                .upsert(
                    &mut conn,
                    self.ws,
                    &id,
                    status,
                    None,
                    Some("waiting on X\nmore"),
                )
                .await
                .unwrap();
            id
        }

        async fn variant(&self, task: &str, plan: &str) {
            let mut conn = self.repos.pool.acquire().await.unwrap();
            let (variant, _) = self
                .repos
                .variants
                .create_batch(&mut conn, self.ws, task, 1)
                .await
                .unwrap()
                .remove(0);
            self.repos
                .variant_plans
                .upsert(&mut *conn, self.ws, task, &variant, plan)
                .await
                .unwrap();
        }
    }

    fn plan_doc() -> String {
        let tasks: Vec<_> = (1..=7).map(|i| json!({"title": format!("T{i}")})).collect();
        let mut doc = json!({
            "version": 1, "title": "T", "problem": "p", "approach": "a", "projects": [],
            "phases": [{"title": "One", "scope": "s", "approach": "a", "tasks": tasks}],
            "risks": "r", "test_plan": "t"
        });
        for i in 0..5 {
            doc["phases"][0]["tasks"][i]["status"] = json!("done");
        }
        doc["phases"][0]["tasks"][5]["status"] = json!("in_progress");
        doc.to_string()
    }

    #[tokio::test]
    async fn lists_active_tasks_per_workspace() {
        let env = env().await;
        let repos = Repositories::new(&env.pool);
        let mut conn = env.pool.acquire().await.unwrap();
        let other = WorkspaceRepository::new()
            .create(&mut conn, "Other", None)
            .await
            .unwrap();
        drop(conn);
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
        let config = ConfigService::new(
            env.pool.clone(),
            repos.workspaces.clone(),
            repos.settings.clone(),
            repos.pipeline.clone(),
            repos.providers.clone(),
            repos.projects.clone(),
            repos.agents.clone(),
        );
        let processes = ProcessesService::new(
            env.pool.clone(),
            repos.agent_processes.clone(),
            LoopRegistry::default(),
        );
        let svc = OverviewService::new(workspaces, config.clone(), processes, &repos);

        let seed = Seed {
            repos: &repos,
            ws: &env.ws,
        };
        let building = seed.task("building", true, "in_progress").await;
        let waiting = seed.task("waiting", true, "needs_clarification").await;
        seed.task("accepted", true, "accepted").await;
        seed.task("unapproved", false, "in_progress").await;
        let broken = seed.task("broken", true, "blocked").await;
        seed.variant(&building, &plan_doc()).await;
        seed.variant(&broken, "not json").await;
        Seed {
            repos: &repos,
            ws: &other,
        }
        .task("done elsewhere", true, "accepted")
        .await;

        let agent = config.get(&env.ws).await.unwrap().agents.remove(0);
        let run = repos
            .agent_runs
            .create(
                &env.pool,
                &env.ws,
                &agent.id,
                &agent.name,
                None,
                &json!({"args": {"task-id": building}}),
            )
            .await
            .unwrap();
        repos
            .agent_processes
            .create(&env.pool, &env.ws, &agent.id, &agent.name, run, 4242)
            .await
            .unwrap();

        let out = svc.get().await.unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].id, env.ws);
        assert!(out[0].pipeline.iter().any(|s| s.awaits_human));
        let tasks = &out[0].tasks;
        let ids: Vec<_> = tasks.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, [waiting.as_str(), broken.as_str(), building.as_str()]);
        assert!(tasks[0].needs_human && tasks[1].needs_human && !tasks[2].needs_human);
        assert_eq!(tasks[1].notes, "waiting on X\nmore");

        let built = &tasks[2];
        assert_eq!(built.agents.len(), 1);
        assert_eq!(built.agents[0].name, agent.name);
        assert_eq!(built.agents[0].provider_type.as_deref(), Some("claude"));
        let progress = built.variants[0].progress.as_ref().unwrap();
        assert_eq!((progress.tasks_total, progress.tasks_done), (7, 5));
        let window: Vec<_> = built.variants[0]
            .recent_tasks
            .iter()
            .map(|t| (t.id.as_str(), t.status))
            .collect();
        assert_eq!(window.len(), 5);
        assert_eq!(window[4], ("1.6", PlanTaskStatus::InProgress));
        assert_eq!(window[0], ("1.2", PlanTaskStatus::Done));

        assert!(tasks[0].agents.is_empty());
        assert!(tasks[0].variants.is_empty());
        assert_eq!(tasks[1].variants[0].progress, None);
        assert!(tasks[1].variants[0].recent_tasks.is_empty());
    }
}
