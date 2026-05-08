//! Migration that creates the Wasm layer of the schema:
//! `wasm_component`, `component_target`.

use crate::entities::{component_target, oci_layer, oci_manifest, wasm_component, wit_world};
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::ConnectionTrait;

#[derive(Debug, DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // wasm_component
        manager
            .create_table(
                Table::create()
                    .table(wasm_component::Entity)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(wasm_component::Column::Id)
                            .big_integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(
                        ColumnDef::new(wasm_component::Column::OciManifestId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(wasm_component::Column::OciLayerId).big_integer())
                    .col(ColumnDef::new(wasm_component::Column::Name).text())
                    .col(ColumnDef::new(wasm_component::Column::Description).text())
                    .col(ColumnDef::new(wasm_component::Column::ProducersJson).text())
                    .col(
                        ColumnDef::new(wasm_component::Column::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_wasm_component_manifest")
                            .from(
                                wasm_component::Entity,
                                wasm_component::Column::OciManifestId,
                            )
                            .to(oci_manifest::Entity, oci_manifest::Column::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_wasm_component_layer")
                            .from(wasm_component::Entity, wasm_component::Column::OciLayerId)
                            .to(oci_layer::Entity, oci_layer::Column::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;
        // Unique index using COALESCE on the nullable oci_layer_id so
        // multiple rows with NULL oci_layer_id for the same manifest are
        // treated as duplicates. SeaORM's index builder doesn't expose
        // expression indexes, so we hand-write SQL (SQLite and Postgres
        // both support expression indexes with the same syntax).
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE UNIQUE INDEX IF NOT EXISTS uq_wasm_component \
                 ON wasm_component(oci_manifest_id, COALESCE(oci_layer_id, -1));",
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_wasm_component_name")
                    .table(wasm_component::Entity)
                    .col(wasm_component::Column::Name)
                    .to_owned(),
            )
            .await?;

        // component_target
        manager
            .create_table(
                Table::create()
                    .table(component_target::Entity)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(component_target::Column::Id)
                            .big_integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(
                        ColumnDef::new(component_target::Column::WasmComponentId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(component_target::Column::DeclaredPackage)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(component_target::Column::DeclaredWorld)
                            .text()
                            .not_null(),
                    )
                    .col(ColumnDef::new(component_target::Column::DeclaredVersion).text())
                    .col(ColumnDef::new(component_target::Column::WitWorldId).big_integer())
                    .col(
                        ColumnDef::new(component_target::Column::IsNativePackage)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_component_target_component")
                            .from(
                                component_target::Entity,
                                component_target::Column::WasmComponentId,
                            )
                            .to(wasm_component::Entity, wasm_component::Column::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_component_target_world")
                            .from(
                                component_target::Entity,
                                component_target::Column::WitWorldId,
                            )
                            .to(wit_world::Entity, wit_world::Column::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;
        // See note above on COALESCE-based expression unique indexes.
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE UNIQUE INDEX IF NOT EXISTS uq_component_target \
                 ON component_target(wasm_component_id, declared_package, declared_world, \
                 COALESCE(declared_version, ''));",
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_target_declared")
                    .table(component_target::Entity)
                    .col(component_target::Column::DeclaredPackage)
                    .col(component_target::Column::DeclaredWorld)
                    .col(component_target::Column::DeclaredVersion)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_target_resolved")
                    .table(component_target::Entity)
                    .col(component_target::Column::WitWorldId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for ent in [
            component_target::Entity.into_table_ref(),
            wasm_component::Entity.into_table_ref(),
        ] {
            manager
                .drop_table(Table::drop().table(ent).if_exists().to_owned())
                .await?;
        }
        Ok(())
    }
}
