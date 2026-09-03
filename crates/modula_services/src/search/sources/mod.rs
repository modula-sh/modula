//! One [`super::SearchSource`] per searchable entity.

pub(super) mod agents;
pub(super) mod conversations;
pub(super) mod projects;
pub(super) mod providers;
pub(super) mod tasks;
pub(super) mod wiki;

use modula_types::{SearchHit, SearchKind};

use super::excerpt::{contains, excerpt, RADIUS};

/// Rows to ask SQL for per row we can return, where a source's `LIKE` is
/// wider than what it renders. `limit` caps results the user sees, so rows
/// dropped after the query must not spend it.
const OVERFETCH: i64 = 4;

/// The hit for one row. A title match wins and carries no excerpt — the title
/// is already row 1 of the result, so repeating it below is noise. Otherwise
/// the first `bodies` entry containing the query supplies the excerpt, which
/// makes the slice order the source's field priority. `None` when the row does
/// not actually match (the SQL `LIKE` can hit a column this source does not
/// render).
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
