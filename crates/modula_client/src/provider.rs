use modula_rpc::v1::{
    CreateProviderRequest, DeleteProviderRequest, GetProviderCatalogRequest, GetProviderRequest,
    ListProvidersRequest, UpdateProviderRequest,
};
use modula_types::{CatalogProvider, Provider};
use serde::{Deserialize, Serialize};

use crate::error::{rpc, ClientError};
use crate::request::{CreateProvider, UpdateProvider};
use crate::ModulaClient;

/// Result of `create_provider` — the new provider's id and name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatedProvider {
    pub id: String,
    pub name: String,
}

impl ModulaClient {
    pub async fn provider_catalog(&self) -> Result<Vec<CatalogProvider>, ClientError> {
        let resp = self
            .providers()
            .await?
            .get_catalog(GetProviderCatalogRequest {})
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(resp
            .providers
            .into_iter()
            .map(CatalogProvider::from)
            .collect())
    }

    pub async fn list_providers(&self, workspace_id: &str) -> Result<Vec<Provider>, ClientError> {
        let resp = self
            .providers()
            .await?
            .list(ListProvidersRequest {
                workspace_id: workspace_id.to_string(),
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(resp.providers.into_iter().map(Provider::from).collect())
    }

    pub async fn get_provider(
        &self,
        workspace_id: &str,
        provider_id: &str,
    ) -> Result<Provider, ClientError> {
        let resp = self
            .providers()
            .await?
            .get(GetProviderRequest {
                workspace_id: workspace_id.to_string(),
                provider_id: provider_id.to_string(),
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(Provider::from(resp))
    }

    pub async fn create_provider(
        &self,
        req: CreateProvider,
    ) -> Result<CreatedProvider, ClientError> {
        let resp = self
            .providers()
            .await?
            .create(CreateProviderRequest {
                workspace_id: req.workspace_id,
                name: req.name,
                r#type: req.r#type,
                config_dir: req.config_dir,
                description: req.description,
                mcp_servers: req.mcp_servers.into_iter().map(Into::into).collect(),
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(CreatedProvider {
            id: resp.id,
            name: resp.name,
        })
    }

    pub async fn update_provider(&self, req: UpdateProvider) -> Result<(), ClientError> {
        let update_mcp_servers = req.mcp_servers.is_some();
        self.providers()
            .await?
            .update(UpdateProviderRequest {
                workspace_id: req.workspace_id,
                provider_id: req.provider_id,
                name: req.name,
                r#type: req.r#type,
                config_dir: req.config_dir,
                description: req.description,
                reset_description: req.clear_description,
                mcp_servers: req
                    .mcp_servers
                    .unwrap_or_default()
                    .into_iter()
                    .map(Into::into)
                    .collect(),
                update_mcp_servers,
            })
            .await
            .map_err(rpc)?;
        Ok(())
    }

    pub async fn delete_provider(
        &self,
        workspace_id: &str,
        provider_id: &str,
    ) -> Result<(), ClientError> {
        self.providers()
            .await?
            .delete(DeleteProviderRequest {
                workspace_id: workspace_id.to_string(),
                provider_id: provider_id.to_string(),
            })
            .await
            .map_err(rpc)?;
        Ok(())
    }
}
