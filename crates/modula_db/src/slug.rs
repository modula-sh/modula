//! Slug generation for human-readable on-disk paths (workspace dirs, spec
//! folders). UUIDs stay canonical everywhere in the DB/API; slugs are purely
//! the filesystem-facing names.

/// Lowercase, collapse any run of non-alphanumerics to a single `-`, and trim
/// leading/trailing `-`. Empty input falls back to `"workspace"`.
pub fn slugify(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_dash = false;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "workspace".to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_examples() {
        assert_eq!(slugify("Modula"), "modula");
        assert_eq!(slugify("Some New Workspace"), "some-new-workspace");
        assert_eq!(
            slugify("MOD-0001 Some new adjustment"),
            "mod-0001-some-new-adjustment"
        );
        assert_eq!(slugify("  Trailing / Punct! "), "trailing-punct");
        assert_eq!(slugify(""), "workspace");
    }
}
