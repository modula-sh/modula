use modula_rpc::v1 as pb;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: String,
    pub base_branch: String,
    pub exists: bool,
    pub worktrees: Vec<String>,
}

impl From<pb::Project> for Project {
    fn from(p: pb::Project) -> Self {
        Self {
            id: p.id,
            name: p.name,
            path: p.path,
            base_branch: p.base_branch,
            exists: p.exists,
            worktrees: p.worktrees,
        }
    }
}

impl From<Project> for pb::Project {
    fn from(p: Project) -> Self {
        Self {
            id: p.id,
            name: p.name,
            path: p.path,
            base_branch: p.base_branch,
            exists: p.exists,
            worktrees: p.worktrees,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommitSummary {
    pub sha: String,
    pub short: String,
    pub author: String,
    pub time: i64,
    pub subject: String,
}

impl From<pb::CommitSummary> for CommitSummary {
    fn from(c: pb::CommitSummary) -> Self {
        Self {
            sha: c.sha,
            short: c.short,
            author: c.author,
            time: c.time,
            subject: c.subject,
        }
    }
}

impl From<CommitSummary> for pb::CommitSummary {
    fn from(c: CommitSummary) -> Self {
        Self {
            sha: c.sha,
            short: c.short,
            author: c.author,
            time: c.time,
            subject: c.subject,
        }
    }
}

/// Branch listing for a repo path (`dto::repo_branches` / frontend `RepoBranches`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RepoBranchInfo {
    pub is_git: bool,
    pub branches: Vec<String>,
    pub default_branch: Option<String>,
}

impl From<pb::RepoBranchInfo> for RepoBranchInfo {
    fn from(i: pb::RepoBranchInfo) -> Self {
        Self {
            is_git: i.is_git,
            branches: i.branches,
            default_branch: i.default_branch,
        }
    }
}

impl From<RepoBranchInfo> for pb::RepoBranchInfo {
    fn from(i: RepoBranchInfo) -> Self {
        Self {
            is_git: i.is_git,
            branches: i.branches,
            default_branch: i.default_branch,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn project_round_trip_and_serde() {
        let d = Project {
            id: "pr1".into(),
            name: "Modula".into(),
            path: "/m".into(),
            base_branch: "main".into(),
            exists: true,
            worktrees: vec!["feature/x".into()],
        };
        assert_eq!(d, Project::from(pb::Project::from(d.clone())));
        let want = json!({
            "id": "pr1", "name": "Modula", "path": "/m", "base_branch": "main",
            "exists": true, "worktrees": ["feature/x"],
        });
        assert_eq!(serde_json::to_value(d).unwrap(), want);
    }

    #[test]
    fn commit_round_trip_and_serde() {
        let d = CommitSummary {
            sha: "abc".into(),
            short: "abc123".into(),
            author: "me".into(),
            time: 1700000000,
            subject: "fix".into(),
        };
        assert_eq!(d, CommitSummary::from(pb::CommitSummary::from(d.clone())));
        let want = json!({
            "sha": "abc", "short": "abc123", "author": "me", "time": 1700000000, "subject": "fix",
        });
        assert_eq!(serde_json::to_value(d).unwrap(), want);
    }

    #[test]
    fn repo_branches_round_trip_and_serde() {
        let d = RepoBranchInfo {
            is_git: true,
            branches: vec!["main".into()],
            default_branch: Some("main".into()),
        };
        assert_eq!(d, RepoBranchInfo::from(pb::RepoBranchInfo::from(d.clone())));
        let want = json!({"is_git": true, "branches": ["main"], "default_branch": "main"});
        assert_eq!(serde_json::to_value(d).unwrap(), want);
    }
}
