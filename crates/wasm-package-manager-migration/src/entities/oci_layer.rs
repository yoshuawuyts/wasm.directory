//! SeaORM entity for the `oci_layer` table.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "oci_layer")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub oci_manifest_id: i64,
    pub digest: String,
    pub media_type: Option<String>,
    pub size_bytes: Option<i64>,
    pub position: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::oci_manifest::Entity",
        from = "Column::OciManifestId",
        to = "super::oci_manifest::Column::Id",
        on_delete = "Cascade"
    )]
    OciManifest,
    #[sea_orm(has_many = "super::oci_layer_annotation::Entity")]
    OciLayerAnnotation,
}

impl Related<super::oci_manifest::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::OciManifest.def()
    }
}
impl Related<super::oci_layer_annotation::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::OciLayerAnnotation.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
