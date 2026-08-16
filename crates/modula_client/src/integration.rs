use modula_rpc::json::json_to_struct;
use modula_rpc::v1::{
    ConnectIntegrationRequest, DeleteIntegrationRequest, FetchIntegrationItemRequest,
    ListIntegrationsRequest, ListReposRequest, SearchIntegrationRequest,
};
use modula_types::{ExternalItem, Integration};
use serde_json::Value;

use crate::error::{rpc, ClientError};
use crate::ModulaClient;

impl ModulaClient {
    pub async fn list_integrations(
        &self,
        workspace_id: &str,
    ) -> Result<Vec<Integration>, ClientError> {
        let resp = self
            .integrations()
            .await?
            .list(ListIntegrationsRequest {
                workspace_id: workspace_id.to_string(),
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(resp
            .integrations
            .into_iter()
            .map(Integration::from)
            .collect())
    }

    /// Health-checks the connection with `data` before the engine persists it.
    pub async fn connect_integration(
        &self,
        workspace_id: &str,
        id: &str,
        data: Value,
    ) -> Result<(), ClientError> {
        self.integrations()
            .await?
            .connect_integration(ConnectIntegrationRequest {
                workspace_id: workspace_id.to_string(),
                id: id.to_string(),
                data: json_to_struct(data),
            })
            .await
            .map_err(rpc)?;
        Ok(())
    }

    pub async fn delete_integration(
        &self,
        workspace_id: &str,
        id: &str,
    ) -> Result<(), ClientError> {
        self.integrations()
            .await?
            .delete(DeleteIntegrationRequest {
                workspace_id: workspace_id.to_string(),
                id: id.to_string(),
            })
            .await
            .map_err(rpc)?;
        Ok(())
    }

    pub async fn search_integration(
        &self,
        workspace_id: &str,
        id: &str,
        query: &str,
        params: Value,
    ) -> Result<Vec<ExternalItem>, ClientError> {
        let resp = self
            .integrations()
            .await?
            .search(SearchIntegrationRequest {
                workspace_id: workspace_id.to_string(),
                id: id.to_string(),
                query: query.to_string(),
                params: json_to_struct(params),
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(resp.items.into_iter().map(ExternalItem::from).collect())
    }

    pub async fn fetch_integration_item(
        &self,
        workspace_id: &str,
        id: &str,
        key: &str,
        params: Value,
    ) -> Result<ExternalItem, ClientError> {
        let resp = self
            .integrations()
            .await?
            .fetch(FetchIntegrationItemRequest {
                workspace_id: workspace_id.to_string(),
                id: id.to_string(),
                key: key.to_string(),
                params: json_to_struct(params),
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(ExternalItem::from(resp))
    }

    pub async fn list_integration_repos(
        &self,
        workspace_id: &str,
        id: &str,
    ) -> Result<Vec<String>, ClientError> {
        let resp = self
            .integrations()
            .await?
            .list_repos(ListReposRequest {
                workspace_id: workspace_id.to_string(),
                id: id.to_string(),
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(resp.repos)
    }
}
