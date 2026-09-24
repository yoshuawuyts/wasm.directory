//! Migrations module aggregating all schema migrations in declared order.

pub(crate) mod triggers;

pub mod m20260101_000001_create_oci_tables;
pub mod m20260101_000002_create_wit_tables;
pub mod m20260101_000003_create_wasm_tables;
pub mod m20260101_000004_create_fetch_queue;
pub mod m20260924_000001_add_fetch_queue_claim;
pub mod m20260924_000002_add_manifest_config_created;
