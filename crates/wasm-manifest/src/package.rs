//! The `[package]` section of a `wasm.toml` manifest.
//!
//! This section holds publish metadata for the single component or
//! WIT interface artifact described by the manifest. It mirrors the
//! shape of Cargo's `[package]` table.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::PackageType;

/// The kind of artifact a manifest publishes.
///
/// This determines which manifest path field applies:
/// * `kind = "component"` uses the `file` field when provided, and must
///   not set `wit`.
/// * `kind = "interface"` uses the `wit` field when provided, and must
///   not set `file`.
///
/// If the applicable field is omitted, path resolution falls back to the
/// default artifact path derived by [`Package::artifact_path`]
/// (`build/<name>.wasm` for components, `wit` for interfaces) rather
/// than requiring the field to be set explicitly.
///
/// # Example
///
/// ```rust
/// use wasm_manifest::PackageKind;
///
/// assert_eq!(PackageKind::Component.as_str(), "component");
/// assert_eq!(PackageKind::Interface.as_str(), "interface");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[must_use]
pub enum PackageKind {
    /// A compiled WebAssembly component (push-only).
    Component,
    /// A WIT interface definition (built then pushed).
    Interface,
}

impl PackageKind {
    /// String form of this kind, matching the TOML representation.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            PackageKind::Component => "component",
            PackageKind::Interface => "interface",
        }
    }
}

impl From<PackageKind> for PackageType {
    fn from(value: PackageKind) -> Self {
        match value {
            PackageKind::Component => PackageType::Component,
            PackageKind::Interface => PackageType::Interface,
        }
    }
}

/// The `[package]` section of a `wasm.toml` manifest.
///
/// The manifest is the single source of truth for the package version: WIT
/// files must not declare their own `@version` (the publisher stamps the
/// manifest version onto package decls during publish).
///
/// # Example
///
/// ```rust
/// use wasm_manifest::{Manifest, PackageKind};
///
/// let toml = r#"
/// [package]
/// name = "my-org:my-component"
/// kind = "component"
/// version = "0.1.0"
/// registry = "ghcr.io/my-org/my-component"
/// file = "build/my-component.wasm"
/// description = "An example component"
/// "#;
///
/// let manifest: Manifest = toml::from_str(toml).unwrap();
/// let pkg = manifest.package.expect("package section");
/// assert_eq!(pkg.kind, PackageKind::Component);
/// assert_eq!(pkg.version, "0.1.0");
/// assert_eq!(pkg.registry, "ghcr.io/my-org/my-component");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[must_use]
pub struct Package {
    /// The package name, in `namespace:name` form (e.g. `wasi:http`).
    pub name: String,
    /// The semver version of this package.
    ///
    /// During publish this version is the single source of truth: WIT
    /// package decls are stamped with it, and it becomes the OCI tag.
    pub version: String,
    /// The full OCI location this package is published to, without a tag,
    /// for example `ghcr.io/webassembly/wasi/http`.
    ///
    /// The published reference is `<registry>:<version>`. There is
    /// intentionally no default and no shorthand: every manifest spells
    /// out its full destination so that publishing is fully reproducible
    /// from the manifest alone.
    ///
    /// This maps onto the registry schema's split of a namespace-level
    /// `registry` base (e.g. `ghcr.io/webassembly`) plus a per-entry
    /// `repository` catalog path (e.g. `wasi/http`); `component registry
    /// publish` derives those two parts from this single value.
    ///
    /// This is the OCI publish location, **not** a source-code URL; use
    /// [`source`](Self::source) for the project's source repository.
    pub registry: String,
    /// What kind of artifact this manifest describes.
    pub kind: PackageKind,
    /// Path to the compiled component artifact, relative to the manifest
    /// directory. Only valid when `kind = "component"`. Optional in the
    /// manifest — when omitted this field stays `None` and
    /// [`Package::artifact_path`] resolves it to `build/<name>.wasm`
    /// (where `<name>` is the part of `name` after `:`) at use time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<PathBuf>,
    /// Path to the WIT directory, relative to the manifest directory.
    /// Only valid when `kind = "interface"`. Optional in the manifest —
    /// when omitted this field stays `None` and
    /// [`Package::artifact_path`] resolves it to `wit` at use time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wit: Option<PathBuf>,
    /// Human-readable short description of the package.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The source-code URL of this package.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// The homepage URL of this package.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
    /// The documentation URL of this package.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub documentation: Option<String>,
    /// The SPDX license expression for this package.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    /// Authors of this package, in any free-form notation (typically
    /// `Name <email>`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<String>,
}

/// Errors produced when validating a [`Package`] section.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub enum PackageError {
    /// `kind = "component"` but a `wit = "..."` field was set.
    ComponentWithWit,
    /// `kind = "interface"` but a `file = "..."` field was set.
    InterfaceWithFile,
    /// The `version` could not be parsed as semver.
    InvalidVersion {
        /// The offending version string.
        version: String,
        /// The underlying parse error.
        reason: String,
    },
    /// The `name` field is empty.
    EmptyName,
    /// The `registry` field is empty.
    EmptyRegistry,
}

impl std::fmt::Display for PackageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PackageError::ComponentWithWit => write!(
                f,
                "[package] kind = \"component\" must not set `wit`; use `file` instead"
            ),
            PackageError::InterfaceWithFile => write!(
                f,
                "[package] kind = \"interface\" must not set `file`; use `wit` instead"
            ),
            PackageError::InvalidVersion { version, reason } => write!(
                f,
                "[package] version '{version}' is not a valid semver: {reason}"
            ),
            PackageError::EmptyName => write!(f, "[package] name must not be empty"),
            PackageError::EmptyRegistry => {
                write!(f, "[package] registry must not be empty")
            }
        }
    }
}

impl std::error::Error for PackageError {}

impl Package {
    /// The default component artifact path: `build/<name-after-colon>.wasm`.
    #[must_use]
    pub fn default_component_path(name: &str) -> PathBuf {
        let bare = name.rsplit_once(':').map_or(name, |(_, n)| n);
        PathBuf::from(format!("build/{bare}.wasm"))
    }

    /// The default WIT directory: `wit`.
    #[must_use]
    pub fn default_wit_dir() -> PathBuf {
        PathBuf::from("wit")
    }

    /// The artifact path declared by this package, falling back to the
    /// default for its [`PackageKind`].
    #[must_use]
    pub fn artifact_path(&self) -> PathBuf {
        match self.kind {
            PackageKind::Component => self
                .file
                .clone()
                .unwrap_or_else(|| Self::default_component_path(&self.name)),
            PackageKind::Interface => self.wit.clone().unwrap_or_else(Self::default_wit_dir),
        }
    }

    /// Validate cross-field invariants.
    ///
    /// # Errors
    ///
    /// Returns a [`PackageError`] when:
    /// * `kind` and `file`/`wit` disagree.
    /// * `version` is not a valid semver string.
    /// * `name` is empty.
    /// * `registry` is empty.
    pub fn validate(&self) -> Result<(), PackageError> {
        if self.name.is_empty() {
            return Err(PackageError::EmptyName);
        }
        if self.registry.is_empty() {
            return Err(PackageError::EmptyRegistry);
        }
        match self.kind {
            PackageKind::Component if self.wit.is_some() => {
                return Err(PackageError::ComponentWithWit);
            }
            PackageKind::Interface if self.file.is_some() => {
                return Err(PackageError::InterfaceWithFile);
            }
            _ => {}
        }
        semver::Version::parse(&self.version).map_err(|e| PackageError::InvalidVersion {
            version: self.version.clone(),
            reason: e.to_string(),
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Manifest;

    // r[verify manifest.package.parse-component]
    #[test]
    fn parse_component_package() {
        let toml = r#"
            [package]
            name = "yoshuawuyts:fetch"
            kind = "component"
            version = "0.1.0"
            registry = "ghcr.io/yoshuawuyts/fetch"
            file = "build/fetch.wasm"
            description = "Tiny HTTP fetch helper"
            license = "Apache-2.0"
            authors = ["Yosh <yosh@example.com>"]
        "#;
        let manifest: Manifest = toml::from_str(toml).expect("parse");
        let pkg = manifest.package.expect("package");
        assert_eq!(pkg.name, "yoshuawuyts:fetch");
        assert_eq!(pkg.registry, "ghcr.io/yoshuawuyts/fetch");
        assert_eq!(pkg.kind, PackageKind::Component);
        assert_eq!(
            pkg.file.as_deref(),
            Some(std::path::Path::new("build/fetch.wasm"))
        );
        assert_eq!(pkg.license.as_deref(), Some("Apache-2.0"));
        assert_eq!(pkg.authors, vec!["Yosh <yosh@example.com>".to_string()]);
        pkg.validate().expect("valid");
    }

    // r[verify manifest.package.parse-interface]
    #[test]
    fn parse_interface_package() {
        let toml = r#"
            [package]
            name = "wasi:logging"
            kind = "interface"
            version = "1.2.3"
            registry = "ghcr.io/wasi/logging"
            wit = "wit"
        "#;
        let manifest: Manifest = toml::from_str(toml).expect("parse");
        let pkg = manifest.package.expect("package");
        assert_eq!(pkg.kind, PackageKind::Interface);
        assert_eq!(pkg.registry, "ghcr.io/wasi/logging");
        pkg.validate().expect("valid");
    }

    // r[verify manifest.package.no-package]
    #[test]
    fn missing_package_is_ok() {
        let toml = r#""#;
        let manifest: Manifest = toml::from_str(toml).expect("parse");
        assert!(manifest.package.is_none());
    }

    // r[verify manifest.package.kind-file-mismatch]
    #[test]
    fn component_with_wit_is_invalid() {
        let pkg = Package {
            name: "a:b".into(),
            version: "0.1.0".into(),
            registry: "ghcr.io/a".into(),
            kind: PackageKind::Component,
            file: None,
            wit: Some(PathBuf::from("wit")),
            description: None,
            source: None,
            homepage: None,
            documentation: None,
            license: None,
            authors: vec![],
        };
        assert_eq!(pkg.validate(), Err(PackageError::ComponentWithWit));
    }

    // r[verify manifest.package.kind-wit-mismatch]
    #[test]
    fn interface_with_file_is_invalid() {
        let pkg = Package {
            name: "a:b".into(),
            version: "0.1.0".into(),
            registry: "ghcr.io/a".into(),
            kind: PackageKind::Interface,
            file: Some(PathBuf::from("x.wasm")),
            wit: None,
            description: None,
            source: None,
            homepage: None,
            documentation: None,
            license: None,
            authors: vec![],
        };
        assert_eq!(pkg.validate(), Err(PackageError::InterfaceWithFile));
    }

    // r[verify manifest.package.invalid-version]
    #[test]
    fn invalid_version_is_rejected() {
        let pkg = Package {
            name: "a:b".into(),
            version: "not-a-version".into(),
            registry: "ghcr.io/a".into(),
            kind: PackageKind::Component,
            file: None,
            wit: None,
            description: None,
            source: None,
            homepage: None,
            documentation: None,
            license: None,
            authors: vec![],
        };
        assert!(matches!(
            pkg.validate(),
            Err(PackageError::InvalidVersion { .. })
        ));
    }

    // r[verify manifest.package.empty-name]
    #[test]
    fn empty_name_is_rejected() {
        let pkg = Package {
            name: String::new(),
            version: "0.1.0".into(),
            registry: "ghcr.io/a".into(),
            kind: PackageKind::Component,
            file: None,
            wit: None,
            description: None,
            source: None,
            homepage: None,
            documentation: None,
            license: None,
            authors: vec![],
        };
        assert_eq!(pkg.validate(), Err(PackageError::EmptyName));
    }

    // r[verify manifest.package.empty-registry]
    #[test]
    fn empty_registry_is_rejected() {
        let pkg = Package {
            name: "a:b".into(),
            version: "0.1.0".into(),
            registry: String::new(),
            kind: PackageKind::Component,
            file: None,
            wit: None,
            description: None,
            source: None,
            homepage: None,
            documentation: None,
            license: None,
            authors: vec![],
        };
        assert_eq!(pkg.validate(), Err(PackageError::EmptyRegistry));
    }

    // r[verify manifest.package.default-paths]
    #[test]
    fn default_paths() {
        let pkg = Package {
            name: "yoshuawuyts:fetch".into(),
            version: "0.1.0".into(),
            registry: "ghcr.io/a".into(),
            kind: PackageKind::Component,
            file: None,
            wit: None,
            description: None,
            source: None,
            homepage: None,
            documentation: None,
            license: None,
            authors: vec![],
        };
        assert_eq!(pkg.artifact_path(), PathBuf::from("build/fetch.wasm"));

        let pkg = Package {
            kind: PackageKind::Interface,
            file: None,
            wit: None,
            ..pkg
        };
        assert_eq!(pkg.artifact_path(), PathBuf::from("wit"));
    }

    // r[verify manifest.package.kind-conversion]
    #[test]
    fn package_kind_to_package_type() {
        assert_eq!(
            PackageType::from(PackageKind::Component),
            PackageType::Component
        );
        assert_eq!(
            PackageType::from(PackageKind::Interface),
            PackageType::Interface
        );
    }
}
