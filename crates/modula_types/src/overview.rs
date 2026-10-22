use modula_rpc::v1 as pb;
use serde::{Deserialize, Serialize};

use crate::{PipelineStatus, PlanProgress, PlanTask};

/// One workspace's active tasks. The pipeline rides along so clients derive
/// stations, labels and tones without a per-workspace config read.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OverviewWorkspace {
    pub id: String,
    pub name: String,
    pub pipeline: Vec<PipelineStatus>,
    pub tasks: Vec<OverviewTask>,
}

/// `status` is the roadmap status key.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OverviewTask {
    pub id: String,
    pub external_id: Option<String>,
    pub title: String,
    pub status: String,
    pub notes: String,
    pub needs_human: bool,
    pub agents: Vec<OverviewAgent>,
    pub variants: Vec<OverviewVariant>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OverviewAgent {
    pub name: String,
    pub provider_type: Option<String>,
    pub provider_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OverviewVariant {
    pub id: String,
    pub position: i64,
    pub status: Option<String>,
    pub progress: Option<PlanProgress>,
    pub recent_tasks: Vec<PlanTask>,
}

impl From<pb::OverviewWorkspace> for OverviewWorkspace {
    fn from(w: pb::OverviewWorkspace) -> Self {
        Self {
            id: w.id,
            name: w.name,
            pipeline: w.pipeline.into_iter().map(Into::into).collect(),
            tasks: w.tasks.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<OverviewWorkspace> for pb::OverviewWorkspace {
    fn from(w: OverviewWorkspace) -> Self {
        Self {
            id: w.id,
            name: w.name,
            pipeline: w.pipeline.into_iter().map(Into::into).collect(),
            tasks: w.tasks.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<pb::OverviewTask> for OverviewTask {
    fn from(t: pb::OverviewTask) -> Self {
        Self {
            id: t.id,
            external_id: t.external_id,
            title: t.title,
            status: t.status,
            notes: t.notes,
            needs_human: t.needs_human,
            agents: t.agents.into_iter().map(Into::into).collect(),
            variants: t.variants.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<OverviewTask> for pb::OverviewTask {
    fn from(t: OverviewTask) -> Self {
        Self {
            id: t.id,
            external_id: t.external_id,
            title: t.title,
            status: t.status,
            notes: t.notes,
            needs_human: t.needs_human,
            agents: t.agents.into_iter().map(Into::into).collect(),
            variants: t.variants.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<pb::OverviewAgent> for OverviewAgent {
    fn from(a: pb::OverviewAgent) -> Self {
        Self {
            name: a.name,
            provider_type: a.provider_type,
            provider_name: a.provider_name,
        }
    }
}

impl From<OverviewAgent> for pb::OverviewAgent {
    fn from(a: OverviewAgent) -> Self {
        Self {
            name: a.name,
            provider_type: a.provider_type,
            provider_name: a.provider_name,
        }
    }
}

impl From<pb::OverviewVariant> for OverviewVariant {
    fn from(v: pb::OverviewVariant) -> Self {
        Self {
            id: v.id,
            position: v.position,
            status: v.status,
            progress: v.progress.map(Into::into),
            recent_tasks: v
                .recent_tasks
                .into_iter()
                .map(|t| PlanTask {
                    id: t.id,
                    title: t.title,
                    status: t.status.parse().unwrap_or_default(),
                    note: None,
                })
                .collect(),
        }
    }
}

impl From<OverviewVariant> for pb::OverviewVariant {
    fn from(v: OverviewVariant) -> Self {
        Self {
            id: v.id,
            position: v.position,
            status: v.status,
            progress: v.progress.map(Into::into),
            recent_tasks: v
                .recent_tasks
                .into_iter()
                .map(|t| pb::OverviewPlanTask {
                    id: t.id,
                    title: t.title,
                    status: t.status.as_str().into(),
                })
                .collect(),
        }
    }
}
