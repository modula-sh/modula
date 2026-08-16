//! Client-owned request structs for the multi-field write calls. They take
//! domain-shaped inputs (`serde_json::Value` for schemaless fields, wire strings
//! for thread enums) and the client converts them to protos at the edge. The
//! single- and two-field calls take plain parameters instead — a one-field
//! request struct would just be noise.

use modula_types::{AgentArgDef, AgentSchedule, McpServer};
use serde_json::Value;

/// Internal create — the engine mints the display id (e.g. `MOD-001`).
pub struct CreateTask {
    pub workspace_id: String,
    pub title: String,
    pub description: Option<String>,
    pub approved: Option<bool>,
    pub max_variants: Option<i64>,
    pub worktree: Option<bool>,
    pub source_data: Option<Value>,
}

/// External upsert from a scanner — caller supplies `external_id` + `source`.
pub struct UpsertTask {
    pub workspace_id: String,
    pub external_id: String,
    pub source: String,
    pub title: String,
    pub description: Option<String>,
    pub source_data: Option<Value>,
    pub status: Option<String>,
    pub url: Option<String>,
    pub synced_at: Option<String>,
    pub approved: Option<bool>,
    pub max_variants: Option<i64>,
    pub worktree: Option<bool>,
}

/// Partial task edit — absent fields are left unchanged.
pub struct UpdateTask {
    pub workspace_id: String,
    pub task_id: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub approved: Option<bool>,
    pub max_variants: Option<i64>,
    pub worktree: Option<bool>,
}

/// Append a thread entry. `author`/`kind`/`verdict` are the wire strings the
/// engine stores; the client converts them to proto enums.
pub struct AppendEntry {
    pub workspace_id: String,
    pub task_id: String,
    pub content: String,
    pub author: String,
    pub kind: String,
    pub variant_id: Option<String>,
    pub round: Option<i64>,
    pub verdict: Option<String>,
    pub affected_variants: Vec<String>,
}

/// Upsert a roadmap row (creates if absent, patches if present).
pub struct SetRoadmapStatus {
    pub workspace_id: String,
    pub task_id: String,
    pub status: String,
    pub depends_on: Vec<String>,
    pub notes: Option<String>,
}

/// The agent create/update form body. `name` is supplied on create and ignored
/// on update (the engine can't rename). On update, an absent `model`/`schedule`
/// clears the stored value; `rules`/`args`/`skills` always replace.
pub struct WriteAgent {
    pub name: Option<String>,
    pub description: String,
    pub provider_id: String,
    pub model: Option<String>,
    pub manual: bool,
    pub schedule: Option<AgentSchedule>,
    pub rules: Vec<String>,
    pub args: Vec<AgentArgDef>,
    pub prompt: String,
    pub spawn_per_variant: bool,
    pub skills: Vec<String>,
}

/// Register a new provider with its managed MCP servers.
pub struct CreateProvider {
    pub workspace_id: String,
    pub name: String,
    pub r#type: String,
    pub config_dir: String,
    pub description: Option<String>,
    pub mcp_servers: Vec<McpServer>,
}

/// Partial provider edit. `mcp_servers: None` leaves the config file untouched;
/// `Some` reconciles it. `clear_description` blanks the description.
pub struct UpdateProvider {
    pub workspace_id: String,
    pub provider_id: String,
    pub name: Option<String>,
    pub r#type: Option<String>,
    pub config_dir: Option<String>,
    pub description: Option<String>,
    pub clear_description: bool,
    pub mcp_servers: Option<Vec<McpServer>>,
}
