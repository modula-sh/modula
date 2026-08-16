use modula_rpc::v1::{
    AttachLabelRequest, CreateLabelRequest, DetachLabelRequest, ListLabelsRequest,
};
use modula_types::Label;

use crate::error::{rpc, ClientError};
use crate::ModulaClient;

impl ModulaClient {
    pub async fn list_labels(
        &self,
        workspace_id: &str,
        label_type: &str,
    ) -> Result<Vec<Label>, ClientError> {
        let resp = self
            .labels()
            .await?
            .list(ListLabelsRequest {
                workspace_id: workspace_id.to_string(),
                r#type: label_type.to_string(),
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(resp.labels.into_iter().map(Label::from).collect())
    }

    /// Get-or-create a label by `(type, name)`; returns the label id.
    pub async fn create_label(
        &self,
        workspace_id: &str,
        name: &str,
        label_type: &str,
    ) -> Result<String, ClientError> {
        let resp = self
            .labels()
            .await?
            .create(CreateLabelRequest {
                workspace_id: workspace_id.to_string(),
                name: name.to_string(),
                r#type: label_type.to_string(),
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(resp.id)
    }

    pub async fn attach_label(
        &self,
        workspace_id: &str,
        task_id: &str,
        label_id: &str,
    ) -> Result<(), ClientError> {
        self.labels()
            .await?
            .attach_to_task(AttachLabelRequest {
                workspace_id: workspace_id.to_string(),
                task_id: task_id.to_string(),
                label_id: label_id.to_string(),
            })
            .await
            .map_err(rpc)?;
        Ok(())
    }

    pub async fn detach_label(
        &self,
        workspace_id: &str,
        task_id: &str,
        label_id: &str,
    ) -> Result<(), ClientError> {
        self.labels()
            .await?
            .detach_from_task(DetachLabelRequest {
                workspace_id: workspace_id.to_string(),
                task_id: task_id.to_string(),
                label_id: label_id.to_string(),
            })
            .await
            .map_err(rpc)?;
        Ok(())
    }
}
