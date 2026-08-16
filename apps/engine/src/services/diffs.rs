//! Git diff orchestration — variant index + per-variant detail.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value as JsonValue};

use modula_db::projects::ProjectRepository;
use modula_db::tasks::TaskRepository;
use modula_db::variants::VariantRepository;
use modula_db::Database;

use super::branches as branches_svc;
use super::workspaces::WorkspaceService;
use crate::core::error::{ApiError, ApiResult};

fn git(args: &[&str], timeout_secs: u64) -> Option<String> {
    let _ = timeout_secs;
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

/// Parse `git diff --numstat`-style output (one row per file, tab-separated)
/// into our standard `{files: [...], totals: {...}}` shape. Binary diffs show
/// `-` for additions/deletions and are counted as zero LOC.
fn parse_numstat_output(out: &str) -> JsonValue {
    let mut files: Vec<JsonValue> = Vec::new();
    let (mut adds, mut dels) = (0u64, 0u64);
    for line in out.lines() {
        let mut cols = line.splitn(3, '\t');
        let (Some(a), Some(d), Some(path)) = (cols.next(), cols.next(), cols.next()) else {
            continue;
        };
        let additions: u64 = a.parse().unwrap_or(0);
        let deletions: u64 = d.parse().unwrap_or(0);
        adds += additions;
        dels += deletions;
        files.push(json!({ "path": path, "additions": additions, "deletions": deletions }));
    }
    json!({
        "files": files,
        "totals": { "files": files.len(), "additions": adds, "deletions": dels },
    })
}

fn numstat_diff(cwd: &Path, extra: &[&str]) -> JsonValue {
    let cwd_str = cwd.to_str().unwrap_or("");
    let mut args = vec!["-C", cwd_str, "diff", "--numstat"];
    args.extend_from_slice(extra);
    parse_numstat_output(&git(&args, 30).unwrap_or_default())
}

fn raw_diff(cwd: &Path, extra: &[&str]) -> String {
    let cwd_str = cwd.to_str().unwrap_or("");
    let mut args = vec!["-C", cwd_str, "diff"];
    args.extend_from_slice(extra);
    git(&args, 30).unwrap_or_default()
}

fn count_patch_lines(diff: &str) -> (u64, u64) {
    let (mut adds, mut dels) = (0u64, 0u64);
    for line in diff.lines() {
        if line.starts_with("+++") || line.starts_with("---") {
            continue;
        }
        match line.as_bytes().first() {
            Some(b'+') => adds += 1,
            Some(b'-') => dels += 1,
            _ => {}
        }
    }
    (adds, dels)
}

fn patches_json(diff_text: &str) -> Vec<JsonValue> {
    split_patches(diff_text)
        .into_iter()
        .map(|(path, diff)| {
            let (additions, deletions) = count_patch_lines(&diff);
            json!({ "path": path, "diff": diff, "additions": additions, "deletions": deletions })
        })
        .collect()
}

fn untracked_files(cwd: &Path) -> Vec<String> {
    let cwd_str = cwd.to_str().unwrap_or("");
    git(
        &["-C", cwd_str, "ls-files", "--others", "--exclude-standard"],
        30,
    )
    .unwrap_or_default()
    .lines()
    .filter(|l| !l.is_empty())
    .map(|l| l.to_string())
    .collect()
}

fn untracked_numstat(cwd: &Path) -> JsonValue {
    let files: Vec<JsonValue> = untracked_files(cwd)
        .into_iter()
        .map(|path| {
            let additions = std::fs::read_to_string(cwd.join(&path))
                .map(|s| s.lines().count() as u64)
                .unwrap_or(0);
            json!({ "path": path, "additions": additions, "deletions": 0 })
        })
        .collect();
    let additions: u64 = files.iter().filter_map(|f| f["additions"].as_u64()).sum();
    json!({
        "files": files.clone(),
        "totals": { "files": files.len(), "additions": additions, "deletions": 0 },
    })
}

fn untracked_patches(cwd: &Path) -> Vec<JsonValue> {
    let cwd_str = cwd.to_str().unwrap_or("");
    untracked_files(cwd)
        .into_iter()
        .map(|path| {
            // `git diff --no-index` exits 1 when there's any diff; bypass the
            // exit-code check by invoking Command directly.
            let diff = std::process::Command::new("git")
                .args([
                    "-C",
                    cwd_str,
                    "diff",
                    "--no-index",
                    "--",
                    crate::platform::NULL_DEVICE,
                    &path,
                ])
                .output()
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .unwrap_or_default();
            let (additions, deletions) = count_patch_lines(&diff);
            json!({
                "path": path, "diff": diff,
                "additions": additions, "deletions": deletions,
            })
        })
        .collect()
}

/// Run `git add -- <files>` at `cwd`. Paths are relative to cwd.
pub fn stage_files(cwd: &Path, files: &[String]) -> ApiResult<()> {
    if files.is_empty() {
        return Ok(());
    }
    let cwd_str = cwd.to_str().unwrap_or("");
    let mut args: Vec<&str> = vec!["-C", cwd_str, "add", "--"];
    args.extend(files.iter().map(String::as_str));
    let out = Command::new("git")
        .args(&args)
        .output()
        .map_err(|e| ApiError::Internal(format!("git add: {e}")))?;
    if !out.status.success() {
        return Err(ApiError::Internal(
            String::from_utf8_lossy(&out.stderr).trim().to_string(),
        ));
    }
    Ok(())
}

/// Run `git restore --staged -- <files>` at `cwd`.
pub fn unstage_files(cwd: &Path, files: &[String]) -> ApiResult<()> {
    if files.is_empty() {
        return Ok(());
    }
    let cwd_str = cwd.to_str().unwrap_or("");
    let mut args: Vec<&str> = vec!["-C", cwd_str, "restore", "--staged", "--"];
    args.extend(files.iter().map(String::as_str));
    let out = Command::new("git")
        .args(&args)
        .output()
        .map_err(|e| ApiError::Internal(format!("git restore: {e}")))?;
    if !out.status.success() {
        return Err(ApiError::Internal(
            String::from_utf8_lossy(&out.stderr).trim().to_string(),
        ));
    }
    Ok(())
}

/// Working-tree state at `cwd`: staged (index vs HEAD), unstaged (working tree
/// vs index), and untracked (new files not yet added). Any side may be empty.
pub fn working_diff(cwd: &Path) -> JsonValue {
    json!({
        "staged": numstat_diff(cwd, &["--cached"]),
        "unstaged": numstat_diff(cwd, &[]),
        "untracked": untracked_numstat(cwd),
        "branch": branches_svc::current_branch(cwd),
    })
}

/// Full diff text per file for the working tree at `cwd`. Same shape as
/// `working_diff`, with raw patch bodies.
pub fn working_diff_text(cwd: &Path) -> JsonValue {
    json!({
        "staged": patches_json(&raw_diff(cwd, &["--cached"])),
        "unstaged": patches_json(&raw_diff(cwd, &[])),
        "untracked": untracked_patches(cwd),
        "branch": branches_svc::current_branch(cwd),
    })
}

/// Numstat for a single commit, in the same shape as `working_diff`'s subobjects.
pub fn commit_diff(cwd: &Path, sha: &str) -> JsonValue {
    let cwd_str = cwd.to_str().unwrap_or("");
    let out = git(
        &["-C", cwd_str, "show", "--numstat", "--format=format:", sha],
        30,
    )
    .unwrap_or_default();
    parse_numstat_output(&out)
}

/// `git log --pretty=… -n <limit> [<since>..]<ref>` from `cwd`. One JSON
/// object per commit; no fields are normalized beyond the columns git itself
/// emits.
pub fn commits_log(cwd: &Path, branch: Option<&str>, since: Option<&str>, limit: u32) -> JsonValue {
    let cwd_str = cwd.to_str().unwrap_or("");
    let limit_str = limit.to_string();
    // %H sha · %h short · %an author · %at unix-ts · %s subject — tab-separated
    // so the parser is just `splitn(5, '\t')`.
    let fmt = "--pretty=format:%H%x09%h%x09%an%x09%at%x09%s";
    let range = match (since, branch) {
        (Some(s), Some(b)) => format!("{s}..{b}"),
        (None, Some(b)) => b.to_string(),
        _ => "HEAD".to_string(),
    };
    let args = vec!["-C", cwd_str, "log", fmt, "-n", &limit_str, &range];
    let out = git(&args, 30).unwrap_or_default();
    let commits: Vec<JsonValue> = out
        .lines()
        .filter_map(|line| {
            let mut cols = line.splitn(5, '\t');
            Some(json!({
                "sha": cols.next()?,
                "short": cols.next()?,
                "author": cols.next()?,
                "time": cols.next()?.parse::<i64>().ok()?,
                "subject": cols.next()?,
            }))
        })
        .collect();
    json!({ "commits": commits })
}

/// Resolve a working directory for `(project, branch?)`. If `branch` names an
/// existing worktree for the project, returns the worktree path; otherwise the
/// project's main checkout. Returns `None` if the project path doesn't exist.
pub fn project_cwd(project_path: &Path, branch: Option<&str>) -> Option<PathBuf> {
    if !project_path.is_dir() {
        return None;
    }
    if let Some(b) = branch {
        for (wb, wt, _) in branches_svc::worktree_rows_for_project(project_path) {
            if wb == b {
                let p = PathBuf::from(wt);
                if p.is_dir() {
                    return Some(p);
                }
            }
        }
    }
    Some(project_path.to_path_buf())
}

fn shortstat(stat_out: &str) -> (u32, u32, u32) {
    let line = stat_out.trim().lines().last().unwrap_or("");
    let mut counts = (0u32, 0u32, 0u32);
    for token in line.split(',') {
        let t = token.trim();
        let first = t.split_whitespace().next().unwrap_or("");
        let n: u32 = first.parse().unwrap_or(0);
        if t.contains("file") {
            counts.0 = n;
        } else if t.contains("insertion") {
            counts.1 = n;
        } else if t.contains("deletion") {
            counts.2 = n;
        }
    }
    counts
}

fn diff_range(mode: &str, _branch: &str, task_slug: &str, position: i64, base: &str) -> String {
    // Working tree vs base — includes both committed and uncommitted changes.
    // The two-dot form (`base..branch`) would miss uncommitted edits.
    if mode == "worktree" {
        base.to_string()
    } else {
        format!("modula/{task_slug}-v{position}/start..{base}")
    }
}

/// Find the worktree path and full branch name for a variant in a project.
/// Branches are slug-named (`…/<task-slug>-v<position>`); no UUIDs.
fn find_variant_worktree(
    project_path: &Path,
    task_slug: &str,
    position: i64,
) -> Option<(String, PathBuf)> {
    let suffix = format!("-v{position}");
    branches_svc::worktree_rows_for_project(project_path)
        .into_iter()
        .find(|(branch, wt, _head)| {
            branches_svc::task_branch_match(branch, task_slug)
                && branch.ends_with(&suffix)
                && PathBuf::from(wt).is_dir()
        })
        .map(|(branch, wt, _)| (branch, PathBuf::from(wt)))
}

/// Variant diff aggregation across a workspace's projects. Owns the repositories
/// it reads and DIs [`WorkspaceService`] to validate the workspace before diffing.
/// The git-plumbing free functions in this module are the agnostic tools it drives.
#[derive(Clone)]
pub struct DiffService {
    pool: Database,
    tasks: TaskRepository,
    variants: VariantRepository,
    projects: ProjectRepository,
    workspaces: WorkspaceService,
}

impl DiffService {
    pub fn new(
        pool: Database,
        tasks: TaskRepository,
        variants: VariantRepository,
        projects: ProjectRepository,
        workspaces: WorkspaceService,
    ) -> Self {
        Self {
            pool,
            tasks,
            variants,
            projects,
            workspaces,
        }
    }

    async fn project_paths(&self, ws_id: &str) -> Vec<(String, PathBuf, String)> {
        self.projects
            .list(&self.pool, ws_id)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|p| {
                let base = if p.base_branch.trim().is_empty() {
                    "main".into()
                } else {
                    p.base_branch
                };
                (p.name, PathBuf::from(p.path), base)
            })
            .collect()
    }

    pub async fn variant_diffs(
        &self,
        ws_id: &str,
        task_id: &str,
        variant_id: &str,
    ) -> ApiResult<JsonValue> {
        // Existence check (404 on an unknown workspace) before any git work.
        self.workspaces.workspace_dir(ws_id).await?;
        let task = self.tasks.get(&self.pool, ws_id, task_id).await?;
        let variants = self
            .variants
            .list_for_task(&self.pool, ws_id, task_id)
            .await?;
        let variant = variants
            .iter()
            .find(|v| v.id == variant_id)
            .ok_or_else(|| {
                ApiError::NotFound(format!("unknown variant {variant_id} on {task_id}"))
            })?;
        let worktree_mode = task.worktree != Some(false);
        let task_slug =
            crate::services::workspaces::task_spec_slug(task.external_id.as_deref(), &task.title);
        let position = variant.position;

        let mut projects_out: Vec<JsonValue> = Vec::new();
        for (name, path, base) in self.project_paths(ws_id).await {
            if !path.is_dir() {
                continue;
            }
            let (branch, cwd) = if worktree_mode {
                let Some((branch, wt)) = find_variant_worktree(&path, &task_slug, position) else {
                    continue;
                };
                (branch, wt)
            } else {
                let tag = format!("modula/{task_slug}-v{position}/start");
                if git(
                    &[
                        "-C",
                        path.to_str().unwrap_or(""),
                        "rev-parse",
                        "--verify",
                        &format!("{tag}^{{commit}}"),
                    ],
                    5,
                )
                .is_none()
                {
                    continue;
                }
                (base.clone(), path.clone())
            };
            let rng = diff_range(
                if worktree_mode { "worktree" } else { "direct" },
                &branch,
                &task_slug,
                position,
                &base,
            );
            let stat = git(
                &[
                    "-C",
                    cwd.to_str().unwrap_or(""),
                    "diff",
                    "--shortstat",
                    &rng,
                ],
                15,
            )
            .unwrap_or_default();
            let num = git(
                &["-C", cwd.to_str().unwrap_or(""), "diff", "--numstat", &rng],
                15,
            )
            .unwrap_or_default();
            let diff =
                git(&["-C", cwd.to_str().unwrap_or(""), "diff", &rng], 30).unwrap_or_default();
            let (files, ins, del) = shortstat(&stat);
            let per_file = parse_numstat(&num);
            let patches = split_patches(&diff)
                .into_iter()
                .map(|(path, body)| {
                    let counts = per_file.get(&path).cloned().unwrap_or((0u32, 0u32));
                    json!({
                        "path": path,
                        "diff": body,
                        "additions": counts.0,
                        "deletions": counts.1,
                    })
                })
                .collect::<Vec<_>>();
            projects_out.push(json!({
                "name": name,
                "branch": branch,
                "base_branch": base,
                "range": rng,
                "files": files,
                "insertions": ins,
                "deletions": del,
                "patches": patches,
            }));
        }
        if projects_out.is_empty() {
            return Err(ApiError::NotFound(format!(
                "no diffs found for {task_id}/{variant_id}"
            )));
        }
        Ok(json!({
            "task": task_id,
            "task_title": task.title,
            "variant": variant_id,
            "variant_status": variant.status,
            "mode": if worktree_mode { "worktree" } else { "direct" },
            "projects": projects_out,
        }))
    }
}

fn parse_numstat(text: &str) -> std::collections::BTreeMap<String, (u32, u32)> {
    let mut out = std::collections::BTreeMap::new();
    for line in text.lines() {
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 3 {
            continue;
        }
        let adds = if cols[0] == "-" {
            0
        } else {
            cols[0].parse().unwrap_or(0)
        };
        let dels = if cols[1] == "-" {
            0
        } else {
            cols[1].parse().unwrap_or(0)
        };
        out.insert(cols[2].to_string(), (adds, dels));
    }
    out
}

fn split_patches(diff: &str) -> Vec<(String, String)> {
    if diff.trim().is_empty() {
        return Vec::new();
    }
    let mut blocks: Vec<String> = Vec::new();
    let mut current = String::new();
    for line in diff.lines() {
        if line.starts_with("diff --git ") && !current.is_empty() {
            blocks.push(std::mem::take(&mut current));
        }
        current.push_str(line);
        current.push('\n');
    }
    if !current.is_empty() {
        blocks.push(current);
    }
    blocks
        .into_iter()
        .map(|block| {
            let first = block.lines().next().unwrap_or("");
            let path = first
                .strip_prefix("diff --git a/")
                .and_then(|rest| rest.split_once(" b/"))
                .map(|(_old, new)| new.to_string())
                .unwrap_or_else(|| first.to_string());
            (path, block)
        })
        .collect()
}
