use modula_rpc::v1::{
    CreateWikiFileRequest, CreateWikiFolderRequest, DeleteWikiPathRequest, GetWikiFileRequest,
    GetWikiTreeRequest, RenameWikiPathRequest, WriteWikiFileRequest,
};
use modula_types::WikiNode;
use serde::{Deserialize, Serialize};

use crate::error::{rpc, ClientError};
use crate::ModulaClient;

/// A wiki file's path and decoded markdown body (`dto::wiki_file`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikiFile {
    pub path: String,
    pub content: String,
}

impl ModulaClient {
    pub async fn wiki_tree(&self, workspace_id: &str) -> Result<Vec<WikiNode>, ClientError> {
        let resp = self
            .wiki()
            .await?
            .get_tree(GetWikiTreeRequest {
                workspace_id: workspace_id.to_string(),
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(resp.nodes.into_iter().map(WikiNode::from).collect())
    }

    pub async fn wiki_file(&self, workspace_id: &str, path: &str) -> Result<WikiFile, ClientError> {
        let resp = self
            .wiki()
            .await?
            .get_file(GetWikiFileRequest {
                workspace_id: workspace_id.to_string(),
                path: path.to_string(),
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(WikiFile {
            path: resp.path,
            content: String::from_utf8_lossy(&resp.content).into_owned(),
        })
    }

    pub async fn wiki_create_file(
        &self,
        workspace_id: &str,
        path: &str,
        content: &str,
    ) -> Result<(), ClientError> {
        self.wiki()
            .await?
            .create_file(CreateWikiFileRequest {
                workspace_id: workspace_id.to_string(),
                path: path.to_string(),
                content: content.as_bytes().to_vec(),
            })
            .await
            .map_err(rpc)?;
        Ok(())
    }

    pub async fn wiki_write_file(
        &self,
        workspace_id: &str,
        path: &str,
        content: &str,
    ) -> Result<(), ClientError> {
        self.wiki()
            .await?
            .write_file(WriteWikiFileRequest {
                workspace_id: workspace_id.to_string(),
                path: path.to_string(),
                content: content.as_bytes().to_vec(),
            })
            .await
            .map_err(rpc)?;
        Ok(())
    }

    pub async fn wiki_create_folder(
        &self,
        workspace_id: &str,
        path: &str,
    ) -> Result<(), ClientError> {
        self.wiki()
            .await?
            .create_folder(CreateWikiFolderRequest {
                workspace_id: workspace_id.to_string(),
                path: path.to_string(),
            })
            .await
            .map_err(rpc)?;
        Ok(())
    }

    pub async fn wiki_rename(
        &self,
        workspace_id: &str,
        from: &str,
        to: &str,
    ) -> Result<(), ClientError> {
        self.wiki()
            .await?
            .rename(RenameWikiPathRequest {
                workspace_id: workspace_id.to_string(),
                from: from.to_string(),
                to: to.to_string(),
            })
            .await
            .map_err(rpc)?;
        Ok(())
    }

    pub async fn wiki_delete(&self, workspace_id: &str, path: &str) -> Result<(), ClientError> {
        self.wiki()
            .await?
            .delete(DeleteWikiPathRequest {
                workspace_id: workspace_id.to_string(),
                path: path.to_string(),
            })
            .await
            .map_err(rpc)?;
        Ok(())
    }
}
