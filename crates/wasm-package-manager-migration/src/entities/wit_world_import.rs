//! SeaORM entity for the `wit_world_import` table.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "wit_world_import")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub wit_world_id: i64,
    pub declared_package: String,
    pub declared_interface: Option<String>,
    pub declared_version: Option<String>,
    pub resolved_package_id: Option<i64>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::wit_world::Entity",
        from = "Column::WitWorldId",
        to = "super::wit_world::Column::Id",
        on_delete = "Cascade"
    )]
    WitWorld,
    #[sea_orm(
        belongs_to = "super::wit_package::Entity",
        from = "Column::ResolvedPackageId",
        to = "super::wit_package::Column::Id",
        on_delete = "SetNull"
    )]
    ResolvedPackage,
}

impl Related<super::wit_world::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::WitWorld.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
