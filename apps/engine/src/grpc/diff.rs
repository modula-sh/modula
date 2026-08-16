use std::pin::Pin;

use modula_rpc::v1::{
    diff_service_server::DiffService, DiffResponseChunk, GetVariantDiffRequest,
    GetVariantPrRequest, GetVariantPrResponse,
};
use tokio_stream::Stream;
use tonic::{Request, Response, Status};

use super::chunk;
use super::error::{internal, to_status};
use crate::state::AppState;

pub struct DiffHandler {
    pub state: AppState,
}

type DiffChunkStream = Pin<Box<dyn Stream<Item = Result<DiffResponseChunk, Status>> + Send>>;

#[tonic::async_trait]
impl DiffService for DiffHandler {
    type DetailStream = DiffChunkStream;

    async fn detail(
        &self,
        req: Request<GetVariantDiffRequest>,
    ) -> Result<Response<Self::DetailStream>, Status> {
        let body = req.into_inner();
        let diff = self
            .state
            .diffs
            .variant_diffs(&body.workspace_id, &body.task_id, &body.variant_id)
            .await
            .map_err(to_status)?;
        let bytes = serde_json::to_vec(&diff).map_err(internal)?;
        let chunks = chunk::split(bytes)
            .into_iter()
            .map(|data| Ok(DiffResponseChunk { data }));
        Ok(Response::new(Box::pin(tokio_stream::iter(chunks))))
    }

    async fn get_pr(
        &self,
        req: Request<GetVariantPrRequest>,
    ) -> Result<Response<GetVariantPrResponse>, Status> {
        let body = req.into_inner();
        let info = self
            .state
            .pr
            .variant_pr(&body.workspace_id, &body.task_id, &body.variant_id)
            .await
            .map_err(to_status)?;
        let pr_json = serde_json::to_vec(&info).map_err(internal)?;
        Ok(Response::new(GetVariantPrResponse { pr_json }))
    }
}
