/// An indexed namespace with at least one released package.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct KnownNamespace {
    /// WIT namespace, or the repository owner when no WIT mapping is available.
    pub name: String,
    /// Number of indexed repositories in this namespace with semver releases.
    pub packages: u64,
}
