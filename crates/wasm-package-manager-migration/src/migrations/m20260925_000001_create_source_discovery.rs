//! Record discovery independently of repository rows, authentication and readiness.

use crate::entities::source_discovery;
use sea_orm_migration::prelude::*;

#[derive(Debug, DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(source_discovery::Entity)
                    .col(
                        ColumnDef::new(source_discovery::Column::Registry)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(source_discovery::Column::Repository)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(source_discovery::Column::LastCompletedAt)
                            .timestamp_with_time_zone(),
                    )
                    .col(
                        ColumnDef::new(source_discovery::Column::NextRetryAt)
                            .timestamp_with_time_zone(),
                    )
                    .col(
                        ColumnDef::new(source_discovery::Column::FailureCount)
                            .big_integer()
                            .not_null()
                            .default(0)
                            .check(
                                Expr::col(source_discovery::Column::FailureCount)
                                    .between(0_i64, i64::from(u32::MAX)),
                            ),
                    )
                    .col(ColumnDef::new(source_discovery::Column::LastError).text())
                    .primary_key(
                        Index::create()
                            .col(source_discovery::Column::Registry)
                            .col(source_discovery::Column::Repository),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(source_discovery::Entity).to_owned())
            .await
    }
}
