//! Manifest and lockfile format types for WebAssembly packages.
//!
//! This crate provides types for parsing and serializing WASM package manifests
//! (`wasm.toml`) and lockfiles (`wasm.lock`).
//!
//! # Example: Parsing a Manifest
//!
//! ```rust
//! use wasm_manifest::Manifest;
//!
//! let toml = r#"
//! [dependencies.interfaces]
//! "wasi:logging" = "1.0.0"
//! "#;
//!
//! let manifest: Manifest = toml::from_str(toml).unwrap();
//! ```
//!
//! # Example: Parsing a Lockfile
//!
//! ```rust
//! use wasm_manifest::Lockfile;
//!
//! let toml = r#"
//! lockfile_version = 3
//!
//! [[interfaces]]
//! name = "wasi:logging"
//! version = "1.0.0"
//! registry = "ghcr.io/webassembly/wasi-logging"
//! digest = "sha256:abc123"
//! "#;
//!
//! let lockfile: Lockfile = toml::from_str(toml).unwrap();
//! ```

#![deny(unsafe_code)]
#![deny(missing_debug_implementations)]
#![warn(missing_docs)]

mod lockfile;
mod manifest;
mod package;
mod permissions;
mod validation;

pub use lockfile::{LOCKFILE_VERSION, LockedPackage, Lockfile, PackageDependency};
pub use manifest::{Dependencies, Dependency, Manifest, PackageType};
pub use package::{Package, PackageError, PackageKind};
pub use permissions::{ResolvedPermissions, RunPermissions};
pub use validation::{ValidationError, validate};
