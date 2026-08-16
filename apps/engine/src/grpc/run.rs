use std::pin::Pin;

use modula_rpc::v1::{
    run_service_server::RunService, AgentRun, ListRecentRunsRequest, ListRecentRunsResponse,
    ListRunsForAgentRequest, ListRunsForAgentResponse, RunStatus, WatchRunStatusRequest,
};
use tokio::sync::broadcast::error::RecvError;
use tokio_stream::Stream;
use tonic::{Request, Response, Status};

use crate::state::AppState;

use super::error::to_status;

pub struct RunHandler {
    pub state: AppState,
}

#[tonic::async_trait]
impl RunService for RunHandler {
    async fn list_recent(
        &self,
        req: Request<ListRecentRunsRequest>,
    ) -> Result<Response<ListRecentRunsResponse>, Status> {
        let ws = req.into_inner().workspace_id;
        let runs = self.state.runs.list_recent(&ws).await.map_err(to_status)?;
        Ok(Response::new(ListRecentRunsResponse {
            runs: runs.into_iter().map(AgentRun::from).collect(),
        }))
    }

    async fn list_for_agent(
        &self,
        req: Request<ListRunsForAgentRequest>,
    ) -> Result<Response<ListRunsForAgentResponse>, Status> {
        let body = req.into_inner();
        let runs = self
            .state
            .runs
            .list_for_agent(&body.workspace_id, &body.agent_id)
            .await
            .map_err(to_status)?;
        Ok(Response::new(ListRunsForAgentResponse {
            runs: runs.into_iter().map(AgentRun::from).collect(),
        }))
    }

    type WatchStatusStream = Pin<Box<dyn Stream<Item = Result<RunStatus, Status>> + Send>>;
    async fn watch_status(
        &self,
        req: Request<WatchRunStatusRequest>,
    ) -> Result<Response<Self::WatchStatusStream>, Status> {
        let body = req.into_inner();
        self.state
            .workspaces
            .get(&body.workspace_id)
            .await
            .map_err(to_status)?;
        let mut rx = self.state.bus.subscribe(&body.workspace_id).await;
        let filter_agent = body.agent_id;
        let stream = async_stream::try_stream! {
            loop {
                match rx.recv().await {
                    Ok(ev) => {
                        if let Some(status) = modula_types::RunStatus::from_parts(&ev.type_, &ev.data) {
                            // Filter by agent when requested. Exit events carry
                            // no agent id, so they always pass through to signal
                            // the run's completion.
                            let keep = match &filter_agent {
                                Some(a) => status.agent_id.is_empty() || &status.agent_id == a,
                                None => true,
                            };
                            if keep {
                                yield RunStatus::from(status);
                            }
                        }
                    }
                    Err(RecvError::Lagged(n)) => {
                        tracing::warn!("[run.watch] subscriber lagged, skipped {n} events");
                    }
                    Err(RecvError::Closed) => break,
                }
            }
        };
        Ok(Response::new(Box::pin(stream)))
    }
}
