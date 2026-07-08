//! Internal helper for emitting per-backend `updated_at` triggers.
//!
//! SeaORM's schema builder doesn't model triggers; we drop down to raw SQL
//! and dispatch on the active backend. This is the only place per-backend
//! SQL fragments live.

use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend};

/// Install an "advance `updated_at` on row update" trigger for the given table.
///
/// On Postgres this also installs a shared `set_updated_at()` PL/pgSQL
/// function on first call (idempotent via `CREATE OR REPLACE`).
pub(crate) async fn install_updated_at_trigger(
    manager: &SchemaManager<'_>,
    table: &str,
) -> Result<(), DbErr> {
    let backend = manager.get_database_backend();
    let conn = manager.get_connection();
    let sql: String = match backend {
        DatabaseBackend::Sqlite => format!(
            "CREATE TRIGGER IF NOT EXISTS trg_{table}_updated_at \
             AFTER UPDATE ON {table} \
             FOR EACH ROW \
             WHEN OLD.updated_at = NEW.updated_at \
             BEGIN \
               UPDATE {table} SET updated_at = CURRENT_TIMESTAMP WHERE id = OLD.id; \
             END;"
        ),
        DatabaseBackend::Postgres => {
            let func = "CREATE OR REPLACE FUNCTION set_updated_at() \
                        RETURNS trigger AS $$ \
                        BEGIN \
                          NEW.updated_at = now(); \
                          RETURN NEW; \
                        END; \
                        $$ LANGUAGE plpgsql;";
            conn.execute_unprepared(func).await?;
            format!(
                "CREATE TRIGGER trg_{table}_updated_at \
                 BEFORE UPDATE ON {table} \
                 FOR EACH ROW EXECUTE FUNCTION set_updated_at();"
            )
        }
        DatabaseBackend::MySql => {
            return Err(DbErr::Custom(
                "MySQL backend is not supported by wasm-package-manager".into(),
            ));
        }
        _ => {
            return Err(DbErr::Custom(format!(
                "unsupported database backend: {backend:?}"
            )));
        }
    };
    conn.execute_unprepared(&sql).await?;
    Ok(())
}

/// Drop the trigger installed by [`install_updated_at_trigger`]. Used in `down()`.
pub(crate) async fn drop_updated_at_trigger(
    manager: &SchemaManager<'_>,
    table: &str,
) -> Result<(), DbErr> {
    let backend = manager.get_database_backend();
    let conn = manager.get_connection();
    let sql = match backend {
        DatabaseBackend::Sqlite => format!("DROP TRIGGER IF EXISTS trg_{table}_updated_at;"),
        DatabaseBackend::Postgres => {
            format!("DROP TRIGGER IF EXISTS trg_{table}_updated_at ON {table};")
        }
        _ => return Ok(()),
    };
    conn.execute_unprepared(&sql).await?;
    Ok(())
}
