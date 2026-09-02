//! Project CRUD/business service plus the agnostic worktree-mapping helpers.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{json, Value as JsonValue};

use modula_db::projects::ProjectRepository;
use modula_db::tasks::TaskRepository;
use modula_db::Database;
use modula_types::Project;

use super::{branches, diffs, workspaces};
use crate::events::{self, EventSink};
use modula_core::error::{ApiError, ApiResult};

/// Fill a project's on-disk enrichment (`exists` + `.worktrees` names) the repo
/// leaves empty. The DB boundary can't see the filesystem, so the owning service
/// does it before a handler sees the value.
fn enrich(p: &mut Project) {
    let path = Path::new(&p.path);
    p.exists = path.is_dir();
    p.worktrees = worktrees_on_disk(path);
}

/// Sorted names of the worktree directories under `<path>/.worktrees`. Empty
/// when the project (or its worktree root) is absent on disk.
pub fn worktrees_on_disk(path: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(path.join(".worktrees")) else {
        return Vec::new();
    };
    let mut worktrees: Vec<String> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .collect();
    worktrees.sort();
    worktrees
}

/// Map task id → list of project names whose worktrees match a branch
/// for that task. Pure file-system inspection; takes already-loaded
/// project paths and task ids (so callers can do the DB read once).
pub fn task_projects(projects: &[(String, PathBuf)], task_ids: &[String]) -> JsonValue {
    let mut out: std::collections::BTreeMap<String, std::collections::BTreeSet<String>> =
        Default::default();
    for tid in task_ids {
        out.insert(tid.clone(), Default::default());
    }
    for (pname, ppath) in projects {
        if !ppath.is_dir() {
            continue;
        }
        for (branch, _wt, _head) in branches::worktree_rows_for_project(ppath.as_path()) {
            for tid in task_ids {
                if branches::task_branch_match(&branch, tid) {
                    out.get_mut(tid).unwrap().insert(pname.clone());
                    break;
                }
            }
        }
    }
    let mut map = serde_json::Map::new();
    for (tid, set) in out {
        let arr: Vec<String> = set.into_iter().collect();
        map.insert(tid, json!(arr));
    }
    JsonValue::Object(map)
}

pub struct CreatedProject {
    pub id: String,
    pub name: String,
}

fn require_nonempty<'a>(value: &'a str, field: &str) -> ApiResult<&'a str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ApiError::BadRequest(format!("{field} is required")));
    }
    Ok(trimmed)
}

fn require_abs_path(value: &str) -> ApiResult<String> {
    let path = require_nonempty(value, "path")?;
    // Platform-aware: `/foo` on Unix, `C:\foo` / `\\server\share` on Windows.
    if !Path::new(path).is_absolute() {
        return Err(ApiError::BadRequest("path must be absolute".into()));
    }
    Ok(path.to_string())
}

/// Validate an optional patch field: a missing or blank value leaves it
/// unchanged, a present value must pass `check`.
fn patch_field(
    value: Option<String>,
    check: impl Fn(&str) -> ApiResult<String>,
) -> ApiResult<Option<String>> {
    match value.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(v) => check(v).map(Some),
        None => Ok(None),
    }
}

/// Project CRUD + git-inspection business service. Owns the project repository
/// (and the task repository, for the task->branch mapping) so gRPC reaches
/// projects and their worktree/diff views through this, never the repos or the
/// low-level git helpers directly.
///
/// Catalog CRUD is otherwise event-free, but `projects` is on the sync feed's
/// whitelist, so these writes publish: without an event a replica never learns
/// the row changed.
#[derive(Clone)]
pub struct ProjectService {
    pool: Database,
    projects: ProjectRepository,
    tasks: TaskRepository,
    events: Arc<dyn EventSink>,
}

impl ProjectService {
    pub fn new(
        pool: Database,
        projects: ProjectRepository,
        tasks: TaskRepository,
        events: Arc<dyn EventSink>,
    ) -> Self {
        Self {
            pool,
            projects,
            tasks,
            events,
        }
    }

    pub async fn list(&self, ws: &str) -> ApiResult<Vec<Project>> {
        let mut projects = self.projects.list(&self.pool, ws).await?;
        for p in &mut projects {
            enrich(p);
        }
        Ok(projects)
    }

    pub async fn get(&self, ws: &str, id: &str) -> ApiResult<Project> {
        let mut p = self.projects.get(&self.pool, ws, id).await?;
        enrich(&mut p);
        Ok(p)
    }

    pub async fn create(
        &self,
        ws: &str,
        name: &str,
        path: &str,
        base_branch: &str,
    ) -> ApiResult<CreatedProject> {
        let name = require_nonempty(name, "name")?.to_string();
        let path = require_abs_path(path)?;
        let base_branch = require_nonempty(base_branch, "base_branch")?.to_string();
        let id = self
            .projects
            .create(&self.pool, ws, &name, &path, &base_branch)
            .await?;
        self.publish(ws, events::PROJECT_CREATE, &id).await;
        Ok(CreatedProject { id, name })
    }

    /// Clone `git_url` into `path`, then register it, defaulting `base_branch`
    /// to the cloned repo's default branch (falling back to `main`).
    pub async fn clone(
        &self,
        ws: &str,
        name: &str,
        path: &str,
        git_url: &str,
    ) -> ApiResult<CreatedProject> {
        let name = require_nonempty(name, "name")?.to_string();
        let path = require_abs_path(path)?;
        let git_url = require_nonempty(git_url, "git_url")?.to_string();

        let (dest, url) = (path.clone(), git_url);
        tokio::task::spawn_blocking(move || branches::clone_repo(&url, Path::new(&dest)))
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .map_err(ApiError::BadRequest)?;

        let base_branch = branches::repo_branches(Path::new(&path))
            .default_branch
            .unwrap_or_else(|| "main".into());
        let id = self
            .projects
            .create(&self.pool, ws, &name, &path, &base_branch)
            .await?;
        self.publish(ws, events::PROJECT_CREATE, &id).await;
        Ok(CreatedProject { id, name })
    }

    pub async fn update(
        &self,
        ws: &str,
        id: &str,
        name: Option<String>,
        path: Option<String>,
        base_branch: Option<String>,
    ) -> ApiResult<()> {
        let name = patch_field(name, |s| Ok(s.to_string()))?;
        let path = patch_field(path, require_abs_path)?;
        let base_branch = patch_field(base_branch, |s| {
            require_nonempty(s, "base_branch").map(str::to_string)
        })?;
        self.projects
            .patch(
                &self.pool,
                ws,
                id,
                name.as_deref(),
                path.as_deref(),
                base_branch.as_deref(),
            )
            .await?;
        self.publish(ws, events::PROJECT_UPDATE, id).await;
        Ok(())
    }

    pub async fn delete(&self, ws: &str, id: &str) -> ApiResult<()> {
        self.projects.delete(&self.pool, ws, id).await?;
        self.publish(ws, events::PROJECT_DELETE, id).await;
        Ok(())
    }

    async fn publish(&self, ws: &str, type_: &str, id: &str) {
        self.events
            .publish(ws, type_, json!({ "project_id": id }))
            .await;
    }

    /// Working directory for `(project, branch?)`: the matching worktree if
    /// `branch` names one, else the project's main checkout.
    async fn resolve_cwd(&self, ws: &str, id: &str, branch: Option<&str>) -> ApiResult<PathBuf> {
        let project = self.projects.get(&self.pool, ws, id).await?;
        diffs::project_cwd(Path::new(&project.path), branch)
            .ok_or_else(|| ApiError::NotFound(format!("project path missing: {}", project.path)))
    }

    pub async fn working_diff(
        &self,
        ws: &str,
        id: &str,
        branch: Option<&str>,
    ) -> ApiResult<JsonValue> {
        let cwd = self.resolve_cwd(ws, id, branch).await?;
        Ok(diffs::working_diff(&cwd))
    }

    pub async fn working_diff_text(
        &self,
        ws: &str,
        id: &str,
        branch: Option<&str>,
    ) -> ApiResult<JsonValue> {
        let cwd = self.resolve_cwd(ws, id, branch).await?;
        Ok(diffs::working_diff_text(&cwd))
    }

    pub async fn commits_log(
        &self,
        ws: &str,
        id: &str,
        branch: Option<&str>,
        since: Option<&str>,
        limit: u32,
    ) -> ApiResult<JsonValue> {
        let cwd = self.resolve_cwd(ws, id, branch).await?;
        let limit = if limit == 0 { 20 } else { limit };
        let limit = limit.clamp(1, 100);
        Ok(diffs::commits_log(&cwd, branch, since, limit))
    }

    pub async fn commit_diff(
        &self,
        ws: &str,
        id: &str,
        branch: Option<&str>,
        sha: &str,
    ) -> ApiResult<JsonValue> {
        let cwd = self.resolve_cwd(ws, id, branch).await?;
        Ok(diffs::commit_diff(&cwd, sha))
    }

    pub async fn stage(
        &self,
        ws: &str,
        id: &str,
        branch: Option<&str>,
        files: &[String],
    ) -> ApiResult<()> {
        let cwd = self.resolve_cwd(ws, id, branch).await?;
        diffs::stage_files(&cwd, files)
    }

    pub async fn unstage(
        &self,
        ws: &str,
        id: &str,
        branch: Option<&str>,
        files: &[String],
    ) -> ApiResult<()> {
        let cwd = self.resolve_cwd(ws, id, branch).await?;
        diffs::unstage_files(&cwd, files)
    }

    /// Worktree branches across the workspace's projects that belong to `task`.
    pub async fn branches_for_task(&self, ws: &str, task_id: &str) -> ApiResult<Vec<JsonValue>> {
        let task = self.tasks.get(&self.pool, ws, task_id).await?;
        let slug = workspaces::task_spec_slug(task.external_id.as_deref(), &task.title);
        let projects = self.projects.list(&self.pool, ws).await?;
        Ok(branches::branches_for_task(&projects, &slug))
    }
}
