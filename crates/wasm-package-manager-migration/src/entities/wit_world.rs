//! SeaORM entity for the `wit_world` table.

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "wit_world")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub wit_package_id: i64,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::wit_package::Entity",
        from = "Column::WitPackageId",
        to = "super::wit_package::Column::Id",
        on_delete = "Cascade"
    )]
    WitPackage,
    #[sea_orm(has_many = "super::wit_world_import::Entity")]
    WitWorldImport,
    #[sea_orm(has_many = "super::wit_world_export::Entity")]
    WitWorldExport,
}

impl Related<super::wit_package::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::WitPackage.def()
    }
}
impl Related<super::wit_world_import::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::WitWorldImport.def()
    }
}
impl Related<super::wit_world_export::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::WitWorldExport.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
