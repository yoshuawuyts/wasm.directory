use crate::KnownPackage;

/// A package whose indexed dependencies reach the requested package.
///
/// Each package appears once, at its newest matching semantic version,
/// even when its latest release no longer declares the dependency.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DependentPackage {
    /// The dependent package's metadata.
    pub package: KnownPackage,
    /// The actual OCI release tag of the matching version.
    pub version: String,
}
