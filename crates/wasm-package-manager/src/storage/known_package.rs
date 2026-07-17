//! Public types for the known-package surface.
//!
//! Backed by direct SeaORM queries on the `oci_repository` entity in
//! [`super::store::Store`] (no separate `Raw*` wrapper struct in this port).

use wasm_meta_registry_types::PackageKind;

// Re-export the canonical `KnownPackage` from the types crate so that
// existing consumers (`wasm_package_manager::storage::KnownPackage`)
// keep working without any source changes.
pub use wasm_meta_registry_types::KnownPackage;

/// Parameters for upserting a known package entry.
#[derive(Debug, Clone)]
pub struct KnownPackageParams<'a> {
    /// OCI registry hostname (e.g. `ghcr.io`).
    pub registry: &'a str,
    /// OCI repository path (e.g. `example/my-component`).
    pub repository: &'a str,
    /// Optional tag to associate with this package.
    pub tag: Option<&'a str>,
    /// Human-readable description from OCI annotations.
    pub description: Option<&'a str>,
    /// WIT namespace (e.g. `wasi` in `wasi:http`).
    pub wit_namespace: Option<&'a str>,
    /// WIT package name (e.g. `http` in `wasi:http`).
    pub wit_name: Option<&'a str>,
    /// Whether this package is a component or interface.
    pub kind: Option<PackageKind>,
}
