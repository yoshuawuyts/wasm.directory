//! Cross-cutting persistence types and database storage.

mod config;
mod db_config;
mod indexer_lease;
mod known_package;
mod models;
mod store;

pub use config::StateInfo;
pub use db_config::{Backend, DbConfig, redact_url};
pub use indexer_lease::IndexerLease;
pub use known_package::{KnownPackage, KnownPackageParams};
pub use models::Migrations;
pub use store::{FetchTask, FetchTaskKind};
pub(crate) use store::{KnownTags, Store};
pub use wasm_meta_registry_types::PackageDependencyRef;
