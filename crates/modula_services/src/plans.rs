//! Variant plan documents. The Researcher sets a whole plan, the Worker
//! patches it; [`Plan`] owns every rule, so this service loads, applies,
//! stores, and publishes `plan.update` after each write.

use std::sync::Arc;

use serde_json::json;
use sqlx::SqlitePool;

use modula_db::projects::ProjectRepository;
use modula_db::variant_plans::VariantPlanRepository;
use modula_db::variants::VariantRepository;
use modula_rpc::status::DomainError;
use modula_types::{Plan, PlanPatch, PlanProgress};

use crate::events::{self, EventSink};

type Result<T> = std::result::Result<T, DomainError>;

#[derive(Clone)]
pub struct PlanService {
    pool: SqlitePool,
    plans: VariantPlanRepository,
    variants: VariantRepository,
    projects: ProjectRepository,
    events: Arc<dyn EventSink>,
}

impl PlanService {
    pub fn new(
        pool: SqlitePool,
        plans: VariantPlanRepository,
        variants: VariantRepository,
        projects: ProjectRepository,
        events: Arc<dyn EventSink>,
    ) -> Self {
        Self {
            pool,
            plans,
            variants,
            projects,
            events,
        }
    }

    pub async fn get(&self, ws: &str, task_id: &str, variant_id: &str) -> Result<String> {
        self.plans
            .get(&self.pool, ws, task_id, variant_id)
            .await?
            .ok_or_else(|| no_plan(variant_id))
    }

    pub async fn set(
        &self,
        ws: &str,
        task_id: &str,
        variant_id: &str,
        text: &str,
    ) -> Result<PlanProgress> {
        if self
            .variants
            .position_of(&self.pool, ws, task_id, variant_id)
            .await?
            .is_none()
        {
            return Err(DomainError::NotFound(format!(
                "variant {variant_id} not found on task {task_id}"
            )));
        }
        let plan = Plan::parse(text).map_err(DomainError::BadRequest)?;
        let names: Vec<String> = self
            .projects
            .list(&self.pool, ws)
            .await?
            .into_iter()
            .map(|p| p.name)
            .collect();
        if let Some(bad) = plan.projects.iter().find(|p| !names.contains(&p.name)) {
            return Err(DomainError::BadRequest(format!(
                "unknown project {:?}; workspace projects: {}",
                bad.name,
                names.join(", ")
            )));
        }
        self.plans
            .upsert(&self.pool, ws, task_id, variant_id, &to_text(&plan)?)
            .await?;
        self.publish(ws, task_id, variant_id).await;
        Ok(plan.progress())
    }

    /// Apply one patch. Returns the id it touched or minted, and the new progress.
    pub async fn patch(
        &self,
        ws: &str,
        task_id: &str,
        variant_id: &str,
        patch: PlanPatch,
    ) -> Result<(String, PlanProgress)> {
        let mut tx = self.pool.begin().await?;
        let text = self
            .plans
            .get(&mut *tx, ws, task_id, variant_id)
            .await?
            .ok_or_else(|| no_plan(variant_id))?;
        let mut plan = Plan::parse(&text).map_err(DomainError::Internal)?;
        let id = plan.apply(patch).map_err(DomainError::BadRequest)?;
        self.plans
            .upsert(&mut *tx, ws, task_id, variant_id, &to_text(&plan)?)
            .await?;
        tx.commit().await?;
        self.publish(ws, task_id, variant_id).await;
        Ok((id, plan.progress()))
    }

    async fn publish(&self, ws: &str, task_id: &str, variant_id: &str) {
        self.events
            .publish(
                ws,
                events::PLAN_UPDATE,
                json!({ "task_id": task_id, "variant_id": variant_id }),
            )
            .await;
    }
}

fn no_plan(variant_id: &str) -> DomainError {
    DomainError::NotFound(format!("variant {variant_id} has no plan"))
}

fn to_text(plan: &Plan) -> Result<String> {
    serde_json::to_string_pretty(plan).map_err(|e| DomainError::Internal(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::{env, seed_task, Env};
    use modula_types::PlanTaskStatus;
    use serde_json::Value;

    fn service(env: &Env) -> PlanService {
        PlanService::new(
            env.pool.clone(),
            VariantPlanRepository::new(),
            VariantRepository::new(),
            ProjectRepository::new(),
            env.sink.clone(),
        )
    }

    async fn seed_variant(env: &Env) -> (String, String) {
        let task = seed_task(env).await;
        let mut conn = env.pool.acquire().await.unwrap();
        let (variant, _) = VariantRepository::new()
            .create_batch(&mut conn, &env.ws, &task, 1)
            .await
            .unwrap()
            .remove(0);
        (task, variant)
    }

    fn doc(project: &str) -> String {
        json!({
            "version": 1, "title": "T", "problem": "p", "approach": "a",
            "projects": [{"name": project, "changes": []}],
            "phases": [{"title": "One", "scope": "s", "approach": "a",
                        "tasks": [{"title": "A"}, {"title": "B"}]}],
            "risks": "r", "test_plan": "t"
        })
        .to_string()
    }

    async fn project_name(env: &Env) -> String {
        ProjectRepository::new()
            .create(&env.pool, &env.ws, "Modula", "/tmp/modula", "main")
            .await
            .unwrap();
        "Modula".into()
    }

    #[tokio::test]
    async fn set_then_get_stores_the_normalized_document() {
        let env = env().await;
        let svc = service(&env);
        let (task, variant) = seed_variant(&env).await;
        let progress = svc
            .set(&env.ws, &task, &variant, &doc(&project_name(&env).await))
            .await
            .unwrap();
        assert_eq!(progress.tasks_total, 2);
        assert_eq!(progress.current_task_id.as_deref(), Some("1.1"));

        let stored: Value =
            serde_json::from_str(&svc.get(&env.ws, &task, &variant).await.unwrap()).unwrap();
        assert_eq!(stored["phases"][0]["tasks"][1]["id"], "1.2");
        assert_eq!(stored["phases"][0]["tasks"][1]["status"], "pending");

        let (_, type_, data) = env.sink.last().unwrap();
        assert_eq!(type_, events::PLAN_UPDATE);
        assert_eq!(data, json!({"task_id": task, "variant_id": variant}));
    }

    #[tokio::test]
    async fn set_and_get_report_missing_variant_and_plan() {
        let env = env().await;
        let svc = service(&env);
        let (task, variant) = seed_variant(&env).await;
        assert!(matches!(
            svc.get(&env.ws, &task, &variant).await,
            Err(DomainError::NotFound(m)) if m == format!("variant {variant} has no plan")
        ));
        assert!(matches!(
            svc.set(&env.ws, &task, "nope", &doc("x")).await,
            Err(DomainError::NotFound(_))
        ));
        assert!(env.sink.types().is_empty());
    }

    #[tokio::test]
    async fn set_rejects_bad_documents_and_unknown_projects() {
        let env = env().await;
        let svc = service(&env);
        let (task, variant) = seed_variant(&env).await;
        project_name(&env).await;
        let Err(DomainError::BadRequest(m)) = svc.set(&env.ws, &task, &variant, "{}").await else {
            panic!("expected BadRequest");
        };
        assert!(m.contains("missing field"), "{m}");
        let Err(DomainError::BadRequest(m)) = svc.set(&env.ws, &task, &variant, &doc("Nope")).await
        else {
            panic!("expected BadRequest");
        };
        assert_eq!(m, "unknown project \"Nope\"; workspace projects: Modula");
        assert!(env.sink.types().is_empty());
    }

    #[tokio::test]
    async fn patch_applies_and_publishes() {
        let env = env().await;
        let svc = service(&env);
        let (task, variant) = seed_variant(&env).await;
        let set_status = |status| PlanPatch::SetTaskStatus {
            task: "1.1".into(),
            status,
            note: None,
        };
        assert!(matches!(
            svc.patch(
                &env.ws,
                &task,
                &variant,
                set_status(PlanTaskStatus::InProgress)
            )
            .await,
            Err(DomainError::NotFound(_))
        ));
        svc.set(&env.ws, &task, &variant, &doc(&project_name(&env).await))
            .await
            .unwrap();
        env.sink.clear();

        let (id, progress) = svc
            .patch(
                &env.ws,
                &task,
                &variant,
                set_status(PlanTaskStatus::InProgress),
            )
            .await
            .unwrap();
        assert_eq!(id, "1.1");
        assert_eq!(progress.current_task_id.as_deref(), Some("1.1"));
        assert_eq!(env.sink.types(), [events::PLAN_UPDATE]);

        assert!(matches!(
            svc.patch(
                &env.ws,
                &task,
                &variant,
                set_status(PlanTaskStatus::Skipped)
            )
            .await,
            Err(DomainError::BadRequest(_))
        ));
        let (_, progress) = svc
            .patch(&env.ws, &task, &variant, set_status(PlanTaskStatus::Done))
            .await
            .unwrap();
        assert_eq!(progress.tasks_done, 1);
        assert_eq!(progress.current_task_id.as_deref(), Some("1.2"));
    }
}
