use modula_rpc::v1::{GetPlanRequest, PatchPlanRequest, SetPlanRequest};
use modula_types::{PlanPatch, PlanProgress};

use crate::error::{rpc, ClientError};
use crate::ModulaClient;

impl ModulaClient {
    /// The variant's plan document as JSON text.
    pub async fn get_plan(
        &self,
        workspace_id: &str,
        task_id: &str,
        variant_id: &str,
    ) -> Result<String, ClientError> {
        let resp = self
            .plans()
            .await?
            .get(GetPlanRequest {
                workspace_id: workspace_id.to_string(),
                task_id: task_id.to_string(),
                variant_id: variant_id.to_string(),
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(resp.plan)
    }

    pub async fn set_plan(
        &self,
        workspace_id: &str,
        task_id: &str,
        variant_id: &str,
        plan: String,
    ) -> Result<PlanProgress, ClientError> {
        let resp = self
            .plans()
            .await?
            .set(SetPlanRequest {
                workspace_id: workspace_id.to_string(),
                task_id: task_id.to_string(),
                variant_id: variant_id.to_string(),
                plan,
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(resp.progress.map(PlanProgress::from).unwrap_or_default())
    }

    /// Apply one edit; returns the id of the task, phase, or summary it touched.
    pub async fn patch_plan(
        &self,
        workspace_id: &str,
        task_id: &str,
        variant_id: &str,
        patch: PlanPatch,
    ) -> Result<(String, PlanProgress), ClientError> {
        let resp = self
            .plans()
            .await?
            .patch(PatchPlanRequest {
                workspace_id: workspace_id.to_string(),
                task_id: task_id.to_string(),
                variant_id: variant_id.to_string(),
                op: Some(patch.into()),
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok((
            resp.id,
            resp.progress.map(PlanProgress::from).unwrap_or_default(),
        ))
    }
}
