//! Renderer for a task's full thread (task-scoped entries then each variant's
//! thread), over the `ThreadBundle` domain type.

use std::fmt::Write as _;

use modula_types::{ThreadBundle, ThreadEntry};

fn entry_block(out: &mut String, e: &ThreadEntry) {
    let _ = writeln!(out, "  id: {}", e.id);
    let _ = writeln!(out, "  ts: {}", e.ts);
    let _ = writeln!(out, "  author: {}", e.author);
    let _ = writeln!(out, "  kind: {}", e.kind);
    if let Some(r) = e.round {
        let _ = writeln!(out, "  round: {r}");
    }
    if let Some(v) = e.verdict.as_deref() {
        let _ = writeln!(out, "  verdict: {v}");
    }
    if !e.affected_variants.is_empty() {
        let _ = writeln!(
            out,
            "  affected_variants: {}",
            e.affected_variants.join(", ")
        );
    }
    let _ = writeln!(out, "  content: {}", e.content);
}

fn entries_block(out: &mut String, header: &str, entries: &[ThreadEntry]) {
    if entries.is_empty() {
        let _ = writeln!(out, "{header}: (none)");
    } else {
        let _ = writeln!(out, "{header}:");
        for (i, e) in entries.iter().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            entry_block(out, e);
        }
    }
}

pub fn threads(t: &ThreadBundle) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "task: {}", t.task);

    out.push('\n');
    entries_block(&mut out, "task_thread", &t.task_thread);

    for (variant_id, entries) in &t.variant_threads {
        out.push('\n');
        entries_block(&mut out, &format!("variant {variant_id}"), entries);
    }
    out
}
