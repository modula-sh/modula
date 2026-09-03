use modula_rpc::v1::{
    provider_service_server::ProviderService, CatalogModel, CatalogProvider, CreateProviderRequest,
    CreateProviderResponse, DeleteProviderRequest, DeleteProviderResponse, GenerateTextRequest,
    GenerateTextResponse, GetProviderCatalogRequest, GetProviderCatalogResponse,
    GetProviderRequest, ListProvidersRequest, ListProvidersResponse, McpServer, Provider,
    UpdateProviderRequest, UpdateProviderResponse,
};
use tonic::{Request, Response, Status};

use super::error::to_status;
use modula_services::generate::GenerateParams;
use modula_services::mcp_config::McpServer as McpServerModel;
use modula_services::providers::{CreateParams, UpdateParams};
use modula_state::AppState;

pub struct ProviderHandler {
    pub state: AppState,
}

#[tonic::async_trait]
impl ProviderService for ProviderHandler {
    async fn get_catalog(
        &self,
        _req: Request<GetProviderCatalogRequest>,
    ) -> Result<Response<GetProviderCatalogResponse>, Status> {
        let entries = self.state.providers.catalog().await.map_err(to_status)?;
        let providers = entries
            .into_iter()
            .map(|c| CatalogProvider {
                id: c.id,
                models: c
                    .models
                    .into_iter()
                    .map(|m| CatalogModel {
                        id: m.id,
                        label: m.label,
                    })
                    .collect(),
            })
            .collect();
        Ok(Response::new(GetProviderCatalogResponse { providers }))
    }

    async fn list(
        &self,
        req: Request<ListProvidersRequest>,
    ) -> Result<Response<ListProvidersResponse>, Status> {
        let ws = req.into_inner().workspace_id;
        let providers = self
            .state
            .providers
            .list(&ws)
            .await
            .map_err(to_status)?
            .into_iter()
            .map(Provider::from)
            .collect();
        Ok(Response::new(ListProvidersResponse { providers }))
    }

    async fn get(&self, req: Request<GetProviderRequest>) -> Result<Response<Provider>, Status> {
        let body = req.into_inner();
        let provider = self
            .state
            .providers
            .get(&body.workspace_id, &body.provider_id)
            .await
            .map_err(to_status)?;
        Ok(Response::new(provider.into()))
    }

    async fn create(
        &self,
        req: Request<CreateProviderRequest>,
    ) -> Result<Response<CreateProviderResponse>, Status> {
        let body = req.into_inner();
        let created = self
            .state
            .providers
            .create(
                &body.workspace_id,
                CreateParams {
                    name: body.name,
                    r#type: Some(body.r#type),
                    config_dir: body.config_dir,
                    description: body.description,
                    mcp_servers: Some(into_models(body.mcp_servers)),
                },
            )
            .await
            .map_err(to_status)?;
        Ok(Response::new(CreateProviderResponse {
            id: created.id,
            name: created.name,
        }))
    }

    async fn update(
        &self,
        req: Request<UpdateProviderRequest>,
    ) -> Result<Response<UpdateProviderResponse>, Status> {
        let body = req.into_inner();
        let description = if body.reset_description {
            Some(None)
        } else {
            body.description.map(Some)
        };
        let mcp_servers = body
            .update_mcp_servers
            .then(|| into_models(body.mcp_servers));
        self.state
            .providers
            .update(
                &body.workspace_id,
                &body.provider_id,
                UpdateParams {
                    name: body.name,
                    r#type: body.r#type,
                    config_dir: body.config_dir,
                    description,
                    mcp_servers,
                },
            )
            .await
            .map_err(to_status)?;
        Ok(Response::new(UpdateProviderResponse {
            id: body.provider_id,
        }))
    }

    async fn delete(
        &self,
        req: Request<DeleteProviderRequest>,
    ) -> Result<Response<DeleteProviderResponse>, Status> {
        let body = req.into_inner();
        self.state
            .providers
            .delete(&body.workspace_id, &body.provider_id)
            .await
            .map_err(to_status)?;
        Ok(Response::new(DeleteProviderResponse {
            id: body.provider_id,
        }))
    }

    async fn generate(
        &self,
        req: Request<GenerateTextRequest>,
    ) -> Result<Response<GenerateTextResponse>, Status> {
        let body = req.into_inner();
        let text = self
            .state
            .generation
            .generate(
                &body.workspace_id,
                GenerateParams {
                    provider_id: body.provider_id,
                    model: body.model,
                    instruction: body.instruction,
                    field_label: body.field_label,
                },
            )
            .await
            .map_err(to_status)?;
        Ok(Response::new(GenerateTextResponse { text }))
    }
}

fn into_models(servers: Vec<McpServer>) -> Vec<McpServerModel> {
    servers
        .into_iter()
        .map(|s| McpServerModel {
            key: s.key,
            url: s.url,
            auth_token: s.auth_token,
        })
        .collect()
}
