//! SeaORM entity for the `oci_manifest` table.

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "oci_manifest")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub oci_repository_id: i64,
    pub digest: String,
    pub media_type: Option<String>,
    pub raw_json: Option<String>,
    pub size_bytes: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub artifact_type: Option<String>,
    pub config_media_type: Option<String>,
    pub config_digest: Option<String>,
    pub oci_created: Option<String>,
    pub oci_authors: Option<String>,
    pub oci_url: Option<String>,
    pub oci_documentation: Option<String>,
    pub oci_source: Option<String>,
    pub oci_version: Option<String>,
    pub oci_revision: Option<String>,
    pub oci_vendor: Option<String>,
    pub oci_licenses: Option<String>,
    pub oci_ref_name: Option<String>,
    pub oci_title: Option<String>,
    pub oci_description: Option<String>,
    pub oci_base_digest: Option<String>,
    pub oci_base_name: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::oci_repository::Entity",
        from = "Column::OciRepositoryId",
        to = "super::oci_repository::Column::Id",
        on_delete = "Cascade"
    )]
    OciRepository,
    #[sea_orm(has_many = "super::oci_manifest_annotation::Entity")]
    OciManifestAnnotation,
    #[sea_orm(has_many = "super::oci_layer::Entity")]
    OciLayer,
    #[sea_orm(has_many = "super::wit_package::Entity")]
    WitPackage,
    #[sea_orm(has_many = "super::wasm_component::Entity")]
    WasmComponent,
}

impl Related<super::oci_repository::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::OciRepository.def()
    }
}
impl Related<super::oci_manifest_annotation::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::OciManifestAnnotation.def()
    }
}
impl Related<super::oci_layer::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::OciLayer.def()
    }
}
impl Related<super::wit_package::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::WitPackage.def()
    }
}
impl Related<super::wasm_component::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::WasmComponent.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
