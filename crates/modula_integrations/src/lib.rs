//! Clients for external services (GitHub via `gh` CLI, Jira, Linear), built
//! from the per-workspace config stored in the `integrations` table.

mod github;
mod jira;
mod linear;

pub use github::{list_repos, GithubConfig, GithubIntegration};
pub use jira::{JiraConfig, JiraIntegration};
pub use linear::{LinearConfig, LinearIntegration};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub const INTEGRATION_IDS: [&str; 3] = ["github", "jira", "linear"];

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0} is not installed")]
    NotInstalled(String),
    #[error("authentication failed: {0}")]
    AuthFailed(String),
    #[error("{0}")]
    Http(String),
    #[error("invalid integration config: {0}")]
    BadConfig(String),
    #[error("unknown integration id: {0}")]
    UnknownId(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// An item in an external system. Deliberately not a task shape — `key` is the
/// human identifier there (`PROJ-123`, `owner/repo#42`, `ENG-123`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalItem {
    pub key: String,
    pub title: String,
    pub description: String,
    pub url: String,
    pub state: String,
}

#[async_trait]
pub trait Integration: Send + Sync {
    async fn health_check(&self) -> Result<()>;
    /// An empty query returns the ~10 most recent items.
    async fn search(&self, query: &str) -> Result<Vec<ExternalItem>>;
    async fn fetch(&self, key: &str) -> Result<ExternalItem>;
}

/// `PROJ-123` shape (`^[A-Za-z][A-Za-z0-9]*-\d+$`) — the exact-lookup branch
/// in Jira and Linear search.
pub(crate) fn is_issue_key(query: &str) -> bool {
    let Some((project, number)) = query.split_once('-') else {
        return false;
    };
    let mut chars = project.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric())
        && !number.is_empty()
        && number.bytes().all(|b| b.is_ascii_digit())
}

/// Build the integration for `id` from its stored `data` JSON.
pub fn from_config(id: &str, data: &serde_json::Value) -> Result<Box<dyn Integration>> {
    fn parse<T: serde::de::DeserializeOwned>(data: &serde_json::Value) -> Result<T> {
        serde_json::from_value(data.clone()).map_err(|e| Error::BadConfig(e.to_string()))
    }
    match id {
        "github" => Ok(Box::new(GithubIntegration::new(parse(data)?))),
        "jira" => Ok(Box::new(JiraIntegration::new(parse(data)?))),
        "linear" => Ok(Box::new(LinearIntegration::new(parse(data)?))),
        other => Err(Error::UnknownId(other.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn from_config_dispatches_on_id() {
        assert!(from_config("github", &json!({"use_gh_cli": true, "repo": "o/r"})).is_ok());
        assert!(from_config(
            "jira",
            &json!({"base_url": "https://x.atlassian.net", "email": "a@b.c", "api_token": "t"})
        )
        .is_ok());
        assert!(from_config("linear", &json!({"api_token": "lin_api_x"})).is_ok());
        assert!(matches!(
            from_config("s3", &json!({})),
            Err(Error::UnknownId(_))
        ));
        assert!(matches!(
            from_config("jira", &json!({})),
            Err(Error::BadConfig(_))
        ));
    }

    #[test]
    fn issue_key_shape() {
        assert!(is_issue_key("proj-123"));
        assert!(is_issue_key("A1-9"));
        assert!(!is_issue_key(""));
        assert!(!is_issue_key("PROJ-"));
        assert!(!is_issue_key("PROJ-12a"));
        assert!(!is_issue_key("1AB-12"));
        assert!(!is_issue_key("ABC-DEF-123"));
        assert!(!is_issue_key("login bug"));
    }

    #[test]
    fn from_config_knows_every_declared_id() {
        for id in INTEGRATION_IDS {
            assert!(!matches!(
                from_config(id, &json!({})),
                Err(Error::UnknownId(_))
            ));
        }
    }
}
