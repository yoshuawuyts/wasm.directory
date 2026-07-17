//! SeaORM entity for the `oci_referrer` table.

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "oci_referrer")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub subject_manifest_id: i64,
    pub referrer_manifest_id: i64,
    pub artifact_type: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    // Two FKs to oci_manifest; declared with explicit `from`/`to` so we can
    // distinguish them. `Related<oci_manifest>` is intentionally not
    // implemented (ambiguous) — callers should join via the named variant.
    #[sea_orm(
        belongs_to = "super::oci_manifest::Entity",
        from = "Column::SubjectManifestId",
        to = "super::oci_manifest::Column::Id",
        on_delete = "Cascade"
    )]
    SubjectManifest,
    #[sea_orm(
        belongs_to = "super::oci_manifest::Entity",
        from = "Column::ReferrerManifestId",
        to = "super::oci_manifest::Column::Id",
        on_delete = "Cascade"
    )]
    ReferrerManifest,
}

impl ActiveModelBehavior for ActiveModel {}
