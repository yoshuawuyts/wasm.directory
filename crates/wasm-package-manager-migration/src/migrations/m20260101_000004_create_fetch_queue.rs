//! Migration that creates the `fetch_queue` table.

use crate::entities::fetch_queue;
use crate::migrations::triggers::{drop_updated_at_trigger, install_updated_at_trigger};
use sea_orm_migration::prelude::*;

#[derive(Debug, DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(fetch_queue::Entity)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(fetch_queue::Column::Id)
                            .big_integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(
                        ColumnDef::new(fetch_queue::Column::Registry)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(fetch_queue::Column::Repository)
                            .text()
                            .not_null(),
                    )
                    .col(ColumnDef::new(fetch_queue::Column::Tag).text().not_null())
                    .col(
                        ColumnDef::new(fetch_queue::Column::Task)
                            .text()
                            .not_null()
                            .default("pull")
                            .check(Expr::col(fetch_queue::Column::Task).is_in(["pull", "reindex"])),
                    )
                    .col(
                        ColumnDef::new(fetch_queue::Column::Status)
                            .text()
                            .not_null()
                            .default("pending")
                            .check(Expr::col(fetch_queue::Column::Status).is_in([
                                "pending",
                                "in_progress",
                                "completed",
                                "failed",
                            ])),
                    )
                    .col(
                        ColumnDef::new(fetch_queue::Column::Priority)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(fetch_queue::Column::Attempts)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(fetch_queue::Column::MaxAttempts)
                            .integer()
                            .not_null()
                            .default(3),
                    )
                    .col(ColumnDef::new(fetch_queue::Column::LastError).text())
                    .col(
                        ColumnDef::new(fetch_queue::Column::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(fetch_queue::Column::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("uq_fetch_queue_item")
                    .table(fetch_queue::Entity)
                    .col(fetch_queue::Column::Registry)
                    .col(fetch_queue::Column::Repository)
                    .col(fetch_queue::Column::Tag)
                    .col(fetch_queue::Column::Task)
                    .unique()
                    .to_owned(),
            )
            .await?;
        // Partial index on pending rows (status = 'pending'). SeaORM's index
        // builder doesn't expose `WHERE`; this is a small backend-divergent
        // optimization where we hand-write SQL.
        let backend = manager.get_database_backend();
        let conn = manager.get_connection();
        let partial_idx = match backend {
            sea_orm::DatabaseBackend::Sqlite | sea_orm::DatabaseBackend::Postgres => {
                "CREATE INDEX IF NOT EXISTS idx_fetch_queue_pending \
                 ON fetch_queue(status, priority, created_at) \
                 WHERE status = 'pending';"
            }
            _ => "",
        };
        if !partial_idx.is_empty() {
            use sea_orm_migration::sea_orm::ConnectionTrait;
            conn.execute_unprepared(partial_idx).await?;
        }
        install_updated_at_trigger(manager, "fetch_queue").await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        drop_updated_at_trigger(manager, "fetch_queue").await?;
        manager
            .drop_table(
                Table::drop()
                    .table(fetch_queue::Entity)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}
