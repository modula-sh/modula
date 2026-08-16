use modula_rpc::v1::{
    workspace_service_server::WorkspaceService, CreateWorkspaceRequest, CreateWorkspaceResponse,
    DeleteWorkspaceRequest, DeleteWorkspaceResponse, GetWorkspaceRequest, ListWorkspacesRequest,
    ListWorkspacesResponse, Workspace,
};
use tonic::{Request, Response, Status};

use super::error::to_status;
use crate::state::AppState;

pub struct WorkspaceHandler {
    pub state: AppState,
}

#[tonic::async_trait]
impl WorkspaceService for WorkspaceHandler {
    async fn list(
        &self,
        _req: Request<ListWorkspacesRequest>,
    ) -> Result<Response<ListWorkspacesResponse>, Status> {
        let workspaces = self
            .state
            .workspaces
            .list()
            .await
            .map_err(to_status)?
            .into_iter()
            .map(Workspace::from)
            .collect();
        Ok(Response::new(ListWorkspacesResponse { workspaces }))
    }

    async fn get(&self, req: Request<GetWorkspaceRequest>) -> Result<Response<Workspace>, Status> {
        let workspace = self
            .state
            .workspaces
            .get(&req.into_inner().id)
            .await
            .map_err(to_status)?;
        Ok(Response::new(workspace.into()))
    }

    async fn create(
        &self,
        req: Request<CreateWorkspaceRequest>,
    ) -> Result<Response<CreateWorkspaceResponse>, Status> {
        let body = req.into_inner();
        let created = self
            .state
            .workspaces
            .create(&body.name, body.description.as_deref())
            .await
            .map_err(to_status)?;
        Ok(Response::new(CreateWorkspaceResponse {
            id: created.id,
            name: created.name,
            slug: created.slug,
            path: created.path,
        }))
    }

    async fn delete(
        &self,
        req: Request<DeleteWorkspaceRequest>,
    ) -> Result<Response<DeleteWorkspaceResponse>, Status> {
        let id = req.into_inner().id;
        self.state.workspaces.delete(&id).await.map_err(to_status)?;
        Ok(Response::new(DeleteWorkspaceResponse { id }))
    }
}
