//! Renderer for `roadmap list`, over the `RoadmapEntry` domain type. Entries
//! arrive in roadmap order (by position); each row carries its task, pipeline
//! status, dependencies, and notes.

use std::fmt::Write as _;

use modula_types::RoadmapEntry;

use super::{or_dash, DASH};

pub fn roadmap_list(entries: &[RoadmapEntry]) -> String {
    if entries.is_empty() {
        return "(no roadmap entries)\n".to_string();
    }
    let mut out = String::new();
    for (i, e) in entries.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let _ = writeln!(out, "task: {}", e.task);
        let _ = writeln!(out, "status: {}", e.status);
        let _ = writeln!(
            out,
            "depends_on: {}",
            if e.depends_on.is_empty() {
                DASH.to_string()
            } else {
                e.depends_on.join(", ")
            }
        );
        let notes = Some(e.notes.as_str()).filter(|s| !s.is_empty());
        let _ = writeln!(out, "notes: {}", or_dash(notes));
    }
    out
}
