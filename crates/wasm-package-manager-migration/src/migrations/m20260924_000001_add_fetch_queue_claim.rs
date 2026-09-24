//! Migration that adds a claim counter to `fetch_queue`.
//!
//! `claim` is bumped every time a worker dequeues a row. Workers pass the
//! value they claimed back when completing or failing a task, so a worker
//! whose task was recovered and handed to someone else can't overwrite the
//! newer worker's result.

use crate::entities::fetch_queue;
use sea_orm_migration::prelude::*;

#[derive(Debug, DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(fetch_queue::Entity)
                    .add_column(
                        ColumnDef::new(fetch_queue::Column::Claim)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(fetch_queue::Entity)
                    .drop_column(fetch_queue::Column::Claim)
                    .to_owned(),
            )
            .await
    }
}
