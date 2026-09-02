use std::path::Path;
use std::pin::Pin;

use modula_rpc::v1::{
    project_service_server::ProjectService, CloneProjectRequest, CloneProjectResponse,
    CommitSummary, CreateProjectRequest, CreateProjectResponse, DeleteProjectRequest,
    DeleteProjectResponse, DiffChunk, GetCommitDiffRequest, GetProjectRequest,
    GetRepoBranchesRequest, GetRepoBranchesResponse, GetTaskBranchesRequest,
    GetTaskBranchesResponse, ListCommitsRequest, ListCommitsResponse, ListProjectsRequest,
    ListProjectsResponse, Project, ProjectDiffRequest, RepoBranchInfo, StageRequest, StageResponse,
    UnstageRequest, UnstageResponse, UpdateProjectRequest, UpdateProjectResponse,
};
use tokio_stream::Stream;
use tonic::{Request, Response, Status};

use super::chunk;
use super::error::{internal, to_status};
use modula_services::branches;
use modula_state::AppState;

pub struct ProjectHandler {
    pub state: AppState,
}

type DiffChunkStream = Pin<Box<dyn Stream<Item = Result<DiffChunk, Status>> + Send>>;

/// Deliver an assembled byte payload as a chunked server stream, keeping every
/// message well under tonic's 4 MB decode cap (see [`chunk`]).
fn chunk_stream(bytes: Vec<u8>) -> DiffChunkStream {
    let chunks = chunk::split(bytes)
        .into_iter()
        .map(|data| Ok(DiffChunk { data }));
    Box::pin(tokio_stream::iter(chunks))
}

#[tonic::async_trait]
impl ProjectService for ProjectHandler {
    async fn list(
        &self,
        req: Request<ListProjectsRequest>,
    ) -> Result<Response<ListProjectsResponse>, Status> {
        let projects = self
            .state
            .projects
            .list(&req.into_inner().workspace_id)
            .await
            .map_err(to_status)?
            .into_iter()
            .map(Project::from)
            .collect();
        Ok(Response::new(ListProjectsResponse { projects }))
    }

    async fn get(&self, req: Request<GetProjectRequest>) -> Result<Response<Project>, Status> {
        let body = req.into_inner();
        let project = self
            .state
            .projects
            .get(&body.workspace_id, &body.project_id)
            .await
            .map_err(to_status)?;
        Ok(Response::new(project.into()))
    }

    async fn create(
        &self,
        req: Request<CreateProjectRequest>,
    ) -> Result<Response<CreateProjectResponse>, Status> {
        let body = req.into_inner();
        let created = self
            .state
            .projects
            .create(
                &body.workspace_id,
                &body.name,
                &body.path,
                &body.base_branch,
            )
            .await
            .map_err(to_status)?;
        Ok(Response::new(CreateProjectResponse {
            id: created.id,
            name: created.name,
        }))
    }

    async fn clone(
        &self,
        req: Request<CloneProjectRequest>,
    ) -> Result<Response<CloneProjectResponse>, Status> {
        let body = req.into_inner();
        let created = self
            .state
            .projects
            .clone(&body.workspace_id, &body.name, &body.path, &body.git_url)
            .await
            .map_err(to_status)?;
        Ok(Response::new(CloneProjectResponse {
            id: created.id,
            name: created.name,
        }))
    }

    async fn update(
        &self,
        req: Request<UpdateProjectRequest>,
    ) -> Result<Response<UpdateProjectResponse>, Status> {
        let body = req.into_inner();
        self.state
            .projects
            .update(
                &body.workspace_id,
                &body.project_id,
                body.name,
                body.path,
                body.base_branch,
            )
            .await
            .map_err(to_status)?;
        Ok(Response::new(UpdateProjectResponse {
            id: body.project_id,
        }))
    }

    async fn delete(
        &self,
        req: Request<DeleteProjectRequest>,
    ) -> Result<Response<DeleteProjectResponse>, Status> {
        let body = req.into_inner();
        self.state
            .projects
            .delete(&body.workspace_id, &body.project_id)
            .await
            .map_err(to_status)?;
        Ok(Response::new(DeleteProjectResponse {
            id: body.project_id,
        }))
    }

    type DiffStream = DiffChunkStream;
    async fn diff(
        &self,
        req: Request<ProjectDiffRequest>,
    ) -> Result<Response<Self::DiffStream>, Status> {
        let body = req.into_inner();
        let diff = self
            .state
            .projects
            .working_diff(&body.workspace_id, &body.project_id, body.branch.as_deref())
            .await
            .map_err(to_status)?;
        let bytes = serde_json::to_vec(&diff).map_err(internal)?;
        Ok(Response::new(chunk_stream(bytes)))
    }

    type DiffTextStream = DiffChunkStream;
    async fn diff_text(
        &self,
        req: Request<ProjectDiffRequest>,
    ) -> Result<Response<Self::DiffTextStream>, Status> {
        let body = req.into_inner();
        let diff = self
            .state
            .projects
            .working_diff_text(&body.workspace_id, &body.project_id, body.branch.as_deref())
            .await
            .map_err(to_status)?;
        let bytes = serde_json::to_vec(&diff).map_err(internal)?;
        Ok(Response::new(chunk_stream(bytes)))
    }

    async fn list_commits(
        &self,
        req: Request<ListCommitsRequest>,
    ) -> Result<Response<ListCommitsResponse>, Status> {
        let body = req.into_inner();
        let log = self
            .state
            .projects
            .commits_log(
                &body.workspace_id,
                &body.project_id,
                body.branch.as_deref(),
                body.since.as_deref(),
                body.limit,
            )
            .await
            .map_err(to_status)?;
        let commits = log["commits"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default()
            .iter()
            .map(|c| CommitSummary {
                sha: c["sha"].as_str().unwrap_or_default().to_string(),
                short: c["short"].as_str().unwrap_or_default().to_string(),
                author: c["author"].as_str().unwrap_or_default().to_string(),
                time: c["time"].as_i64().unwrap_or_default(),
                subject: c["subject"].as_str().unwrap_or_default().to_string(),
            })
            .collect();
        Ok(Response::new(ListCommitsResponse { commits }))
    }

    type GetCommitDiffStream = DiffChunkStream;
    async fn get_commit_diff(
        &self,
        req: Request<GetCommitDiffRequest>,
    ) -> Result<Response<Self::GetCommitDiffStream>, Status> {
        let body = req.into_inner();
        let diff = self
            .state
            .projects
            .commit_diff(
                &body.workspace_id,
                &body.project_id,
                body.branch.as_deref(),
                &body.sha,
            )
            .await
            .map_err(to_status)?;
        let bytes = serde_json::to_vec(&diff).map_err(internal)?;
        Ok(Response::new(chunk_stream(bytes)))
    }

    async fn stage(&self, req: Request<StageRequest>) -> Result<Response<StageResponse>, Status> {
        let body = req.into_inner();
        self.state
            .projects
            .stage(
                &body.workspace_id,
                &body.project_id,
                body.branch.as_deref(),
                &body.files,
            )
            .await
            .map_err(to_status)?;
        Ok(Response::new(StageResponse { ok: true }))
    }

    async fn unstage(
        &self,
        req: Request<UnstageRequest>,
    ) -> Result<Response<UnstageResponse>, Status> {
        let body = req.into_inner();
        self.state
            .projects
            .unstage(
                &body.workspace_id,
                &body.project_id,
                body.branch.as_deref(),
                &body.files,
            )
            .await
            .map_err(to_status)?;
        Ok(Response::new(UnstageResponse { ok: true }))
    }

    async fn get_task_branches(
        &self,
        req: Request<GetTaskBranchesRequest>,
    ) -> Result<Response<GetTaskBranchesResponse>, Status> {
        let body = req.into_inner();
        let rows = self
            .state
            .projects
            .branches_for_task(&body.workspace_id, &body.task_id)
            .await
            .map_err(to_status)?;
        let branches = rows
            .iter()
            .map(serde_json::to_string)
            .collect::<Result<_, _>>()
            .map_err(internal)?;
        Ok(Response::new(GetTaskBranchesResponse { branches }))
    }

    async fn get_repo_branches(
        &self,
        req: Request<GetRepoBranchesRequest>,
    ) -> Result<Response<GetRepoBranchesResponse>, Status> {
        // Pure git inspection of a caller-supplied path (no repo/DB access), so
        // it reads the agnostic `branches` helper directly rather than through a
        // service method that would only forward the call.
        let path = req.into_inner().path;
        let path = path.trim();
        if path.is_empty() {
            return Err(Status::invalid_argument("path is required"));
        }
        let rb = branches::repo_branches(Path::new(path));
        Ok(Response::new(GetRepoBranchesResponse {
            info: Some(RepoBranchInfo {
                is_git: rb.is_git,
                branches: rb.branches,
                default_branch: rb.default_branch,
            }),
        }))
    }
}
