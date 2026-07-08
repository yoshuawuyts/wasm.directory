//! SeaORM entity for the `component_target` table.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "component_target")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub wasm_component_id: i64,
    pub declared_package: String,
    pub declared_world: String,
    pub declared_version: Option<String>,
    pub wit_world_id: Option<i64>,
    pub is_native_package: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::wasm_component::Entity",
        from = "Column::WasmComponentId",
        to = "super::wasm_component::Column::Id",
        on_delete = "Cascade"
    )]
    WasmComponent,
    #[sea_orm(
        belongs_to = "super::wit_world::Entity",
        from = "Column::WitWorldId",
        to = "super::wit_world::Column::Id",
        on_delete = "SetNull"
    )]
    WitWorld,
}

impl Related<super::wasm_component::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::WasmComponent.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
