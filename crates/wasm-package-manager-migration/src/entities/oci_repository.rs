//! SeaORM entity for the `oci_repository` table.

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "oci_repository")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub registry: String,
    pub repository: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub wit_namespace: Option<String>,
    pub wit_name: Option<String>,
    pub kind: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::oci_manifest::Entity")]
    OciManifest,
    #[sea_orm(has_many = "super::oci_tag::Entity")]
    OciTag,
}

impl Related<super::oci_manifest::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::OciManifest.def()
    }
}

impl Related<super::oci_tag::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::OciTag.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
