use modula_rpc::v1::GetUsageRequest;
use modula_types::UsageEntry;

use crate::error::{rpc, ClientError};
use crate::ModulaClient;

impl ModulaClient {
    pub async fn get_usage(&self, workspace_id: &str) -> Result<Vec<UsageEntry>, ClientError> {
        let resp = self
            .usage()
            .await?
            .get(GetUsageRequest {
                workspace_id: workspace_id.to_string(),
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(resp.entries.into_iter().map(UsageEntry::from).collect())
    }
}
