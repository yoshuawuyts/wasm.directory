use crate::KnownPackage;

/// A world importing or exporting the requested package or interface.
///
/// Each owning package/world pair appears once, at its newest matching
/// semantic version. A compiled component's synthetic world is presented
/// on its owning package page rather than a separate world page.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MatchingWorld {
    /// The world's owning package.
    pub package: KnownPackage,
    /// The world's name within its package.
    pub name: String,
    /// The world's description, when indexed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The actual OCI release tag containing the matching world.
    pub version: String,
    /// Whether this is the `root` world of an indexed `root:component` envelope.
    ///
    /// Synthetic worlds are displayed on the owning package page, even when
    /// that repository's package kind has not been classified.
    #[serde(default)]
    pub is_synthetic: bool,
}
