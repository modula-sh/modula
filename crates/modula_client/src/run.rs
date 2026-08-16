//! Live run/agent status watch (spawn → running → exited). The returned stream
//! yields domain [`RunStatus`] frames; dropping it ends the watch.

use modula_rpc::v1::WatchRunStatusRequest;
use modula_types::RunStatus;
use tokio_stream::{Stream, StreamExt};

use crate::error::{rpc, ClientError};
use crate::ModulaClient;

impl ModulaClient {
    /// Subscribe to the run/agent status stream, optionally filtered to a single
    /// `agent_id`. Each item is a [`RunStatus`] frame.
    pub async fn watch_run_status(
        &self,
        workspace_id: &str,
        agent_id: Option<String>,
    ) -> Result<impl Stream<Item = Result<RunStatus, ClientError>>, ClientError> {
        let stream = self
            .runs()
            .await?
            .watch_status(WatchRunStatusRequest {
                workspace_id: workspace_id.to_string(),
                agent_id,
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(stream.map(|item| item.map(RunStatus::from).map_err(rpc)))
    }
}
