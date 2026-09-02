use std::pin::Pin;

use modula_rpc::v1::{log_service_server::LogService, LogChunk, StreamLogRequest};
use tokio_stream::{Stream, StreamExt};
use tonic::{Request, Response, Status};

use super::error::to_status;
use modula_services::logs;
use modula_state::AppState;

pub struct LogHandler {
    pub state: AppState,
}

type LogChunkStream = Pin<Box<dyn Stream<Item = Result<LogChunk, Status>> + Send>>;

#[tonic::async_trait]
impl LogService for LogHandler {
    type StreamStream = LogChunkStream;

    async fn stream(
        &self,
        req: Request<StreamLogRequest>,
    ) -> Result<Response<Self::StreamStream>, Status> {
        let body = req.into_inner();
        let path = self
            .state
            .logs
            .resolve_path(&body.workspace_id, &body.log_name)
            .await
            .map_err(to_status)?;
        // One newline-terminated line per chunk. Dropping the stream stops the
        // tail without affecting the run that writes the log.
        let stream = logs::tail_lines(path).map(|line| {
            Ok(LogChunk {
                data: format!("{line}\n").into_bytes(),
            })
        });
        Ok(Response::new(Box::pin(stream)))
    }
}
