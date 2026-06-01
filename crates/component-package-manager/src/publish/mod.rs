//! Publish a component or WIT interface to an OCI registry.
//!
//! This module turns a `wasm.toml` manifest with a `[package]` section
//! into a single OCI artifact pushed to a registry:
//!
//! * **Components** are *push-only*: read the compiled `.wasm` from
//!   `[package].file` (default `build/<name>.wasm`) and upload it.
//! * **Interfaces** are *built then pushed*: the WIT directory at
//!   `[package].wit` (default `wit`) is parsed via [`wit_packager`]
//!   (which stamps `[package].version` onto the WIT package decls and
//!   rejects pre-existing `@version` annotations) and the resulting
//!   WIT-only WASM is uploaded.
//!
//! Both paths share the OCI push primitive in
//! [`crate::oci::client::Client::push`] (which mirrors the existing
//! `pull`) and produce the same set of `org.opencontainers.image.*`
//! annotations.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use oci_client::Reference;

use component_manifest::{Manifest, Package, PackageKind};

mod wit_packager;

pub use wit_packager::{
    WitPackaged, WitPackagerError, build_wit_package, build_wit_package_anyhow,
};

/// A description of what `publish` would do, returned by
/// [`crate::manager::Manager::publish_dry_run`].
///
/// This is the structured form of the dry-run output; the CLI formats
/// it for display.
#[derive(Debug, Clone)]
pub struct PublishPlan {
    /// The fully-resolved OCI reference we would push to.
    pub reference: Reference,
    /// The artifact path on disk that was used as the source.
    pub source_path: PathBuf,
    /// Whether the artifact was built (true for `kind = "interface"`)
    /// or just read from disk (false for `kind = "component"`).
    pub built: bool,
    /// Size in bytes of the artifact.
    pub size_bytes: u64,
    /// The `org.opencontainers.image.*` annotations that would be
    /// attached to the OCI manifest.
    pub annotations: BTreeMap<String, String>,
    /// The encoded artifact bytes, retained on the plan so the publish
    /// path can hand them off to the OCI client without re-reading the
    /// source. For non-dry-run callers these bytes are moved out
    /// (via [`std::mem::take`]) before the push, so the field will be
    /// empty on the returned plan. The dry-run renderer does not
    /// currently print per-layer digests.
    pub bytes: Vec<u8>,
}

impl PublishPlan {
    /// Render the plan as a human-readable multi-line string suitable
    /// for `--dry-run` output.
    #[must_use]
    pub fn render(&self) -> String {
        use std::fmt::Write;
        let mut s = String::new();
        let _ = writeln!(s, "Target reference: {}", self.reference);
        let _ = writeln!(s, "Source: {}", self.source_path.display());
        let _ = writeln!(
            s,
            "Action: {}",
            if self.built {
                "build WIT + push"
            } else {
                "push existing component"
            }
        );
        let _ = writeln!(s, "Layers: 1");
        let _ = writeln!(s, "Layer size: {} bytes", self.size_bytes);
        let _ = writeln!(s, "Annotations:");
        for (k, v) in &self.annotations {
            let _ = writeln!(s, "  {k} = {v}");
        }
        s
    }
}

/// Resolve the artifact bytes (and on-disk source path) for the given
/// `[package]` section, using the manifest directory as the root.
///
/// For components this just reads the file from disk; for interfaces it
/// invokes the WIT packager.
pub(crate) async fn load_artifact(
    manifest_dir: &Path,
    pkg: &Package,
) -> Result<(Vec<u8>, PathBuf, bool)> {
    let rel = pkg.artifact_path();
    let abs = manifest_dir.join(&rel);
    match pkg.kind {
        PackageKind::Component => {
            let bytes = tokio::fs::read(&abs)
                .await
                .with_context(|| format!("failed to read component file `{}`", abs.display()))?;
            Ok((bytes, abs, false))
        }
        PackageKind::Interface => {
            let version = pkg.version.clone();
            let abs_clone = abs.clone();
            // Run the synchronous wit-component encoder on the blocking
            // pool so we don't block the runtime.
            let packaged =
                tokio::task::spawn_blocking(move || build_wit_package_anyhow(&abs_clone, &version))
                    .await
                    .context("WIT packager task panicked")??;
            Ok((packaged.bytes, abs, true))
        }
    }
}

/// Build the canonical set of `org.opencontainers.image.*` annotations
/// for a given `[package]` section.
///
/// `created` is supplied separately so callers (notably tests) can pin
/// it to a deterministic value.
#[must_use]
pub fn build_annotations(pkg: &Package, created: DateTime<Utc>) -> BTreeMap<String, String> {
    let mut a = BTreeMap::new();
    a.insert("org.opencontainers.image.title".into(), pkg.name.clone());
    a.insert(
        "org.opencontainers.image.version".into(),
        pkg.version.clone(),
    );
    a.insert(
        "org.opencontainers.image.created".into(),
        created.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    );
    if let Some(d) = &pkg.description {
        a.insert("org.opencontainers.image.description".into(), d.clone());
    }
    if let Some(s) = &pkg.source {
        a.insert("org.opencontainers.image.source".into(), s.clone());
    }
    if let Some(h) = &pkg.homepage {
        a.insert("org.opencontainers.image.url".into(), h.clone());
    }
    if let Some(d) = &pkg.documentation {
        a.insert("org.opencontainers.image.documentation".into(), d.clone());
    }
    if let Some(l) = &pkg.license {
        a.insert("org.opencontainers.image.licenses".into(), l.clone());
    }
    if !pkg.authors.is_empty() {
        a.insert(
            "org.opencontainers.image.authors".into(),
            pkg.authors.join(", "),
        );
    }
    a
}

/// Resolve the OCI reference for a given `[package]` section.
///
/// The reference is built from `[package].registry`,
/// `[package].repository`, and `[package].version` as
/// `<registry>/<repository>:<version>` — `registry` is the OCI registry
/// base (host + optional path, e.g. `ghcr.io/yoshuawuyts`) and
/// `repository` is the catalog path within it (e.g. `yoshuawuyts/fetch`).
///
/// # Errors
///
/// Returns an error when the resulting reference cannot be parsed.
pub fn resolve_reference(pkg: &Package) -> Result<Reference> {
    let s = format!("{}/{}:{}", pkg.registry, pkg.repository, pkg.version);
    s.parse::<Reference>()
        .with_context(|| format!("failed to parse OCI reference `{s}`"))
}

/// Validate the manifest's `[package]` section is present and internally
/// consistent.
///
/// # Errors
///
/// Returns an error when there is no `[package]` section or
/// [`Package::validate`] fails.
pub fn require_package(manifest: &Manifest) -> Result<&Package> {
    let pkg = manifest
        .package
        .as_ref()
        .context("manifest is missing a `[package]` section; cannot publish")?;
    pkg.validate()?;
    Ok(pkg)
}

/// Build the [`PublishPlan`] for the given manifest without performing
/// any network I/O.
///
/// `manifest_dir` is the directory containing `wasm.toml` (used to
/// resolve relative paths in `[package].file` / `[package].wit`). The
/// target reference is read from the manifest's `[package].registry`,
/// `[package].repository`, and `[package].version` fields — there is no
/// implicit default.
pub async fn plan(manifest: &Manifest, manifest_dir: &Path) -> Result<PublishPlan> {
    let pkg = require_package(manifest)?;
    let reference = resolve_reference(pkg)?;
    let (bytes, source_path, built) = load_artifact(manifest_dir, pkg).await?;
    let size_bytes = bytes.len() as u64;
    let annotations = build_annotations(pkg, Utc::now());
    Ok(PublishPlan {
        reference,
        source_path,
        built,
        size_bytes,
        annotations,
        bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use component_manifest::PackageKind;

    fn sample_pkg() -> Package {
        Package {
            name: "yoshuawuyts:fetch".into(),
            version: "0.1.0".into(),
            registry: "ghcr.io/yoshuawuyts".into(),
            repository: "fetch".into(),
            kind: PackageKind::Component,
            file: None,
            wit: None,
            description: Some("Fetch helper".into()),
            source: Some("https://github.com/yoshuawuyts/fetch".into()),
            homepage: Some("https://example.com".into()),
            documentation: Some("https://docs.example.com".into()),
            license: Some("Apache-2.0".into()),
            authors: vec!["Yosh <yosh@example.com>".into()],
        }
    }

    // r[verify publish.annotations.full]
    #[test]
    fn annotations_include_all_known_keys() {
        let pkg = sample_pkg();
        let annot = build_annotations(&pkg, Utc::now());
        assert_eq!(
            annot
                .get("org.opencontainers.image.title")
                .map(String::as_str),
            Some("yoshuawuyts:fetch"),
        );
        assert_eq!(
            annot
                .get("org.opencontainers.image.version")
                .map(String::as_str),
            Some("0.1.0"),
        );
        assert_eq!(
            annot
                .get("org.opencontainers.image.licenses")
                .map(String::as_str),
            Some("Apache-2.0"),
        );
        assert_eq!(
            annot
                .get("org.opencontainers.image.authors")
                .map(String::as_str),
            Some("Yosh <yosh@example.com>"),
        );
        assert!(annot.contains_key("org.opencontainers.image.created"));
        assert!(annot.contains_key("org.opencontainers.image.description"));
        assert!(annot.contains_key("org.opencontainers.image.source"));
        assert!(annot.contains_key("org.opencontainers.image.url"));
        assert!(annot.contains_key("org.opencontainers.image.documentation"));
    }

    // r[verify publish.annotations.minimal]
    #[test]
    fn annotations_skip_unset_optional_fields() {
        let mut pkg = sample_pkg();
        pkg.description = None;
        pkg.source = None;
        pkg.homepage = None;
        pkg.documentation = None;
        pkg.license = None;
        pkg.authors = vec![];
        let annot = build_annotations(&pkg, Utc::now());
        assert!(annot.contains_key("org.opencontainers.image.title"));
        assert!(annot.contains_key("org.opencontainers.image.version"));
        assert!(annot.contains_key("org.opencontainers.image.created"));
        assert!(!annot.contains_key("org.opencontainers.image.description"));
        assert!(!annot.contains_key("org.opencontainers.image.licenses"));
        assert!(!annot.contains_key("org.opencontainers.image.authors"));
    }

    // r[verify publish.reference.resolves]
    #[test]
    fn reference_is_built_from_registry_and_repository() {
        let pkg = sample_pkg();
        let r = resolve_reference(&pkg).expect("ok");
        assert_eq!(r.registry(), "ghcr.io");
        assert_eq!(r.repository(), "yoshuawuyts/fetch");
        assert_eq!(r.tag(), Some("0.1.0"));
    }

    // r[verify publish.reference.invalid-ref]
    #[test]
    fn reference_rejects_unparseable_ref() {
        let mut pkg = sample_pkg();
        pkg.registry = "not a valid ref".into();
        assert!(resolve_reference(&pkg).is_err());
    }

    // r[verify publish.require-package.missing]
    #[test]
    fn require_package_errors_when_missing() {
        let manifest = Manifest::default();
        assert!(require_package(&manifest).is_err());
    }
}
