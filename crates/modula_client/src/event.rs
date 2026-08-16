//! Live workspace event watch. The returned stream yields domain
//! [`WorkspaceEvent`]s; dropping it ends the watch (the engine has no other
//! cleanup to do).

use modula_rpc::v1::WatchEventsRequest;
use modula_types::WorkspaceEvent;
use tokio_stream::{Stream, StreamExt};

use crate::error::{rpc, ClientError};
use crate::ModulaClient;

impl ModulaClient {
    /// Subscribe to the live workspace event stream, resuming after `after_seq`
    /// (`0` = from now). Each item is a typed [`WorkspaceEvent`]; the outer
    /// `Result` reports the initial subscribe failure.
    pub async fn watch_events(
        &self,
        workspace_id: &str,
        after_seq: i64,
    ) -> Result<impl Stream<Item = Result<WorkspaceEvent, ClientError>>, ClientError> {
        let stream = self
            .events()
            .await?
            .watch(WatchEventsRequest {
                workspace_id: workspace_id.to_string(),
                after_seq,
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(stream.map(|item| item.map(WorkspaceEvent::from).map_err(rpc)))
    }
}
