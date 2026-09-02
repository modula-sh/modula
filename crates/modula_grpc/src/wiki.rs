use modula_rpc::v1::{
    wiki_service_server::WikiService, CreateWikiFileRequest, CreateWikiFolderRequest,
    DeleteWikiPathRequest, DeleteWikiPathResponse, GetWikiFileRequest, GetWikiFileResponse,
    GetWikiTreeRequest, GetWikiTreeResponse, RenameWikiPathRequest, RenameWikiPathResponse,
    WikiFileResponse, WikiFolderResponse, WikiTreeNode, WriteWikiFileRequest,
};
use tonic::{Request, Response, Status};

use super::error::to_status;
use modula_services::wiki;
use modula_state::AppState;

pub struct WikiHandler {
    pub state: AppState,
}

/// Resolve the workspace's slug-named on-disk directory via the canonical
/// `WorkspaceService` resolver. Errors (rather than inventing a path) when the
/// workspace id or its directory doesn't exist, so a bad id can never create
/// stray dirs under `<modula>`.
async fn ws_dir(state: &AppState, ws: &str) -> Result<std::path::PathBuf, Status> {
    state.workspaces.workspace_dir(ws).await.map_err(to_status)
}

#[tonic::async_trait]
impl WikiService for WikiHandler {
    async fn get_tree(
        &self,
        req: Request<GetWikiTreeRequest>,
    ) -> Result<Response<GetWikiTreeResponse>, Status> {
        let ws = req.into_inner().workspace_id;
        let dir = ws_dir(&self.state, &ws).await?;
        let root = wiki::wiki_root(&dir).map_err(|e| Status::internal(e.to_string()))?;
        let nodes = wiki::build_tree(&root)
            .into_iter()
            .map(WikiTreeNode::from)
            .collect();
        Ok(Response::new(GetWikiTreeResponse { nodes }))
    }

    async fn get_file(
        &self,
        req: Request<GetWikiFileRequest>,
    ) -> Result<Response<GetWikiFileResponse>, Status> {
        let body = req.into_inner();
        let dir = ws_dir(&self.state, &body.workspace_id).await?;
        let root = wiki::wiki_root(&dir).map_err(|e| Status::internal(e.to_string()))?;
        let content =
            wiki::read_file(&root, &body.path).map_err(|e| Status::not_found(e.to_string()))?;
        Ok(Response::new(GetWikiFileResponse {
            path: body.path,
            content: content.into_bytes(),
        }))
    }

    async fn create_file(
        &self,
        req: Request<CreateWikiFileRequest>,
    ) -> Result<Response<WikiFileResponse>, Status> {
        let body = req.into_inner();
        let dir = ws_dir(&self.state, &body.workspace_id).await?;
        let root = wiki::wiki_root(&dir).map_err(|e| Status::internal(e.to_string()))?;
        let content = String::from_utf8(body.content)
            .map_err(|_| Status::invalid_argument("content is not valid UTF-8"))?;
        wiki::create_file(&root, &body.path, &content)
            .map_err(|e| Status::already_exists(e.to_string()))?;
        Ok(Response::new(WikiFileResponse {
            ok: true,
            path: body.path,
        }))
    }

    async fn write_file(
        &self,
        req: Request<WriteWikiFileRequest>,
    ) -> Result<Response<WikiFileResponse>, Status> {
        let body = req.into_inner();
        let dir = ws_dir(&self.state, &body.workspace_id).await?;
        let root = wiki::wiki_root(&dir).map_err(|e| Status::internal(e.to_string()))?;
        let content = String::from_utf8(body.content)
            .map_err(|_| Status::invalid_argument("content is not valid UTF-8"))?;
        wiki::write_file(&root, &body.path, &content)
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(WikiFileResponse {
            ok: true,
            path: body.path,
        }))
    }

    async fn create_folder(
        &self,
        req: Request<CreateWikiFolderRequest>,
    ) -> Result<Response<WikiFolderResponse>, Status> {
        let body = req.into_inner();
        let dir = ws_dir(&self.state, &body.workspace_id).await?;
        let root = wiki::wiki_root(&dir).map_err(|e| Status::internal(e.to_string()))?;
        wiki::create_folder(&root, &body.path).map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(WikiFolderResponse {
            ok: true,
            path: body.path,
        }))
    }

    async fn rename(
        &self,
        req: Request<RenameWikiPathRequest>,
    ) -> Result<Response<RenameWikiPathResponse>, Status> {
        let body = req.into_inner();
        let dir = ws_dir(&self.state, &body.workspace_id).await?;
        let root = wiki::wiki_root(&dir).map_err(|e| Status::internal(e.to_string()))?;
        wiki::rename(&root, &body.from, &body.to).map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(RenameWikiPathResponse {
            ok: true,
            from: body.from,
            to: body.to,
        }))
    }

    async fn delete(
        &self,
        req: Request<DeleteWikiPathRequest>,
    ) -> Result<Response<DeleteWikiPathResponse>, Status> {
        let body = req.into_inner();
        let dir = ws_dir(&self.state, &body.workspace_id).await?;
        let root = wiki::wiki_root(&dir).map_err(|e| Status::internal(e.to_string()))?;
        wiki::delete(&root, &body.path).map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(DeleteWikiPathResponse {
            ok: true,
            path: body.path,
        }))
    }
}
