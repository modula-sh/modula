use modula_rpc::v1::{
    roadmap_service_server::RoadmapService, ListRoadmapRequest, ListRoadmapResponse,
    SetRoadmapStatusRequest, SetRoadmapStatusResponse,
};
use tonic::{Request, Response, Status};

use modula_state::AppState;

pub struct RoadmapHandler {
    pub state: AppState,
}

#[tonic::async_trait]
impl RoadmapService for RoadmapHandler {
    async fn list(
        &self,
        req: Request<ListRoadmapRequest>,
    ) -> Result<Response<ListRoadmapResponse>, Status> {
        let ws = req.into_inner().workspace_id;
        let entries = self
            .state
            .tasks
            .list_roadmap(&ws)
            .await?
            .into_iter()
            .map(Into::into)
            .collect();
        Ok(Response::new(ListRoadmapResponse { entries }))
    }

    async fn set_status(
        &self,
        req: Request<SetRoadmapStatusRequest>,
    ) -> Result<Response<SetRoadmapStatusResponse>, Status> {
        let body = req.into_inner();
        let depends_on_json = if body.depends_on.is_empty() {
            None
        } else {
            Some(serde_json::json!(body.depends_on))
        };
        let created = self
            .state
            .tasks
            .set_roadmap_status(
                &body.workspace_id,
                &body.task_id,
                &body.status,
                depends_on_json.as_ref(),
                body.notes.as_deref(),
            )
            .await?;
        Ok(Response::new(SetRoadmapStatusResponse {
            task_id: body.task_id,
            status: body.status,
            created,
        }))
    }
}
