use std::time::Duration;

use tokio::time::Instant;
use tracing::{debug, error, info, warn};
use wasm_package_manager::manager::{Manager, ManagerError};

use super::schedule::ScheduledSource;
use super::{DISCOVERY_TIMEOUT, Indexer, LEADER_RETRY_INTERVAL};

impl Indexer {
    pub(super) async fn discover_next(&mut self) -> bool {
        self.discover_next_with(Self::discover_source).await
    }

    pub(super) async fn discover_next_with(
        &mut self,
        discover: impl AsyncFnOnce(&Manager, &ScheduledSource, bool) -> anyhow::Result<()>,
    ) -> bool {
        let Some(source) = self.discovery.sources.pop_front() else {
            if self.discovery.routine {
                self.finish_discovery_pass().await;
            }
            return false;
        };
        if !self.check_leadership().await {
            return false;
        }
        let result = tokio::time::timeout(
            DISCOVERY_TIMEOUT,
            discover(&self.manager, &source, self.refetch),
        )
        .await
        .unwrap_or_else(|_| Err(anyhow::anyhow!("source discovery timed out")));
        if !self.check_leadership().await {
            return false;
        }
        self.record_discovery_result(&source, result, chrono::Utc::now())
            .await;
        if self.discovery.sources.is_empty() {
            self.finish_discovery_pass().await;
        }
        true
    }

    pub(super) async fn record_discovery_result(
        &mut self,
        source: &ScheduledSource,
        result: anyhow::Result<()>,
        now: chrono::DateTime<chrono::Utc>,
    ) {
        let persisted = match result {
            Ok(()) => {
                self.manager
                    .record_source_discovery_success(&source.reference, now)
                    .await
            }
            Err(e) => {
                error!(reference = %source.reference, error = %e, "Failed to discover source");
                self.manager
                    .record_source_discovery_failure(&source.reference, now, &e.to_string())
                    .await
            }
        };
        if let Err(e) = persisted {
            self.discovery.can_complete = false;
            error!(reference = %source.reference, error = %e, "Failed to persist source discovery state");
        }
    }

    async fn discover_source(
        manager: &Manager,
        scheduled: &ScheduledSource,
        refetch: bool,
    ) -> anyhow::Result<()> {
        let ScheduledSource { source, reference } = scheduled;
        let result = if refetch {
            manager
                .index_package_refetch(
                    reference,
                    Some(&source.namespace),
                    Some(&source.name),
                    Some(source.kind),
                )
                .await
        } else {
            manager
                .index_package(
                    reference,
                    Some(&source.namespace),
                    Some(&source.name),
                    Some(source.kind),
                )
                .await
        };
        match result {
            Ok(_) => Ok(()),
            Err(e) if no_supported_releases(&e) => {
                debug!(%reference, "Source has no supported releases");
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    async fn finish_discovery_pass(&mut self) {
        if !self.discovery.routine {
            if self.discovery.next_check <= Instant::now() {
                self.discovery.next_check = Instant::now()
                    + Duration::from_secs(self.config.sync_interval).min(LEADER_RETRY_INTERVAL);
            }
            return;
        }
        if !self.check_leadership().await {
            return;
        }
        self.refetch = false;
        self.discovery.routine = false;
        self.discovery.next_check = Instant::now() + LEADER_RETRY_INTERVAL;
        if !self.discovery.can_complete {
            warn!("Not recording routine discovery: source state persistence failed");
            return;
        }
        match self.manager.record_index_discovery().await {
            Ok(()) => {
                self.discovery.next_check = Instant::now()
                    + Duration::from_secs(self.config.sync_interval).min(LEADER_RETRY_INTERVAL);
                info!("Routine discovery complete");
            }
            Err(e) => warn!(error = %e, "Failed to record routine discovery time"),
        }
    }
}

fn no_supported_releases(error: &anyhow::Error) -> bool {
    matches!(
        error.downcast_ref::<ManagerError>(),
        Some(ManagerError::NoTagsFound { .. } | ManagerError::NoSemverTags { .. })
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_explicit_successful_empty_or_unsupported_lists_finish_discovery() {
        for error in [
            ManagerError::NoTagsFound {
                registry: "example.test".into(),
                repository: "owner/empty".into(),
            },
            ManagerError::NoSemverTags {
                registry: "example.test".into(),
                repository: "owner/unsupported".into(),
            },
        ] {
            assert!(no_supported_releases(&error.into()));
        }
        assert!(!no_supported_releases(&anyhow::anyhow!(
            "malformed tags response"
        )));
        assert!(!no_supported_releases(&anyhow::anyhow!("HTTP 401")));
        assert!(!no_supported_releases(&ManagerError::OfflineIndex.into()));
    }
}
