//! SeaORM entity for the `wit_package` table.

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "wit_package")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub package_name: String,
    pub version: Option<String>,
    pub description: Option<String>,
    pub wit_text: Option<String>,
    pub oci_manifest_id: Option<i64>,
    pub oci_layer_id: Option<i64>,
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::oci_manifest::Entity",
        from = "Column::OciManifestId",
        to = "super::oci_manifest::Column::Id",
        on_delete = "SetNull"
    )]
    OciManifest,
    #[sea_orm(
        belongs_to = "super::oci_layer::Entity",
        from = "Column::OciLayerId",
        to = "super::oci_layer::Column::Id",
        on_delete = "SetNull"
    )]
    OciLayer,
    #[sea_orm(has_many = "super::wit_world::Entity")]
    WitWorld,
}

impl Related<super::oci_manifest::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::OciManifest.def()
    }
}
impl Related<super::oci_layer::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::OciLayer.def()
    }
}
impl Related<super::wit_world::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::WitWorld.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
