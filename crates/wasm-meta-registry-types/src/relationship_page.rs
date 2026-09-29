/// A page of deduplicated package or world relationship matches.
///
/// The total describes this relationship query, not the whole registry.
/// Pagination is independent of total availability.
///
/// ```
/// use wasm_meta_registry_types::{DependentPackage, RelationshipPage};
///
/// let page = RelationshipPage::<DependentPackage> {
///     results: vec![],
///     total: Some(0),
///     offset: 0,
///     limit: 100,
///     has_next: false,
/// };
/// assert!(page.results.is_empty());
/// ```
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RelationshipPage<T> {
    /// Matches on this page, in stable identity order.
    pub results: Vec<T>,
    /// Total eligible, deduplicated matches, or unavailable if omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total: Option<u64>,
    /// The effective pagination offset.
    pub offset: u32,
    /// The effective page size.
    pub limit: u32,
    /// Whether more matching results follow this page.
    pub has_next: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_total_is_unavailable_not_zero() {
        let page: RelationshipPage<String> =
            serde_json::from_str(r#"{"results":["match"],"offset":0,"limit":1,"has_next":true}"#)
                .expect("parse a page without a total");
        assert_eq!(page.total, None);
        assert!(page.has_next);
        assert_eq!(page.results, ["match"]);
    }
}
