//! Renderer for the `variant get` detail block, over the `Variant` domain type
//! and its owning `Task`.

use std::fmt::Write as _;

use modula_types::{Task, Variant};

use super::or_dash;

pub fn variant_detail(variant: &Variant, owner: &Task) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "variant: {}", variant.id);
    let _ = writeln!(out, "status: {}", or_dash(variant.status.as_deref()));
    let _ = writeln!(out, "position: {}", variant.position);
    out.push('\n');
    let _ = writeln!(out, "task: {}", owner.id);
    let _ = writeln!(out, "task_title: {}", owner.title);
    out
}
