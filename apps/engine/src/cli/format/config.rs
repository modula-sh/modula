//! Renderer for `config get` (limits, pipeline, projects, providers, agents),
//! over the `WorkspaceConfig` domain type.

use std::fmt::Write as _;

use modula_types::WorkspaceConfig;

use super::{or_dash, str_dash, DASH};

pub fn config(c: &WorkspaceConfig) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "limits:");
    let _ = writeln!(out, "  max_spawns_per_run: {}", c.limits.max_spawns_per_run);

    out.push('\n');
    let _ = writeln!(out, "pipeline:");
    for e in &c.pipeline {
        let mut flags = Vec::new();
        if let Some(s) = e.station.as_deref() {
            flags.push(format!("station={s}"));
        }
        if e.terminal {
            flags.push("terminal".to_string());
        }
        if e.error {
            flags.push("error".to_string());
        }
        let suffix = if flags.is_empty() {
            String::new()
        } else {
            format!(" ({})", flags.join(", "))
        };
        let _ = writeln!(out, "  {} — {} [{}]{}", e.key, e.label, e.tone, suffix);
    }

    out.push('\n');
    let _ = writeln!(out, "projects:");
    for p in &c.projects {
        let _ = writeln!(out, "  {} ({})", p.name, p.id);
        let _ = writeln!(out, "    path: {}", p.path);
        let _ = writeln!(out, "    base_branch: {}", p.base_branch);
    }

    out.push('\n');
    let _ = writeln!(out, "providers:");
    for p in &c.providers {
        let _ = writeln!(out, "  {} ({}) — type={}", p.name, p.id, p.r#type);
        let _ = writeln!(out, "    config_dir: {}", str_dash(&p.config_dir));
        if let Some(d) = p.description.as_deref().filter(|s| !s.is_empty()) {
            let _ = writeln!(out, "    {d}");
        }
    }

    out.push('\n');
    let _ = writeln!(out, "agents:");
    for a in &c.agents {
        let provider = if a.provider_id.is_empty() {
            DASH
        } else {
            &a.provider_id
        };
        let _ = writeln!(
            out,
            "  {} ({}) — provider={} model={}",
            a.name,
            a.id,
            provider,
            or_dash(a.model.as_deref())
        );
        if !a.description.is_empty() {
            let _ = writeln!(out, "    {}", a.description);
        }
    }
    out
}
