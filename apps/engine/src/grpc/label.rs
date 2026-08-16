use modula_rpc::v1::{
    label_service_server::LabelService, AttachLabelRequest, AttachLabelResponse,
    CreateLabelRequest, CreateLabelResponse, DetachLabelRequest, DetachLabelResponse,
    ListLabelsRequest, ListLabelsResponse,
};
use tonic::{Request, Response, Status};

use crate::state::AppState;

pub struct LabelHandler {
    pub state: AppState,
}

#[tonic::async_trait]
impl LabelService for LabelHandler {
    async fn list(
        &self,
        req: Request<ListLabelsRequest>,
    ) -> Result<Response<ListLabelsResponse>, Status> {
        let body = req.into_inner();
        let labels = self
            .state
            .labels
            .list(&body.workspace_id, &body.r#type)
            .await?
            .into_iter()
            .map(Into::into)
            .collect();
        Ok(Response::new(ListLabelsResponse { labels }))
    }

    async fn create(
        &self,
        req: Request<CreateLabelRequest>,
    ) -> Result<Response<CreateLabelResponse>, Status> {
        let body = req.into_inner();
        let id = self
            .state
            .labels
            .create(&body.workspace_id, &body.r#type, &body.name)
            .await?;
        Ok(Response::new(CreateLabelResponse { id }))
    }

    async fn attach_to_task(
        &self,
        req: Request<AttachLabelRequest>,
    ) -> Result<Response<AttachLabelResponse>, Status> {
        let body = req.into_inner();
        self.state
            .labels
            .attach(&body.workspace_id, &body.task_id, &body.label_id)
            .await?;
        Ok(Response::new(AttachLabelResponse { ok: true }))
    }

    async fn detach_from_task(
        &self,
        req: Request<DetachLabelRequest>,
    ) -> Result<Response<DetachLabelResponse>, Status> {
        let body = req.into_inner();
        self.state
            .labels
            .detach(&body.workspace_id, &body.task_id, &body.label_id)
            .await?;
        Ok(Response::new(DetachLabelResponse { ok: true }))
    }
}
