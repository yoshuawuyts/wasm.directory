//! Background indexer that syncs package metadata from OCI registries.
//!
//! The indexer periodically iterates over configured package sources, fetches
//! tags and metadata, and stores them in the local database via `Manager`.
//!
//! The indexer uses its own `Manager` instance, separate from the HTTP server's
//! instance. SQLite in WAL mode allows concurrent readers and a single writer,
//! making this safe.
//!
//! When several replicas share one Postgres database, only the replica that
//! holds the [`IndexerLease`] indexes; the others only serve HTTP and retry
//! the lease periodically so one of them takes over if the leader goes away.
//! The time of the last discovery pass is stored in the database, so a
//! restart doesn't trigger a new full discovery before it is due.

use std::time::{Duration, Instant};

use tracing::{error, info, warn};
use wasm_package_manager::Reference;
use wasm_package_manager::manager::{Manager, ManagerError, TaskOutcome};
use wasm_package_manager::storage::{IndexerLease, PendingConfig};

use crate::config::{Config, PackageSource};

/// How often a replica that is not the leader retries the indexer lease.
const LEADER_RETRY_INTERVAL: Duration = Duration::from_mins(1);

/// Background indexer that syncs package metadata from OCI registries.
///
/// # Example
///
/// ```no_run
/// use component_meta_registry::{Config, Indexer};
/// use wasm_package_manager::manager::Manager;
/// use std::path::Path;
///
/// # async fn example() -> anyhow::Result<()> {
/// let config = Config::from_registry_dir(
///     Path::new("registry/"),
///     3600,
///     "0.0.0.0:8080".to_string(),
/// )?;
/// let manager = Manager::open().await?;
/// let indexer = Indexer::new(config, manager);
///
/// // Run the indexer loop (blocks indefinitely)
/// indexer.run().await;
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct Indexer {
    config: Config,
    manager: Manager,
    /// When `true`, re-fetch every version from the registry on the next
    /// discovery pass.
    refetch: bool,
    /// Leader-election handle, opened lazily on the first cycle.
    lease: Option<IndexerLease>,
    /// Whether this process held the lease during the previous check.
    is_leader: bool,
}

impl Indexer {
    /// Create a new indexer with the given configuration and its own manager.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use component_meta_registry::{Config, Indexer};
    /// use wasm_package_manager::manager::Manager;
    /// use std::path::Path;
    ///
    /// # async fn example() -> anyhow::Result<()> {
    /// let config = Config::from_registry_dir(
    ///     Path::new("registry/"),
    ///     3600,
    ///     "0.0.0.0:8080".to_string(),
    /// )?;
    /// let manager = Manager::open().await?;
    /// let indexer = Indexer::new(config, manager);
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn new(config: Config, manager: Manager) -> Self {
        Self {
            config,
            manager,
            refetch: false,
            lease: None,
            is_leader: false,
        }
    }

    /// Enable refetch mode: re-download every version tag from the registry
    /// on the first discovery pass.
    #[must_use]
    pub fn with_refetch(mut self, refetch: bool) -> Self {
        self.refetch = refetch;
        self
    }

    /// Run a single sync cycle: discover tags and enqueue work, then
    /// process the queue until it is drained.
    ///
    /// Does nothing if another replica holds the indexer lease.
    pub async fn sync(&mut self) {
        if !self.check_leadership().await {
            info!("Skipping sync: another replica holds the indexer lease");
            return;
        }
        self.discover().await;
        self.process_queue().await;
        self.backfill_config_created().await;
    }

    /// Discovery phase: iterate configured packages, fetch tags from
    /// the registries, and enqueue pull tasks for any new versions.
    ///
    /// Stops early, without recording the pass as complete, if this
    /// replica loses the indexer lease along the way.
    async fn discover(&mut self) {
        info!(
            "Starting discovery for {} packages",
            self.config.packages.len()
        );

        let mut last_lease_check = Instant::now();
        // Cloned so the lease can be re-checked (`&mut self`) mid-loop.
        let packages = self.config.packages.clone();
        for source in &packages {
            if !self.lease_still_held(&mut last_lease_check).await {
                warn!("Stopping discovery early: indexer lease lost");
                return;
            }
            self.discover_package(source).await;
        }

        // The last packages may have run after the lease was lost, so
        // re-confirm it before marking the pass complete.
        if !self.check_leadership().await {
            warn!("Not recording discovery: indexer lease lost");
            return;
        }

        // Only refetch on the first cycle.
        self.refetch = false;

        if let Err(e) = self.manager.record_index_discovery().await {
            warn!(error = %e, "Failed to record discovery time");
        }
        info!("Discovery complete");
    }

    /// Discover a single package and enqueue its new versions.
    async fn discover_package(&self, source: &PackageSource) {
        let reference_str = format!("{}/{}", source.registry, source.repository);
        let reference = match reference_str.parse::<Reference>() {
            Ok(r) => r,
            Err(e) => {
                warn!(
                    registry = %source.registry,
                    repository = %source.repository,
                    error = %e,
                    "Failed to parse package reference, skipping"
                );
                return;
            }
        };

        let result = if self.refetch {
            self.manager
                .index_package_refetch(
                    &reference,
                    Some(&source.namespace),
                    Some(&source.name),
                    Some(source.kind),
                )
                .await
        } else {
            self.manager
                .index_package(
                    &reference,
                    Some(&source.namespace),
                    Some(&source.name),
                    Some(source.kind),
                )
                .await
        };
        match result {
            Ok(pkg) => {
                tracing::debug!(
                    registry = %pkg.registry,
                    repository = %pkg.repository,
                    tags = pkg.tags.len(),
                    "Discovered package"
                );
            }
            // Packages whose tags are all non-semver (e.g. `vX.Y.Z`) are
            // expected and noisy — demote to debug.
            Err(e) if is_no_semver_tags(&e) => {
                tracing::debug!(
                    registry = %source.registry,
                    repository = %source.repository,
                    "Skipping package — no semver-tagged versions"
                );
            }
            Err(e) => {
                error!(
                    registry = %source.registry,
                    repository = %source.repository,
                    error = %e,
                    "Failed to discover package"
                );
            }
        }
    }

    /// Processing phase: drain the fetch queue, pulling or reindexing
    /// each enqueued version.  A short delay between tasks avoids
    /// hammering upstream registries.
    ///
    /// Tasks abandoned by a worker that died are re-queued first. Stops
    /// early if this replica loses the indexer lease.
    async fn process_queue(&mut self) {
        // Discovery may have taken a while, so confirm the lease before
        // touching the queue and start the re-check timer from here.
        if !self.check_leadership().await {
            return;
        }
        let mut last_lease_check = Instant::now();
        match self.manager.recover_in_progress_tasks().await {
            Ok(0) => {}
            Ok(n) => info!(count = n, "Re-queued tasks abandoned by a previous worker"),
            Err(e) => warn!(error = %e, "Failed to recover in-progress tasks"),
        }

        let mut processed = 0u64;
        // Only counts queue-level errors (network/DB failures from
        // `process_next_task` itself). Individual task failures (a single
        // bad pull or reindex) do NOT count — those are isolated and the
        // queue's own retry/backoff logic already handles them. Counting
        // them here used to cause a few unrelated bad tasks at the head of
        // the queue to block every other (working) task behind them.
        let mut consecutive_queue_errors = 0u64;
        loop {
            if !self.lease_still_held(&mut last_lease_check).await {
                warn!("Stopping queue processing early: indexer lease lost");
                break;
            }
            match self.manager.process_next_task().await {
                Ok(TaskOutcome::Succeeded) => {
                    processed += 1;
                    consecutive_queue_errors = 0;
                    // Brief pause between tasks to be a good citizen to
                    // upstream registries and let the HTTP server breathe.
                    tokio::time::sleep(Duration::from_millis(250)).await;
                }
                Ok(TaskOutcome::Failed) => {
                    processed += 1;
                    // Back off briefly before processing the next task; the
                    // failure is recorded in the queue with its own attempt
                    // counter, so we don't need to gate the worker on it.
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
                Ok(TaskOutcome::Empty) => break, // queue is empty
                Err(e) => {
                    consecutive_queue_errors += 1;
                    error!(error = %e, "Error processing fetch queue");
                    if consecutive_queue_errors >= 5 {
                        error!("Too many consecutive queue errors, pausing until next cycle");
                        break;
                    }
                    // Back off before retrying after an error.
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            }
        }
        if processed > 0 {
            info!(processed, "Fetch queue drained");
        }
    }

    /// Backfill phase: record config-blob publish times for manifests
    /// indexed before we stored them. New pulls record them directly, so
    /// this only has work to do after an upgrade or when earlier fetches
    /// failed (those are retried on the next cycle).
    ///
    /// Stops early if this replica loses the indexer lease.
    async fn backfill_config_created(&mut self) {
        const BATCH: u64 = 100;
        if !self.check_leadership().await {
            return;
        }
        let mut last_lease_check = Instant::now();
        let mut after_id = 0;
        let mut filled = 0u64;
        loop {
            let batch = match self
                .manager
                .manifests_missing_config_created(after_id, BATCH)
                .await
            {
                Ok(batch) => batch,
                Err(e) => {
                    error!(error = %e, "Failed to list manifests missing config times");
                    break;
                }
            };
            let Some(last) = batch.last() else { break };
            after_id = last.manifest_id;
            let (done, lease_held) = self
                .backfill_config_batch(&batch, &mut last_lease_check)
                .await;
            filled += done;
            if !lease_held {
                warn!("Stopping config backfill early: indexer lease lost");
                break;
            }
        }
        if filled > 0 {
            info!(filled, "Backfilled config publish times");
        }
    }

    /// Fetch and record config publish times for one batch, pausing
    /// between fetches. Returns how many succeeded, and whether the indexer
    /// lease is still held (the batch stops early once it is lost).
    async fn backfill_config_batch(
        &mut self,
        batch: &[PendingConfig],
        last_lease_check: &mut Instant,
    ) -> (u64, bool) {
        let mut filled = 0;
        for pending in batch {
            if !self.lease_still_held(last_lease_check).await {
                return (filled, false);
            }
            match self.manager.backfill_config_created(pending).await {
                Ok(()) => filled += 1,
                Err(e) => warn!(
                    registry = %pending.registry,
                    repository = %pending.repository,
                    digest = %pending.config_digest,
                    error = %e,
                    "Failed to fetch config blob"
                ),
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
        (filled, true)
    }

    /// Run the indexer in a loop, syncing at the configured interval.
    ///
    /// Only the replica holding the indexer lease does any work. Discovery
    /// runs at most once per `sync_interval`, including across restarts; the
    /// fetch queue is drained on every cycle.
    ///
    /// This method runs indefinitely and should be spawned as a background task.
    #[allow(clippy::infinite_loop)]
    pub async fn run(mut self) {
        let interval = Duration::from_secs(self.config.sync_interval);
        loop {
            if !self.check_leadership().await {
                tokio::time::sleep(LEADER_RETRY_INTERVAL).await;
                continue;
            }
            let until_due = self.time_until_discovery(interval).await;
            if until_due.is_zero() {
                self.discover().await;
            }
            self.process_queue().await;
            // Backfill alongside discovery so manifests whose config blob
            // fails to fetch are retried once per interval, not every wake-up.
            if until_due.is_zero() {
                self.backfill_config_created().await;
            }
            let next = if until_due.is_zero() {
                interval
            } else {
                until_due
            };
            // Wake up at least every `LEADER_RETRY_INTERVAL` so the lease is
            // renewed and tasks enqueued by `notify` are picked up promptly.
            tokio::time::sleep(next.min(LEADER_RETRY_INTERVAL)).await;
        }
    }

    /// Acquire or re-confirm the indexer lease. Returns `true` when this
    /// process should index.
    async fn check_leadership(&mut self) -> bool {
        if self.lease.is_none() {
            match self.manager.indexer_lease().await {
                Ok(lease) => self.lease = Some(lease),
                Err(e) => {
                    error!(error = %e, "Failed to open indexer lease");
                    self.is_leader = false;
                    return false;
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
        if held && !self.is_leader {
            info!("Acquired indexer lease; this replica will run the indexer");
        } else if !held && self.is_leader {
            warn!("Lost indexer lease; another replica now runs the indexer");
        }
        self.is_leader = held;
        held
    }

    /// Re-confirm the lease if the last check is older than
    /// `LEADER_RETRY_INTERVAL`, so a replica that lost the lease mid-cycle
    /// stops working instead of competing with the new leader.
    async fn lease_still_held(&mut self, last_check: &mut Instant) -> bool {
        if !self.is_leader {
            return false;
        }
        if last_check.elapsed() < LEADER_RETRY_INTERVAL {
            return true;
        }
        *last_check = Instant::now();
        self.check_leadership().await
    }

    /// How long until the next discovery pass is due. Zero means now.
    async fn time_until_discovery(&self, interval: Duration) -> Duration {
        if self.refetch {
            return Duration::ZERO;
        }
        let last = match self.manager.last_index_discovery_at().await {
            Ok(Some(last)) => last,
            Ok(None) => return Duration::ZERO,
            Err(e) => {
                warn!(error = %e, "Failed to read last discovery time");
                return Duration::ZERO;
            }
        };
        discovery_delay(last, chrono::Utc::now(), interval)
    }
}

/// Whether an indexing error means the package has no semver tags.
fn is_no_semver_tags(e: &anyhow::Error) -> bool {
    matches!(
        e.downcast_ref::<ManagerError>(),
        Some(ManagerError::NoSemverTags { .. })
    )
}

/// Remaining time until a discovery pass is due, given when the last one
/// finished. A timestamp in the future (clock skew) counts as "just ran".
fn discovery_delay(
    last: chrono::DateTime<chrono::Utc>,
    now: chrono::DateTime<chrono::Utc>,
    interval: Duration,
) -> Duration {
    let elapsed = (now - last).to_std().unwrap_or(Duration::ZERO);
    interval.saturating_sub(elapsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovery_due_when_interval_elapsed() {
        let now = chrono::Utc::now();
        let last = now - chrono::Duration::seconds(7200);
        assert_eq!(
            discovery_delay(last, now, Duration::from_secs(3600)),
            Duration::ZERO
        );
    }

    #[test]
    fn discovery_waits_for_remaining_interval() {
        let now = chrono::Utc::now();
        let last = now - chrono::Duration::seconds(600);
        assert_eq!(
            discovery_delay(last, now, Duration::from_secs(3600)),
            Duration::from_secs(3000)
        );
    }

    #[test]
    fn discovery_delay_handles_future_timestamps() {
        let now = chrono::Utc::now();
        let last = now + chrono::Duration::seconds(60);
        assert_eq!(
            discovery_delay(last, now, Duration::from_secs(3600)),
            Duration::from_secs(3600)
        );
    }
}
