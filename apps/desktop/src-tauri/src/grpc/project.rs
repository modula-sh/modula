//! Project unary commands plus the chunked diff/commit-diff reads.
//!
//! The engine delivers working-tree diffs, raw diff text, and commit diffs as
//! chunked server streams purely to clear tonic's 4 MB decode cap — the whole
//! payload exists at request time — so `modula-client` reassembles each into the
//! original JSON value and these commands return it from a single `invoke`.
//! Only genuinely live streams (logs, conversation deltas, event/run watch) stay
//! streaming over a `Channel`.

use modula_client::{CreatedProject, ModulaClient};
use modula_types::{Project, RepoBranchInfo};
use serde_json::{json, Value};
use tauri::State;

#[tauri::command]
pub async fn project_list(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
) -> Result<Vec<Project>, String> {
    Ok(engine.list_projects(&workspace_id).await?)
}

#[tauri::command]
pub async fn project_get(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    project_id: String,
) -> Result<Project, String> {
    Ok(engine.get_project(&workspace_id, &project_id).await?)
}

#[tauri::command]
pub async fn project_create(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    name: String,
    path: String,
    base_branch: String,
) -> Result<CreatedProject, String> {
    Ok(engine
        .create_project(&workspace_id, &name, &path, &base_branch)
        .await?)
}

#[tauri::command]
pub async fn project_clone(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    name: String,
    path: String,
    git_url: String,
) -> Result<CreatedProject, String> {
    Ok(engine
        .clone_project(&workspace_id, &name, &path, &git_url)
        .await?)
}

#[tauri::command]
pub async fn project_update(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    project_id: String,
    name: Option<String>,
    path: Option<String>,
    base_branch: Option<String>,
) -> Result<(), String> {
    engine
        .update_project(&workspace_id, &project_id, name, path, base_branch)
        .await?;
    Ok(())
}

#[tauri::command]
pub async fn project_delete(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    project_id: String,
) -> Result<(), String> {
    engine.delete_project(&workspace_id, &project_id).await?;
    Ok(())
}

#[tauri::command]
pub async fn project_diff(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    project_id: String,
    branch: Option<String>,
) -> Result<Value, String> {
    Ok(engine
        .project_diff(&workspace_id, &project_id, branch)
        .await?)
}

#[tauri::command]
pub async fn project_diff_text(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    project_id: String,
    branch: Option<String>,
) -> Result<Value, String> {
    Ok(engine
        .project_diff_text(&workspace_id, &project_id, branch)
        .await?)
}

#[tauri::command]
pub async fn project_commits(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    project_id: String,
    branch: Option<String>,
    since: Option<String>,
    limit: Option<u32>,
) -> Result<Value, String> {
    let commits = engine
        .list_commits(
            &workspace_id,
            &project_id,
            branch,
            since,
            limit.unwrap_or(0),
        )
        .await?;
    Ok(json!({ "commits": commits }))
}

#[tauri::command]
pub async fn project_commit_diff(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    project_id: String,
    sha: String,
    branch: Option<String>,
) -> Result<Value, String> {
    Ok(engine
        .commit_diff(&workspace_id, &project_id, &sha, branch)
        .await?)
}

#[tauri::command]
pub async fn project_stage(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    project_id: String,
    files: Vec<String>,
    branch: Option<String>,
) -> Result<(), String> {
    engine
        .stage(&workspace_id, &project_id, files, branch)
        .await?;
    Ok(())
}

#[tauri::command]
pub async fn project_unstage(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    project_id: String,
    files: Vec<String>,
    branch: Option<String>,
) -> Result<(), String> {
    engine
        .unstage(&workspace_id, &project_id, files, branch)
        .await?;
    Ok(())
}

#[tauri::command]
pub async fn project_task_branches(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    task_id: String,
) -> Result<Vec<Value>, String> {
    Ok(engine.task_branches(&workspace_id, &task_id).await?)
}

#[tauri::command]
pub async fn project_repo_branches(
    engine: State<'_, ModulaClient>,
    workspace_id: String,
    path: String,
) -> Result<RepoBranchInfo, String> {
    Ok(engine.repo_branches(&workspace_id, &path).await?)
}
