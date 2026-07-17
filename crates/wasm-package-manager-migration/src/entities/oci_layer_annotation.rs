//! SeaORM entity for the `oci_layer_annotation` table.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "oci_layer_annotation")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub oci_layer_id: i64,
    #[sea_orm(column_name = "key")]
    pub key: String,
    #[sea_orm(column_name = "value")]
    pub value: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::oci_layer::Entity",
        from = "Column::OciLayerId",
        to = "super::oci_layer::Column::Id",
        on_delete = "Cascade"
    )]
    OciLayer,
}

impl Related<super::oci_layer::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::OciLayer.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
