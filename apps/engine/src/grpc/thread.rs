use modula_rpc::convert::{kind_to_str, verdict_to_str};
use modula_rpc::v1::{
    thread_service_server::ThreadService, AppendEntryRequest, AppendEntryResponse,
    DeleteEntryRequest, DeleteEntryResponse, EditEntryRequest, EditEntryResponse,
    GetThreadsRequest, GetThreadsResponse,
};
use tonic::{Request, Response, Status};

use crate::services::threads::AppendInput;
use crate::state::AppState;

pub struct ThreadHandler {
    pub state: AppState,
}

#[tonic::async_trait]
impl ThreadService for ThreadHandler {
    async fn get_threads(
        &self,
        req: Request<GetThreadsRequest>,
    ) -> Result<Response<GetThreadsResponse>, Status> {
        let body = req.into_inner();
        let bundle = self
            .state
            .threads
            .list_threads(&body.workspace_id, &body.task_id)
            .await?;
        Ok(Response::new(bundle.into()))
    }

    async fn append_entry(
        &self,
        req: Request<AppendEntryRequest>,
    ) -> Result<Response<AppendEntryResponse>, Status> {
        let body = req.into_inner();
        let ws = body.workspace_id;
        let task_id = body.task_id;
        let verdict = body
            .verdict
            .and_then(|v| if v == 0 { None } else { verdict_to_str(v) })
            .map(str::to_string);
        let affected_variants = if body.affected_variants.is_empty() {
            None
        } else {
            Some(body.affected_variants)
        };
        let input = AppendInput {
            content: body.content,
            variant: body.variant_id,
            author: body.author,
            kind: kind_to_str(body.kind).to_string(),
            round: body.round,
            verdict,
            affected_variants,
        };
        let entry = self.state.threads.create(&ws, &task_id, input).await?;
        Ok(Response::new(AppendEntryResponse {
            entry: Some(entry.into()),
        }))
    }

    async fn edit_entry(
        &self,
        req: Request<EditEntryRequest>,
    ) -> Result<Response<EditEntryResponse>, Status> {
        let body = req.into_inner();
        let ws = body.workspace_id;
        let task_id = body.task_id;
        let entry_id = body.entry_id;
        let entry = self
            .state
            .threads
            .update(&ws, &task_id, entry_id, &body.author, &body.content)
            .await?;
        Ok(Response::new(EditEntryResponse {
            entry: Some(entry.into()),
        }))
    }

    async fn delete_entry(
        &self,
        req: Request<DeleteEntryRequest>,
    ) -> Result<Response<DeleteEntryResponse>, Status> {
        let body = req.into_inner();
        self.state
            .threads
            .delete(
                &body.workspace_id,
                &body.task_id,
                body.entry_id,
                &body.author,
            )
            .await?;
        Ok(Response::new(DeleteEntryResponse { ok: true }))
    }
}
