//! GitHub integration: shells the local `gh` CLI (sync `.output()`, matching
//! `services/pr.rs`); no HTTP client or token of its own.

use std::process::Command;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::{Error, ExternalItem, Integration, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubConfig {
    pub use_gh_cli: bool,
    /// `owner/repo` — `gh issue` needs `--repo` outside a checkout. Supplied
    /// per search/fetch request (import modal), not in the stored config.
    #[serde(default)]
    pub repo: String,
    /// gh account to run as; empty means the CLI's active account.
    #[serde(default)]
    pub account: String,
}

pub struct GithubIntegration {
    config: GithubConfig,
}

impl GithubIntegration {
    pub fn new(config: GithubConfig) -> Self {
        Self { config }
    }
}

fn exec(mut cmd: Command) -> Result<Vec<u8>> {
    let output = cmd.output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            Error::NotInstalled("gh".into())
        } else {
            Error::Http(e.to_string())
        }
    })?;
    if !output.status.success() {
        return Err(Error::Http(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }
    Ok(output.stdout)
}

fn token_args(account: &str) -> [&str; 4] {
    ["auth", "token", "--user", account]
}

/// The stored token for `account`, so a call can pin an account via
/// `GH_TOKEN` without mutating the CLI's active account (`gh auth switch`).
fn account_token(account: &str) -> Result<String> {
    let mut cmd = Command::new("gh");
    cmd.args(token_args(account));
    let stdout = exec(cmd).map_err(|e| match e {
        Error::NotInstalled(_) => e,
        Error::Http(msg) | Error::AuthFailed(msg) => Error::AuthFailed(msg),
        other => other,
    })?;
    Ok(String::from_utf8_lossy(&stdout).trim().to_string())
}

fn run_as(account: &str, args: &[&str]) -> Result<Vec<u8>> {
    let mut cmd = Command::new("gh");
    cmd.args(args);
    if !account.is_empty() {
        cmd.env("GH_TOKEN", account_token(account)?);
    }
    exec(cmd)
}

const ISSUE_FIELDS: &str = "number,title,state,body,url";

/// Without `--search` gh lists newest-created first, so an empty query
/// becomes the 10 most recent issues.
fn list_args<'a>(repo: &'a str, query: &'a str) -> Vec<&'a str> {
    let mut args = vec!["issue", "list", "--repo", repo, "--state", "all"];
    if query.is_empty() {
        args.extend(["--limit", "10"]);
    } else {
        args.extend(["--search", query, "--limit", "20"]);
    }
    args.extend(["--json", ISSUE_FIELDS]);
    args
}

/// `42` / `#42` — issue-number queries resolve exactly via `issue view`.
fn issue_number(query: &str) -> Option<&str> {
    let number = query.strip_prefix('#').unwrap_or(query);
    (!number.is_empty() && number.bytes().all(|b| b.is_ascii_digit())).then_some(number)
}

fn view_args<'a>(repo: &'a str, number: &'a str) -> Vec<&'a str> {
    vec![
        "issue",
        "view",
        number,
        "--repo",
        repo,
        "--json",
        ISSUE_FIELDS,
    ]
}

const REPO_LIST_ARGS: [&str; 6] = ["repo", "list", "--limit", "100", "--json", "nameWithOwner"];

#[derive(Deserialize)]
struct GhRepo {
    #[serde(rename = "nameWithOwner")]
    name_with_owner: String,
}

fn parse_repos(stdout: &[u8]) -> Result<Vec<String>> {
    let repos: Vec<GhRepo> =
        serde_json::from_slice(stdout).map_err(|e| Error::Http(e.to_string()))?;
    Ok(repos.into_iter().map(|r| r.name_with_owner).collect())
}

/// The configured account's repos (`owner/repo`), newest-pushed first.
/// Takes the stored `data` JSON so the account pin applies here too.
pub fn list_repos(data: &serde_json::Value) -> Result<Vec<String>> {
    let config: GithubConfig =
        serde_json::from_value(data.clone()).map_err(|e| Error::BadConfig(e.to_string()))?;
    parse_repos(&run_as(&config.account, &REPO_LIST_ARGS)?)
}

#[derive(Deserialize)]
struct GhIssue {
    number: u64,
    title: String,
    state: String,
    #[serde(default)]
    body: Option<String>,
    url: String,
}

fn to_item(issue: GhIssue, repo: &str) -> ExternalItem {
    ExternalItem {
        key: format!("{repo}#{}", issue.number),
        title: issue.title,
        description: issue.body.unwrap_or_default(),
        url: issue.url,
        state: issue.state,
    }
}

fn parse_issues(stdout: &[u8], repo: &str) -> Result<Vec<ExternalItem>> {
    let issues: Vec<GhIssue> =
        serde_json::from_slice(stdout).map_err(|e| Error::Http(e.to_string()))?;
    Ok(issues.into_iter().map(|i| to_item(i, repo)).collect())
}

fn parse_issue(stdout: &[u8], repo: &str) -> Result<ExternalItem> {
    let issue: GhIssue = serde_json::from_slice(stdout).map_err(|e| Error::Http(e.to_string()))?;
    Ok(to_item(issue, repo))
}

#[async_trait]
impl Integration for GithubIntegration {
    async fn health_check(&self) -> Result<()> {
        run_as(&self.config.account, &["auth", "status"])
            .map(|_| ())
            .map_err(|e| match e {
                Error::NotInstalled(_) | Error::AuthFailed(_) => e,
                _ => Error::AuthFailed("gh is not authenticated".into()),
            })
    }

    async fn search(&self, query: &str) -> Result<Vec<ExternalItem>> {
        let repo = &self.config.repo;
        if let Some(number) = issue_number(query) {
            if let Ok(stdout) = run_as(&self.config.account, &view_args(repo, number)) {
                return parse_issue(&stdout, repo).map(|item| vec![item]);
            }
        }
        let stdout = run_as(&self.config.account, &list_args(repo, query))?;
        parse_issues(&stdout, repo)
    }

    async fn fetch(&self, key: &str) -> Result<ExternalItem> {
        let number = key.rsplit('#').next().unwrap_or(key);
        let stdout = run_as(&self.config.account, &view_args(&self.config.repo, number))?;
        parse_issue(&stdout, &self.config.repo)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_args_shape() {
        let args = list_args("owner/repo", "login bug");
        assert_eq!(
            args,
            [
                "issue",
                "list",
                "--repo",
                "owner/repo",
                "--state",
                "all",
                "--search",
                "login bug",
                "--limit",
                "20",
                "--json",
                ISSUE_FIELDS
            ]
        );
    }

    #[test]
    fn empty_query_lists_recents() {
        let args = list_args("owner/repo", "");
        assert_eq!(
            args,
            [
                "issue",
                "list",
                "--repo",
                "owner/repo",
                "--state",
                "all",
                "--limit",
                "10",
                "--json",
                ISSUE_FIELDS
            ]
        );
    }

    #[test]
    fn issue_number_shapes() {
        assert_eq!(issue_number("42"), Some("42"));
        assert_eq!(issue_number("#42"), Some("42"));
        assert_eq!(issue_number(""), None);
        assert_eq!(issue_number("#"), None);
        assert_eq!(issue_number("4a"), None);
        assert_eq!(issue_number("login"), None);
    }

    #[test]
    fn view_args_shape() {
        let args = view_args("owner/repo", "42");
        assert_eq!(
            args,
            [
                "issue",
                "view",
                "42",
                "--repo",
                "owner/repo",
                "--json",
                ISSUE_FIELDS
            ]
        );
    }

    #[test]
    fn parses_issue_list_fixture() {
        let fixture = br#"[
            {"number": 42, "title": "Login broken", "state": "OPEN",
             "body": "Steps to reproduce", "url": "https://github.com/o/r/issues/42"},
            {"number": 7, "title": "No body", "state": "CLOSED", "body": null,
             "url": "https://github.com/o/r/issues/7"}
        ]"#;
        let items = parse_issues(fixture, "o/r").unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].key, "o/r#42");
        assert_eq!(items[0].title, "Login broken");
        assert_eq!(items[0].state, "OPEN");
        assert_eq!(items[1].description, "");
    }

    #[test]
    fn config_account_defaults_empty() {
        let config: GithubConfig =
            serde_json::from_value(serde_json::json!({"use_gh_cli": true})).unwrap();
        assert_eq!(config.account, "");
        let config: GithubConfig =
            serde_json::from_value(serde_json::json!({"use_gh_cli": true, "account": "octocat"}))
                .unwrap();
        assert_eq!(config.account, "octocat");
    }

    #[test]
    fn token_args_shape() {
        assert_eq!(
            token_args("octocat"),
            ["auth", "token", "--user", "octocat"]
        );
    }

    #[test]
    fn parses_repo_list_fixture() {
        let fixture = br#"[
            {"nameWithOwner": "acme/site"},
            {"nameWithOwner": "acme/api"}
        ]"#;
        assert_eq!(parse_repos(fixture).unwrap(), ["acme/site", "acme/api"]);
        assert!(parse_repos(b"[]").unwrap().is_empty());
    }

    #[test]
    fn parses_issue_view_fixture() {
        let fixture = br#"{"number": 42, "title": "Login broken", "state": "OPEN",
            "body": "x", "url": "https://github.com/o/r/issues/42"}"#;
        let item = parse_issue(fixture, "o/r").unwrap();
        assert_eq!(item.key, "o/r#42");
        assert_eq!(item.url, "https://github.com/o/r/issues/42");
    }
}
