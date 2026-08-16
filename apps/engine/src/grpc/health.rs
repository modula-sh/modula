use modula_rpc::v1::{
    health_service_server::HealthService, HealthCheckRequest, HealthCheckResponse, HealthStatus,
};
use tonic::{Request, Response, Status};

use crate::state::AppState;

pub struct HealthHandler {
    pub state: AppState,
}

#[tonic::async_trait]
impl HealthService for HealthHandler {
    async fn check(
        &self,
        _req: Request<HealthCheckRequest>,
    ) -> Result<Response<HealthCheckResponse>, Status> {
        Ok(Response::new(HealthCheckResponse {
            status: HealthStatus::Serving as i32,
            pid: std::process::id(),
        }))
    }
}
