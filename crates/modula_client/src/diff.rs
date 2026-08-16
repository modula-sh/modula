//! Per-variant diff and PR reads. The variant diff is a chunked
//! `DiffResponseChunk` server stream (a size workaround, not a live stream), so
//! it is reassembled into the original JSON; the PR info is unary JSON bytes.
//! Both are schemaless payloads the frontend consumes directly, so they return
//! `serde_json::Value`.

use modula_rpc::v1::{GetVariantDiffRequest, GetVariantPrRequest};
use serde_json::Value;

use crate::error::{rpc, ClientError};
use crate::ModulaClient;

impl ModulaClient {
    pub async fn variant_diff(
        &self,
        workspace_id: &str,
        task_id: &str,
        variant_id: &str,
    ) -> Result<Value, ClientError> {
        let mut stream = self
            .diffs()
            .await?
            .detail(GetVariantDiffRequest {
                workspace_id: workspace_id.to_string(),
                task_id: task_id.to_string(),
                variant_id: variant_id.to_string(),
            })
            .await
            .map_err(rpc)?
            .into_inner();
        let mut bytes = Vec::new();
        while let Some(chunk) = stream.message().await.map_err(rpc)? {
            bytes.extend_from_slice(&chunk.data);
        }
        serde_json::from_slice(&bytes).map_err(|e| ClientError::Rpc(e.to_string()))
    }

    pub async fn variant_pr(
        &self,
        workspace_id: &str,
        task_id: &str,
        variant_id: &str,
    ) -> Result<Value, ClientError> {
        let resp = self
            .diffs()
            .await?
            .get_pr(GetVariantPrRequest {
                workspace_id: workspace_id.to_string(),
                task_id: task_id.to_string(),
                variant_id: variant_id.to_string(),
            })
            .await
            .map_err(rpc)?
            .into_inner();
        serde_json::from_slice(&resp.pr_json).map_err(|e| ClientError::Rpc(e.to_string()))
    }
}
