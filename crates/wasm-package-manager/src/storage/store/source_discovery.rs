//! Source discovery persistence, deliberately independent of ingested repositories.

use anyhow::Context;
use chrono::{DateTime, Duration, Utc};
use oci_client::Reference;
use sea_orm::{
    ActiveModelTrait, EntityTrait, Set, TransactionTrait,
    sea_query::{Expr, OnConflict},
};
use wasm_package_manager_migration::entities::source_discovery;

use super::Store;
use crate::storage::SourceDiscoveryState;

impl Store {
    pub(crate) async fn source_discovery_state(
        &self,
        reference: &Reference,
    ) -> anyhow::Result<Option<SourceDiscoveryState>> {
        source_discovery::Entity::find_by_id((
            reference.registry().to_owned(),
            reference.repository().to_owned(),
        ))
        .one(&self.db)
        .await?
        .map(discovery_state)
        .transpose()
    }

    pub(crate) async fn record_source_discovery_success(
        &self,
        reference: &Reference,
        now: DateTime<Utc>,
    ) -> anyhow::Result<()> {
        source_discovery::Entity::insert(source_discovery::ActiveModel {
            registry: Set(reference.registry().to_owned()),
            repository: Set(reference.repository().to_owned()),
            last_completed_at: Set(Some(now)),
            next_retry_at: Set(None),
            failure_count: Set(0),
            last_error: Set(None),
        })
        .on_conflict(
            source_identity_conflict()
                .update_columns([
                    source_discovery::Column::LastCompletedAt,
                    source_discovery::Column::NextRetryAt,
                    source_discovery::Column::FailureCount,
                    source_discovery::Column::LastError,
                ])
                .to_owned(),
        )
        .exec(&self.db)
        .await?;
        Ok(())
    }

    pub(crate) async fn record_source_discovery_failure(
        &self,
        reference: &Reference,
        now: DateTime<Utc>,
        error: &str,
    ) -> anyhow::Result<()> {
        let transaction = self.db.begin().await?;
        // The upsert locks this identity before reading its count. Keep the
        // retry deadline update in the same transaction to avoid lost retries.
        let row = source_discovery::Entity::insert(source_discovery::ActiveModel {
            registry: Set(reference.registry().to_owned()),
            repository: Set(reference.repository().to_owned()),
            last_completed_at: Set(None),
            next_retry_at: Set(None),
            failure_count: Set(1),
            last_error: Set(Some(error.to_owned())),
        })
        .on_conflict(
            source_identity_conflict()
                .value(
                    source_discovery::Column::FailureCount,
                    Expr::cust(
                        "CASE WHEN source_discovery.failure_count < 4294967295 \
                         THEN source_discovery.failure_count + 1 \
                         ELSE source_discovery.failure_count END",
                    ),
                )
                .update_column(source_discovery::Column::LastError)
                .to_owned(),
        )
        .exec_with_returning(&transaction)
        .await?;
        let retry_at = retry_deadline(now, u32::try_from(row.failure_count)?)?;
        let mut row: source_discovery::ActiveModel = row.into();
        row.next_retry_at = Set(Some(retry_at));
        row.update(&transaction).await?;
        transaction.commit().await?;
        Ok(())
    }
}

fn source_identity_conflict() -> OnConflict {
    OnConflict::columns([
        source_discovery::Column::Registry,
        source_discovery::Column::Repository,
    ])
}

fn discovery_state(row: source_discovery::Model) -> anyhow::Result<SourceDiscoveryState> {
    Ok(SourceDiscoveryState {
        last_completed_at: row.last_completed_at,
        next_retry_at: row.next_retry_at,
        failure_count: row
            .failure_count
            .try_into()
            .context("invalid source discovery failure count")?,
        last_error: row.last_error,
    })
}

fn retry_deadline(now: DateTime<Utc>, failure_count: u32) -> anyhow::Result<DateTime<Utc>> {
    let exponent = failure_count.saturating_sub(1).min(6);
    let seconds = (60_i64 * (1_i64 << exponent)).min(3600);
    now.checked_add_signed(Duration::seconds(seconds))
        .context("source discovery retry deadline exceeds timestamp range")
}

#[cfg(test)]
mod tests;
