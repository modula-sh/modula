use modula_rpc::v1::GetOverviewRequest;
use modula_types::OverviewWorkspace;

use crate::error::{rpc, ClientError};
use crate::ModulaClient;

impl ModulaClient {
    /// Every workspace's active tasks. Host-global, so no workspace id.
    pub async fn overview(&self) -> Result<Vec<OverviewWorkspace>, ClientError> {
        let resp = self
            .overview_client()
            .await?
            .get(GetOverviewRequest {})
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(resp
            .workspaces
            .into_iter()
            .map(OverviewWorkspace::from)
            .collect())
    }
}
