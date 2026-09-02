use std::pin::Pin;

use modula_rpc::v1::{
    snapshot_service_server::SnapshotService, GetSnapshotRequest, GetSnapshotResponse,
    SnapshotChunk, StreamSnapshotRequest,
};
use tokio_stream::Stream;
use tonic::{Request, Response, Status};

use super::chunk;
use super::error::{internal, to_status};
use modula_state::AppState;

pub struct SnapshotHandler {
    pub state: AppState,
}

type SnapshotChunkStream = Pin<Box<dyn Stream<Item = Result<SnapshotChunk, Status>> + Send>>;

#[tonic::async_trait]
impl SnapshotService for SnapshotHandler {
    async fn get(
        &self,
        req: Request<GetSnapshotRequest>,
    ) -> Result<Response<GetSnapshotResponse>, Status> {
        let ws = req.into_inner().workspace_id;
        let snap = self
            .state
            .snapshot
            .workspace_snapshot(&ws)
            .await
            .map_err(to_status)?;
        let snapshot_json = serde_json::to_vec(&snap).map_err(internal)?;
        Ok(Response::new(GetSnapshotResponse { snapshot_json }))
    }

    type StreamStream = SnapshotChunkStream;

    async fn stream(
        &self,
        req: Request<StreamSnapshotRequest>,
    ) -> Result<Response<Self::StreamStream>, Status> {
        let ws = req.into_inner().workspace_id;
        let snap = self
            .state
            .snapshot
            .workspace_snapshot(&ws)
            .await
            .map_err(to_status)?;
        let bytes = serde_json::to_vec(&snap).map_err(internal)?;
        let chunks = chunk::split(bytes)
            .into_iter()
            .map(|data| Ok(SnapshotChunk { data }));
        Ok(Response::new(Box::pin(tokio_stream::iter(chunks))))
    }
}
