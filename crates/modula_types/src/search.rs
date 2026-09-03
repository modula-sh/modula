use std::str::FromStr;

use modula_rpc::v1 as pb;
use serde::{Deserialize, Serialize};

/// Carried on the wire as the lowercase string [`SearchKind::as_str`] returns,
/// matching `task.source` / `provider.type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SearchKind {
    Task,
    Agent,
    Project,
    Provider,
    Wiki,
    Conversation,
}

impl SearchKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Task => "task",
            Self::Agent => "agent",
            Self::Project => "project",
            Self::Provider => "provider",
            Self::Wiki => "wiki",
            Self::Conversation => "conversation",
        }
    }
}

impl FromStr for SearchKind {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "task" => Ok(Self::Task),
            "agent" => Ok(Self::Agent),
            "project" => Ok(Self::Project),
            "provider" => Ok(Self::Provider),
            "wiki" => Ok(Self::Wiki),
            "conversation" => Ok(Self::Conversation),
            _ => Err(()),
        }
    }
}

/// Spans rather than offsets, so no client has to agree with the engine on byte
/// vs. char vs. UTF-16 indexing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExcerptSpan {
    pub text: String,
    pub is_match: bool,
}

/// `field` names the content type that matched; `excerpt` is empty when the
/// title itself matched.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchHit {
    pub kind: String,
    pub id: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub field: String,
    pub excerpt: Vec<ExcerptSpan>,
}

impl From<pb::ExcerptSpan> for ExcerptSpan {
    fn from(s: pb::ExcerptSpan) -> Self {
        Self {
            text: s.text,
            is_match: s.is_match,
        }
    }
}

impl From<ExcerptSpan> for pb::ExcerptSpan {
    fn from(s: ExcerptSpan) -> Self {
        Self {
            text: s.text,
            is_match: s.is_match,
        }
    }
}

impl From<pb::SearchHit> for SearchHit {
    fn from(h: pb::SearchHit) -> Self {
        Self {
            kind: h.kind,
            id: h.id,
            title: h.title,
            subtitle: h.subtitle,
            field: h.field,
            excerpt: h.excerpt.into_iter().map(ExcerptSpan::from).collect(),
        }
    }
}

impl From<SearchHit> for pb::SearchHit {
    fn from(h: SearchHit) -> Self {
        Self {
            kind: h.kind,
            id: h.id,
            title: h.title,
            subtitle: h.subtitle,
            field: h.field,
            excerpt: h.excerpt.into_iter().map(pb::ExcerptSpan::from).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn hit() -> SearchHit {
        SearchHit {
            kind: SearchKind::Task.as_str().into(),
            id: "t1".into(),
            title: "Implement search".into(),
            subtitle: Some("MOD-033".into()),
            field: "description".into(),
            excerpt: vec![
                ExcerptSpan {
                    text: "add ".into(),
                    is_match: false,
                },
                ExcerptSpan {
                    text: "search".into(),
                    is_match: true,
                },
            ],
        }
    }

    #[test]
    fn round_trip() {
        let d = hit();
        assert_eq!(d, SearchHit::from(pb::SearchHit::from(d.clone())));
    }

    #[test]
    fn kind_round_trips_through_its_key() {
        for kind in [
            SearchKind::Task,
            SearchKind::Agent,
            SearchKind::Project,
            SearchKind::Provider,
            SearchKind::Wiki,
            SearchKind::Conversation,
        ] {
            assert_eq!(Ok(kind), kind.as_str().parse());
        }
        assert_eq!(Err(()), "nope".parse::<SearchKind>());
    }

    #[test]
    fn serde_matches_dto() {
        let want = json!({
            "kind": "task",
            "id": "t1",
            "title": "Implement search",
            "subtitle": "MOD-033",
            "field": "description",
            "excerpt": [
                {"text": "add ", "is_match": false},
                {"text": "search", "is_match": true},
            ],
        });
        assert_eq!(serde_json::to_value(hit()).unwrap(), want);
    }
}
