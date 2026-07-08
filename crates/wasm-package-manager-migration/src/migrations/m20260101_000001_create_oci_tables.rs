//! Migration that creates the OCI layer of the schema:
//! `oci_repository`, `oci_manifest`, `oci_manifest_annotation`,
//! `oci_tag`, `oci_layer`, `oci_layer_annotation`, `oci_referrer`.

use crate::entities::{
    oci_layer, oci_layer_annotation, oci_manifest, oci_manifest_annotation, oci_referrer,
    oci_repository, oci_tag, sync_meta,
};
use crate::migrations::triggers::{drop_updated_at_trigger, install_updated_at_trigger};
use sea_orm_migration::prelude::*;

#[derive(Debug, DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // _sync_meta — KV store. Plain `key` PK, no surrogate id.
        manager
            .create_table(
                Table::create()
                    .table(sync_meta::Entity)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(sync_meta::Column::Key)
                            .text()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(sync_meta::Column::Value).text().not_null())
                    .to_owned(),
            )
            .await?;

        // oci_repository
        manager
            .create_table(
                Table::create()
                    .table(oci_repository::Entity)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(oci_repository::Column::Id)
                            .big_integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(
                        ColumnDef::new(oci_repository::Column::Registry)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(oci_repository::Column::Repository)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(oci_repository::Column::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(oci_repository::Column::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(oci_repository::Column::WitNamespace).text())
                    .col(ColumnDef::new(oci_repository::Column::WitName).text())
                    .col(ColumnDef::new(oci_repository::Column::Kind).text())
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("uq_oci_repository_registry_repo")
                    .table(oci_repository::Entity)
                    .col(oci_repository::Column::Registry)
                    .col(oci_repository::Column::Repository)
                    .unique()
                    .to_owned(),
            )
            .await?;
        install_updated_at_trigger(manager, "oci_repository").await?;

        // oci_manifest
        manager
            .create_table(
                Table::create()
                    .table(oci_manifest::Entity)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(oci_manifest::Column::Id)
                            .big_integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(
                        ColumnDef::new(oci_manifest::Column::OciRepositoryId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(oci_manifest::Column::Digest)
                            .text()
                            .not_null(),
                    )
                    .col(ColumnDef::new(oci_manifest::Column::MediaType).text())
                    .col(ColumnDef::new(oci_manifest::Column::RawJson).text())
                    .col(ColumnDef::new(oci_manifest::Column::SizeBytes).big_integer())
                    .col(
                        ColumnDef::new(oci_manifest::Column::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(oci_manifest::Column::ArtifactType).text())
                    .col(ColumnDef::new(oci_manifest::Column::ConfigMediaType).text())
                    .col(ColumnDef::new(oci_manifest::Column::ConfigDigest).text())
                    .col(ColumnDef::new(oci_manifest::Column::OciCreated).text())
                    .col(ColumnDef::new(oci_manifest::Column::OciAuthors).text())
                    .col(ColumnDef::new(oci_manifest::Column::OciUrl).text())
                    .col(ColumnDef::new(oci_manifest::Column::OciDocumentation).text())
                    .col(ColumnDef::new(oci_manifest::Column::OciSource).text())
                    .col(ColumnDef::new(oci_manifest::Column::OciVersion).text())
                    .col(ColumnDef::new(oci_manifest::Column::OciRevision).text())
                    .col(ColumnDef::new(oci_manifest::Column::OciVendor).text())
                    .col(ColumnDef::new(oci_manifest::Column::OciLicenses).text())
                    .col(ColumnDef::new(oci_manifest::Column::OciRefName).text())
                    .col(ColumnDef::new(oci_manifest::Column::OciTitle).text())
                    .col(ColumnDef::new(oci_manifest::Column::OciDescription).text())
                    .col(ColumnDef::new(oci_manifest::Column::OciBaseDigest).text())
                    .col(ColumnDef::new(oci_manifest::Column::OciBaseName).text())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_oci_manifest_repository")
                            .from(oci_manifest::Entity, oci_manifest::Column::OciRepositoryId)
                            .to(oci_repository::Entity, oci_repository::Column::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("uq_oci_manifest_repo_digest")
                    .table(oci_manifest::Entity)
                    .col(oci_manifest::Column::OciRepositoryId)
                    .col(oci_manifest::Column::Digest)
                    .unique()
                    .to_owned(),
            )
            .await?;
        for (name, col) in [
            ("idx_oci_manifest_digest", oci_manifest::Column::Digest),
            (
                "idx_oci_manifest_artifact_type",
                oci_manifest::Column::ArtifactType,
            ),
            ("idx_oci_manifest_version", oci_manifest::Column::OciVersion),
            ("idx_oci_manifest_vendor", oci_manifest::Column::OciVendor),
            (
                "idx_oci_manifest_licenses",
                oci_manifest::Column::OciLicenses,
            ),
        ] {
            manager
                .create_index(
                    Index::create()
                        .name(name)
                        .table(oci_manifest::Entity)
                        .col(col)
                        .to_owned(),
                )
                .await?;
        }

        // oci_manifest_annotation
        manager
            .create_table(
                Table::create()
                    .table(oci_manifest_annotation::Entity)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(oci_manifest_annotation::Column::Id)
                            .big_integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(
                        ColumnDef::new(oci_manifest_annotation::Column::OciManifestId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(oci_manifest_annotation::Column::Key)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(oci_manifest_annotation::Column::Value)
                            .text()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_oci_manifest_annotation_manifest")
                            .from(
                                oci_manifest_annotation::Entity,
                                oci_manifest_annotation::Column::OciManifestId,
                            )
                            .to(oci_manifest::Entity, oci_manifest::Column::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("uq_oci_manifest_annotation")
                    .table(oci_manifest_annotation::Entity)
                    .col(oci_manifest_annotation::Column::OciManifestId)
                    .col(oci_manifest_annotation::Column::Key)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_oci_manifest_annotation_key")
                    .table(oci_manifest_annotation::Entity)
                    .col(oci_manifest_annotation::Column::Key)
                    .to_owned(),
            )
            .await?;

        // oci_tag — composite FK (oci_repository_id, manifest_digest) ->
        // oci_manifest(oci_repository_id, digest) requires a UNIQUE in the parent
        // (already declared above as uq_oci_manifest_repo_digest).
        manager
            .create_table(
                Table::create()
                    .table(oci_tag::Entity)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(oci_tag::Column::Id)
                            .big_integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(
                        ColumnDef::new(oci_tag::Column::OciRepositoryId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(oci_tag::Column::ManifestDigest)
                            .text()
                            .not_null(),
                    )
                    .col(ColumnDef::new(oci_tag::Column::Tag).text().not_null())
                    .col(
                        ColumnDef::new(oci_tag::Column::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(oci_tag::Column::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_oci_tag_repository")
                            .from(oci_tag::Entity, oci_tag::Column::OciRepositoryId)
                            .to(oci_repository::Entity, oci_repository::Column::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_oci_tag_manifest")
                            .from_tbl(oci_tag::Entity)
                            .from_col(oci_tag::Column::OciRepositoryId)
                            .from_col(oci_tag::Column::ManifestDigest)
                            .to_tbl(oci_manifest::Entity)
                            .to_col(oci_manifest::Column::OciRepositoryId)
                            .to_col(oci_manifest::Column::Digest)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("uq_oci_tag_repo_tag")
                    .table(oci_tag::Entity)
                    .col(oci_tag::Column::OciRepositoryId)
                    .col(oci_tag::Column::Tag)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_oci_tag_digest")
                    .table(oci_tag::Entity)
                    .col(oci_tag::Column::ManifestDigest)
                    .to_owned(),
            )
            .await?;
        install_updated_at_trigger(manager, "oci_tag").await?;

        // oci_layer
        manager
            .create_table(
                Table::create()
                    .table(oci_layer::Entity)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(oci_layer::Column::Id)
                            .big_integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(
                        ColumnDef::new(oci_layer::Column::OciManifestId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(oci_layer::Column::Digest).text().not_null())
                    .col(ColumnDef::new(oci_layer::Column::MediaType).text())
                    .col(ColumnDef::new(oci_layer::Column::SizeBytes).big_integer())
                    .col(
                        ColumnDef::new(oci_layer::Column::Position)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_oci_layer_manifest")
                            .from(oci_layer::Entity, oci_layer::Column::OciManifestId)
                            .to(oci_manifest::Entity, oci_manifest::Column::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("uq_oci_layer_digest")
                    .table(oci_layer::Entity)
                    .col(oci_layer::Column::OciManifestId)
                    .col(oci_layer::Column::Digest)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("uq_oci_layer_position")
                    .table(oci_layer::Entity)
                    .col(oci_layer::Column::OciManifestId)
                    .col(oci_layer::Column::Position)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // oci_layer_annotation
        manager
            .create_table(
                Table::create()
                    .table(oci_layer_annotation::Entity)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(oci_layer_annotation::Column::Id)
                            .big_integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(
                        ColumnDef::new(oci_layer_annotation::Column::OciLayerId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(oci_layer_annotation::Column::Key)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(oci_layer_annotation::Column::Value)
                            .text()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_oci_layer_annotation_layer")
                            .from(
                                oci_layer_annotation::Entity,
                                oci_layer_annotation::Column::OciLayerId,
                            )
                            .to(oci_layer::Entity, oci_layer::Column::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("uq_oci_layer_annotation")
                    .table(oci_layer_annotation::Entity)
                    .col(oci_layer_annotation::Column::OciLayerId)
                    .col(oci_layer_annotation::Column::Key)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_oci_layer_annotation_key")
                    .table(oci_layer_annotation::Entity)
                    .col(oci_layer_annotation::Column::Key)
                    .to_owned(),
            )
            .await?;

        // oci_referrer
        manager
            .create_table(
                Table::create()
                    .table(oci_referrer::Entity)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(oci_referrer::Column::Id)
                            .big_integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(
                        ColumnDef::new(oci_referrer::Column::SubjectManifestId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(oci_referrer::Column::ReferrerManifestId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(oci_referrer::Column::ArtifactType)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(oci_referrer::Column::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_oci_referrer_subject")
                            .from(
                                oci_referrer::Entity,
                                oci_referrer::Column::SubjectManifestId,
                            )
                            .to(oci_manifest::Entity, oci_manifest::Column::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_oci_referrer_referrer")
                            .from(
                                oci_referrer::Entity,
                                oci_referrer::Column::ReferrerManifestId,
                            )
                            .to(oci_manifest::Entity, oci_manifest::Column::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("uq_oci_referrer")
                    .table(oci_referrer::Entity)
                    .col(oci_referrer::Column::SubjectManifestId)
                    .col(oci_referrer::Column::ReferrerManifestId)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_oci_referrer_type")
                    .table(oci_referrer::Entity)
                    .col(oci_referrer::Column::SubjectManifestId)
                    .col(oci_referrer::Column::ArtifactType)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_oci_referrer_referrer")
                    .table(oci_referrer::Entity)
                    .col(oci_referrer::Column::ReferrerManifestId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        drop_updated_at_trigger(manager, "oci_tag").await?;
        drop_updated_at_trigger(manager, "oci_repository").await?;
        for ent in [
            oci_referrer::Entity.into_table_ref(),
            oci_layer_annotation::Entity.into_table_ref(),
            oci_layer::Entity.into_table_ref(),
            oci_tag::Entity.into_table_ref(),
            oci_manifest_annotation::Entity.into_table_ref(),
            oci_manifest::Entity.into_table_ref(),
            oci_repository::Entity.into_table_ref(),
            sync_meta::Entity.into_table_ref(),
        ] {
            manager
                .drop_table(Table::drop().table(ent).if_exists().to_owned())
                .await?;
        }
        Ok(())
    }
}
