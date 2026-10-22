use modula_rpc::v1::{
    overview_service_server::OverviewService, GetOverviewRequest, GetOverviewResponse,
};
use tonic::{Request, Response, Status};

use modula_state::AppState;

pub struct OverviewHandler {
    pub state: AppState,
}

#[tonic::async_trait]
impl OverviewService for OverviewHandler {
    async fn get(
        &self,
        _req: Request<GetOverviewRequest>,
    ) -> Result<Response<GetOverviewResponse>, Status> {
        let workspaces = self
            .state
            .overview
            .get()
            .await
            .map_err(super::error::to_status)?
            .into_iter()
            .map(Into::into)
            .collect();
        Ok(Response::new(GetOverviewResponse { workspaces }))
    }
}
