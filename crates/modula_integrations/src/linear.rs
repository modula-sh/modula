//! Linear integration over its GraphQL API. Personal API keys go in the
//! `Authorization` header WITHOUT a `Bearer` prefix; GraphQL errors come back
//! as HTTP 200 with an `errors` array.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{is_issue_key, Error, ExternalItem, Integration, Result};

const GRAPHQL_URL: &str = "https://api.linear.app/graphql";
const ISSUE_SELECTION: &str = "identifier title description url state { name }";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinearConfig {
    pub api_token: String,
}

pub struct LinearIntegration {
    config: LinearConfig,
    client: reqwest::Client,
}

impl LinearIntegration {
    pub fn new(config: LinearConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    async fn graphql(&self, body: &Value) -> Result<Value> {
        let resp = self
            .client
            .post(GRAPHQL_URL)
            .header("Authorization", &self.config.api_token)
            .json(body)
            .send()
            .await
            .map_err(|e| Error::Http(e.to_string()))?;
        match resp.status() {
            s if s.is_success() => {}
            reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::BAD_REQUEST => {
                return Err(Error::AuthFailed("Linear rejected the API key".into()))
            }
            s => return Err(Error::Http(format!("Linear returned {s}"))),
        }
        let payload: Value = resp.json().await.map_err(|e| Error::Http(e.to_string()))?;
        if let Some(errors) = payload.get("errors").and_then(Value::as_array) {
            let messages: Vec<&str> = errors
                .iter()
                .filter_map(|e| e.get("message").and_then(Value::as_str))
                .collect();
            return Err(Error::Http(messages.join("; ")));
        }
        Ok(payload)
    }
}

fn search_body(term: &str) -> Value {
    json!({
        "query": format!(
            "query Search($term: String!) {{ searchIssues(term: $term, first: 20) {{ nodes {{ {ISSUE_SELECTION} }} }} }}"
        ),
        "variables": { "term": term },
    })
}

/// `searchIssues` needs a non-empty term; recents come from `issues`, where
/// `orderBy: updatedAt` returns the most recently updated first.
fn recent_body() -> Value {
    json!({
        "query": format!(
            "query Recent {{ issues(first: 10, orderBy: updatedAt) {{ nodes {{ {ISSUE_SELECTION} }} }} }}"
        ),
    })
}

fn fetch_body(id: &str) -> Value {
    json!({
        "query": format!("query Issue($id: String!) {{ issue(id: $id) {{ {ISSUE_SELECTION} }} }}"),
        "variables": { "id": id },
    })
}

#[derive(Deserialize)]
struct LinearIssue {
    identifier: String,
    title: String,
    #[serde(default)]
    description: Option<String>,
    url: String,
    #[serde(default)]
    state: Option<LinearState>,
}

#[derive(Deserialize)]
struct LinearState {
    name: String,
}

fn to_item(issue: LinearIssue) -> ExternalItem {
    ExternalItem {
        key: issue.identifier,
        title: issue.title,
        description: issue.description.unwrap_or_default(),
        url: issue.url,
        state: issue.state.map(|s| s.name).unwrap_or_default(),
    }
}

fn parse_issue(node: Value) -> Result<ExternalItem> {
    let issue: LinearIssue =
        serde_json::from_value(node).map_err(|e| Error::Http(e.to_string()))?;
    Ok(to_item(issue))
}

#[async_trait]
impl Integration for LinearIntegration {
    async fn health_check(&self) -> Result<()> {
        self.graphql(&json!({ "query": "query { viewer { id name } }" }))
            .await
            .map(|_| ())
    }

    async fn search(&self, query: &str) -> Result<Vec<ExternalItem>> {
        if is_issue_key(query) {
            if let Ok(item) = self.fetch(&query.to_uppercase()).await {
                return Ok(vec![item]);
            }
        }
        let (body, nodes_path) = if query.is_empty() {
            (recent_body(), "/data/issues/nodes")
        } else {
            (search_body(query), "/data/searchIssues/nodes")
        };
        let payload = self.graphql(&body).await?;
        let nodes = payload
            .pointer(nodes_path)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        nodes.into_iter().map(parse_issue).collect()
    }

    async fn fetch(&self, key: &str) -> Result<ExternalItem> {
        let payload = self.graphql(&fetch_body(key)).await?;
        let node = payload
            .pointer("/data/issue")
            .cloned()
            .ok_or_else(|| Error::Http(format!("Linear issue {key} not found")))?;
        parse_issue(node)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_body_shape() {
        let body = search_body("login");
        assert_eq!(body["variables"]["term"], "login");
        let query = body["query"].as_str().unwrap();
        assert!(query.contains("searchIssues(term: $term, first: 20)"));
        assert!(query.contains("identifier title description url state { name }"));
    }

    #[test]
    fn recent_body_shape() {
        let query = recent_body()["query"].as_str().unwrap().to_string();
        assert!(query.contains("issues(first: 10, orderBy: updatedAt)"));
        assert!(query.contains(ISSUE_SELECTION));
    }

    #[test]
    fn fetch_body_shape() {
        let body = fetch_body("ENG-123");
        assert_eq!(body["variables"]["id"], "ENG-123");
        assert!(body["query"].as_str().unwrap().contains("issue(id: $id)"));
    }

    #[test]
    fn parses_issue_fixture() {
        let node = json!({
            "identifier": "ENG-123",
            "title": "Fix login",
            "description": "markdown body",
            "url": "https://linear.app/acme/issue/ENG-123",
            "state": { "name": "In Progress" }
        });
        let item = parse_issue(node).unwrap();
        assert_eq!(item.key, "ENG-123");
        assert_eq!(item.state, "In Progress");
    }

    #[test]
    fn parses_issue_with_null_optionals() {
        let node = json!({
            "identifier": "ENG-1",
            "title": "t",
            "description": null,
            "url": "https://linear.app/acme/issue/ENG-1",
            "state": null
        });
        let item = parse_issue(node).unwrap();
        assert_eq!(item.description, "");
        assert_eq!(item.state, "");
    }
}
