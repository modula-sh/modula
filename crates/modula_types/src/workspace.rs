use modula_rpc::v1 as pb;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub path: String,
    pub created_at: String,
}

impl From<pb::Workspace> for Workspace {
    fn from(w: pb::Workspace) -> Self {
        Self {
            id: w.id,
            name: w.name,
            slug: w.slug,
            description: w.description,
            path: w.path,
            created_at: w.created_at,
        }
    }
}

impl From<Workspace> for pb::Workspace {
    fn from(w: Workspace) -> Self {
        Self {
            id: w.id,
            name: w.name,
            slug: w.slug,
            description: w.description,
            created_at: w.created_at,
            path: w.path,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample() -> Workspace {
        Workspace {
            id: "w1".into(),
            name: "Modula".into(),
            slug: "modula".into(),
            description: None,
            path: "/ws".into(),
            created_at: "2026-01-01T00:00:00Z".into(),
        }
    }

    #[test]
    fn round_trip() {
        let d = sample();
        assert_eq!(d, Workspace::from(pb::Workspace::from(d.clone())));
    }

    // Locks the JSON the frontend consumes via `dto::workspace`.
    #[test]
    fn serde_matches_dto() {
        let want = json!({
            "id": "w1", "name": "Modula", "slug": "modula",
            "description": null, "path": "/ws", "created_at": "2026-01-01T00:00:00Z",
        });
        assert_eq!(serde_json::to_value(sample()).unwrap(), want);
    }
}
