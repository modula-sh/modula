use modula_rpc::v1 as pb;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Label {
    pub id: String,
    pub name: String,
}

impl From<pb::Label> for Label {
    fn from(l: pb::Label) -> Self {
        Self {
            id: l.id,
            name: l.name,
        }
    }
}

impl From<Label> for pb::Label {
    fn from(l: Label) -> Self {
        Self {
            id: l.id,
            name: l.name,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn round_trip() {
        let d = Label {
            id: "l1".into(),
            name: "refactor".into(),
        };
        assert_eq!(d, Label::from(pb::Label::from(d.clone())));
    }

    #[test]
    fn serde_matches_dto() {
        let d = Label {
            id: "l1".into(),
            name: "refactor".into(),
        };
        assert_eq!(
            serde_json::to_value(d).unwrap(),
            json!({"id": "l1", "name": "refactor"})
        );
    }
}
