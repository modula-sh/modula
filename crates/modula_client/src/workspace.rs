use modula_rpc::v1::{
    CreateWorkspaceRequest, DeleteWorkspaceRequest, GetWorkspaceRequest, ListWorkspacesRequest,
};
use modula_types::Workspace;
use serde::{Deserialize, Serialize};

use crate::error::{rpc, ClientError};
use crate::ModulaClient;

/// Result of `create_workspace` — the new workspace's identity and on-disk path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatedWorkspace {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub path: String,
}

impl ModulaClient {
    pub async fn list_workspaces(&self) -> Result<Vec<Workspace>, ClientError> {
        let resp = self
            .workspaces()
            .await?
            .list(ListWorkspacesRequest {})
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(resp.workspaces.into_iter().map(Workspace::from).collect())
    }

    pub async fn get_workspace(&self, id: &str) -> Result<Workspace, ClientError> {
        let resp = self
            .workspaces()
            .await?
            .get(GetWorkspaceRequest { id: id.to_string() })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(Workspace::from(resp))
    }

    pub async fn create_workspace(
        &self,
        name: &str,
        description: Option<String>,
    ) -> Result<CreatedWorkspace, ClientError> {
        let resp = self
            .workspaces()
            .await?
            .create(CreateWorkspaceRequest {
                name: name.to_string(),
                description,
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(CreatedWorkspace {
            id: resp.id,
            name: resp.name,
            slug: resp.slug,
            path: resp.path,
        })
    }

    pub async fn delete_workspace(&self, id: &str) -> Result<(), ClientError> {
        self.workspaces()
            .await?
            .delete(DeleteWorkspaceRequest { id: id.to_string() })
            .await
            .map_err(rpc)?;
        Ok(())
    }

    /// Match a workspace by canonical id or by engine-supplied slug. There is no
    /// by-ref RPC, so this scans the list.
    pub async fn workspace_by_ref(&self, reference: &str) -> Result<Workspace, ClientError> {
        self.list_workspaces()
            .await?
            .into_iter()
            .find(|w| w.id == reference || w.slug == reference)
            .ok_or_else(|| {
                ClientError::NotFound(format!(
                    "no workspace matches '{reference}' (expected an id or slug)"
                ))
            })
    }
}
