//! Domain types shared by the engine, CLI, and Tauri backend, and the single
//! place protos are converted to/from them. Every component converts at the
//! edge to these types instead of threading `modula_rpc::v1` protos and ad-hoc
//! JSON through the stack.
//!
//! Each entity owns one `From<pb::X> for X` (proto→domain) and one
//! `From<X> for pb::X` (domain→proto). The `serde` output of every type
//! reproduces the JSON the desktop frontend already consumes (`apps/desktop/
//! src/types.ts`), so adopting these types does not change the wire contract.

mod agent;
mod config;
mod conversation;
mod event;
mod integration;
mod label;
mod project;
mod provider;
mod roadmap;
mod run;
mod search;
mod task;
mod thread;
mod usage;
mod wiki;
mod workspace;

pub use agent::{Agent, AgentSkill, RunningAgent, SystemTool};
pub use config::{
    AgentArgDef, AgentSchedule, ConfigAgent, ConfigLimits, ConfigProject, ConfigProvider,
    PipelineStatus, WorkspaceConfig,
};
pub use conversation::{ChatMessage, ConvEvent, Conversation};
pub use event::{event_types, WorkspaceEvent, WorkspaceEventKind};
pub use integration::{ExternalItem, Integration};
pub use label::Label;
pub use project::{CommitSummary, Project, RepoBranchInfo};
pub use provider::{CatalogModel, CatalogProvider, McpServer, Provider};
pub use roadmap::RoadmapEntry;
pub use run::{AgentRun, RunStatus};
pub use search::{ExcerptSpan, SearchHit, SearchKind};
pub use task::{AgentLoop, Task, TaskAgentSetting, TaskLabel, Variant};
pub use thread::{ThreadBundle, ThreadEntry};
pub use usage::{UsageEntry, UsageTokens};
pub use wiki::WikiNode;
pub use workspace::Workspace;
