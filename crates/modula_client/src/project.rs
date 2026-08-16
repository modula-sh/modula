//! Project unary calls plus the chunked diff/commit-diff reads. The engine
//! delivers working-tree diffs, raw diff text, and commit diffs as chunked
//! `DiffChunk` server streams purely to clear tonic's 4 MB decode cap — the
//! whole payload exists at request time — so these methods reassemble the chunks
//! into the original JSON value and return it from one call.

use modula_rpc::v1::{
    project_service_client::ProjectServiceClient, CloneProjectRequest, CreateProjectRequest,
    DeleteProjectRequest, DiffChunk, GetCommitDiffRequest, GetProjectRequest,
    GetRepoBranchesRequest, GetTaskBranchesRequest, ListCommitsRequest, ListProjectsRequest,
    ProjectDiffRequest, StageRequest, UnstageRequest, UpdateProjectRequest,
};
use modula_types::{CommitSummary, Project, RepoBranchInfo};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tonic::Streaming;

use crate::error::{rpc, ClientError};
use crate::ModulaClient;

/// Result of `create_project` / `clone_project` — the new project's id and name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatedProject {
    pub id: String,
    pub name: String,
}

/// Concatenate a `DiffChunk` server stream and parse the assembled JSON bytes.
async fn collect_diff(mut stream: Streaming<DiffChunk>) -> Result<Value, ClientError> {
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.message().await.map_err(rpc)? {
        bytes.extend_from_slice(&chunk.data);
    }
    serde_json::from_slice(&bytes).map_err(|e| ClientError::Rpc(e.to_string()))
}

impl ModulaClient {
    pub async fn list_projects(&self, workspace_id: &str) -> Result<Vec<Project>, ClientError> {
        let resp = self
            .projects()
            .await?
            .list(ListProjectsRequest {
                workspace_id: workspace_id.to_string(),
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(resp.projects.into_iter().map(Project::from).collect())
    }

    pub async fn get_project(
        &self,
        workspace_id: &str,
        project_id: &str,
    ) -> Result<Project, ClientError> {
        let resp = self
            .projects()
            .await?
            .get(GetProjectRequest {
                workspace_id: workspace_id.to_string(),
                project_id: project_id.to_string(),
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(Project::from(resp))
    }

    pub async fn create_project(
        &self,
        workspace_id: &str,
        name: &str,
        path: &str,
        base_branch: &str,
    ) -> Result<CreatedProject, ClientError> {
        let resp = self
            .projects()
            .await?
            .create(CreateProjectRequest {
                workspace_id: workspace_id.to_string(),
                name: name.to_string(),
                path: path.to_string(),
                base_branch: base_branch.to_string(),
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(CreatedProject {
            id: resp.id,
            name: resp.name,
        })
    }

    pub async fn clone_project(
        &self,
        workspace_id: &str,
        name: &str,
        path: &str,
        git_url: &str,
    ) -> Result<CreatedProject, ClientError> {
        let mut client = self.projects().await?;
        // The generated `Clone` RPC method is named `clone`, which collides with
        // the client's derived `Clone::clone`; call through UFCS to select the
        // inherent gRPC method.
        let resp = ProjectServiceClient::clone(
            &mut client,
            CloneProjectRequest {
                workspace_id: workspace_id.to_string(),
                name: name.to_string(),
                path: path.to_string(),
                git_url: git_url.to_string(),
            },
        )
        .await
        .map_err(rpc)?
        .into_inner();
        Ok(CreatedProject {
            id: resp.id,
            name: resp.name,
        })
    }

    pub async fn update_project(
        &self,
        workspace_id: &str,
        project_id: &str,
        name: Option<String>,
        path: Option<String>,
        base_branch: Option<String>,
    ) -> Result<(), ClientError> {
        self.projects()
            .await?
            .update(UpdateProjectRequest {
                workspace_id: workspace_id.to_string(),
                project_id: project_id.to_string(),
                name,
                path,
                base_branch,
            })
            .await
            .map_err(rpc)?;
        Ok(())
    }

    pub async fn delete_project(
        &self,
        workspace_id: &str,
        project_id: &str,
    ) -> Result<(), ClientError> {
        self.projects()
            .await?
            .delete(DeleteProjectRequest {
                workspace_id: workspace_id.to_string(),
                project_id: project_id.to_string(),
            })
            .await
            .map_err(rpc)?;
        Ok(())
    }

    /// Working-tree diff as the assembled JSON the frontend consumes.
    pub async fn project_diff(
        &self,
        workspace_id: &str,
        project_id: &str,
        branch: Option<String>,
    ) -> Result<Value, ClientError> {
        let stream = self
            .projects()
            .await?
            .diff(ProjectDiffRequest {
                workspace_id: workspace_id.to_string(),
                project_id: project_id.to_string(),
                branch,
            })
            .await
            .map_err(rpc)?
            .into_inner();
        collect_diff(stream).await
    }

    /// Raw unified-diff text as the assembled JSON the frontend consumes.
    pub async fn project_diff_text(
        &self,
        workspace_id: &str,
        project_id: &str,
        branch: Option<String>,
    ) -> Result<Value, ClientError> {
        let stream = self
            .projects()
            .await?
            .diff_text(ProjectDiffRequest {
                workspace_id: workspace_id.to_string(),
                project_id: project_id.to_string(),
                branch,
            })
            .await
            .map_err(rpc)?
            .into_inner();
        collect_diff(stream).await
    }

    pub async fn list_commits(
        &self,
        workspace_id: &str,
        project_id: &str,
        branch: Option<String>,
        since: Option<String>,
        limit: u32,
    ) -> Result<Vec<CommitSummary>, ClientError> {
        let resp = self
            .projects()
            .await?
            .list_commits(ListCommitsRequest {
                workspace_id: workspace_id.to_string(),
                project_id: project_id.to_string(),
                branch,
                since,
                limit,
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(resp.commits.into_iter().map(CommitSummary::from).collect())
    }

    /// A single commit's diff as the assembled JSON the frontend consumes.
    pub async fn commit_diff(
        &self,
        workspace_id: &str,
        project_id: &str,
        sha: &str,
        branch: Option<String>,
    ) -> Result<Value, ClientError> {
        let stream = self
            .projects()
            .await?
            .get_commit_diff(GetCommitDiffRequest {
                workspace_id: workspace_id.to_string(),
                project_id: project_id.to_string(),
                sha: sha.to_string(),
                branch,
            })
            .await
            .map_err(rpc)?
            .into_inner();
        collect_diff(stream).await
    }

    pub async fn stage(
        &self,
        workspace_id: &str,
        project_id: &str,
        files: Vec<String>,
        branch: Option<String>,
    ) -> Result<(), ClientError> {
        self.projects()
            .await?
            .stage(StageRequest {
                workspace_id: workspace_id.to_string(),
                project_id: project_id.to_string(),
                branch,
                files,
            })
            .await
            .map_err(rpc)?;
        Ok(())
    }

    pub async fn unstage(
        &self,
        workspace_id: &str,
        project_id: &str,
        files: Vec<String>,
        branch: Option<String>,
    ) -> Result<(), ClientError> {
        self.projects()
            .await?
            .unstage(UnstageRequest {
                workspace_id: workspace_id.to_string(),
                project_id: project_id.to_string(),
                branch,
                files,
            })
            .await
            .map_err(rpc)?;
        Ok(())
    }

    /// Branch info for a task's variants. The engine returns each entry as a
    /// JSON string; they are parsed into values the frontend consumes directly.
    pub async fn task_branches(
        &self,
        workspace_id: &str,
        task_id: &str,
    ) -> Result<Vec<Value>, ClientError> {
        let resp = self
            .projects()
            .await?
            .get_task_branches(GetTaskBranchesRequest {
                workspace_id: workspace_id.to_string(),
                task_id: task_id.to_string(),
            })
            .await
            .map_err(rpc)?
            .into_inner();
        resp.branches
            .iter()
            .map(|s| serde_json::from_str(s).map_err(|e| ClientError::Rpc(e.to_string())))
            .collect()
    }

    pub async fn repo_branches(
        &self,
        workspace_id: &str,
        path: &str,
    ) -> Result<RepoBranchInfo, ClientError> {
        let resp = self
            .projects()
            .await?
            .get_repo_branches(GetRepoBranchesRequest {
                workspace_id: workspace_id.to_string(),
                path: path.to_string(),
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(resp
            .info
            .map(RepoBranchInfo::from)
            .unwrap_or(RepoBranchInfo {
                is_git: false,
                branches: vec![],
                default_branch: None,
            }))
    }
}
