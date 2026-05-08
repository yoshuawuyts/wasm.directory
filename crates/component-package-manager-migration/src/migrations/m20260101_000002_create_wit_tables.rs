//! Migration that creates the WIT layer of the schema:
//! `wit_package`, `wit_world`, `wit_world_import`, `wit_world_export`,
//! `wit_package_dependency`.

use crate::entities::{
    oci_layer, oci_manifest, wit_package, wit_package_dependency, wit_world, wit_world_export,
    wit_world_import,
};
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::ConnectionTrait;

#[derive(Debug, DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // wit_package
        manager
            .create_table(
                Table::create()
                    .table(wit_package::Entity)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(wit_package::Column::Id)
                            .big_integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(
                        ColumnDef::new(wit_package::Column::PackageName)
                            .text()
                            .not_null(),
                    )
                    .col(ColumnDef::new(wit_package::Column::Version).text())
                    .col(ColumnDef::new(wit_package::Column::Description).text())
                    .col(ColumnDef::new(wit_package::Column::WitText).text())
                    .col(ColumnDef::new(wit_package::Column::OciManifestId).big_integer())
                    .col(ColumnDef::new(wit_package::Column::OciLayerId).big_integer())
                    .col(
                        ColumnDef::new(wit_package::Column::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_wit_package_manifest")
                            .from(wit_package::Entity, wit_package::Column::OciManifestId)
                            .to(oci_manifest::Entity, oci_manifest::Column::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_wit_package_layer")
                            .from(wit_package::Entity, wit_package::Column::OciLayerId)
                            .to(oci_layer::Entity, oci_layer::Column::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;
        // Unique index using COALESCE on nullable columns so NULLs are
        // treated as equal. SeaORM's index builder doesn't expose
        // expression indexes, so we hand-write SQL (SQLite and Postgres
        // both support expression indexes with the same syntax).
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE UNIQUE INDEX IF NOT EXISTS uq_wit_packages \
                 ON wit_package(package_name, COALESCE(version, ''), COALESCE(oci_layer_id, -1));",
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_wit_package_name_version")
                    .table(wit_package::Entity)
                    .col(wit_package::Column::PackageName)
                    .col(wit_package::Column::Version)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_wit_package_provenance")
                    .table(wit_package::Entity)
                    .col(wit_package::Column::OciManifestId)
                    .to_owned(),
            )
            .await?;

        // wit_world
        manager
            .create_table(
                Table::create()
                    .table(wit_world::Entity)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(wit_world::Column::Id)
                            .big_integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(
                        ColumnDef::new(wit_world::Column::WitPackageId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(wit_world::Column::Name).text().not_null())
                    .col(ColumnDef::new(wit_world::Column::Description).text())
                    .col(
                        ColumnDef::new(wit_world::Column::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_wit_world_package")
                            .from(wit_world::Entity, wit_world::Column::WitPackageId)
                            .to(wit_package::Entity, wit_package::Column::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("uq_wit_world_pkg_name")
                    .table(wit_world::Entity)
                    .col(wit_world::Column::WitPackageId)
                    .col(wit_world::Column::Name)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_wit_world_name")
                    .table(wit_world::Entity)
                    .col(wit_world::Column::Name)
                    .to_owned(),
            )
            .await?;

        // wit_world_import
        manager
            .create_table(
                Table::create()
                    .table(wit_world_import::Entity)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(wit_world_import::Column::Id)
                            .big_integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(
                        ColumnDef::new(wit_world_import::Column::WitWorldId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(wit_world_import::Column::DeclaredPackage)
                            .text()
                            .not_null(),
                    )
                    .col(ColumnDef::new(wit_world_import::Column::DeclaredInterface).text())
                    .col(ColumnDef::new(wit_world_import::Column::DeclaredVersion).text())
                    .col(ColumnDef::new(wit_world_import::Column::ResolvedPackageId).big_integer())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_wit_world_import_world")
                            .from(
                                wit_world_import::Entity,
                                wit_world_import::Column::WitWorldId,
                            )
                            .to(wit_world::Entity, wit_world::Column::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_wit_world_import_resolved")
                            .from(
                                wit_world_import::Entity,
                                wit_world_import::Column::ResolvedPackageId,
                            )
                            .to(wit_package::Entity, wit_package::Column::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;
        // See note above on COALESCE-based expression unique indexes.
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE UNIQUE INDEX IF NOT EXISTS uq_wit_world_import \
                 ON wit_world_import(wit_world_id, declared_package, \
                 COALESCE(declared_interface, ''), COALESCE(declared_version, ''));",
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_world_import_declared")
                    .table(wit_world_import::Entity)
                    .col(wit_world_import::Column::DeclaredPackage)
                    .col(wit_world_import::Column::DeclaredVersion)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_world_import_resolved")
                    .table(wit_world_import::Entity)
                    .col(wit_world_import::Column::ResolvedPackageId)
                    .to_owned(),
            )
            .await?;

        // wit_world_export
        manager
            .create_table(
                Table::create()
                    .table(wit_world_export::Entity)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(wit_world_export::Column::Id)
                            .big_integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(
                        ColumnDef::new(wit_world_export::Column::WitWorldId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(wit_world_export::Column::DeclaredPackage)
                            .text()
                            .not_null(),
                    )
                    .col(ColumnDef::new(wit_world_export::Column::DeclaredInterface).text())
                    .col(ColumnDef::new(wit_world_export::Column::DeclaredVersion).text())
                    .col(ColumnDef::new(wit_world_export::Column::ResolvedPackageId).big_integer())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_wit_world_export_world")
                            .from(
                                wit_world_export::Entity,
                                wit_world_export::Column::WitWorldId,
                            )
                            .to(wit_world::Entity, wit_world::Column::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_wit_world_export_resolved")
                            .from(
                                wit_world_export::Entity,
                                wit_world_export::Column::ResolvedPackageId,
                            )
                            .to(wit_package::Entity, wit_package::Column::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;
        // See note above on COALESCE-based expression unique indexes.
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE UNIQUE INDEX IF NOT EXISTS uq_wit_world_export \
                 ON wit_world_export(wit_world_id, declared_package, \
                 COALESCE(declared_interface, ''), COALESCE(declared_version, ''));",
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_world_export_declared")
                    .table(wit_world_export::Entity)
                    .col(wit_world_export::Column::DeclaredPackage)
                    .col(wit_world_export::Column::DeclaredVersion)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_world_export_resolved")
                    .table(wit_world_export::Entity)
                    .col(wit_world_export::Column::ResolvedPackageId)
                    .to_owned(),
            )
            .await?;

        // wit_package_dependency
        manager
            .create_table(
                Table::create()
                    .table(wit_package_dependency::Entity)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(wit_package_dependency::Column::Id)
                            .big_integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(
                        ColumnDef::new(wit_package_dependency::Column::DependentId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(wit_package_dependency::Column::DeclaredPackage)
                            .text()
                            .not_null(),
                    )
                    .col(ColumnDef::new(wit_package_dependency::Column::DeclaredVersion).text())
                    .col(
                        ColumnDef::new(wit_package_dependency::Column::ResolvedPackageId)
                            .big_integer(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_wit_package_dep_dependent")
                            .from(
                                wit_package_dependency::Entity,
                                wit_package_dependency::Column::DependentId,
                            )
                            .to(wit_package::Entity, wit_package::Column::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_wit_package_dep_resolved")
                            .from(
                                wit_package_dependency::Entity,
                                wit_package_dependency::Column::ResolvedPackageId,
                            )
                            .to(wit_package::Entity, wit_package::Column::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;
        // See note above on COALESCE-based expression unique indexes.
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE UNIQUE INDEX IF NOT EXISTS uq_wit_package_dependency \
                 ON wit_package_dependency(dependent_id, declared_package, \
                 COALESCE(declared_version, ''));",
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_wit_dep_declared")
                    .table(wit_package_dependency::Entity)
                    .col(wit_package_dependency::Column::DeclaredPackage)
                    .col(wit_package_dependency::Column::DeclaredVersion)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_wit_dep_resolved")
                    .table(wit_package_dependency::Entity)
                    .col(wit_package_dependency::Column::ResolvedPackageId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for ent in [
            wit_package_dependency::Entity.into_table_ref(),
            wit_world_export::Entity.into_table_ref(),
            wit_world_import::Entity.into_table_ref(),
            wit_world::Entity.into_table_ref(),
            wit_package::Entity.into_table_ref(),
        ] {
            manager
                .drop_table(Table::drop().table(ent).if_exists().to_owned())
                .await?;
        }
        Ok(())
    }
}
