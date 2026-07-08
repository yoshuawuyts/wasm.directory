//! SeaORM entity for the `wit_package_dependency` table.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "wit_package_dependency")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub dependent_id: i64,
    pub declared_package: String,
    pub declared_version: Option<String>,
    pub resolved_package_id: Option<i64>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::wit_package::Entity",
        from = "Column::DependentId",
        to = "super::wit_package::Column::Id",
        on_delete = "Cascade"
    )]
    Dependent,
    #[sea_orm(
        belongs_to = "super::wit_package::Entity",
        from = "Column::ResolvedPackageId",
        to = "super::wit_package::Column::Id",
        on_delete = "SetNull"
    )]
    ResolvedPackage,
}

impl ActiveModelBehavior for ActiveModel {}
