//! Migration that adds `oci_manifest.config_created`: the `created`
//! timestamp from the manifest's config blob.
//!
//! Many publishers don't set the `org.opencontainers.image.created`
//! annotation, but OCI image configs (including wasm configs) carry a
//! `created` field, which gives us a real publish time for those releases.

use crate::entities::oci_manifest;
use sea_orm_migration::prelude::*;

#[derive(Debug, DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(oci_manifest::Entity)
                    .add_column(ColumnDef::new(oci_manifest::Column::ConfigCreated).text())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(oci_manifest::Entity)
                    .drop_column(oci_manifest::Column::ConfigCreated)
                    .to_owned(),
            )
            .await
    }
}
