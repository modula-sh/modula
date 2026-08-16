//! Run-log tail. The server drains existing lines then follows; the returned
//! stream yields each chunk as decoded text. Dropping it detaches from the
//! engine without affecting the run.

use modula_rpc::v1::StreamLogRequest;
use tokio_stream::{Stream, StreamExt};

use crate::error::{rpc, ClientError};
use crate::ModulaClient;

impl ModulaClient {
    /// Stream a run log file by basename. Each item is one or more newline-
    /// terminated lines, decoded lossily from the chunk bytes.
    pub async fn stream_log(
        &self,
        workspace_id: &str,
        log_name: &str,
    ) -> Result<impl Stream<Item = Result<String, ClientError>>, ClientError> {
        let stream = self
            .logs()
            .await?
            .stream(StreamLogRequest {
                workspace_id: workspace_id.to_string(),
                log_name: log_name.to_string(),
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(stream.map(|item| {
            item.map(|chunk| String::from_utf8_lossy(&chunk.data).into_owned())
                .map_err(rpc)
        }))
    }
}
