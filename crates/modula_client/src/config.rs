use modula_rpc::v1::GetConfigRequest;
use modula_types::WorkspaceConfig;

use crate::error::{rpc, ClientError};
use crate::ModulaClient;

impl ModulaClient {
    pub async fn get_config(&self, workspace_id: &str) -> Result<WorkspaceConfig, ClientError> {
        let resp = self
            .config_client()
            .await?
            .get(GetConfigRequest {
                workspace_id: workspace_id.to_string(),
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(WorkspaceConfig::from(resp))
    }
}
