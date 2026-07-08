//! Cross-cutting persistence types and database storage.

mod config;
mod db_config;
mod known_package;
mod models;
mod store;

pub use config::StateInfo;
pub use db_config::{Backend, DbConfig, redact_url};
pub use known_package::{KnownPackage, KnownPackageParams};
pub use models::Migrations;
pub(crate) use store::Store;
pub use store::{FetchTask, FetchTaskKind};
pub use wasm_meta_registry_types::PackageDependencyRef;
