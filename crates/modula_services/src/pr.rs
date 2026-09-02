//! Resolve PR links for a variant's branches: per project the variant touches,
//! a GitHub "create PR" compare URL (offline, from the `origin` remote) and an
//! existing open-PR URL + number (via `gh`).

use std::path::Path;
use std::process::Command;

use serde_json::{json, Value as JsonValue};

use modula_db::projects::ProjectRepository;
use modula_db::tasks::TaskRepository;
use modula_db::variants::VariantRepository;
use modula_db::Database;

use super::branches as branches_svc;
use modula_core::error::{ApiError, ApiResult};

/// Canonical `https://github.com/<owner>/<repo>` for the repo's `origin` remote.
/// `None` when there is no origin or it isn't a GitHub remote.
pub fn remote_web_base(repo: &Path) -> Option<String> {
    let url = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["remote", "get-url", "origin"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())?;
    github_web_base(url.trim())
}

/// Parse an `origin` URL (SSH or HTTPS) into `https://github.com/owner/repo`.
/// Tolerates SSH host aliases (`git@github.com-work:o/r.git`) and HTTPS
/// userinfo by canonicalizing any `github.com*` host back to `github.com`.
fn github_web_base(remote: &str) -> Option<String> {
    let (host, path) = if let Some(rest) = remote.strip_prefix("git@") {
        rest.split_once(':')?
    } else if let Some(rest) = remote
        .strip_prefix("https://")
        .or_else(|| remote.strip_prefix("http://"))
    {
        rest.split_once('/')?
    } else {
        return None;
    };
    let host = host.rsplit('@').next().unwrap_or(host);
    if !host.starts_with("github.com") {
        return None;
    }
    let path = path.trim_end_matches('/').trim_end_matches(".git");
    let mut parts = path.split('/').filter(|s| !s.is_empty());
    let (owner, repo) = (parts.next()?, parts.next()?);
    Some(format!("https://github.com/{owner}/{repo}"))
}

/// GitHub prefilled compare/PR-creation URL. Branch slashes are left raw —
/// GitHub accepts them in `compare/`.
fn create_pr_url(web_base: &str, base_branch: &str, branch: &str) -> String {
    format!("{web_base}/compare/{base_branch}...{branch}?expand=1")
}

/// URL and number of the OPEN PR for `branch`, via `gh`. `None` on any failure
/// (`gh` absent, unauthenticated, or no open PR) — never an error.
fn existing_pr(repo: &Path, branch: &str) -> Option<(String, u64)> {
    let output = Command::new("gh")
        .current_dir(repo)
        .args(["pr", "view", branch, "--json", "state,url,number"])
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    parse_open_pr(&output.stdout)
}

/// Extract `(url, number)` from a `gh pr view --json state,url,number` payload,
/// `None` unless the PR is OPEN.
fn parse_open_pr(stdout: &[u8]) -> Option<(String, u64)> {
    let v: JsonValue = serde_json::from_slice(stdout).ok()?;
    if v.get("state")?.as_str()? != "OPEN" {
        return None;
    }
    let url = v.get("url")?.as_str()?.to_string();
    let number = v.get("number")?.as_u64()?;
    Some((url, number))
}

/// `{ projects: [{ name, create_url, pr_url, pr_number }] }` — one entry per
/// project that has a worktree branch for this variant. `create_url` is null
/// when the project's remote isn't GitHub; `pr_url`/`pr_number` are null
/// together when there is no open PR (or `gh` is unavailable). Empty `projects`
/// when the variant has no matching branch anywhere (e.g. direct mode).
/// Variant PR-link resolution across a workspace's projects. Owns the
/// repositories it reads; the `git`/`gh` helpers in this module are the agnostic
/// tools it drives.
#[derive(Clone)]
pub struct PrService {
    pool: Database,
    tasks: TaskRepository,
    variants: VariantRepository,
    projects: ProjectRepository,
}

impl PrService {
    pub fn new(
        pool: Database,
        tasks: TaskRepository,
        variants: VariantRepository,
        projects: ProjectRepository,
    ) -> Self {
        Self {
            pool,
            tasks,
            variants,
            projects,
        }
    }

    pub async fn variant_pr(
        &self,
        ws_id: &str,
        task_id: &str,
        variant_id: &str,
    ) -> ApiResult<JsonValue> {
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
        let task_slug = crate::workspaces::task_spec_slug(task.external_id.as_deref(), &task.title);

        let mut projects = Vec::new();
        for p in self.projects.list(&self.pool, ws_id).await? {
            let path = std::path::PathBuf::from(&p.path);
            if !path.is_dir() {
                continue;
            }
            let Some((branch, _, _)) = branches_svc::worktree_rows_for_project(&path)
                .into_iter()
                .find(|(branch, _, _)| {
                    branches_svc::task_branch_match(branch, &task_slug)
                        && branches_svc::variant_position(branch) == Some(variant.position)
                })
            else {
                continue;
            };
            let base = if p.base_branch.trim().is_empty() {
                "main"
            } else {
                &p.base_branch
            };
            let create_url =
                remote_web_base(&path).map(|web_base| create_pr_url(&web_base, base, &branch));
            let (pr_url, pr_number) = match existing_pr(&path, &branch) {
                Some((url, number)) => (Some(url), Some(number)),
                None => (None, None),
            };
            projects.push(json!({
                "name": p.name,
                "create_url": create_url,
                "pr_url": pr_url,
                "pr_number": pr_number,
            }));
        }
        Ok(json!({ "projects": projects }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_web_base_ssh() {
        assert_eq!(
            github_web_base("git@github.com:owner/repo.git").as_deref(),
            Some("https://github.com/owner/repo")
        );
    }

    #[test]
    fn github_web_base_ssh_host_alias() {
        assert_eq!(
            github_web_base("git@github.com-work:owner/repo.git").as_deref(),
            Some("https://github.com/owner/repo")
        );
    }

    #[test]
    fn github_web_base_https() {
        assert_eq!(
            github_web_base("https://github.com/owner/repo.git").as_deref(),
            Some("https://github.com/owner/repo")
        );
        assert_eq!(
            github_web_base("https://github.com/owner/repo").as_deref(),
            Some("https://github.com/owner/repo")
        );
    }

    #[test]
    fn github_web_base_https_userinfo() {
        assert_eq!(
            github_web_base("https://user@github.com/owner/repo.git").as_deref(),
            Some("https://github.com/owner/repo")
        );
    }

    #[test]
    fn github_web_base_non_github() {
        assert_eq!(github_web_base("git@gitlab.com:owner/repo.git"), None);
        assert_eq!(
            github_web_base("https://bitbucket.org/owner/repo.git"),
            None
        );
        assert_eq!(github_web_base("file:///tmp/repo"), None);
        // GitHub host but no repo segment.
        assert_eq!(github_web_base("git@github.com:owner"), None);
    }

    #[test]
    fn parse_open_pr_extracts_url_and_number() {
        let payload = br#"{"state":"OPEN","url":"https://github.com/o/r/pull/423","number":423}"#;
        assert_eq!(
            parse_open_pr(payload),
            Some(("https://github.com/o/r/pull/423".to_string(), 423))
        );
    }

    #[test]
    fn parse_open_pr_skips_non_open_state() {
        let merged = br#"{"state":"MERGED","url":"https://github.com/o/r/pull/1","number":1}"#;
        let closed = br#"{"state":"CLOSED","url":"https://github.com/o/r/pull/2","number":2}"#;
        assert_eq!(parse_open_pr(merged), None);
        assert_eq!(parse_open_pr(closed), None);
    }

    #[test]
    fn parse_open_pr_none_on_garbage() {
        assert_eq!(parse_open_pr(b"not json"), None);
        assert_eq!(parse_open_pr(b"{}"), None);
    }

    #[test]
    fn create_pr_url_keeps_branch_slashes() {
        assert_eq!(
            create_pr_url("https://github.com/o/r", "main", "feature/mod-014-x-v1"),
            "https://github.com/o/r/compare/main...feature/mod-014-x-v1?expand=1"
        );
    }
}
