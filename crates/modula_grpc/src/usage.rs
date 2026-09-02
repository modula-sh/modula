use modula_rpc::v1::{
    usage_service_server::UsageService, GetUsageRequest, GetUsageResponse, UsageEntry, UsageTokens,
};
use tonic::{Request, Response, Status};

use super::error::to_status;
use modula_services::usage::UsageRun;
use modula_state::AppState;

pub struct UsageHandler {
    pub state: AppState,
}

#[tonic::async_trait]
impl UsageService for UsageHandler {
    async fn get(
        &self,
        req: Request<GetUsageRequest>,
    ) -> Result<Response<GetUsageResponse>, Status> {
        let ws = req.into_inner().workspace_id;
        let entries = self
            .state
            .runs
            .usage(&ws)
            .await
            .map_err(to_status)?
            .into_iter()
            .map(into_entry)
            .collect();
        Ok(Response::new(GetUsageResponse { entries }))
    }
}

fn into_entry(r: UsageRun) -> UsageEntry {
    UsageEntry {
        run_id: r.run_id,
        log: r.log,
        agent: r.agent,
        mtime: r.mtime,
        duration_ms: r.duration_ms,
        cost_usd: r.cost_usd,
        tokens: Some(UsageTokens {
            input: r.tokens.input,
            output: r.tokens.output,
            cache_creation: r.tokens.cache_creation,
            cache_read: r.tokens.cache_read,
        }),
    }
}
