use oci_client::manifest::OciImageManifest;

use super::raw::RawImageEntry;

/// A public view of an OCI image entry, without internal database IDs.
///
/// This type is freely constructable and is the primary public API type
/// for representing stored OCI images. Internal code uses [`RawImageEntry`]
/// with database IDs; this type strips those away.
///
/// # Example
///
/// ```
/// use oci_client::manifest::OciImageManifest;
/// use wasm_package_manager::oci::ImageEntry;
///
/// let entry = ImageEntry {
///     ref_registry: "ghcr.io".to_string(),
///     ref_repository: "user/repo".to_string(),
///     ref_mirror_registry: None,
///     ref_tag: Some("v1.0".to_string()),
///     ref_digest: None,
///     manifest: OciImageManifest::default(),
///     size_on_disk: 2048,
/// };
/// assert_eq!(entry.ref_registry, "ghcr.io");
/// ```
#[derive(Debug, Clone)]
pub struct ImageEntry {
    /// Registry hostname
    pub ref_registry: String,
    /// Repository path
    pub ref_repository: String,
    /// Optional mirror registry hostname
    pub ref_mirror_registry: Option<String>,
    /// Optional tag
    pub ref_tag: Option<String>,
    /// Optional digest
    pub ref_digest: Option<String>,
    /// OCI image manifest
    pub manifest: OciImageManifest,
    /// Size of the image on disk in bytes
    pub size_on_disk: u64,
}

impl ImageEntry {
    /// Returns the full reference string for this image (e.g., "ghcr.io/user/repo:tag").
    ///
    /// # Example
    ///
    /// ```
    /// use oci_client::manifest::OciImageManifest;
    /// use wasm_package_manager::oci::ImageEntry;
    ///
    /// let entry = ImageEntry {
    ///     ref_registry: "ghcr.io".to_string(),
    ///     ref_repository: "user/repo".to_string(),
    ///     ref_mirror_registry: None,
    ///     ref_tag: Some("v1.0".to_string()),
    ///     ref_digest: None,
    ///     manifest: OciImageManifest::default(),
    ///     size_on_disk: 0,
    /// };
    /// assert_eq!(entry.reference(), "ghcr.io/user/repo:v1.0");
    /// ```
    #[must_use]
    pub fn reference(&self) -> String {
        let mut reference = format!("{}/{}", self.ref_registry, self.ref_repository);
        if let Some(tag) = &self.ref_tag {
            reference.push(':');
            reference.push_str(tag);
        } else if let Some(digest) = &self.ref_digest {
            reference.push('@');
            reference.push_str(digest);
        }
        reference
    }
}

impl From<RawImageEntry> for ImageEntry {
    fn from(entry: RawImageEntry) -> Self {
        Self {
            ref_registry: entry.ref_registry,
            ref_repository: entry.ref_repository,
            ref_mirror_registry: entry.ref_mirror_registry,
            ref_tag: entry.ref_tag,
            ref_digest: entry.ref_digest,
            manifest: entry.manifest,
            size_on_disk: entry.size_on_disk,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── ImageEntry ──────────────────────────────────────────────────────

    #[test]
    fn image_entry_reference_with_tag() {
        let entry = ImageEntry {
            ref_registry: "ghcr.io".into(),
            ref_repository: "user/repo".into(),
            ref_mirror_registry: None,
            ref_tag: Some("v1.0".into()),
            ref_digest: Some("sha256:abc123".into()),
            manifest: OciImageManifest::default(),
            size_on_disk: 0,
        };
        assert_eq!(entry.reference(), "ghcr.io/user/repo:v1.0");
    }

    #[test]
    fn image_entry_reference_with_digest_only() {
        let entry = ImageEntry {
            ref_registry: "docker.io".into(),
            ref_repository: "library/nginx".into(),
            ref_mirror_registry: None,
            ref_tag: None,
            ref_digest: Some("sha256:abc123".into()),
            manifest: OciImageManifest::default(),
            size_on_disk: 0,
        };
        assert_eq!(entry.reference(), "docker.io/library/nginx@sha256:abc123");
    }

    #[test]
    fn image_entry_reference_no_tag_no_digest() {
        let entry = ImageEntry {
            ref_registry: "ghcr.io".into(),
            ref_repository: "user/repo".into(),
            ref_mirror_registry: None,
            ref_tag: None,
            ref_digest: None,
            manifest: OciImageManifest::default(),
            size_on_disk: 0,
        };
        assert_eq!(entry.reference(), "ghcr.io/user/repo");
    }
}
