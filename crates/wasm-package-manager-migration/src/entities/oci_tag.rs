//! SeaORM entity for the `oci_tag` table.

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "oci_tag")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub oci_repository_id: i64,
    pub manifest_digest: String,
    pub tag: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::oci_repository::Entity",
        from = "Column::OciRepositoryId",
        to = "super::oci_repository::Column::Id",
        on_delete = "Cascade"
    )]
    OciRepository,
}

impl Related<super::oci_repository::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::OciRepository.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
