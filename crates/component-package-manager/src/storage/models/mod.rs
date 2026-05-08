//! Internal storage shims kept for backwards-compatible exports during the
//! SeaORM port.
//!
//! The legacy rusqlite-backed `RawKnownPackage` and migration runner were
//! deleted. Migrations now live in the
//! [`component_package_manager_migration`] crate; known-package operations
//! are private methods on [`super::store::Store`] backed by SeaORM entities.

use sea_orm::{DatabaseConnection, FromQueryResult, Statement};

use component_package_manager_migration::MigratorTrait;

/// Information about the current migration state.
///
/// # Example
///
/// ```
/// use component_package_manager::storage::Migrations;
///
/// let migrations = Migrations { current: 2, total: 3 };
/// assert_eq!(migrations.current, 2);
/// assert_eq!(migrations.total, 3);
/// ```
#[derive(Debug, Clone, Default)]
pub struct Migrations {
    /// Number of migrations applied to the database.
    pub current: u32,
    /// Total number of migrations defined in the migrator.
    pub total: u32,
}

impl Migrations {
    /// Total number of migrations defined.
    pub(crate) fn total_count() -> u32 {
        u32::try_from(component_package_manager_migration::Migrator::migrations().len())
            .unwrap_or(u32::MAX)
    }

    /// Number of migrations that have been applied to `db`.
    ///
    /// Reads from SeaORM's `seaql_migrations` bookkeeping table.
    pub(crate) async fn current_count(db: &DatabaseConnection) -> u32 {
        #[derive(FromQueryResult)]
        struct Row {
            count: i64,
        }
        let backend = db.get_database_backend();
        // The seaql_migrations table is created by the migrator on its first
        // run, so this query may legitimately fail until then; treat that as
        // "0 migrations applied".
        let stmt =
            Statement::from_string(backend, "SELECT COUNT(*) AS count FROM seaql_migrations");
        match Row::find_by_statement(stmt).one(db).await {
            Ok(Some(row)) => u32::try_from(row.count).unwrap_or(0),
            _ => 0,
        }
    }

    /// Snapshot of the current migration state.
    pub(crate) async fn snapshot(db: &DatabaseConnection) -> Self {
        Self {
            current: Self::current_count(db).await,
            total: Self::total_count(),
        }
    }
}
