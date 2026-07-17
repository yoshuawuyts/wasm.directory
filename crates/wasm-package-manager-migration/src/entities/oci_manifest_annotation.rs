//! SeaORM entity for the `oci_manifest_annotation` table.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "oci_manifest_annotation")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub oci_manifest_id: i64,
    #[sea_orm(column_name = "key")]
    pub key: String,
    #[sea_orm(column_name = "value")]
    pub value: String,
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
}

impl Related<super::oci_manifest::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::OciManifest.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
