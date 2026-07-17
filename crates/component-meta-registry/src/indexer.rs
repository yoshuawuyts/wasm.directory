//! Background indexer that syncs package metadata from OCI registries.
//!
//! The indexer periodically iterates over configured package sources, fetches
//! tags and metadata, and stores them in the local database via `Manager`.
//!
//! The indexer uses its own `Manager` instance, separate from the HTTP server's
//! instance. SQLite in WAL mode allows concurrent readers and a single writer,
//! making this safe.

use std::time::Duration;

use tracing::{error, info, warn};
use wasm_package_manager::Reference;
use wasm_package_manager::manager::{Manager, TaskOutcome};

use crate::config::Config;

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
    /// When `true`, bypass the per-tag pull cooldown and re-fetch every
    /// version from the registry.
    refetch: bool,
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
        }
    }

    /// Enable refetch mode: bypass pull cooldowns and re-download every
    /// version tag from the registry.
    #[must_use]
    pub fn with_refetch(mut self, refetch: bool) -> Self {
        self.refetch = refetch;
        self
    }

    /// Run a single sync cycle: discover tags and enqueue work, then
    /// process the queue until it is drained.
    pub async fn sync(&mut self) {
        self.discover().await;
        self.process_queue().await;
    }

    /// Discovery phase: iterate configured packages, fetch tags from
    /// the registries, and enqueue pull tasks for any new or stale
    /// versions.
    async fn discover(&mut self) {
        info!(
            "Starting discovery for {} packages",
            self.config.packages.len()
        );

        for source in &self.config.packages {
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
                    continue;
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
                Err(e) => {
                    // Packages whose tags are all non-semver (e.g. `vX.Y.Z`)
                    // are expected and noisy — demote to debug.
                    if e.downcast_ref::<wasm_package_manager::manager::ManagerError>()
                        .is_some_and(|m| {
                            matches!(
                                m,
                                wasm_package_manager::manager::ManagerError::NoSemverTags { .. }
                            )
                        })
                    {
                        tracing::debug!(
                            registry = %source.registry,
                            repository = %source.repository,
                            "Skipping package — no semver-tagged versions"
                        );
                    } else {
                        error!(
                            registry = %source.registry,
                            repository = %source.repository,
                            error = %e,
                            "Failed to discover package"
                        );
                    }
                }
            }
        }

        // Only refetch on the first cycle.
        self.refetch = false;

        info!("Discovery complete");
    }

    /// Processing phase: drain the fetch queue, pulling or reindexing
    /// each enqueued version.  A short delay between tasks avoids
    /// hammering upstream registries.
    async fn process_queue(&mut self) {
        let mut processed = 0u64;
        // Only counts queue-level errors (network/DB failures from
        // `process_next_task` itself). Individual task failures (a single
        // bad pull or reindex) do NOT count — those are isolated and the
        // queue's own retry/backoff logic already handles them. Counting
        // them here used to cause a few unrelated bad tasks at the head of
        // the queue to block every other (working) task behind them.
        let mut consecutive_queue_errors = 0u64;
        loop {
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

    /// Run the indexer in a loop, syncing at the configured interval.
    ///
    /// This method runs indefinitely and should be spawned as a background task.
    #[allow(clippy::infinite_loop)]
    pub async fn run(mut self) {
        let interval = Duration::from_secs(self.config.sync_interval);

        // Run an initial sync immediately
        self.sync().await;

        loop {
            tokio::time::sleep(interval).await;
            self.sync().await;
        }
    }
}
