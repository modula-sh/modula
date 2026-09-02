//! Shared bits of the per-repository `search` methods.

/// Turn a user query into a `LIKE` pattern, escaping the wildcards so `100%`
/// does not match everything. Every `search` method pairs this with an explicit
/// `ESCAPE '\'` clause.
pub(crate) fn like_pattern(query: &str) -> String {
    let mut out = String::with_capacity(query.len() + 2);
    out.push('%');
    for ch in query.chars() {
        if matches!(ch, '\\' | '%' | '_') {
            out.push('\\');
        }
        out.push(ch);
    }
    out.push('%');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_a_plain_query() {
        assert_eq!(like_pattern("search"), "%search%");
    }

    #[test]
    fn escapes_wildcards() {
        assert_eq!(like_pattern("100%"), r"%100\%%");
        assert_eq!(like_pattern("a_b"), r"%a\_b%");
        assert_eq!(like_pattern(r"c\d"), r"%c\\d%");
    }
}
