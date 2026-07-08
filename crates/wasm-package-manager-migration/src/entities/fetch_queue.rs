//! SeaORM entity for the `fetch_queue` table.

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;

/// What kind of work to perform on the fetch_queue row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum FetchTask {
    #[sea_orm(string_value = "pull")]
    Pull,
    #[sea_orm(string_value = "reindex")]
    Reindex,
}

/// Lifecycle status for a fetch_queue row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum FetchStatus {
    #[sea_orm(string_value = "pending")]
    Pending,
    #[sea_orm(string_value = "in_progress")]
    InProgress,
    #[sea_orm(string_value = "completed")]
    Completed,
    #[sea_orm(string_value = "failed")]
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "fetch_queue")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub registry: String,
    pub repository: String,
    pub tag: String,
    pub task: FetchTask,
    pub status: FetchStatus,
    pub priority: i32,
    pub attempts: i32,
    pub max_attempts: i32,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
