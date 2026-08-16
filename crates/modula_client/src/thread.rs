use modula_rpc::convert::{str_to_kind, str_to_verdict};
use modula_rpc::v1::{AppendEntryRequest, DeleteEntryRequest, EditEntryRequest, GetThreadsRequest};
use modula_types::{ThreadBundle, ThreadEntry};

use crate::error::{rpc, ClientError};
use crate::request::AppendEntry;
use crate::ModulaClient;

impl ModulaClient {
    pub async fn get_threads(
        &self,
        workspace_id: &str,
        task_id: &str,
    ) -> Result<ThreadBundle, ClientError> {
        let resp = self
            .threads()
            .await?
            .get_threads(GetThreadsRequest {
                workspace_id: workspace_id.to_string(),
                task_id: task_id.to_string(),
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(ThreadBundle::from(resp))
    }

    /// Append an entry; returns the entry the engine stored.
    pub async fn append_entry(&self, req: AppendEntry) -> Result<ThreadEntry, ClientError> {
        let resp = self
            .threads()
            .await?
            .append_entry(AppendEntryRequest {
                workspace_id: req.workspace_id,
                task_id: req.task_id,
                content: req.content,
                author: req.author,
                kind: str_to_kind(&req.kind),
                variant_id: req.variant_id,
                round: req.round,
                verdict: req.verdict.as_deref().and_then(str_to_verdict),
                affected_variants: req.affected_variants,
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(ThreadEntry::from(resp.entry.unwrap_or_default()))
    }

    pub async fn edit_entry(
        &self,
        workspace_id: &str,
        task_id: &str,
        entry_id: i64,
        content: &str,
        author: &str,
    ) -> Result<(), ClientError> {
        self.threads()
            .await?
            .edit_entry(EditEntryRequest {
                workspace_id: workspace_id.to_string(),
                task_id: task_id.to_string(),
                entry_id,
                content: content.to_string(),
                author: author.to_string(),
            })
            .await
            .map_err(rpc)?;
        Ok(())
    }

    pub async fn delete_entry(
        &self,
        workspace_id: &str,
        task_id: &str,
        entry_id: i64,
        author: &str,
    ) -> Result<(), ClientError> {
        self.threads()
            .await?
            .delete_entry(DeleteEntryRequest {
                workspace_id: workspace_id.to_string(),
                task_id: task_id.to_string(),
                entry_id,
                author: author.to_string(),
            })
            .await
            .map_err(rpc)?;
        Ok(())
    }
}
