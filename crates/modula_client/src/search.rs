use modula_rpc::v1::SearchRequest;
use modula_types::SearchHit;

use crate::error::{rpc, ClientError};
use crate::ModulaClient;

impl ModulaClient {
    /// Workspace-wide search, unlike `search_integration`, which queries an
    /// external issue tracker.
    pub async fn search(
        &self,
        workspace_id: &str,
        query: &str,
        kinds: &[String],
        limit: u32,
    ) -> Result<Vec<SearchHit>, ClientError> {
        let resp = self
            .search_client()
            .await?
            .search(SearchRequest {
                workspace_id: workspace_id.to_string(),
                query: query.to_string(),
                kinds: kinds.to_vec(),
                limit,
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(resp.hits.into_iter().map(SearchHit::from).collect())
    }
}
