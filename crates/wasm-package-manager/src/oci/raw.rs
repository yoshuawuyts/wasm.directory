//! `RawImageEntry` — joined view over `oci_manifest` × `oci_repository`.

use oci_client::manifest::OciImageManifest;

/// Metadata for a stored OCI image.
///
/// Constructed by joining `oci_manifest`, `oci_repository`, and optionally
/// `oci_tag`. Not backed by its own table. Built by
/// [`crate::storage::Store::list_all`] using SeaORM queries.
///
/// The public API exposes [`super::ImageEntry`] instead, which strips away
/// internal IDs.
#[derive(Debug, Clone)]
pub struct RawImageEntry {
    /// Internal `oci_manifest.id` for joining.
    #[allow(dead_code)]
    pub(crate) id: i64,
    /// Registry hostname.
    pub ref_registry: String,
    /// Repository path.
    pub ref_repository: String,
    /// Optional mirror registry hostname (always `None` in the new schema).
    pub ref_mirror_registry: Option<String>,
    /// Optional tag.
    pub ref_tag: Option<String>,
    /// Optional digest.
    pub ref_digest: Option<String>,
    /// OCI image manifest.
    pub manifest: OciImageManifest,
    /// Size of the image on disk in bytes.
    pub size_on_disk: u64,
}

impl RawImageEntry {
    /// Returns the full reference string for this image (e.g.
    /// "ghcr.io/user/repo:tag").
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_entry_reference_with_tag() {
        let entry = RawImageEntry {
            id: 1,
            ref_registry: "ghcr.io".to_string(),
            ref_repository: "user/repo".to_string(),
            ref_mirror_registry: None,
            ref_tag: Some("latest".to_string()),
            ref_digest: Some("sha256:abc".to_string()),
            manifest: OciImageManifest::default(),
            size_on_disk: 1024,
        };
        assert_eq!(entry.reference(), "ghcr.io/user/repo:latest");
    }

    #[test]
    fn test_image_entry_reference_with_digest_only() {
        let entry = RawImageEntry {
            id: 1,
            ref_registry: "ghcr.io".to_string(),
            ref_repository: "user/repo".to_string(),
            ref_mirror_registry: None,
            ref_tag: None,
            ref_digest: Some("sha256:abc123".to_string()),
            manifest: OciImageManifest::default(),
            size_on_disk: 512,
        };
        assert_eq!(entry.reference(), "ghcr.io/user/repo@sha256:abc123");
    }

    #[test]
    fn test_image_entry_reference_bare() {
        let entry = RawImageEntry {
            id: 1,
            ref_registry: "ghcr.io".to_string(),
            ref_repository: "user/repo".to_string(),
            ref_mirror_registry: None,
            ref_tag: None,
            ref_digest: None,
            manifest: OciImageManifest::default(),
            size_on_disk: 0,
        };
        assert_eq!(entry.reference(), "ghcr.io/user/repo");
    }
}
