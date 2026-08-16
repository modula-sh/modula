//! Jira Cloud integration over the v2 REST API (v2 so `description` is a plain
//! wiki-markup string, not v3 ADF). Search must use `/search/jql` — the old
//! `/search` endpoint was removed by Atlassian in Oct 2025 — and that endpoint
//! returns only `id` unless `fields` is explicit.

use async_trait::async_trait;
use base64::Engine as _;
use serde::{Deserialize, Serialize};

use crate::{is_issue_key, Error, ExternalItem, Integration, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JiraConfig {
    pub base_url: String,
    pub email: String,
    pub api_token: String,
}

pub struct JiraIntegration {
    config: JiraConfig,
    client: reqwest::Client,
}

const FIELDS: &str = "summary,description,status";

impl JiraIntegration {
    pub fn new(config: JiraConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    fn base(&self) -> &str {
        self.config.base_url.trim_end_matches('/')
    }

    async fn get(&self, url: &str, query: &[(&str, &str)]) -> Result<reqwest::Response> {
        let basic = base64::engine::general_purpose::STANDARD
            .encode(format!("{}:{}", self.config.email, self.config.api_token));
        let resp = self
            .client
            .get(url)
            .query(query)
            .header("Authorization", format!("Basic {basic}"))
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| Error::Http(e.to_string()))?;
        match resp.status() {
            s if s.is_success() => Ok(resp),
            reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN => {
                Err(Error::AuthFailed("Jira rejected the credentials".into()))
            }
            s => Err(Error::Http(format!("Jira returned {s} for {url}"))),
        }
    }

    async fn run_jql(&self, jql: &str, max_results: &str) -> Result<Vec<ExternalItem>> {
        let url = format!("{}/rest/api/2/search/jql", self.base());
        let resp = self
            .get(
                &url,
                &[
                    ("jql", jql),
                    ("fields", FIELDS),
                    ("maxResults", max_results),
                ],
            )
            .await?;
        let body: SearchResponse = resp.json().await.map_err(|e| Error::Http(e.to_string()))?;
        Ok(body.issues.into_iter().map(|i| self.item(i)).collect())
    }

    fn item(&self, issue: JiraIssue) -> ExternalItem {
        ExternalItem {
            url: format!("{}/browse/{}", self.base(), issue.key),
            key: issue.key,
            title: issue.fields.summary.unwrap_or_default(),
            description: issue.fields.description.unwrap_or_default(),
            state: issue.fields.status.map(|s| s.name).unwrap_or_default(),
        }
    }
}

/// `/search/jql` 400s on unbounded JQL (empty or order-by-only), so recents
/// need this harmless global bound.
const RECENT_JQL: &str = "project is not EMPTY ORDER BY created DESC";

fn escape(query: &str) -> String {
    query.replace('\\', "\\\\").replace('"', "\\\"")
}

/// `key = "X"` is exact and immune to tokenization, but 400s when the key
/// (or its project) doesn't exist — callers fall back to `text_jql`.
fn key_jql(query: &str) -> String {
    format!("key = \"{}\"", escape(query).to_uppercase())
}

fn text_jql(query: &str) -> String {
    if query.is_empty() {
        RECENT_JQL.to_string()
    } else {
        format!("text ~ \"{}*\"", escape(query))
    }
}

#[derive(Deserialize)]
struct SearchResponse {
    #[serde(default)]
    issues: Vec<JiraIssue>,
}

#[derive(Deserialize)]
struct JiraIssue {
    key: String,
    fields: JiraFields,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct JiraFields {
    summary: Option<String>,
    description: Option<String>,
    status: Option<JiraStatus>,
}

#[derive(Deserialize)]
struct JiraStatus {
    name: String,
}

#[async_trait]
impl Integration for JiraIntegration {
    async fn health_check(&self) -> Result<()> {
        self.get(&format!("{}/rest/api/2/myself", self.base()), &[])
            .await
            .map(|_| ())
    }

    async fn search(&self, query: &str) -> Result<Vec<ExternalItem>> {
        if is_issue_key(query) {
            if let Ok(items) = self.run_jql(&key_jql(query), "1").await {
                if !items.is_empty() {
                    return Ok(items);
                }
            }
        }
        let max = if query.is_empty() { "10" } else { "20" };
        self.run_jql(&text_jql(query), max).await
    }

    async fn fetch(&self, key: &str) -> Result<ExternalItem> {
        let url = format!("{}/rest/api/2/issue/{key}", self.base());
        let resp = self.get(&url, &[("fields", FIELDS)]).await?;
        let issue: JiraIssue = resp.json().await.map_err(|e| Error::Http(e.to_string()))?;
        Ok(self.item(issue))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_shaped_queries_use_exact_key_jql() {
        assert_eq!(key_jql("proj-123"), "key = \"PROJ-123\"");
        assert_eq!(key_jql("A1-9"), "key = \"A1-9\"");
    }

    #[test]
    fn free_text_queries_use_text_search() {
        assert_eq!(text_jql("login bug"), "text ~ \"login bug*\"");
        assert_eq!(text_jql("PROJ-"), "text ~ \"PROJ-*\"");
    }

    #[test]
    fn empty_query_uses_bounded_recents_jql() {
        assert_eq!(text_jql(""), "project is not EMPTY ORDER BY created DESC");
    }

    #[test]
    fn quotes_in_input_are_escaped() {
        assert_eq!(text_jql(r#"say "hi""#), r#"text ~ "say \"hi\"*""#);
    }

    #[test]
    fn deserializes_search_fixture() {
        let fixture = r#"{
            "issues": [
                {"key": "PROJ-1", "fields": {"summary": "Fix login",
                 "description": "plain text", "status": {"name": "In Progress"}}},
                {"key": "PROJ-2", "fields": {"summary": null, "description": null, "status": null}}
            ],
            "isLast": true
        }"#;
        let parsed: SearchResponse = serde_json::from_str(fixture).unwrap();
        assert_eq!(parsed.issues.len(), 2);
        assert_eq!(parsed.issues[0].key, "PROJ-1");
        assert_eq!(
            parsed.issues[0].fields.summary.as_deref(),
            Some("Fix login")
        );
        assert!(parsed.issues[1].fields.status.is_none());
    }
}
