//! One [`super::SearchSource`] per searchable entity.

pub(super) mod agents;
pub(super) mod conversations;
pub(super) mod projects;
pub(super) mod providers;
pub(super) mod tasks;
pub(super) mod wiki;

use modula_types::{SearchHit, SearchKind};

use super::excerpt::{contains, excerpt, RADIUS};

/// Extra rows to fetch where a source's `LIKE` is wider than what it renders:
/// `limit` counts rendered results, so rows dropped later must not spend it.
const OVERFETCH: i64 = 4;

/// The hit for one row. A title match carries no excerpt; otherwise the first
/// matching `bodies` entry supplies it, so their order is the field priority.
/// `None` when the row does not really match — the SQL `LIKE` is wider.
fn hit(
    kind: SearchKind,
    id: String,
    title: &str,
    subtitle: Option<String>,
    query: &str,
    bodies: &[(&str, &str)],
) -> Option<SearchHit> {
    let (field, excerpt) = if contains(title, query) {
        ("title", Vec::new())
    } else {
        bodies
            .iter()
            .find_map(|(field, text)| excerpt(text, query, RADIUS).map(|spans| (*field, spans)))?
    };
    Some(SearchHit {
        kind: kind.as_str().to_string(),
        id,
        title: title.to_string(),
        subtitle,
        field: field.to_string(),
        excerpt,
    })
}
