//! Leader-elected indexing with independent discovery, ingestion, and backfill.
//!
//! Routine catalog discovery uses a persisted completion timestamp. Initial
//! source discovery and failed scans use their own durable state; neither waits
//! for the routine interval. A completed scan means history was scheduled, not
//! that every release has finished indexing.

use std::time::Duration;

use tokio::time::Instant;
use tracing::{error, info, warn};
use wasm_package_manager::manager::Manager;
use wasm_package_manager::storage::IndexerLease;

use crate::config::Config;

mod backfill;
mod discovery;
mod queue;
mod schedule;

#[cfg(test)]
mod tests;

use backfill::Backfill;
use schedule::DiscoverySchedule;

/// Maximum idle delay, also used when retrying the leader lease.
const LEADER_RETRY_INTERVAL: Duration = Duration::from_mins(1);
/// Bound discovery separately from the worker's existing fetch-task timeout.
const DISCOVERY_TIMEOUT: Duration = Duration::from_mins(1);

/// Background indexer for the approved sources in a registry configuration.
///
/// Only the holder of the database's indexer lease performs work. Source
/// initialization schedules all supported releases; the existing fetch queue
/// retains responsibility for ingestion, retries, and completion.
///
/// ```no_run
/// use component_meta_registry::{Config, Indexer};
/// use wasm_package_manager::manager::Manager;
///
/// # async fn example() -> anyhow::Result<()> {
/// let config = Config::from_registry_dir(
///     std::path::Path::new("registry/"),
///     Config::DEFAULT_SYNC_INTERVAL,
///     "0.0.0.0:8081".into(),
/// )?;
/// Indexer::new(config, Manager::open().await?).run().await;
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct Indexer {
    config: Config,
    manager: Manager,
    refetch: bool,
    lease: Option<IndexerLease>,
    is_leader: bool,
    discovery: DiscoverySchedule,
    backfill: Option<Backfill>,
    next_backfill_check: Instant,
    next_recovery: Instant,
    queue_errors: u32,
    queue_paused_until: Instant,
}

impl Indexer {
    /// Create an indexer with a manager independent of the HTTP server.
    #[must_use]
    pub fn new(config: Config, manager: Manager) -> Self {
        Self {
            config,
            manager,
            refetch: false,
            lease: None,
            is_leader: false,
            discovery: DiscoverySchedule::default(),
            backfill: None,
            next_backfill_check: Instant::now(),
            next_recovery: Instant::now(),
            queue_errors: 0,
            queue_paused_until: Instant::now(),
        }
    }

    /// Explicitly re-fetch all versions during the next routine discovery pass.
    #[must_use]
    pub fn with_refetch(mut self, refetch: bool) -> Self {
        self.refetch = refetch;
        self
    }

    /// Force one discovery pass, draining queued work and running metadata
    /// backfill. Existing version deduplication still applies unless refetch
    /// mode was explicitly enabled.
    ///
    /// Does nothing when another replica holds the indexer lease.
    pub async fn sync(&mut self) {
        if !self.check_leadership().await {
            return;
        }
        if let Err(e) = self.schedule_discovery(chrono::Utc::now(), true).await {
            error!(error = %e, "Failed to schedule discovery");
            return;
        }
        self.backfill = Some(Backfill::default());
        while self.work_step().await && self.is_leader {}
    }

    /// Run indefinitely, with daily-by-default routine scans and independent
    /// initial discovery, retries, queue pickup, and metadata backfill.
    #[allow(clippy::infinite_loop)]
    pub async fn run(mut self) {
        loop {
            if !self.check_leadership().await {
                self.reset_schedule();
                tokio::time::sleep(LEADER_RETRY_INTERVAL).await;
                continue;
            }
            self.prepare_work(chrono::Utc::now()).await;
            if !self.work_step().await {
                tokio::time::sleep(self.idle_delay()).await;
            }
        }
    }

    async fn prepare_work(&mut self, now: chrono::DateTime<chrono::Utc>) {
        if self.discovery.is_idle() && Instant::now() >= self.discovery.next_check {
            // A storage failure should pause discovery, not starve the queue or
            // make us repeatedly sweep all upstreams without reading state.
            self.discovery.next_check = Instant::now() + LEADER_RETRY_INTERVAL;
            if let Err(e) = self.schedule_discovery(now, false).await {
                error!(error = %e, "Failed to schedule discovery");
            }
        }
        self.prepare_backfill(now).await;
    }

    async fn work_step(&mut self) -> bool {
        if !self.check_leadership().await {
            return false;
        }
        let discovered = self.discover_next().await;
        let processed = self.process_queue_step().await;
        let backfilled = self.backfill_step().await;
        discovered || processed || backfilled
    }

    fn idle_delay(&self) -> Duration {
        let now = Instant::now();
        let discovery = self.discovery.next_check.saturating_duration_since(now);
        let backfill = self.next_backfill_check.saturating_duration_since(now);
        LEADER_RETRY_INTERVAL.min(discovery).min(backfill)
    }

    fn reset_schedule(&mut self) {
        self.discovery = DiscoverySchedule::default();
        self.backfill = None;
        self.next_backfill_check = Instant::now();
        self.next_recovery = Instant::now();
    }

    async fn check_leadership(&mut self) -> bool {
        if self.lease.is_none() {
            match self.manager.indexer_lease().await {
                Ok(lease) => self.lease = Some(lease),
                Err(e) => {
                    error!(error = %e, "Failed to open indexer lease");
                    return self.update_leadership(false);
                }
            }
        }
        let lease = self.lease.as_ref().expect("lease opened above");
        let held = match lease.try_hold().await {
            Ok(held) => held,
            Err(e) => {
                warn!(error = %e, "Failed to check indexer lease; dropping it");
                self.lease = None;
                false
            }
        };
        self.update_leadership(held)
    }

    fn update_leadership(&mut self, held: bool) -> bool {
        if held && !self.is_leader {
            info!("Acquired indexer lease; this replica will run the indexer");
        } else if !held && self.is_leader {
            warn!("Lost indexer lease; another replica now runs the indexer");
        }
        self.is_leader = held;
        if !held {
            self.reset_schedule();
        }
        held
    }
}
