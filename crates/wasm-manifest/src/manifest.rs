//! Types for the WASM manifest file (`wasm.toml`).

use crate::permissions::RunPermissions;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The type of a WASM package.
///
/// # Example
///
/// ```rust
/// use wasm_manifest::PackageType;
///
/// let component = PackageType::Component;
/// let interface = PackageType::Interface;
/// assert_ne!(component, interface);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[must_use]
pub enum PackageType {
    /// A compiled WebAssembly component.
    Component,
    /// A WIT interface definition.
    Interface,
}

/// The root manifest structure for a WASM package.
///
/// The manifest file (`wasm.toml`) defines dependencies for a WASM package,
/// grouped under `[dependencies.components]` and `[dependencies.interfaces]`.
///
/// # Example
///
/// ```rust
/// use wasm_manifest::Manifest;
///
/// let toml = r#"
/// [dependencies.components]
/// "root:component" = "0.1.0"
///
/// [dependencies.interfaces]
/// "wasi:clocks" = "0.2.5"
/// "#;
///
/// let manifest: Manifest = toml::from_str(toml).unwrap();
/// assert_eq!(manifest.dependencies.components.len(), 1);
/// assert_eq!(manifest.dependencies.interfaces.len(), 1);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[must_use]
pub struct Manifest {
    /// Optional `[package]` section with publish metadata for this manifest's
    /// single artifact (component or WIT interface).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package: Option<crate::package::Package>,
    /// All dependency sections of the manifest.
    #[serde(default)]
    pub dependencies: Dependencies,
}

/// Container for all dependency sections in the manifest.
///
/// Groups component and interface dependencies under a single
/// `[dependencies]` table in the TOML manifest.
///
/// # Example
///
/// ```rust
/// use wasm_manifest::Dependencies;
///
/// let toml = r#"
/// [components]
/// "root:component" = "1.0.0"
///
/// [interfaces]
/// "wasi:logging" = "1.0.0"
/// "#;
///
/// let deps: Dependencies = toml::from_str(toml).unwrap();
/// assert_eq!(deps.components.len(), 1);
/// assert_eq!(deps.interfaces.len(), 1);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[must_use]
pub struct Dependencies {
    /// Component dependencies.
    #[serde(default)]
    pub components: HashMap<String, Dependency>,
    /// Interface/type dependencies.
    #[serde(default)]
    pub interfaces: HashMap<String, Dependency>,
}

impl Dependencies {
    /// Iterate over all dependencies with their package type.
    ///
    /// # Example
    ///
    /// ```rust
    /// use wasm_manifest::{Dependencies, Dependency, PackageType};
    /// use std::collections::HashMap;
    ///
    /// let mut components = HashMap::new();
    /// components.insert(
    ///     "root:component".to_string(),
    ///     Dependency::Compact("0.1.0".to_string()),
    /// );
    /// let deps = Dependencies { components, ..Default::default() };
    /// let all: Vec<_> = deps.all_dependencies().collect();
    /// assert_eq!(all.len(), 1);
    /// assert!(all.iter().any(|(_, _, pt)| *pt == PackageType::Component));
    /// ```
    pub fn all_dependencies(&self) -> impl Iterator<Item = (&String, &Dependency, PackageType)> {
        self.components
            .iter()
            .map(|(k, v)| (k, v, PackageType::Component))
            .chain(
                self.interfaces
                    .iter()
                    .map(|(k, v)| (k, v, PackageType::Interface)),
            )
    }
}

impl Manifest {
    /// Iterate over all dependencies with their package type.
    ///
    /// Delegates to [`Dependencies::all_dependencies`].
    ///
    /// # Example
    ///
    /// ```rust
    /// use wasm_manifest::{Manifest, PackageType};
    ///
    /// let toml = r#"
    /// [dependencies.components]
    /// "root:component" = "0.1.0"
    ///
    /// [dependencies.interfaces]
    /// "wasi:logging" = "1.0.0"
    /// "#;
    ///
    /// let manifest: Manifest = toml::from_str(toml).unwrap();
    /// let all: Vec<_> = manifest.all_dependencies().collect();
    /// assert_eq!(all.len(), 2);
    /// assert!(all.iter().any(|(_, _, pt)| *pt == PackageType::Component));
    /// assert!(all.iter().any(|(_, _, pt)| *pt == PackageType::Interface));
    /// ```
    pub fn all_dependencies(&self) -> impl Iterator<Item = (&String, &Dependency, PackageType)> {
        self.dependencies.all_dependencies()
    }
}

/// A dependency specification in the manifest.
///
/// Dependencies can be specified in two formats:
///
/// 1. Compact format (version string):
///    ```toml
///    [dependencies.interfaces]
///    "wasi:logging" = "1.0.0"
///    ```
///
///    Bare versions follow Cargo-style semantics: `"1.0.0"` means `^1.0.0`
///    (>=1.0.0, <2.0.0). Explicit operators are also supported:
///    `">=1.0, <2.0"`, `"~1.2"`, `"=1.2.3"`, `"*"`.
///
/// 2. Explicit format (table):
///    ```toml
///    [dependencies.interfaces."wasi:logging"]
///    registry = "ghcr.io"
///    namespace = "webassembly"
///    package = "wasi-logging"
///    version = "1.0.0"
///    ```
///
/// # Example
///
/// ```rust
/// use wasm_manifest::{Manifest, Dependency};
///
/// let toml = r#"
/// [dependencies.interfaces]
/// "wasi:logging" = "1.0.0"
///
/// [dependencies.interfaces."wasi:key-value"]
/// registry = "ghcr.io"
/// namespace = "webassembly"
/// package = "wasi-key-value"
/// version = "2.0.0"
/// "#;
///
/// let manifest: Manifest = toml::from_str(toml).unwrap();
///
/// assert!(matches!(
///     &manifest.dependencies.interfaces["wasi:logging"],
///     Dependency::Compact(_)
/// ));
/// assert!(matches!(
///     &manifest.dependencies.interfaces["wasi:key-value"],
///     Dependency::Explicit { .. }
/// ));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
#[must_use]
pub enum Dependency {
    /// Compact format: a version string or constraint.
    ///
    /// Bare versions use Cargo-style semver: `"1.0.0"` means `^1.0.0`.
    /// Explicit operators are supported: `">=1.0, <2.0"`, `"~1.2"`, `"=1.2.3"`, `"*"`.
    /// Special values `""` and `"latest"` skip semver validation.
    ///
    /// # Example
    /// ```text
    /// "1.0.0"
    /// ```
    Compact(String),

    /// Explicit format: a table with individual fields.
    Explicit {
        /// The registry host (e.g., "ghcr.io").
        registry: String,
        /// The namespace or organization (e.g., "webassembly").
        namespace: String,
        /// The package name (e.g., "wasi-logging").
        package: String,
        /// The package version or version constraint (e.g., "1.0.0", ">=1.0, <2.0").
        /// Bare versions use Cargo-style semver: `"1.0.0"` means `^1.0.0`.
        version: String,
        /// Optional sandbox permissions for running this component.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        permissions: Option<RunPermissions>,
    },
}

impl Dependency {
    /// Return the version string from either variant.
    ///
    /// # Example
    ///
    /// ```rust
    /// use wasm_manifest::Dependency;
    ///
    /// let compact = Dependency::Compact("1.0.0".to_string());
    /// assert_eq!(compact.version(), "1.0.0");
    ///
    /// let explicit = Dependency::Explicit {
    ///     registry: "ghcr.io".to_string(),
    ///     namespace: "webassembly".to_string(),
    ///     package: "wasi-logging".to_string(),
    ///     version: "2.0.0".to_string(),
    ///     permissions: None,
    /// };
    /// assert_eq!(explicit.version(), "2.0.0");
    /// ```
    #[must_use]
    pub fn version(&self) -> &str {
        match self {
            Dependency::Compact(v) => v,
            Dependency::Explicit { version, .. } => version,
        }
    }

    /// Parse the version string as a [`semver::VersionReq`].
    ///
    /// Bare versions use Cargo-style semantics: `"1.0.0"` is treated as
    /// `^1.0.0` (>=1.0.0, <2.0.0). Explicit operators like `">=1.0, <2.0"`,
    /// `"~1.2"`, `"=1.2.3"`, and `"*"` are also supported.
    ///
    /// Special values `""` and `"latest"` return a wildcard requirement
    /// that matches any version.
    ///
    /// # Errors
    ///
    /// Returns an error if the version string cannot be parsed as a valid
    /// semver requirement.
    ///
    /// # Example
    ///
    /// ```rust
    /// use wasm_manifest::Dependency;
    ///
    /// let dep = Dependency::Compact("1.0.0".to_string());
    /// let req = dep.parse_version_req().unwrap();
    /// assert!(req.matches(&semver::Version::new(1, 2, 0)));
    /// assert!(!req.matches(&semver::Version::new(2, 0, 0)));
    /// ```
    pub fn parse_version_req(&self) -> Result<semver::VersionReq, semver::Error> {
        let v = self.version();
        if v.is_empty() || v == "latest" {
            return Ok(semver::VersionReq::STAR);
        }
        semver::VersionReq::parse(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // r[verify manifest.parse.compact]
    #[test]
    fn test_parse_compact_format() {
        let toml = r#"
            [dependencies.interfaces]
            "wasi:logging" = "1.0.0"
            "wasi:key-value" = "2.0.0"
        "#;

        let manifest: Manifest = toml::from_str(toml).expect("Failed to parse manifest");

        assert_eq!(manifest.dependencies.interfaces.len(), 2);
        assert!(
            manifest
                .dependencies
                .interfaces
                .contains_key("wasi:logging")
        );
        assert!(
            manifest
                .dependencies
                .interfaces
                .contains_key("wasi:key-value")
        );

        match &manifest.dependencies.interfaces["wasi:logging"] {
            Dependency::Compact(s) => {
                assert_eq!(s, "1.0.0");
            }
            _ => panic!("Expected compact format"),
        }
    }

    // r[verify manifest.parse.explicit]
    #[test]
    fn test_parse_explicit_format() {
        let toml = r#"
            [dependencies.interfaces."wasi:logging"]
            registry = "ghcr.io"
            namespace = "webassembly"
            package = "wasi-logging"
            version = "1.0.0"

            [dependencies.interfaces."wasi:key-value"]
            registry = "ghcr.io"
            namespace = "webassembly"
            package = "wasi-key-value"
            version = "2.0.0"
        "#;

        let manifest: Manifest = toml::from_str(toml).expect("Failed to parse manifest");

        assert_eq!(manifest.dependencies.interfaces.len(), 2);

        match &manifest.dependencies.interfaces["wasi:logging"] {
            Dependency::Explicit {
                registry,
                namespace,
                package,
                version,
                ..
            } => {
                assert_eq!(registry, "ghcr.io");
                assert_eq!(namespace, "webassembly");
                assert_eq!(package, "wasi-logging");
                assert_eq!(version, "1.0.0");
            }
            _ => panic!("Expected explicit format"),
        }
    }

    // r[verify manifest.serialize.compact]
    #[test]
    fn test_serialize_compact_format() {
        let mut interfaces = HashMap::new();
        interfaces.insert(
            "wasi:logging".to_string(),
            Dependency::Compact("1.0.0".to_string()),
        );

        let manifest = Manifest {
            package: None,
            dependencies: Dependencies {
                interfaces,
                ..Default::default()
            },
        };
        let toml = toml::to_string(&manifest).expect("Failed to serialize manifest");

        assert!(toml.contains("wasi:logging"));
        assert!(toml.contains("1.0.0"));
    }

    // r[verify manifest.serialize.explicit]
    #[test]
    fn test_serialize_explicit_format() {
        let mut interfaces = HashMap::new();
        interfaces.insert(
            "wasi:logging".to_string(),
            Dependency::Explicit {
                registry: "ghcr.io".to_string(),
                namespace: "webassembly".to_string(),
                package: "wasi-logging".to_string(),
                version: "1.0.0".to_string(),
                permissions: None,
            },
        );

        let manifest = Manifest {
            package: None,
            dependencies: Dependencies {
                interfaces,
                ..Default::default()
            },
        };
        let toml = toml::to_string(&manifest).expect("Failed to serialize manifest");

        assert!(toml.contains("wasi:logging"));
        assert!(toml.contains("registry"));
        assert!(toml.contains("ghcr.io"));
    }

    // r[verify manifest.parse.empty]
    #[test]
    fn test_empty_manifest() {
        let toml = r#""#;
        let manifest: Manifest = toml::from_str(toml).expect("Failed to parse empty manifest");
        assert_eq!(manifest.dependencies.components.len(), 0);
        assert_eq!(manifest.dependencies.interfaces.len(), 0);
    }

    // r[verify manifest.parse.mixed]
    #[test]
    fn test_parse_components_and_interfaces() {
        let toml = r#"
            [dependencies.components]
            "root:component" = "0.1.0"

            [dependencies.interfaces]
            "wasi:clocks" = "0.2.5"
        "#;

        let manifest: Manifest = toml::from_str(toml).expect("Failed to parse manifest");

        assert_eq!(manifest.dependencies.components.len(), 1);
        assert_eq!(manifest.dependencies.interfaces.len(), 1);
        assert!(
            manifest
                .dependencies
                .components
                .contains_key("root:component")
        );
        assert!(manifest.dependencies.interfaces.contains_key("wasi:clocks"));
    }

    // r[verify manifest.parse.all-dependencies]
    #[test]
    fn test_all_dependencies() {
        let mut components = HashMap::new();
        components.insert(
            "root:component".to_string(),
            Dependency::Compact("0.1.0".to_string()),
        );
        let mut interfaces = HashMap::new();
        interfaces.insert(
            "wasi:logging".to_string(),
            Dependency::Compact("1.0.0".to_string()),
        );

        let manifest = Manifest {
            package: None,
            dependencies: Dependencies {
                components,
                interfaces,
            },
        };

        let all: Vec<_> = manifest.all_dependencies().collect();
        assert_eq!(all.len(), 2);

        let has_component = all.iter().any(|(_, _, pt)| *pt == PackageType::Component);
        let has_interface = all.iter().any(|(_, _, pt)| *pt == PackageType::Interface);
        assert!(has_component);
        assert!(has_interface);
    }

    // r[verify manifest.parse.permissions]
    #[test]
    fn test_parse_explicit_with_permissions() {
        let toml = r#"
            [dependencies.components."root:component"]
            registry = "ghcr.io"
            namespace = "yoshuawuyts"
            package = "fetch"
            version = "latest"
            permissions.inherit-env = true
            permissions.allow-dirs = ["/data", "./output"]
        "#;

        let manifest: Manifest = toml::from_str(toml).expect("Failed to parse manifest");

        match &manifest.dependencies.components["root:component"] {
            Dependency::Explicit {
                registry,
                permissions,
                ..
            } => {
                assert_eq!(registry, "ghcr.io");
                let perms = permissions.as_ref().expect("Expected permissions");
                assert_eq!(perms.inherit_env, Some(true));
                assert_eq!(
                    perms.allow_dirs,
                    Some(vec![
                        std::path::PathBuf::from("/data"),
                        std::path::PathBuf::from("./output"),
                    ])
                );
            }
            _ => panic!("Expected explicit format"),
        }
    }

    // r[verify manifest.parse.no-permissions]
    #[test]
    fn test_explicit_without_permissions_still_works() {
        let toml = r#"
            [dependencies.components."root:component"]
            registry = "ghcr.io"
            namespace = "yoshuawuyts"
            package = "fetch"
            version = "latest"
        "#;

        let manifest: Manifest = toml::from_str(toml).expect("Failed to parse manifest");

        match &manifest.dependencies.components["root:component"] {
            Dependency::Explicit { permissions, .. } => {
                assert!(permissions.is_none());
            }
            _ => panic!("Expected explicit format"),
        }
    }

    // r[verify manifest.version.semver-default]
    #[test]
    fn test_bare_version_treated_as_caret() {
        let dep = Dependency::Compact("1.0.0".to_string());
        let req = dep.parse_version_req().unwrap();
        // "1.0.0" → ^1.0.0 → >=1.0.0, <2.0.0
        assert!(req.matches(&semver::Version::new(1, 0, 0)));
        assert!(req.matches(&semver::Version::new(1, 2, 0)));
        assert!(!req.matches(&semver::Version::new(2, 0, 0)));
    }

    // r[verify manifest.version.semver-pre-1]
    #[test]
    fn test_pre_1_version_narrow_range() {
        let dep = Dependency::Compact("0.2.3".to_string());
        let req = dep.parse_version_req().unwrap();
        // "0.2.3" → ^0.2.3 → >=0.2.3, <0.3.0
        assert!(req.matches(&semver::Version::new(0, 2, 3)));
        assert!(req.matches(&semver::Version::new(0, 2, 9)));
        assert!(!req.matches(&semver::Version::new(0, 3, 0)));
    }

    // r[verify manifest.version.explicit-operators]
    #[test]
    fn test_explicit_version_operators() {
        // Tilde requirement
        let dep = Dependency::Compact("~1.2".to_string());
        let req = dep.parse_version_req().unwrap();
        assert!(req.matches(&semver::Version::new(1, 2, 0)));
        assert!(req.matches(&semver::Version::new(1, 2, 9)));
        assert!(!req.matches(&semver::Version::new(1, 3, 0)));

        // Exact pin
        let dep = Dependency::Compact("=1.2.3".to_string());
        let req = dep.parse_version_req().unwrap();
        assert!(req.matches(&semver::Version::new(1, 2, 3)));
        assert!(!req.matches(&semver::Version::new(1, 2, 4)));

        // Wildcard
        let dep = Dependency::Compact("*".to_string());
        let req = dep.parse_version_req().unwrap();
        assert!(req.matches(&semver::Version::new(0, 0, 1)));
        assert!(req.matches(&semver::Version::new(99, 99, 99)));

        // Range
        let dep = Dependency::Compact(">=1.0, <2.0".to_string());
        let req = dep.parse_version_req().unwrap();
        assert!(req.matches(&semver::Version::new(1, 5, 0)));
        assert!(!req.matches(&semver::Version::new(2, 0, 0)));
    }

    // r[verify manifest.version.special-values]
    #[test]
    fn test_special_version_values() {
        // Empty string → wildcard
        let dep = Dependency::Compact(String::new());
        let req = dep.parse_version_req().unwrap();
        assert!(req.matches(&semver::Version::new(1, 0, 0)));

        // "latest" → wildcard
        let dep = Dependency::Compact("latest".to_string());
        let req = dep.parse_version_req().unwrap();
        assert!(req.matches(&semver::Version::new(1, 0, 0)));
    }

    // r[verify manifest.version.invalid]
    #[test]
    fn test_invalid_version_string() {
        let dep = Dependency::Compact("not-a-version".to_string());
        assert!(dep.parse_version_req().is_err());
    }

    // r[verify manifest.dependency.version-accessor]
    #[test]
    fn test_version_accessor() {
        let compact = Dependency::Compact("1.0.0".to_string());
        assert_eq!(compact.version(), "1.0.0");

        let explicit = Dependency::Explicit {
            registry: "ghcr.io".to_string(),
            namespace: "webassembly".to_string(),
            package: "wasi-logging".to_string(),
            version: "2.0.0".to_string(),
            permissions: None,
        };
        assert_eq!(explicit.version(), "2.0.0");
    }
}
