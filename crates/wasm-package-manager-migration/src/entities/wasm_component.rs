//! SeaORM entity for the `wasm_component` table.

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "wasm_component")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub oci_manifest_id: i64,
    pub oci_layer_id: Option<i64>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub producers_json: Option<String>,
    pub created_at: DateTime<Utc>,
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
    #[sea_orm(
        belongs_to = "super::oci_layer::Entity",
        from = "Column::OciLayerId",
        to = "super::oci_layer::Column::Id",
        on_delete = "SetNull"
    )]
    OciLayer,
    #[sea_orm(has_many = "super::component_target::Entity")]
    ComponentTarget,
}

impl Related<super::oci_manifest::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::OciManifest.def()
    }
}
impl Related<super::component_target::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ComponentTarget.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
