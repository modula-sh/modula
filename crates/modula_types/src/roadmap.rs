use modula_rpc::v1 as pb;
use serde::{Deserialize, Serialize};

/// A roadmap row. The proto `position` orders rows but is not part of the
/// frontend `RoadmapItem` shape, so it is skipped in `serde` and kept only for
/// in-process ordering.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoadmapEntry {
    pub task: String,
    pub status: String,
    pub depends_on: Vec<String>,
    pub notes: String,
    #[serde(skip)]
    pub position: i64,
}

impl From<pb::RoadmapEntry> for RoadmapEntry {
    fn from(r: pb::RoadmapEntry) -> Self {
        Self {
            task: r.task_id,
            status: r.status,
            depends_on: r.depends_on,
            notes: r.notes,
            position: r.position,
        }
    }
}

impl From<RoadmapEntry> for pb::RoadmapEntry {
    fn from(r: RoadmapEntry) -> Self {
        Self {
            task_id: r.task,
            status: r.status,
            depends_on: r.depends_on,
            notes: r.notes,
            position: r.position,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample() -> RoadmapEntry {
        RoadmapEntry {
            task: "t1".into(),
            status: "in_progress".into(),
            depends_on: vec!["t0".into()],
            notes: "n".into(),
            position: 3,
        }
    }

    #[test]
    fn round_trip() {
        let d = sample();
        assert_eq!(d, RoadmapEntry::from(pb::RoadmapEntry::from(d.clone())));
    }

    // Locks the frontend `RoadmapItem` shape (snapshot `roadmap_json`).
    #[test]
    fn serde_matches_snapshot() {
        let want = json!({
            "task": "t1", "status": "in_progress",
            "depends_on": ["t0"], "notes": "n",
        });
        assert_eq!(serde_json::to_value(sample()).unwrap(), want);
    }
}
