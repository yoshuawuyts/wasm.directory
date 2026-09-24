//! Short-lived cache for index-wide registry stats.
//!
//! Computing [`RegistryStats`] scans the whole index, and the frontend asks
//! for it on every home page render. The counts only change when the indexer
//! syncs, so serving a value up to [`STATS_TTL`] old is fine.

use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};

use wasm_meta_registry_types::RegistryStats;

/// How long a computed [`RegistryStats`] value is served before recomputing.
pub const STATS_TTL: Duration = Duration::from_mins(1);

/// Caches the most recent [`RegistryStats`] for a fixed time-to-live.
///
/// Cloning is cheap and clones share the same cached value. The lock is held
/// while recomputing, so concurrent requests on a cold cache wait for a single
/// computation instead of each scanning the index.
///
/// # Example
///
/// ```
/// use std::time::Duration;
/// use component_meta_registry::stats_cache::StatsCache;
/// use wasm_meta_registry_types::RegistryStats;
///
/// # async fn example() -> anyhow::Result<()> {
/// let cache = StatsCache::new(Duration::from_mins(1));
/// let stats = cache
///     .get_or_compute(|| async { Ok(RegistryStats::default()) })
///     .await?;
/// assert_eq!(stats, RegistryStats::default());
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct StatsCache {
    ttl: Duration,
    entry: Arc<tokio::sync::Mutex<Option<(Instant, RegistryStats)>>>,
}

impl StatsCache {
    /// Create an empty cache whose entries expire after `ttl`.
    #[must_use]
    pub fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            entry: Arc::default(),
        }
    }

    /// Return the cached stats if still fresh, otherwise run `compute`,
    /// cache its result, and return it. Errors are not cached.
    pub async fn get_or_compute<F, Fut>(&self, compute: F) -> anyhow::Result<RegistryStats>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = anyhow::Result<RegistryStats>>,
    {
        let mut entry = self.entry.lock().await;
        if let Some((computed_at, stats)) = *entry
            && computed_at.elapsed() < self.ttl
        {
            return Ok(stats);
        }
        let stats = compute().await?;
        *entry = Some((Instant::now(), stats));
        Ok(stats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    async fn counted(cache: &StatsCache, calls: &AtomicU64) -> RegistryStats {
        cache
            .get_or_compute(|| async {
                let n = calls.fetch_add(1, Ordering::SeqCst) + 1;
                Ok(RegistryStats {
                    packages: n,
                    namespaces: 0,
                    versions: 0,
                })
            })
            .await
            .expect("compute should succeed")
    }

    #[tokio::test]
    async fn fresh_value_is_reused() {
        let cache = StatsCache::new(Duration::from_mins(1));
        let calls = AtomicU64::new(0);
        assert_eq!(counted(&cache, &calls).await.packages, 1);
        assert_eq!(counted(&cache, &calls).await.packages, 1);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn expired_value_is_recomputed() {
        let cache = StatsCache::new(Duration::ZERO);
        let calls = AtomicU64::new(0);
        assert_eq!(counted(&cache, &calls).await.packages, 1);
        assert_eq!(counted(&cache, &calls).await.packages, 2);
    }

    #[tokio::test]
    async fn errors_are_not_cached() {
        let cache = StatsCache::new(Duration::from_mins(1));
        let err = cache
            .get_or_compute(|| async { Err(anyhow::anyhow!("db down")) })
            .await;
        assert!(err.is_err());
        let calls = AtomicU64::new(0);
        assert_eq!(counted(&cache, &calls).await.packages, 1);
    }
}
