//! WIT packager — produces a WIT-only WebAssembly binary from a WIT
//! source directory, stamping the manifest version into top-level
//! package decls.
//!
//! This is the equivalent of `wkg wit build`: walk the WIT directory,
//! reject any file that already contains a `@version` annotation on its
//! `package` decl (the manifest is the single source of truth), then
//! parse the directory with [`wit_parser::Resolve`], stamp the manifest
//! version onto every top-level package, and encode the main package as
//! a WIT WASM via [`wit_component::encode`].

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use wit_parser::{PackageName, Resolve, UnresolvedPackageGroup};

/// The output of a successful WIT packaging operation.
#[derive(Debug, Clone)]
pub struct WitPackaged {
    /// The encoded WIT WebAssembly bytes.
    pub bytes: Vec<u8>,
    /// The fully-qualified package name including the stamped version
    /// (e.g. `wasi:logging@0.1.0`).
    pub package_name: String,
}

/// Errors produced by the WIT packager.
#[derive(Debug)]
pub enum WitPackagerError {
    /// A WIT file already contains an `@version` declaration on its
    /// `package` decl. The manifest's `[package].version` must be the
    /// single source of truth.
    PreexistingVersion {
        /// Path of the WIT directory that contained the offending file.
        path: PathBuf,
        /// The package name that carried the version.
        package: String,
        /// The pre-existing version string.
        version: String,
    },
    /// The manifest version is not a valid semver.
    InvalidVersion {
        /// The version string from the manifest.
        version: String,
        /// The underlying parse error.
        reason: String,
    },
    /// The WIT directory could not be parsed.
    Parse {
        /// The directory we tried to parse.
        path: PathBuf,
        /// The underlying error.
        source: anyhow::Error,
    },
    /// Encoding the WIT package as WASM failed.
    Encode(anyhow::Error),
}

impl std::fmt::Display for WitPackagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WitPackagerError::PreexistingVersion {
                path,
                package,
                version,
            } => write!(
                f,
                "WIT package decl `package {package}@{version}` in `{}` already carries an `@version`; \
                remove it — the [package].version in wasm.toml is the single source of truth",
                path.display()
            ),
            WitPackagerError::InvalidVersion { version, reason } => {
                write!(
                    f,
                    "manifest version '{version}' is not valid semver: {reason}"
                )
            }
            WitPackagerError::Parse { path, source } => write!(
                f,
                "failed to parse WIT directory `{}`: {source}",
                path.display()
            ),
            WitPackagerError::Encode(e) => write!(f, "failed to encode WIT package as wasm: {e}"),
        }
    }
}

impl std::error::Error for WitPackagerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            WitPackagerError::Parse { source, .. } | WitPackagerError::Encode(source) => {
                Some(source.as_ref())
            }
            _ => None,
        }
    }
}

/// Build a WIT-only WebAssembly binary from `wit_dir`, stamping `version`
/// onto top-level package declarations that don't already carry one.
///
/// Only packages whose `(namespace, name)` matches a top-level package
/// in `wit_dir` *and* currently have no `@version` are stamped, so
/// dependencies under `wit/deps/` (which are typically already
/// versioned) are left untouched.
///
/// The directory is expected to look like:
/// ```text
/// wit/
///   my-package.wit
///   deps/
///     foo/...
/// ```
///
/// # Errors
///
/// * Returns [`WitPackagerError::PreexistingVersion`] when any top-level
///   WIT file's `package` decl already carries an `@version`.
/// * Returns [`WitPackagerError::InvalidVersion`] when `version` is not
///   valid semver.
/// * Returns [`WitPackagerError::Parse`] when the directory cannot be
///   parsed by `wit_parser`.
/// * Returns [`WitPackagerError::Encode`] when `wit_component::encode`
///   fails.
pub fn build_wit_package(wit_dir: &Path, version: &str) -> Result<WitPackaged, WitPackagerError> {
    // Parse the manifest version up front so we fail fast.
    let parsed_version =
        semver::Version::parse(version).map_err(|e| WitPackagerError::InvalidVersion {
            version: version.to_string(),
            reason: e.to_string(),
        })?;

    // Step 1: parse the top-level WIT dir to inspect its packages and
    // reject any pre-existing `@version` annotations.
    let group =
        UnresolvedPackageGroup::parse_dir(wit_dir).map_err(|e| WitPackagerError::Parse {
            path: wit_dir.to_path_buf(),
            source: e,
        })?;

    reject_versioned_package(&group.main, wit_dir)?;
    for nested in &group.nested {
        reject_versioned_package(nested, wit_dir)?;
    }

    // Step 2: collect the (namespace, name) tuples of every top-level
    // package so we can stamp the version onto matching packages in the
    // resolve.
    let mut top_level: Vec<(String, String)> = Vec::with_capacity(1 + group.nested.len());
    top_level.push((
        group.main.name.namespace.clone(),
        group.main.name.name.clone(),
    ));
    for nested in &group.nested {
        top_level.push((nested.name.namespace.clone(), nested.name.name.clone()));
    }

    // Step 3: build a fully-resolved Resolve including any deps under
    // `wit/deps/`, then stamp the version onto matching packages.
    let mut resolve = Resolve::default();
    let (main_pkg_id, _src_map) =
        resolve
            .push_dir(wit_dir)
            .map_err(|e| WitPackagerError::Parse {
                path: wit_dir.to_path_buf(),
                source: e,
            })?;

    for (_id, pkg) in &mut resolve.packages {
        if pkg.name.version.is_none()
            && top_level
                .iter()
                .any(|(ns, n)| ns == &pkg.name.namespace && n == &pkg.name.name)
        {
            pkg.name = PackageName {
                namespace: pkg.name.namespace.clone(),
                name: pkg.name.name.clone(),
                version: Some(parsed_version.clone()),
            };
        }
    }
    let stamped_main = resolve
        .packages
        .get(main_pkg_id)
        .map(|p| p.name.to_string())
        .ok_or_else(|| WitPackagerError::Encode(anyhow!("main package missing after stamping")))?;

    // Step 4: encode the main package as a WIT-only wasm component.
    let bytes = wit_component::encode(&resolve, main_pkg_id).map_err(WitPackagerError::Encode)?;

    Ok(WitPackaged {
        bytes,
        package_name: stamped_main,
    })
}

/// Returns an error when the package name carries an `@version`.
fn reject_versioned_package(
    pkg: &wit_parser::UnresolvedPackage,
    wit_dir: &Path,
) -> Result<(), WitPackagerError> {
    if let Some(v) = &pkg.name.version {
        return Err(WitPackagerError::PreexistingVersion {
            path: wit_dir.to_path_buf(),
            package: format!("{}:{}", pkg.name.namespace, pkg.name.name),
            version: v.to_string(),
        });
    }
    Ok(())
}

/// Adapter that exposes [`build_wit_package`] over an `anyhow::Result`,
/// for callers that prefer to bubble up via `?`.
///
/// # Errors
///
/// Returns the underlying [`WitPackagerError`] wrapped in [`anyhow::Error`].
pub fn build_wit_package_anyhow(wit_dir: &Path, version: &str) -> Result<WitPackaged> {
    build_wit_package(wit_dir, version)
        .with_context(|| format!("failed to build WIT package from {}", wit_dir.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write(dir: &Path, name: &str, contents: &str) {
        fs::write(dir.join(name), contents).unwrap();
    }

    // r[verify wit-packager.stamps-version]
    #[test]
    fn stamps_manifest_version_onto_top_level_package() {
        let tmp = TempDir::new().unwrap();
        write(
            tmp.path(),
            "iface.wit",
            "package example:hello;\n\
             interface greet {\n\
                 hello: func() -> string;\n\
             }\n",
        );

        let result = build_wit_package(tmp.path(), "1.2.3").expect("build ok");
        assert_eq!(result.package_name, "example:hello@1.2.3");
        assert!(!result.bytes.is_empty());

        // Round-trip: decode and confirm the version got stamped in.
        let decoded = wit_component::decode(&result.bytes).expect("decode");
        match decoded {
            wit_component::DecodedWasm::WitPackage(resolve, pkg_id) => {
                let pkg = &resolve.packages[pkg_id];
                assert_eq!(pkg.name.namespace, "example");
                assert_eq!(pkg.name.name, "hello");
                assert_eq!(
                    pkg.name.version.as_ref().map(ToString::to_string),
                    Some("1.2.3".to_string())
                );
            }
            wit_component::DecodedWasm::Component(_, _) => panic!("expected WIT package"),
        }
    }

    // r[verify wit-packager.rejects-existing-version]
    #[test]
    fn rejects_preexisting_version() {
        let tmp = TempDir::new().unwrap();
        write(
            tmp.path(),
            "iface.wit",
            "package example:hello@0.0.1;\n\
             interface greet {\n\
                 hello: func() -> string;\n\
             }\n",
        );
        let err = build_wit_package(tmp.path(), "1.2.3").expect_err("should reject");
        match err {
            WitPackagerError::PreexistingVersion {
                package, version, ..
            } => {
                assert_eq!(package, "example:hello");
                assert_eq!(version, "0.0.1");
            }
            other => panic!("expected PreexistingVersion, got {other:?}"),
        }
    }

    // r[verify wit-packager.invalid-manifest-version]
    #[test]
    fn rejects_invalid_manifest_version() {
        let tmp = TempDir::new().unwrap();
        write(
            tmp.path(),
            "iface.wit",
            "package example:hello;\n\
             interface greet {\n\
                 hello: func() -> string;\n\
             }\n",
        );
        let err = build_wit_package(tmp.path(), "not-semver").expect_err("invalid");
        assert!(matches!(err, WitPackagerError::InvalidVersion { .. }));
    }

    // r[verify wit-packager.empty-dir]
    #[test]
    fn empty_dir_is_a_parse_error() {
        let tmp = TempDir::new().unwrap();
        let err = build_wit_package(tmp.path(), "1.0.0").expect_err("no packages");
        assert!(matches!(err, WitPackagerError::Parse { .. }));
    }
}
