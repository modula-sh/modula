//! Plain-text renderers for the `modula_types` domain types, one module per
//! resource.
//!
//! Each renderer turns a domain type into label-driven plain text — never JSON,
//! since agents read the output directly. Optional/omitted fields render as a
//! literal em dash so every record has a stable shape.

mod comment;
mod config;
mod roadmap;
mod task;
mod variant;
mod workspace;

pub use comment::threads;
pub use config::config;
pub use roadmap::roadmap_list;
pub use task::{task_detail, task_list};
pub use variant::variant_detail;
pub use workspace::{workspace_detail, workspace_list};

const DASH: &str = "—";

fn or_dash(s: Option<&str>) -> &str {
    s.unwrap_or(DASH)
}

/// Render a proto `string` field, treating empty as absent.
fn str_dash(s: &str) -> &str {
    if s.is_empty() {
        DASH
    } else {
        s
    }
}

fn bool_dash(b: Option<bool>) -> &'static str {
    match b {
        Some(true) => "true",
        Some(false) => "false",
        None => DASH,
    }
}

fn num_dash(n: Option<i64>) -> String {
    n.map(|n| n.to_string()).unwrap_or_else(|| DASH.to_string())
}
