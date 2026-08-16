use modula_rpc::v1::{snapshot_service_client::SnapshotServiceClient, GetSnapshotRequest};
use serde_json::Value;

use crate::error::{rpc, ClientError};
use crate::ModulaClient;

/// Decode cap for the assembled snapshot, mirroring the engine's
/// `MAX_UNARY_MESSAGE_SIZE`. The snapshot bundles config, tasks, runs, and
/// conversations and can exceed tonic's 4 MB default.
const MAX_SNAPSHOT_SIZE: usize = 64 * 1024 * 1024;

impl ModulaClient {
    /// Unary fetch of the full workspace snapshot as the assembled JSON the
    /// frontend `SnapshotContext` consumes directly.
    pub async fn get_snapshot(&self, workspace_id: &str) -> Result<Value, ClientError> {
        let resp = SnapshotServiceClient::new(self.channel().await?)
            .max_decoding_message_size(MAX_SNAPSHOT_SIZE)
            .get(GetSnapshotRequest {
                workspace_id: workspace_id.to_string(),
            })
            .await
            .map_err(rpc)?
            .into_inner();
        serde_json::from_slice(&resp.snapshot_json).map_err(|e| ClientError::Rpc(e.to_string()))
    }
}
