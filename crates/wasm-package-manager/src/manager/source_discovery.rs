//! Durable source discovery and independently scheduled config-backfill progress.

use chrono::{DateTime, Utc};
use oci_client::Reference;

use super::Manager;
use crate::storage::SourceDiscoveryState;

const LAST_CONFIG_BACKFILL_KEY: &str = "indexer_last_config_backfill_at";

impl Manager {
    /// Return discovery progress for the reference's canonical registry/repository.
    ///
    /// Tags and digests do not affect identity. Missing progress is not inferred
    /// from existing source, repository or fetch-queue rows.
    ///
    /// # Errors
    ///
    /// Returns an error if the database query or stored-state decoding fails.
    pub async fn source_discovery_state(
        &self,
        reference: &Reference,
    ) -> anyhow::Result<Option<SourceDiscoveryState>> {
        self.store.source_discovery_state(reference).await
    }

    /// Record successful tag discovery, clearing the source's retry state.
    ///
    /// Call only after fully enumerating and durably enqueuing the source's tags.
    /// This records discovery, NOT ingestion or readiness, and does not complete
    /// fetch tasks. No ingested repository row is required.
    ///
    /// # Errors
    ///
    /// Returns an error if the database write fails.
    pub async fn record_source_discovery_success(
        &self,
        reference: &Reference,
        now: DateTime<Utc>,
    ) -> anyhow::Result<()> {
        self.store
            .record_source_discovery_success(reference, now)
            .await
    }

    /// Record a discovery failure without clearing any previous completion.
    ///
    /// Consecutive failures are durable. Retries start at 60 seconds and double
    /// up to 3600 seconds. The error and retry deadline are replaced atomically.
    ///
    /// # Errors
    ///
    /// Returns an error if the database write fails or the retry date overflows.
    pub async fn record_source_discovery_failure(
        &self,
        reference: &Reference,
        now: DateTime<Utc>,
        error: &str,
    ) -> anyhow::Result<()> {
        self.store
            .record_source_discovery_failure(reference, now, error)
            .await
    }

    /// When config backfill last completed, independently of tag discovery.
    ///
    /// Missing or malformed metadata is treated as never completed, matching
    /// the discovery watermark's RFC 3339 parsing.
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub async fn last_config_backfill_at(&self) -> anyhow::Result<Option<DateTime<Utc>>> {
        let value = self.store.get_sync_meta(LAST_CONFIG_BACKFILL_KEY).await?;
        Ok(value
            .and_then(|value| DateTime::parse_from_rfc3339(&value).ok())
            .map(Into::into))
    }

    /// Record a completed config-backfill pass without changing discovery state.
    ///
    /// # Errors
    ///
    /// Returns an error if the database write fails.
    pub async fn record_config_backfill_at(&self, now: DateTime<Utc>) -> anyhow::Result<()> {
        self.store
            .set_sync_meta(LAST_CONFIG_BACKFILL_KEY, &now.to_rfc3339())
            .await
    }
}

#[cfg(test)]
mod tests;
