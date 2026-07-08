//! SeaORM entity for the `_sync_meta` table.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "_sync_meta")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false, column_name = "key")]
    pub key: String,
    #[sea_orm(column_name = "value")]
    pub value: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
