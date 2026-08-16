use modula_rpc::json::{json_to_struct, struct_to_json};
use modula_rpc::v1 as pb;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Integration {
    pub id: String,
    pub data: Value,
}

impl From<pb::Integration> for Integration {
    fn from(i: pb::Integration) -> Self {
        Self {
            id: i.id,
            data: i
                .data
                .map(struct_to_json)
                .unwrap_or_else(|| serde_json::json!({})),
        }
    }
}

impl From<Integration> for pb::Integration {
    fn from(i: Integration) -> Self {
        Self {
            id: i.id,
            data: json_to_struct(i.data),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExternalItem {
    pub key: String,
    pub title: String,
    pub description: String,
    pub url: String,
    pub state: String,
}

impl From<pb::ExternalItem> for ExternalItem {
    fn from(i: pb::ExternalItem) -> Self {
        Self {
            key: i.key,
            title: i.title,
            description: i.description,
            url: i.url,
            state: i.state,
        }
    }
}

impl From<ExternalItem> for pb::ExternalItem {
    fn from(i: ExternalItem) -> Self {
        Self {
            key: i.key,
            title: i.title,
            description: i.description,
            url: i.url,
            state: i.state,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn round_trip() {
        let d = Integration {
            id: "jira".into(),
            data: json!({"base_url": "https://x.atlassian.net"}),
        };
        assert_eq!(d, Integration::from(pb::Integration::from(d.clone())));
    }

    #[test]
    fn serde_matches_dto() {
        let d = Integration {
            id: "linear".into(),
            data: json!({"api_token": "t"}),
        };
        assert_eq!(
            serde_json::to_value(d).unwrap(),
            json!({"id": "linear", "data": {"api_token": "t"}})
        );
    }
}
