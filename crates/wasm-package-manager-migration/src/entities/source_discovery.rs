//! Discovery progress keyed by canonical OCI coordinates, independent of ingestion.

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "source_discovery")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub registry: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub repository: String,
    pub last_completed_at: Option<DateTime<Utc>>,
    pub next_retry_at: Option<DateTime<Utc>>,
    pub failure_count: i64,
    pub last_error: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
