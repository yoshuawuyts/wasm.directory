/// A registered namespace, including registrations without indexed packages.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct KnownNamespace {
    /// Namespace name from the registry configuration.
    pub name: String,
    /// Number of indexed repositories in this namespace with semver releases.
    pub packages: u64,
}
