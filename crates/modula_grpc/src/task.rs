use modula_rpc::json::struct_to_json;
use modula_rpc::v1::{
    task_service_server::TaskService, variant_service_server::VariantService, CreateTaskRequest,
    CreateTaskResponse, CreateVariantsRequest, CreateVariantsResponse, CreatedVariant,
    DeleteAgentSettingsRequest, DeleteAgentSettingsResponse, DeleteTaskRequest, DeleteTaskResponse,
    ListAgentSettingsRequest, ListAgentSettingsResponse, ListTasksRequest, ListTasksResponse,
    ResetTaskRequest, ResetTaskResponse, SetAgentSettingsRequest, TaskAgentSetting,
    UpdateTaskRequest, UpdateTaskResponse, UpdateVariantRequest, UpdateVariantResponse,
    UpsertTaskRequest, UpsertTaskResponse,
};
use tonic::{Request, Response, Status};

use modula_services::tasks::{CreateInternalInput, UpsertExternalInput};
use modula_state::AppState;

use super::error::to_status;

pub struct TaskHandler {
    pub state: AppState,
}

pub struct VariantHandler {
    pub state: AppState,
}

#[tonic::async_trait]
impl TaskService for TaskHandler {
    async fn list(
        &self,
        req: Request<ListTasksRequest>,
    ) -> Result<Response<ListTasksResponse>, Status> {
        let ws = req.into_inner().workspace_id;
        let tasks = self
            .state
            .tasks
            .list(&ws)
            .await?
            .into_iter()
            .map(Into::into)
            .collect();
        Ok(Response::new(ListTasksResponse { tasks }))
    }

    async fn create(
        &self,
        req: Request<CreateTaskRequest>,
    ) -> Result<Response<CreateTaskResponse>, Status> {
        let body = req.into_inner();
        let ws = body.workspace_id;
        let title = body.title.trim().to_string();
        if title.is_empty() {
            return Err(Status::invalid_argument("title is required"));
        }
        let source_data = body
            .source_data
            .map(|s| serde_json::to_string(&struct_to_json(s)).unwrap_or_default())
            .unwrap_or_else(|| "{}".into());
        let input = CreateInternalInput {
            title,
            description: body.description.unwrap_or_default(),
            source_data,
            approved: body.approved,
            max_variants: body.max_variants,
            worktree: body.worktree,
        };
        let (id, external_id) = self.state.tasks.create_internal(&ws, input).await?;
        Ok(Response::new(CreateTaskResponse { id, external_id }))
    }

    async fn upsert(
        &self,
        req: Request<UpsertTaskRequest>,
    ) -> Result<Response<UpsertTaskResponse>, Status> {
        let body = req.into_inner();
        let ws = body.workspace_id;
        let source_data = body
            .source_data
            .map(|s| Some(serde_json::to_string(&struct_to_json(s)).unwrap_or_default()));
        let today = chrono::Local::now().date_naive().to_string();
        let input = UpsertExternalInput {
            title: body.title,
            description: body.description.unwrap_or_default(),
            source: body.source,
            external_id: body.external_id.clone(),
            source_data: source_data.flatten(),
            synced_at: body.synced_at.unwrap_or(today),
            approved: body.approved,
            max_variants: body.max_variants,
            worktree: body.worktree,
            status: body.status,
            url: body.url,
        };
        let result = self.state.tasks.upsert_external(&ws, input).await?;
        Ok(Response::new(UpsertTaskResponse {
            id: result.id().to_string(),
            external_id: body.external_id,
            upserted: result.verb().to_string(),
        }))
    }

    async fn update(
        &self,
        req: Request<UpdateTaskRequest>,
    ) -> Result<Response<UpdateTaskResponse>, Status> {
        let body = req.into_inner();
        let ws = body.workspace_id;
        let task_id = body.task_id;
        self.state
            .tasks
            .update(
                &ws,
                &task_id,
                body.title,
                body.description,
                body.approved.map(Some),
                body.max_variants.map(Some),
                body.worktree.map(Some),
            )
            .await?;
        Ok(Response::new(UpdateTaskResponse { id: task_id }))
    }

    async fn delete(
        &self,
        req: Request<DeleteTaskRequest>,
    ) -> Result<Response<DeleteTaskResponse>, Status> {
        let body = req.into_inner();
        self.state
            .tasks
            .delete(&body.workspace_id, &body.task_id)
            .await?;
        Ok(Response::new(DeleteTaskResponse { id: body.task_id }))
    }

    async fn reset(
        &self,
        req: Request<ResetTaskRequest>,
    ) -> Result<Response<ResetTaskResponse>, Status> {
        let body = req.into_inner();
        let summary = self
            .state
            .tasks
            .reset(&body.workspace_id, &body.task_id)
            .await
            .map_err(to_status)?;
        let files = summary["files"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        let errors = summary["errors"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        Ok(Response::new(ResetTaskResponse {
            task_id: body.task_id,
            files,
            errors,
        }))
    }

    async fn list_agent_settings(
        &self,
        req: Request<ListAgentSettingsRequest>,
    ) -> Result<Response<ListAgentSettingsResponse>, Status> {
        let body = req.into_inner();
        let settings = self
            .state
            .tasks
            .list_agent_settings(&body.workspace_id, &body.task_id)
            .await?
            .into_iter()
            .map(Into::into)
            .collect();
        Ok(Response::new(ListAgentSettingsResponse { settings }))
    }

    async fn set_agent_settings(
        &self,
        req: Request<SetAgentSettingsRequest>,
    ) -> Result<Response<TaskAgentSetting>, Status> {
        let body = req.into_inner();
        self.state
            .tasks
            .set_agent_settings(
                &body.workspace_id,
                &body.task_id,
                &body.agent_id,
                body.loop_amount,
            )
            .await?;
        Ok(Response::new(TaskAgentSetting {
            agent_id: body.agent_id,
            loop_amount: body.loop_amount,
        }))
    }

    async fn delete_agent_settings(
        &self,
        req: Request<DeleteAgentSettingsRequest>,
    ) -> Result<Response<DeleteAgentSettingsResponse>, Status> {
        let body = req.into_inner();
        self.state
            .tasks
            .delete_agent_settings(&body.workspace_id, &body.task_id, &body.agent_id)
            .await?;
        Ok(Response::new(DeleteAgentSettingsResponse { ok: true }))
    }
}

#[tonic::async_trait]
impl VariantService for VariantHandler {
    async fn create(
        &self,
        req: Request<CreateVariantsRequest>,
    ) -> Result<Response<CreateVariantsResponse>, Status> {
        let body = req.into_inner();
        let created = self
            .state
            .tasks
            .create_variants(&body.workspace_id, &body.task_id, body.count)
            .await?;
        Ok(Response::new(CreateVariantsResponse {
            task_id: body.task_id,
            created: created
                .into_iter()
                .map(|(id, position)| CreatedVariant { id, position })
                .collect(),
        }))
    }

    async fn update(
        &self,
        req: Request<UpdateVariantRequest>,
    ) -> Result<Response<UpdateVariantResponse>, Status> {
        let body = req.into_inner();
        let new_status = if let Some(s) = body.status.filter(|s| !s.is_empty()) {
            s
        } else if let Some(a) = body.action.filter(|s| !s.is_empty()) {
            match a.trim() {
                "accept" => "accepted".to_string(),
                "rework" => "rework".to_string(),
                _ => {
                    return Err(Status::invalid_argument(
                        "action must be one of ['accept', 'rework']",
                    ))
                }
            }
        } else {
            return Err(Status::invalid_argument(
                "request must include either status or action",
            ));
        };
        self.state
            .tasks
            .update_variant(
                &body.workspace_id,
                &body.task_id,
                &body.variant_id,
                &new_status,
            )
            .await?;
        Ok(Response::new(UpdateVariantResponse {
            task_id: body.task_id,
            variant_id: body.variant_id,
            status: new_status,
        }))
    }
}
