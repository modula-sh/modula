//! Renderers for the task list and the full task detail block, over the
//! `Task` / `RoadmapEntry` domain types.

use std::fmt::Write as _;

use modula_types::{RoadmapEntry, Task, Variant};

use super::{bool_dash, num_dash, or_dash, str_dash, DASH};

/// `2 [1:in_progress, 2:—]`, or `0` when there are none.
fn variants_summary(variants: &[Variant]) -> String {
    if variants.is_empty() {
        return "0".to_string();
    }
    let inner: Vec<String> = variants
        .iter()
        .map(|v| format!("{}:{}", v.position, or_dash(v.status.as_deref())))
        .collect();
    format!("{} [{}]", variants.len(), inner.join(", "))
}

fn roadmap_for<'a>(roadmap: &'a [RoadmapEntry], task_id: &str) -> Option<&'a RoadmapEntry> {
    roadmap.iter().find(|r| r.task == task_id)
}

pub fn task_list(tasks: &[Task], roadmap: &[RoadmapEntry]) -> String {
    if tasks.is_empty() {
        return "(no tasks)\n".to_string();
    }
    let mut out = String::new();
    for (i, t) in tasks.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let status = roadmap_for(roadmap, &t.id).map(|r| r.status.as_str());
        let _ = writeln!(out, "id: {}", t.id);
        let _ = writeln!(out, "external_id: {}", or_dash(t.external_id.as_deref()));
        let _ = writeln!(out, "title: {}", t.title);
        let _ = writeln!(out, "source: {}", t.source);
        let _ = writeln!(out, "pipeline_status: {}", or_dash(status));
        let _ = writeln!(out, "approved: {}", bool_dash(t.approved));
        let _ = writeln!(out, "max_variants: {}", num_dash(t.max_variants));
        let _ = writeln!(out, "variants: {}", variants_summary(&t.variants));
    }
    out
}

pub fn task_detail(task: &Task, roadmap: Option<&RoadmapEntry>) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "id: {}", task.id);
    let _ = writeln!(out, "external_id: {}", or_dash(task.external_id.as_deref()));
    let _ = writeln!(out, "title: {}", task.title);
    let _ = writeln!(out, "source: {}", task.source);
    let _ = writeln!(
        out,
        "pipeline_status: {}",
        or_dash(roadmap.map(|r| r.status.as_str()))
    );
    let _ = writeln!(out, "source_status: {}", or_dash(task.status.as_deref()));
    let _ = writeln!(out, "approved: {}", bool_dash(task.approved));
    let _ = writeln!(out, "max_variants: {}", num_dash(task.max_variants));
    let _ = writeln!(out, "worktree: {}", bool_dash(task.worktree));
    let _ = writeln!(out, "url: {}", or_dash(task.url.as_deref()));
    let _ = writeln!(out, "synced_at: {}", or_dash(task.synced_at.as_deref()));
    let _ = writeln!(out, "created_at: {}", or_dash(task.created_at.as_deref()));
    let _ = writeln!(out, "description: {}", str_dash(&task.description));

    let depends_on: &[String] = roadmap.map(|r| r.depends_on.as_slice()).unwrap_or(&[]);
    let notes = roadmap.map(|r| r.notes.as_str()).filter(|s| !s.is_empty());
    out.push('\n');
    let _ = writeln!(
        out,
        "depends_on: {}",
        if depends_on.is_empty() {
            DASH.to_string()
        } else {
            depends_on.join(", ")
        }
    );
    let _ = writeln!(out, "notes: {}", or_dash(notes));

    out.push('\n');
    if task.variants.is_empty() {
        let _ = writeln!(out, "variants: (none)");
    } else {
        let _ = writeln!(out, "variants:");
        for (i, v) in task.variants.iter().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            let _ = writeln!(out, "  id: {}", v.id);
            let _ = writeln!(out, "  status: {}", or_dash(v.status.as_deref()));
            let _ = writeln!(out, "  position: {}", v.position);
        }
    }

    out.push('\n');
    if task.labels.is_empty() {
        let _ = writeln!(out, "labels: (none)");
    } else {
        let _ = writeln!(out, "labels:");
        for l in &task.labels {
            let _ = writeln!(out, "  {}: {}", l.id, l.name);
        }
    }
    out
}
