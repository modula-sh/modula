use modula_rpc::v1::{ListRoadmapRequest, SetRoadmapStatusRequest};
use modula_types::RoadmapEntry;
use serde::{Deserialize, Serialize};

use crate::error::{rpc, ClientError};
use crate::request::SetRoadmapStatus;
use crate::ModulaClient;

/// Result of `set_roadmap_status` — the settled status and whether the row was
/// newly created (vs. patched).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoadmapStatus {
    pub task_id: String,
    pub status: String,
    pub created: bool,
}

impl ModulaClient {
    pub async fn list_roadmap(&self, workspace_id: &str) -> Result<Vec<RoadmapEntry>, ClientError> {
        let resp = self
            .roadmap()
            .await?
            .list(ListRoadmapRequest {
                workspace_id: workspace_id.to_string(),
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(resp.entries.into_iter().map(RoadmapEntry::from).collect())
    }

    pub async fn set_roadmap_status(
        &self,
        req: SetRoadmapStatus,
    ) -> Result<RoadmapStatus, ClientError> {
        let resp = self
            .roadmap()
            .await?
            .set_status(SetRoadmapStatusRequest {
                workspace_id: req.workspace_id,
                task_id: req.task_id,
                status: req.status,
                depends_on: req.depends_on,
                notes: req.notes,
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(RoadmapStatus {
            task_id: resp.task_id,
            status: resp.status,
            created: resp.created,
        })
    }
}
