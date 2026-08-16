use modula_rpc::v1 as pb;
use serde::{Deserialize, Serialize};

/// A wiki tree node (`dto::wiki_node`). The proto `is_dir` flag is rendered as
/// the `type` string (`"dir"` / `"file"`) the frontend switches on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WikiNode {
    pub name: String,
    pub path: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub children: Vec<WikiNode>,
}

impl From<pb::WikiTreeNode> for WikiNode {
    fn from(n: pb::WikiTreeNode) -> Self {
        Self {
            name: n.name,
            path: n.path,
            node_type: if n.is_dir { "dir" } else { "file" }.to_string(),
            children: n.children.into_iter().map(WikiNode::from).collect(),
        }
    }
}

impl From<WikiNode> for pb::WikiTreeNode {
    fn from(n: WikiNode) -> Self {
        Self {
            path: n.path,
            name: n.name,
            is_dir: n.node_type == "dir",
            children: n.children.into_iter().map(pb::WikiTreeNode::from).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn node() -> WikiNode {
        WikiNode {
            name: "Modula".into(),
            path: "Modula".into(),
            node_type: "dir".into(),
            children: vec![WikiNode {
                name: "index.md".into(),
                path: "Modula/index.md".into(),
                node_type: "file".into(),
                children: vec![],
            }],
        }
    }

    #[test]
    fn round_trip() {
        let d = node();
        assert_eq!(d, WikiNode::from(pb::WikiTreeNode::from(d.clone())));
    }

    #[test]
    fn serde_matches_dto() {
        let want = json!({
            "name": "Modula", "path": "Modula", "type": "dir",
            "children": [{"name": "index.md", "path": "Modula/index.md", "type": "file", "children": []}],
        });
        assert_eq!(serde_json::to_value(node()).unwrap(), want);
    }
}
