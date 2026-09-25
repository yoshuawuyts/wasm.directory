//! Source-pinned detail URLs, kept separate from version-independent searches.

use crate::components::ds::search_bar::encode_query;

/// Attach an exact OCI source to a query-free package detail path.
#[must_use]
pub(crate) fn with_source(path: &str, registry: &str, repository: &str) -> String {
    format!(
        "{path}?registry={}&repository={}",
        encode_query(registry),
        encode_query(repository)
    )
}

/// Append a detail path suffix before any source query parameters.
#[must_use]
pub(crate) fn append_path(base: &str, suffix: &str) -> String {
    match base.split_once('?') {
        Some((path, query)) => format!("{path}{suffix}?{query}"),
        None => format!("{base}{suffix}"),
    }
}

/// Encode one path segment without treating spaces as query-string `+` signs.
#[must_use]
pub(crate) fn encode_segment(segment: &str) -> String {
    encode_query(segment).replace('+', "%20")
}
