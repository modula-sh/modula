use modula_rpc::v1::{config_service_server::ConfigService, GetConfigRequest, WorkspaceConfig};
use tonic::{Request, Response, Status};

use super::error::to_status;
use crate::state::AppState;

pub struct ConfigHandler {
    pub state: AppState,
}

#[tonic::async_trait]
impl ConfigService for ConfigHandler {
    async fn get(
        &self,
        req: Request<GetConfigRequest>,
    ) -> Result<Response<WorkspaceConfig>, Status> {
        let ws = req.into_inner().workspace_id;
        let config = self.state.config.get(&ws).await.map_err(to_status)?;
        Ok(Response::new(config.into()))
    }
}
