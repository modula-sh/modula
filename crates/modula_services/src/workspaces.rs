//! Workspace filesystem helpers.
//!
//! All structured per-workspace state (config, tasks, roadmap, variants)
//! lives in SQLite — see `modula_db`. This module owns the on-disk side:
//! the workspace directory itself plus the embedded markdown templates
//! (wiki, agent prompts, overview/workflow docs) seeded on creation.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use chrono::{DateTime, SecondsFormat, Utc};
use include_dir::{include_dir, Dir};
use modula_db::workspaces::WorkspaceRepository;
use modula_db::Database;
use modula_types::Workspace;

use crate::scheduler::SchedulerHandle;
use modula_core::error::{ApiError, ApiResult};
use modula_core::paths::Paths;
use modula_core::slug;

const TEMPLATES: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/templates");

/// Slug for a task's spec folder, e.g. "MOD-0001 Some new adjustment" →
/// "mod-0001-some-new-adjustment". Title-only when there's no external id.
/// Never contains a UUID.
pub fn task_spec_slug(external_id: Option<&str>, title: &str) -> String {
    let combined = match external_id {
        Some(e) if !e.trim().is_empty() => format!("{e} {title}"),
        _ => title.to_string(),
    };
    slug::slugify(&combined)
}

/// `<ws_dir>/specs/<task-slug>` — the folder holding a task's variant specs.
pub fn task_spec_dir(ws_dir: &Path, external_id: Option<&str>, title: &str) -> PathBuf {
    ws_dir
        .join("specs")
        .join(task_spec_slug(external_id, title))
}

/// Scaffold the on-disk side of a workspace (markdown artifacts only —
/// structured state lives in SQLite). Caller is responsible for the DB row.
/// `slug` is the directory name under `<modula>`.
pub fn scaffold_workspace_files(modula: &Path, slug: &str) -> Result<PathBuf, ApiError> {
    let ws_dir = modula.join(slug);
    if ws_dir.exists() {
        return Err(ApiError::Conflict(format!(
            "workspace directory already exists: {slug}"
        )));
    }

    fs::create_dir_all(&ws_dir)?;
    fs::create_dir_all(ws_dir.join("specs"))?;
    fs::create_dir_all(ws_dir.join("logs"))?;

    if let Some(wiki_dir) = TEMPLATES.get_dir("wiki") {
        copy_tree(wiki_dir, &ws_dir.join("wiki"))?;
    }
    for doc in ["overview.md", "workflow.md"] {
        if let Some(f) = TEMPLATES.get_file(doc) {
            fs::write(ws_dir.join(doc), f.contents())?;
        }
    }

    Ok(ws_dir)
}

fn copy_tree(src: &Dir<'_>, dst: &Path) -> Result<(), ApiError> {
    fs::create_dir_all(dst)?;
    for entry in src.entries() {
        match entry {
            include_dir::DirEntry::File(f) => {
                let name = f
                    .path()
                    .file_name()
                    .ok_or_else(|| ApiError::Internal("template file has no name".into()))?;
                fs::write(dst.join(name), f.contents())?;
            }
            include_dir::DirEntry::Dir(d) => {
                let name = d
                    .path()
                    .file_name()
                    .ok_or_else(|| ApiError::Internal("template dir has no name".into()))?;
                copy_tree(d, &dst.join(name))?;
            }
        }
    }
    Ok(())
}

pub fn format_ts(time: SystemTime) -> String {
    let dt: DateTime<Utc> = time.into();
    dt.to_rfc3339_opts(SecondsFormat::Secs, true)
}

/// Result of [`WorkspaceService::create`]: the identifiers and on-disk path the
/// handler echoes back to the caller.
pub struct CreatedWorkspace {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub path: String,
}

/// Workspace CRUD business service. Owns the workspace repository, the on-disk
/// root (`paths`), and the scheduler handle it re-syncs after a create or
/// delete. gRPC reaches workspaces through this, never the repo.
#[derive(Clone)]
pub struct WorkspaceService {
    pool: Database,
    workspaces: WorkspaceRepository,
    paths: Arc<Paths>,
    scheduler: SchedulerHandle,
}

impl WorkspaceService {
    pub fn new(
        pool: Database,
        workspaces: WorkspaceRepository,
        paths: Arc<Paths>,
        scheduler: SchedulerHandle,
    ) -> Self {
        Self {
            pool,
            workspaces,
            paths,
            scheduler,
        }
    }

    pub async fn list(&self) -> ApiResult<Vec<Workspace>> {
        let mut workspaces = self.workspaces.list(&self.pool).await?;
        for w in &mut workspaces {
            self.fill_path(w);
        }
        Ok(workspaces)
    }

    pub async fn get(&self, id: &str) -> ApiResult<Workspace> {
        let mut w = self.workspaces.get(&self.pool, id).await?;
        self.fill_path(&mut w);
        Ok(w)
    }

    /// Fill the on-disk `path` (`<modula>/<slug>` — the slug is the filesystem
    /// name, matching `workspace_dir`) the repo leaves empty. Built from the
    /// engine's root, so it's a service concern, not a DB column.
    fn fill_path(&self, w: &mut Workspace) {
        w.path = self
            .paths
            .modula
            .join(&w.slug)
            .to_string_lossy()
            .into_owned();
    }

    /// Resolve the workspace's on-disk directory (`<modula>/<slug>`), validating
    /// that both the DB row and the directory exist. Peer services DI this
    /// service and call the method instead of resolving a repo by hand. The slug
    /// is purely the filesystem name; the UUID `id` stays canonical elsewhere.
    pub async fn workspace_dir(&self, id: &str) -> ApiResult<PathBuf> {
        let slug = self.workspaces.slug_for(&self.pool, id).await?;
        let dir = self.paths.modula.join(&slug);
        if !dir.is_dir() {
            return Err(ApiError::NotFound(format!("workspace not found: {id}")));
        }
        Ok(dir)
    }

    /// Create a workspace atomically (the row + its seeds inside one
    /// transaction), then scaffold its on-disk directory and re-sync the
    /// scheduler. Commit precedes the fs/scheduler side effects so a rolled-back
    /// create leaves nothing behind.
    pub async fn create(
        &self,
        name: &str,
        description: Option<&str>,
    ) -> ApiResult<CreatedWorkspace> {
        let name = name.trim();
        if name.is_empty() {
            return Err(ApiError::BadRequest("name is required".into()));
        }
        let desc = description.map(str::trim).filter(|s| !s.is_empty());

        let mut tx = self.pool.begin().await?;
        let id = self.workspaces.create(&mut tx, name, desc).await?;
        tx.commit().await?;

        let slug = self.workspaces.slug_for(&self.pool, &id).await?;
        let ws_dir = scaffold_workspace_files(&self.paths.modula, &slug)?;
        self.scheduler
            .reconfigure()
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;
        Ok(CreatedWorkspace {
            id,
            name: name.to_string(),
            slug,
            path: ws_dir.to_string_lossy().into_owned(),
        })
    }

    pub async fn delete(&self, id: &str) -> ApiResult<()> {
        self.workspaces.delete(&self.pool, id).await?;
        let dir = self.paths.modula.join(id);
        if dir.is_dir() {
            fs::remove_dir_all(&dir)?;
        }
        self.scheduler
            .reconfigure()
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_spec_slug_examples() {
        assert_eq!(
            task_spec_slug(Some("MOD-0001"), "Some new adjustment"),
            "mod-0001-some-new-adjustment"
        );
        assert_eq!(task_spec_slug(None, "Just a title"), "just-a-title");
        assert_eq!(task_spec_slug(Some(""), "Title only"), "title-only");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn create_validates_and_seeds() {
        use crate::loop_registry::LoopRegistry;
        use modula_core::repositories::Repositories;

        let env = crate::testkit::env().await;
        let repos = Repositories::new(&env.pool);
        let paths = env.paths();
        let scheduler = SchedulerHandle::start(
            paths.modula.clone(),
            LoopRegistry::default(),
            String::new(),
            env.sink.clone(),
            repos.clone(),
        )
        .await
        .unwrap();
        let svc = WorkspaceService::new(
            env.pool.clone(),
            repos.workspaces.clone(),
            paths.clone(),
            scheduler,
        );

        // Name is required.
        assert!(svc.create("   ", None).await.is_err());

        // Blank description normalizes to NULL, not an empty string, and the
        // atomic seed produces both the DB row and the on-disk directory.
        let created = svc.create("Test WS", Some("  ")).await.unwrap();
        assert_eq!(created.name, "Test WS");
        let row = svc.get(&created.id).await.unwrap();
        assert_eq!(row.description, None);
        assert!(std::path::Path::new(&created.path).is_dir());
    }
}
