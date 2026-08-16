use modula_rpc::json::json_to_struct;
use modula_rpc::v1::{
    CreateTaskRequest, CreateVariantsRequest, DeleteAgentSettingsRequest, DeleteTaskRequest,
    ListAgentSettingsRequest, ListTasksRequest, ResetTaskRequest, SetAgentSettingsRequest,
    UpdateTaskRequest, UpdateVariantRequest, UpsertTaskRequest,
};
use modula_types::{Task, TaskAgentSetting};
use serde::{Deserialize, Serialize};

use crate::error::{rpc, ClientError};
use crate::request::{CreateTask, UpdateTask, UpsertTask};
use crate::ModulaClient;

/// Result of an internal `create_task` — the engine-minted ids.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatedTask {
    pub id: String,
    pub external_id: String,
}

/// Result of an external `upsert_task`; `upserted` is `"created"` or `"updated"`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertOutcome {
    pub id: String,
    pub external_id: String,
    pub upserted: String,
}

/// Result of `reset_task` — the spec files removed and any per-file errors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResetOutcome {
    pub task_id: String,
    pub files: Vec<String>,
    pub errors: Vec<String>,
}

/// A freshly registered variant (id + position); has no status yet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatedVariant {
    pub id: String,
    pub position: i64,
}

impl ModulaClient {
    pub async fn list_tasks(&self, workspace_id: &str) -> Result<Vec<Task>, ClientError> {
        let resp = self
            .tasks()
            .await?
            .list(ListTasksRequest {
                workspace_id: workspace_id.to_string(),
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(resp.tasks.into_iter().map(Task::from).collect())
    }

    pub async fn create_task(&self, req: CreateTask) -> Result<CreatedTask, ClientError> {
        let resp = self
            .tasks()
            .await?
            .create(CreateTaskRequest {
                workspace_id: req.workspace_id,
                title: req.title,
                description: req.description,
                approved: req.approved,
                max_variants: req.max_variants,
                worktree: req.worktree,
                source_data: req.source_data.and_then(json_to_struct),
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(CreatedTask {
            id: resp.id,
            external_id: resp.external_id,
        })
    }

    pub async fn upsert_task(&self, req: UpsertTask) -> Result<UpsertOutcome, ClientError> {
        let resp = self
            .tasks()
            .await?
            .upsert(UpsertTaskRequest {
                workspace_id: req.workspace_id,
                external_id: req.external_id,
                source: req.source,
                title: req.title,
                description: req.description,
                source_data: req.source_data.and_then(json_to_struct),
                status: req.status,
                url: req.url,
                synced_at: req.synced_at,
                approved: req.approved,
                max_variants: req.max_variants,
                worktree: req.worktree,
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(UpsertOutcome {
            id: resp.id,
            external_id: resp.external_id,
            upserted: resp.upserted,
        })
    }

    /// Edit a task row; returns the task id the engine confirmed.
    pub async fn update_task(&self, req: UpdateTask) -> Result<String, ClientError> {
        let resp = self
            .tasks()
            .await?
            .update(UpdateTaskRequest {
                workspace_id: req.workspace_id,
                task_id: req.task_id,
                approved: req.approved,
                max_variants: req.max_variants,
                worktree: req.worktree,
                description: req.description,
                title: req.title,
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(resp.id)
    }

    pub async fn delete_task(&self, workspace_id: &str, task_id: &str) -> Result<(), ClientError> {
        self.tasks()
            .await?
            .delete(DeleteTaskRequest {
                workspace_id: workspace_id.to_string(),
                task_id: task_id.to_string(),
            })
            .await
            .map_err(rpc)?;
        Ok(())
    }

    pub async fn reset_task(
        &self,
        workspace_id: &str,
        task_id: &str,
    ) -> Result<ResetOutcome, ClientError> {
        let resp = self
            .tasks()
            .await?
            .reset(ResetTaskRequest {
                workspace_id: workspace_id.to_string(),
                task_id: task_id.to_string(),
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(ResetOutcome {
            task_id: resp.task_id,
            files: resp.files,
            errors: resp.errors,
        })
    }

    pub async fn list_agent_settings(
        &self,
        workspace_id: &str,
        task_id: &str,
    ) -> Result<Vec<TaskAgentSetting>, ClientError> {
        let resp = self
            .tasks()
            .await?
            .list_agent_settings(ListAgentSettingsRequest {
                workspace_id: workspace_id.to_string(),
                task_id: task_id.to_string(),
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(resp
            .settings
            .into_iter()
            .map(TaskAgentSetting::from)
            .collect())
    }

    pub async fn set_agent_settings(
        &self,
        workspace_id: &str,
        task_id: &str,
        agent_id: &str,
        loop_amount: i64,
    ) -> Result<TaskAgentSetting, ClientError> {
        let resp = self
            .tasks()
            .await?
            .set_agent_settings(SetAgentSettingsRequest {
                workspace_id: workspace_id.to_string(),
                task_id: task_id.to_string(),
                agent_id: agent_id.to_string(),
                loop_amount,
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(TaskAgentSetting::from(resp))
    }

    pub async fn delete_agent_settings(
        &self,
        workspace_id: &str,
        task_id: &str,
        agent_id: &str,
    ) -> Result<(), ClientError> {
        self.tasks()
            .await?
            .delete_agent_settings(DeleteAgentSettingsRequest {
                workspace_id: workspace_id.to_string(),
                task_id: task_id.to_string(),
                agent_id: agent_id.to_string(),
            })
            .await
            .map_err(rpc)?;
        Ok(())
    }

    pub async fn create_variants(
        &self,
        workspace_id: &str,
        task_id: &str,
        count: u32,
    ) -> Result<Vec<CreatedVariant>, ClientError> {
        let resp = self
            .variants()
            .await?
            .create(CreateVariantsRequest {
                workspace_id: workspace_id.to_string(),
                task_id: task_id.to_string(),
                count,
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(resp
            .created
            .into_iter()
            .map(|v| CreatedVariant {
                id: v.id,
                position: v.position,
            })
            .collect())
    }

    /// Apply a status transition or named action to a variant; returns the new
    /// status the engine settled on. Set exactly one of `status` / `action`.
    pub async fn update_variant(
        &self,
        workspace_id: &str,
        task_id: &str,
        variant_id: &str,
        status: Option<String>,
        action: Option<String>,
    ) -> Result<String, ClientError> {
        let resp = self
            .variants()
            .await?
            .update(UpdateVariantRequest {
                workspace_id: workspace_id.to_string(),
                task_id: task_id.to_string(),
                variant_id: variant_id.to_string(),
                status,
                action,
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(resp.status)
    }

    /// Find a task by id. There is no get-by-id RPC, so this scans the list.
    pub async fn task_by_id(&self, workspace_id: &str, id: &str) -> Result<Task, ClientError> {
        self.list_tasks(workspace_id)
            .await?
            .into_iter()
            .find(|t| t.id == id)
            .ok_or_else(|| ClientError::NotFound(format!("no task with id {id}")))
    }

    /// Resolve the task that owns a variant by scanning every task's variants.
    pub async fn task_owning_variant(
        &self,
        workspace_id: &str,
        variant_id: &str,
    ) -> Result<Task, ClientError> {
        self.list_tasks(workspace_id)
            .await?
            .into_iter()
            .find(|t| t.variants.iter().any(|v| v.id == variant_id))
            .ok_or_else(|| ClientError::NotFound(format!("no task owns variant {variant_id}")))
    }
}
