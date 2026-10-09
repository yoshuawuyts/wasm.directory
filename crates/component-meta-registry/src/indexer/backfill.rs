use std::collections::VecDeque;
use std::time::Duration;

use chrono::{DateTime, Utc};
use tokio::time::Instant;
use tracing::{error, warn};
use wasm_package_manager::storage::PendingConfig;

use super::schedule::discovery_delay;
use super::{DISCOVERY_TIMEOUT, Indexer, LEADER_RETRY_INTERVAL};

#[derive(Debug, Default)]
pub(super) struct Backfill {
    after_id: i64,
    pending: VecDeque<PendingConfig>,
}

impl Indexer {
    pub(super) fn backfill_interval(&self) -> Duration {
        Duration::from_secs(self.config.sync_interval).min(Duration::from_hours(1))
    }

    pub(super) async fn prepare_backfill(&mut self, now: DateTime<Utc>) {
        if self.backfill.is_some() || Instant::now() < self.next_backfill_check {
            return;
        }
        self.next_backfill_check = Instant::now() + LEADER_RETRY_INTERVAL;
        match self.manager.last_config_backfill_at().await {
            Ok(last) => {
                let delay = last.map_or(Duration::ZERO, |last| {
                    discovery_delay(last, now, self.backfill_interval())
                });
                if delay.is_zero() {
                    self.backfill = Some(Backfill::default());
                } else {
                    self.next_backfill_check = Instant::now() + delay;
                }
            }
            Err(e) => error!(error = %e, "Failed to read metadata backfill time"),
        }
    }

    pub(super) async fn backfill_step(&mut self) -> bool {
        if self.backfill.is_none() || !self.check_leadership().await {
            return false;
        }
        let backfill = self.backfill.as_mut().expect("backfill is active");
        if backfill.pending.is_empty() {
            match self
                .manager
                .manifests_missing_config_created(backfill.after_id, 100)
                .await
            {
                Ok(batch) => backfill.pending = batch.into(),
                Err(e) => {
                    error!(error = %e, "Failed to list manifests missing config times");
                    self.backfill = None;
                    self.next_backfill_check = Instant::now() + LEADER_RETRY_INTERVAL;
                    return false;
                }
            }
        }
        let Some(pending) = backfill.pending.pop_front() else {
            self.finish_backfill().await;
            return false;
        };
        backfill.after_id = pending.manifest_id;
        let result = tokio::time::timeout(
            DISCOVERY_TIMEOUT,
            self.manager.backfill_config_created(&pending),
        )
        .await
        .unwrap_or_else(|_| Err(anyhow::anyhow!("config backfill timed out")));
        if let Err(e) = result {
            warn!(registry = %pending.registry, repository = %pending.repository, error = %e, "Failed to fetch config blob");
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
        true
    }

    async fn finish_backfill(&mut self) {
        if !self.check_leadership().await {
            return;
        }
        self.backfill = None;
        match self.manager.record_config_backfill_at(Utc::now()).await {
            Ok(()) => self.next_backfill_check = Instant::now() + self.backfill_interval(),
            Err(e) => {
                error!(error = %e, "Failed to record metadata backfill time");
                self.next_backfill_check = Instant::now() + LEADER_RETRY_INTERVAL;
            }
        }
    }
}
