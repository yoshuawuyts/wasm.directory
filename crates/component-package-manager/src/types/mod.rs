//! WIT package and world types.
//!
//! Pure types and parsing helpers. The DB-backed `Raw*` shims and per-table
//! wrapper structs were folded into the SeaORM-based `storage::Store`.

mod detect;
mod parser;
mod wit_package;

pub use detect::is_wit_package;
pub use parser::DependencyItem;
pub(crate) use parser::WitMetadata;
pub(crate) use parser::extract_wit_metadata;
pub use parser::extract_wit_text;
pub use wit_package::WitPackage;
