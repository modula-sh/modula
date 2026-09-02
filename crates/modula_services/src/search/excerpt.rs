//! Case-insensitive matching and the excerpt/highlight helper every source
//! shares.
//!
//! Folding is ASCII-only on purpose. SQLite's default `LIKE` folds ASCII only,
//! so anything wider would disagree with the query that produced the row; and
//! `to_ascii_lowercase` is length-preserving, so an offset found in the lowered
//! copy indexes the original correctly. Non-ASCII queries therefore match
//! case-sensitively.

use modula_types::ExcerptSpan;

/// Bytes of context kept either side of the match.
pub(super) const RADIUS: usize = 60;

pub(crate) fn contains(haystack: &str, needle: &str) -> bool {
    haystack
        .to_ascii_lowercase()
        .contains(&needle.to_ascii_lowercase())
}

/// Spans covering the first occurrence of `needle`, with up to `radius` bytes
/// of context either side, whitespace runs collapsed and `…` marking each
/// truncated end. `None` when `needle` does not occur.
pub(super) fn excerpt(haystack: &str, needle: &str, radius: usize) -> Option<Vec<ExcerptSpan>> {
    let start = haystack
        .to_ascii_lowercase()
        .find(&needle.to_ascii_lowercase())?;
    let end = start + needle.len();

    let lead = floor_boundary(haystack, start.saturating_sub(radius));
    let tail = ceil_boundary(haystack, end.saturating_add(radius));

    let mut before = collapse(&haystack[lead..start]);
    if lead > 0 {
        before.insert(0, '…');
    }
    let mut after = collapse(&haystack[end..tail]);
    if tail < haystack.len() {
        after.push('…');
    }

    Some(
        [
            (before, false),
            (collapse(&haystack[start..end]), true),
            (after, false),
        ]
        .into_iter()
        .filter(|(text, _)| !text.is_empty())
        .map(|(text, is_match)| ExcerptSpan { text, is_match })
        .collect(),
    )
}

/// Every run of whitespace becomes a single space, so a markdown body or a
/// multi-line comment renders as one readable line.
fn collapse(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last_ws = false;
    for ch in s.chars() {
        let ws = ch.is_whitespace();
        if !ws {
            out.push(ch);
        } else if !last_ws {
            out.push(' ');
        }
        last_ws = ws;
    }
    out
}

fn floor_boundary(s: &str, mut i: usize) -> usize {
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn ceil_boundary(s: &str, mut i: usize) -> usize {
    if i >= s.len() {
        return s.len();
    }
    while !s.is_char_boundary(i) {
        i += 1;
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spans(haystack: &str, needle: &str, radius: usize) -> Vec<(String, bool)> {
        excerpt(haystack, needle, radius)
            .unwrap()
            .into_iter()
            .map(|s| (s.text, s.is_match))
            .collect()
    }

    #[test]
    fn no_match_yields_none() {
        assert!(excerpt("the quick brown fox", "cat", RADIUS).is_none());
    }

    #[test]
    fn match_at_the_start_has_no_leading_span() {
        assert_eq!(
            spans("Search the workspace", "search", RADIUS),
            [("Search".into(), true), (" the workspace".into(), false)]
        );
    }

    #[test]
    fn match_in_the_middle_keeps_both_sides() {
        assert_eq!(
            spans("please search here", "SEARCH", RADIUS),
            [
                ("please ".into(), false),
                ("search".into(), true),
                (" here".into(), false),
            ]
        );
    }

    #[test]
    fn match_at_the_end_has_no_trailing_span() {
        assert_eq!(
            spans("workspace search", "search", RADIUS),
            [("workspace ".into(), false), ("search".into(), true)]
        );
    }

    #[test]
    fn truncation_ellipses_land_on_the_truncated_side_only() {
        let haystack = format!("{}needle{}", "a".repeat(50), "b".repeat(50));
        assert_eq!(
            spans(&haystack, "needle", 5),
            [
                ("…aaaaa".into(), false),
                ("needle".into(), true),
                ("bbbbb…".into(), false),
            ]
        );
    }

    #[test]
    fn whitespace_and_newlines_collapse_to_single_spaces() {
        assert_eq!(
            spans("a\n\n  b needle c\t\td", "needle", RADIUS),
            [
                ("a b ".into(), false),
                ("needle".into(), true),
                (" c d".into(), false),
            ]
        );
    }

    #[test]
    fn multi_byte_haystack_snaps_to_char_boundaries() {
        // The radius lands mid-`é` on both sides; slicing must widen, not panic.
        let haystack = "ééééé needle ééééé";
        let out = spans(haystack, "needle", 3);
        assert_eq!(out[1], ("needle".into(), true));
        assert!(out[0].0.ends_with("é "), "{out:?}");
        assert!(out[2].0.starts_with(" é"), "{out:?}");
    }
}
