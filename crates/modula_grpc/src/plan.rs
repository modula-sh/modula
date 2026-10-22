use modula_rpc::v1::{
    plan_service_server::PlanService, GetPlanRequest, GetPlanResponse, PatchPlanRequest,
    PatchPlanResponse, SetPlanRequest, SetPlanResponse,
};
use modula_types::PlanPatch;
use tonic::{Request, Response, Status};

use modula_state::AppState;

pub struct PlanHandler {
    pub state: AppState,
}

#[tonic::async_trait]
impl PlanService for PlanHandler {
    async fn get(&self, req: Request<GetPlanRequest>) -> Result<Response<GetPlanResponse>, Status> {
        let body = req.into_inner();
        let plan = self
            .state
            .plans
            .get(&body.workspace_id, &body.task_id, &body.variant_id)
            .await?;
        Ok(Response::new(GetPlanResponse { plan }))
    }

    async fn set(&self, req: Request<SetPlanRequest>) -> Result<Response<SetPlanResponse>, Status> {
        let body = req.into_inner();
        let progress = self
            .state
            .plans
            .set(
                &body.workspace_id,
                &body.task_id,
                &body.variant_id,
                &body.plan,
            )
            .await?;
        Ok(Response::new(SetPlanResponse {
            progress: Some(progress.into()),
        }))
    }

    async fn patch(
        &self,
        req: Request<PatchPlanRequest>,
    ) -> Result<Response<PatchPlanResponse>, Status> {
        let body = req.into_inner();
        let op = body
            .op
            .ok_or_else(|| Status::invalid_argument("op is required"))?;
        let patch = PlanPatch::try_from(op).map_err(Status::invalid_argument)?;
        let (id, progress) = self
            .state
            .plans
            .patch(&body.workspace_id, &body.task_id, &body.variant_id, patch)
            .await?;
        Ok(Response::new(PatchPlanResponse {
            id,
            progress: Some(progress.into()),
        }))
    }
}
