//! List git worktrees across configured projects whose branch matches a task.

use std::path::Path;
use std::process::Command;

use serde::Serialize;
use serde_json::{json, Value as JsonValue};

use modula_types::Project;

/// Current checked-out branch at `cwd`. `None` for detached HEAD or non-git paths.
pub fn current_branch(cwd: &Path) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .arg("branch")
        .arg("--show-current")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let s = String::from_utf8(output.stdout).ok()?.trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Path-based view of a git repo for the project-create/edit branch dropdown.
#[derive(Serialize)]
pub struct RepoBranches {
    pub is_git: bool,
    pub branches: Vec<String>,
    pub default_branch: Option<String>,
}

/// True if `path` is inside a git work tree.
fn is_git_repo(path: &Path) -> bool {
    let output = match Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("rev-parse")
        .arg("--is-inside-work-tree")
        .output()
    {
        Ok(o) if o.status.success() => o.stdout,
        _ => return false,
    };
    String::from_utf8_lossy(&output).trim() == "true"
}

/// Local branch names at `path`, sorted by refname (`for-each-ref` default).
fn local_branches(path: &Path) -> Vec<String> {
    let output = match Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("for-each-ref")
        .arg("--format=%(refname:short)")
        .arg("refs/heads/")
        .output()
    {
        Ok(o) if o.status.success() => o.stdout,
        _ => return Vec::new(),
    };
    String::from_utf8_lossy(&output)
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

/// Repo's default branch: `origin/HEAD` if set, else the current branch, else
/// the first of `main`/`master` that exists locally.
fn default_branch(path: &Path) -> Option<String> {
    let origin = Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("symbolic-ref")
        .arg("--short")
        .arg("refs/remotes/origin/HEAD")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().trim_start_matches("origin/").to_string())
        .filter(|s| !s.is_empty());
    if origin.is_some() {
        return origin;
    }
    if let Some(branch) = current_branch(path) {
        return Some(branch);
    }
    let locals = local_branches(path);
    ["main", "master"]
        .into_iter()
        .find(|b| locals.iter().any(|l| l == b))
        .map(str::to_string)
}

/// Clone `url` into `dest` via `git clone -- <url> <dest>`. The `--` separator
/// keeps a URL beginning with `-` from being read as a flag. On failure returns
/// git's trimmed stderr so the caller can surface the underlying message.
pub fn clone_repo(url: &str, dest: &Path) -> Result<(), String> {
    let output = Command::new("git")
        .arg("clone")
        .arg("--")
        .arg(url)
        .arg(dest)
        .output()
        .map_err(|e| format!("failed to run git: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

/// Probe `path` for the branch dropdown. Non-git paths yield an empty result.
pub fn repo_branches(path: &Path) -> RepoBranches {
    if !is_git_repo(path) {
        return RepoBranches {
            is_git: false,
            branches: Vec::new(),
            default_branch: None,
        };
    }
    RepoBranches {
        is_git: true,
        branches: local_branches(path),
        default_branch: default_branch(path),
    }
}

/// Parse `git worktree list --porcelain` output for one project.
/// Returns `(branch, worktree_path, head_sha)` for every worktree that has a
/// branch (bare worktrees and detached HEADs are skipped).
pub fn worktree_rows_for_project(path: &Path) -> Vec<(String, String, String)> {
    let output = match Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("worktree")
        .arg("list")
        .arg("--porcelain")
        .output()
    {
        Ok(o) if o.status.success() => o.stdout,
        _ => return Vec::new(),
    };
    // `git worktree list --porcelain` separates records with a blank line; on
    // Windows the stream carries `\r\n`, so normalize before splitting on `\n\n`.
    let text = String::from_utf8_lossy(&output).replace("\r\n", "\n");
    let mut rows = Vec::new();
    for block in text.split("\n\n") {
        let mut wt = String::new();
        let mut head = String::new();
        let mut branch = String::new();
        for line in block.lines() {
            if let Some(rest) = line.strip_prefix("worktree ") {
                wt = rest.to_string();
            } else if let Some(rest) = line.strip_prefix("HEAD ") {
                head = rest.to_string();
            } else if let Some(rest) = line.strip_prefix("branch refs/heads/") {
                branch = rest.to_string();
            }
        }
        if !branch.is_empty() {
            rows.push((branch, wt, head));
        }
    }
    rows
}

/// True if `branch` is associated with `task_id`.
///
/// Matches both the legacy flat shape (`fac-0006-...`) and the git-flow prefixed
/// shape (`bugfix/fac-0006-...`): the task id must appear either at the start
/// of the branch string or immediately after a `/`.
pub fn task_branch_match(branch: &str, task_id: &str) -> bool {
    let prefix = format!("{}-", task_id.to_lowercase());
    branch.starts_with(&prefix) || branch.contains(&format!("/{prefix}"))
}

/// Extract the variant position from a slug-named branch (`…-v<position>`).
/// `None` when the branch carries no `-v<n>` suffix (e.g. a task-scoped branch).
pub fn variant_position(branch: &str) -> Option<i64> {
    let (_, num) = branch.rsplit_once("-v")?;
    num.parse::<i64>().ok()
}

/// Count commits on `branch` that are not on `base_branch`. `None` when the
/// base ref isn't resolvable (e.g. fresh clone, base not fetched yet).
fn commits_ahead(repo: &Path, base_branch: &str, branch: &str) -> Option<i64> {
    let range = format!("{base_branch}..{branch}");
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("rev-list")
        .arg("--count")
        .arg(&range)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse::<i64>()
        .ok()
}

/// Assemble the worktree branches matching `task_slug` across the given
/// projects. Pure git inspection over already-loaded rows — the repo reads
/// (task → slug, project list) live in `ProjectService::branches_for_task`.
/// Branches are slug-named (`…/<task-slug>-v<position>`); match on the slug,
/// never the UUID.
pub fn branches_for_task(projects: &[Project], task_slug: &str) -> Vec<JsonValue> {
    let mut out: Vec<JsonValue> = Vec::new();
    for p in projects {
        let path = std::path::PathBuf::from(&p.path);
        if !path.is_dir() {
            continue;
        }
        for (branch, wt, head) in worktree_rows_for_project(&path) {
            if task_branch_match(&branch, task_slug) {
                let head_short = head.chars().take(12).collect::<String>();
                let commits = commits_ahead(&path, &p.base_branch, &branch);
                // Variant this branch belongs to, parsed here so consumers
                // don't need to know the branch-name format.
                let variant_pos = variant_position(&branch);
                out.push(json!({
                    // The lean `ProjectConfigEntry` shape the frontend `Branch`
                    // embeds — the domain `Project`'s on-disk enrichment
                    // (`exists`/`worktrees`) is not part of this contract.
                    "project": {
                        "id": p.id,
                        "name": p.name,
                        "path": p.path,
                        "base_branch": p.base_branch,
                    },
                    "branch": branch,
                    "variant_position": variant_pos,
                    "worktree": wt,
                    "head": if head_short.is_empty() { None } else { Some(head_short) },
                    "commits": commits,
                }));
            }
        }
    }
    out.sort_by(|a, b| {
        let pp = a["project"]["id"]
            .as_str()
            .unwrap_or("")
            .cmp(b["project"]["id"].as_str().unwrap_or(""));
        if pp != std::cmp::Ordering::Equal {
            return pp;
        }
        a["branch"]
            .as_str()
            .unwrap_or("")
            .cmp(b["branch"].as_str().unwrap_or(""))
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn git(dir: &Path, args: &[&str]) {
        let ok = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .expect("git runs")
            .status
            .success();
        assert!(ok, "git {args:?} failed");
    }

    fn temp_repo() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("modula-branches-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir temp repo");
        git(&dir, &["init", "-q", "-b", "main"]);
        git(&dir, &["config", "user.email", "t@t.t"]);
        git(&dir, &["config", "user.name", "t"]);
        git(&dir, &["commit", "-q", "--allow-empty", "-m", "init"]);
        git(&dir, &["branch", "feature-x"]);
        dir
    }

    #[test]
    fn repo_branches_lists_and_defaults() {
        let dir = temp_repo();
        let rb = repo_branches(&dir);
        assert!(rb.is_git);
        assert_eq!(
            rb.branches,
            vec!["feature-x".to_string(), "main".to_string()]
        );
        // No origin remote → falls back to current branch (`main`).
        assert_eq!(rb.default_branch.as_deref(), Some("main"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn clone_repo_clones_into_dest() {
        let base = std::env::temp_dir().join(format!("modula-clone-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let src = base.join("src");
        std::fs::create_dir_all(&src).expect("mkdir src");
        git(&src, &["init", "-q", "-b", "main"]);
        git(&src, &["config", "user.email", "t@t.t"]);
        git(&src, &["config", "user.name", "t"]);
        git(&src, &["commit", "-q", "--allow-empty", "-m", "init"]);

        let dest = base.join("dest");
        let url = format!("file://{}", src.display());
        clone_repo(&url, &dest).expect("clone succeeds");

        let rb = repo_branches(&dest);
        assert!(rb.is_git);
        assert!(rb.default_branch.is_some());
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn repo_branches_non_git() {
        let dir = std::env::temp_dir().join(format!("modula-nongit-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("mkdir");
        let rb = repo_branches(&dir);
        assert!(!rb.is_git);
        assert!(rb.branches.is_empty());
        assert_eq!(rb.default_branch, None);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn task_branch_match_prefixed() {
        // git-flow prefixed shape
        assert!(task_branch_match(
            "bugfix/fac-0006-restore-branch-listing-v1",
            "FAC-0006"
        ));
        assert!(task_branch_match(
            "feature/eng-1234-add-pagination-v1",
            "ENG-1234"
        ));
        assert!(task_branch_match(
            "chore/fac-0001-add-model-selection-v1",
            "FAC-0001"
        ));
    }

    #[test]
    fn task_branch_match_legacy() {
        // Legacy flat shape (no git-flow prefix)
        assert!(task_branch_match(
            "fac-0006-restore-branch-listing-v1",
            "FAC-0006"
        ));
        assert!(task_branch_match("eng-1234-add-pagination-v1", "ENG-1234"));
    }

    #[test]
    fn variant_position_parses_trailing_suffix() {
        assert_eq!(
            variant_position("bugfix/fac-0006-restore-branch-listing-v1"),
            Some(1)
        );
        assert_eq!(
            variant_position("fac-0001-add-model-selection-v12"),
            Some(12)
        );
        // No `-v<n>` suffix (task-scoped branch) → None.
        assert_eq!(variant_position("fac-0006-restore-branch-listing"), None);
        assert_eq!(variant_position("main"), None);
    }

    #[test]
    fn task_branch_match_rejects_unrelated() {
        assert!(!task_branch_match(
            "bugfix/fac-0007-something-else-v1",
            "FAC-0006"
        ));
        assert!(!task_branch_match("main", "FAC-0006"));
        assert!(!task_branch_match(
            "feature/fac-00061-look-alike-v1",
            "FAC-0006"
        ));
        // task id must be followed by '-', not just contained anywhere
        assert!(!task_branch_match("bugfix/other-fac-0006-v1", "FAC-0006"));
    }
}
