//! SeaORM entity definitions for the wasm-package-manager database.
//!
//! Entities are defined here (rather than in `wasm-package-manager`)
//! during the SeaORM port because that crate cannot depend on `sea-orm`
//! while `rusqlite` is still present (the two crates both link `sqlite3`).
//! After the rusqlite removal in Phase 4, these entities will be re-exported
//! (or moved) so the package manager can use them directly.

#![allow(missing_docs)]

pub mod component_target;
pub mod fetch_queue;
pub mod oci_layer;
pub mod oci_layer_annotation;
pub mod oci_manifest;
pub mod oci_manifest_annotation;
pub mod oci_referrer;
pub mod oci_repository;
pub mod oci_tag;
pub mod sync_meta;
pub mod wasm_component;
pub mod wit_package;
pub mod wit_package_dependency;
pub mod wit_world;
pub mod wit_world_export;
pub mod wit_world_import;
