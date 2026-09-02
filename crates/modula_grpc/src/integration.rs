use modula_rpc::json::struct_to_json;
use modula_rpc::v1::{
    integration_service_server::IntegrationService, ConnectIntegrationRequest,
    ConnectIntegrationResponse, DeleteIntegrationRequest, DeleteIntegrationResponse, ExternalItem,
    FetchIntegrationItemRequest, ListIntegrationsRequest, ListIntegrationsResponse,
    ListReposRequest, ListReposResponse, SearchIntegrationRequest, SearchIntegrationResponse,
};
use tonic::{Request, Response, Status};

use modula_state::AppState;

pub struct IntegrationHandler {
    pub state: AppState,
}

fn item_to_pb(i: modula_integrations::ExternalItem) -> ExternalItem {
    ExternalItem {
        key: i.key,
        title: i.title,
        description: i.description,
        url: i.url,
        state: i.state,
    }
}

#[tonic::async_trait]
impl IntegrationService for IntegrationHandler {
    async fn list(
        &self,
        req: Request<ListIntegrationsRequest>,
    ) -> Result<Response<ListIntegrationsResponse>, Status> {
        let body = req.into_inner();
        let integrations = self
            .state
            .integrations
            .list(&body.workspace_id)
            .await?
            .into_iter()
            .map(Into::into)
            .collect();
        Ok(Response::new(ListIntegrationsResponse { integrations }))
    }

    async fn connect_integration(
        &self,
        req: Request<ConnectIntegrationRequest>,
    ) -> Result<Response<ConnectIntegrationResponse>, Status> {
        let body = req.into_inner();
        let data = body
            .data
            .map(struct_to_json)
            .unwrap_or_else(|| serde_json::json!({}));
        self.state
            .integrations
            .connect(&body.workspace_id, &body.id, data)
            .await?;
        Ok(Response::new(ConnectIntegrationResponse { ok: true }))
    }

    async fn delete(
        &self,
        req: Request<DeleteIntegrationRequest>,
    ) -> Result<Response<DeleteIntegrationResponse>, Status> {
        let body = req.into_inner();
        self.state
            .integrations
            .delete(&body.workspace_id, &body.id)
            .await?;
        Ok(Response::new(DeleteIntegrationResponse { ok: true }))
    }

    async fn search(
        &self,
        req: Request<SearchIntegrationRequest>,
    ) -> Result<Response<SearchIntegrationResponse>, Status> {
        let body = req.into_inner();
        let params = params_json(body.params);
        let items = self
            .state
            .integrations
            .search(&body.workspace_id, &body.id, &body.query, params)
            .await?
            .into_iter()
            .map(item_to_pb)
            .collect();
        Ok(Response::new(SearchIntegrationResponse { items }))
    }

    async fn fetch(
        &self,
        req: Request<FetchIntegrationItemRequest>,
    ) -> Result<Response<ExternalItem>, Status> {
        let body = req.into_inner();
        let params = params_json(body.params);
        let item = self
            .state
            .integrations
            .fetch(&body.workspace_id, &body.id, &body.key, params)
            .await?;
        Ok(Response::new(item_to_pb(item)))
    }

    async fn list_repos(
        &self,
        req: Request<ListReposRequest>,
    ) -> Result<Response<ListReposResponse>, Status> {
        let body = req.into_inner();
        let repos = self
            .state
            .integrations
            .repos(&body.workspace_id, &body.id)
            .await?;
        Ok(Response::new(ListReposResponse { repos }))
    }
}

fn params_json(params: Option<prost_types::Struct>) -> serde_json::Value {
    params
        .map(struct_to_json)
        .unwrap_or_else(|| serde_json::json!({}))
}
