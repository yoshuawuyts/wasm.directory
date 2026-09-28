/// A page of registry entries, filtered and sorted before pagination.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RegistryPage<T> {
    /// Entries on this page, in stable identity order.
    pub results: Vec<T>,
    /// Total matching entries across all pages.
    pub total: u64,
    /// The effective pagination offset.
    pub offset: u32,
    /// The effective page size.
    pub limit: u32,
    /// Whether more entries follow this page.
    pub has_next: bool,
}
