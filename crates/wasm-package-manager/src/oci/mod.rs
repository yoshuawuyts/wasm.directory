//! OCI-specific types and logic.
//!
//! Pure types and logic for OCI registry interactions. DB-backed wrapper
//! structs (`OciRepository`, `OciManifest`, etc.) were folded into the
//! SeaORM-based `storage::Store` during the SeaORM port. Use the entity
//! types from `wasm_package_manager_migration::entities` directly when
//! a typed row is needed.

mod client;
mod errors;
mod image_entry;
mod logic;
mod raw;

pub(crate) use client::Client;
pub use errors::OciLayerError;
pub use image_entry::ImageEntry;
pub use logic::{
    TagKind, classify_tag, classify_tags, compute_orphaned_layers, filter_wasm_layers,
    validate_single_wasm_layer,
};
pub(crate) use raw::RawImageEntry;

/// Result of an insert operation.
///
/// # Example
///
/// ```
/// use wasm_package_manager::oci::InsertResult;
///
/// let result = InsertResult::Inserted;
/// assert_eq!(result, InsertResult::Inserted);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertResult {
    /// The entry was inserted successfully.
    Inserted,
    /// The entry already existed in the database.
    AlreadyExists,
}
