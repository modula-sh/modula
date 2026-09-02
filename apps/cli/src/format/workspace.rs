//! Renderers for `workspace list` / `workspace get`, over the `Workspace` proto
//! message. The engine returns the canonical `slug` (set once at create), so
//! it's the value the CLI prints and matches `--workspace` against — never
//! re-slugified.

use std::fmt::Write as _;

use modula_types::Workspace;

use super::str_dash;

fn description(w: &Workspace) -> &str {
    str_dash(w.description.as_deref().unwrap_or(""))
}

pub fn workspace_list(workspaces: &[Workspace]) -> String {
    if workspaces.is_empty() {
        return "(no workspaces)\n".to_string();
    }
    let mut out = String::new();
    for (i, w) in workspaces.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let _ = writeln!(out, "id: {}", w.id);
        let _ = writeln!(out, "name: {}", w.name);
        let _ = writeln!(out, "slug: {}", w.slug);
        let _ = writeln!(out, "description: {}", description(w));
    }
    out
}

pub fn workspace_detail(w: &Workspace) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "id: {}", w.id);
    let _ = writeln!(out, "name: {}", w.name);
    let _ = writeln!(out, "slug: {}", w.slug);
    let _ = writeln!(out, "description: {}", description(w));
    let _ = writeln!(out, "created_at: {}", str_dash(&w.created_at));
    let _ = writeln!(out, "path: {}", str_dash(&w.path));
    out
}
