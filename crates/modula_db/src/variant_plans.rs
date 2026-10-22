//! Variant plan documents, one JSON text per variant. The repository stores
//! the text as given; `modula_types::Plan` owns parsing and validation.
//!
//! Methods take a caller-provided executor (`&pool` standalone, or `&mut *tx` to
//! enlist in a service-owned transaction). The repository holds no pool.

use sqlx::{Executor, Sqlite};

use crate::Result;

#[derive(Clone, Default)]
pub struct VariantPlanRepository;

impl VariantPlanRepository {
    pub fn new() -> Self {
        Self
    }

    pub async fn get<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
        task_id: &str,
        variant_id: &str,
    ) -> Result<Option<String>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        Ok(sqlx::query_scalar(
            "SELECT plan FROM variant_plans \
             WHERE workspace_id = ? AND task_id = ? AND variant_id = ?",
        )
        .bind(ws_id)
        .bind(task_id)
        .bind(variant_id)
        .fetch_optional(exec)
        .await?)
    }

    pub async fn upsert<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
        task_id: &str,
        variant_id: &str,
        plan: &str,
    ) -> Result<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        sqlx::query(
            "INSERT INTO variant_plans (workspace_id, task_id, variant_id, plan) \
             VALUES (?, ?, ?, ?) \
             ON CONFLICT (workspace_id, task_id, variant_id) DO UPDATE SET plan = excluded.plan",
        )
        .bind(ws_id)
        .bind(task_id)
        .bind(variant_id)
        .bind(plan)
        .execute(exec)
        .await?;
        Ok(())
    }

    /// Every plan in the workspace as `(task_id, variant_id, plan)`.
    pub async fn list_for_workspace<'e, E>(
        &self,
        exec: E,
        ws_id: &str,
    ) -> Result<Vec<(String, String, String)>>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        Ok(sqlx::query_as(
            "SELECT task_id, variant_id, plan FROM variant_plans WHERE workspace_id = ?",
        )
        .bind(ws_id)
        .fetch_all(exec)
        .await?)
    }
}
