//! SeaORM migrations and entity definitions for the
//! `wasm-package-manager` database.
//!
//! Migrations are authored as Rust modules and applied in the order they are
//! registered in [`Migrator::migrations`]. Both SQLite and PostgreSQL are
//! supported; per-backend SQL fragments (e.g. trigger bodies) dispatch on
//! [`SchemaManager::get_database_backend`].

#![allow(missing_docs)]

pub use sea_orm_migration::prelude::*;

pub mod entities;
pub mod migrations;

#[derive(Debug)]
pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(migrations::m20260101_000001_create_oci_tables::Migration),
            Box::new(migrations::m20260101_000002_create_wit_tables::Migration),
            Box::new(migrations::m20260101_000003_create_wasm_tables::Migration),
            Box::new(migrations::m20260101_000004_create_fetch_queue::Migration),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm_migration::sea_orm::Database;

    #[tokio::test]
    async fn migrations_apply_to_sqlite_in_memory() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("connect in-memory sqlite");
        Migrator::up(&db, None).await.expect("migrations up");
        Migrator::down(&db, None).await.expect("migrations down");
        Migrator::up(&db, None).await.expect("migrations re-up");
    }
}
