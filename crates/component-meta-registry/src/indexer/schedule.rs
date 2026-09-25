use std::collections::{HashSet, VecDeque};
use std::time::Duration;

use chrono::{DateTime, Utc};
use tokio::time::Instant;
use tracing::error;
use wasm_package_manager::Reference;
use wasm_package_manager::storage::SourceDiscoveryState;

use super::{Indexer, LEADER_RETRY_INTERVAL};
use crate::config::PackageSource;

#[derive(Debug)]
pub(super) struct ScheduledSource {
    pub source: PackageSource,
    pub reference: Reference,
}

#[derive(Debug)]
pub(super) struct DiscoverySchedule {
    pub sources: VecDeque<ScheduledSource>,
    pub routine: bool,
    pub can_complete: bool,
    pub next_check: Instant,
}

impl Default for DiscoverySchedule {
    fn default() -> Self {
        Self {
            sources: VecDeque::new(),
            routine: false,
            can_complete: true,
            next_check: Instant::now(),
        }
    }
}

impl DiscoverySchedule {
    pub(super) fn is_idle(&self) -> bool {
        self.sources.is_empty() && !self.routine
    }
}

impl Indexer {
    pub(super) async fn schedule_discovery(
        &mut self,
        now: DateTime<Utc>,
        force: bool,
    ) -> anyhow::Result<()> {
        let interval = Duration::from_secs(self.config.sync_interval);
        let last = self.manager.last_index_discovery_at().await?;
        let delay = last.map_or(Duration::ZERO, |last| discovery_delay(last, now, interval));
        let routine = force || self.refetch || delay.is_zero();
        let mut initial = VecDeque::new();
        let mut rescans = VecDeque::new();
        let mut seen = HashSet::new();
        let mut can_complete = true;
        let mut next = LEADER_RETRY_INTERVAL.min(if routine { interval } else { delay });

        for source in &self.config.packages {
            let reference = match format!("{}/{}", source.registry, source.repository)
                .parse::<Reference>()
            {
                Ok(reference) => reference,
                Err(e) => {
                    error!(registry = %source.registry, repository = %source.repository, error = %e, "Invalid configured source");
                    can_complete = false;
                    continue;
                }
            };
            let key = (
                reference.registry().to_owned(),
                reference.repository().to_owned(),
            );
            if !seen.insert(key) {
                continue;
            }
            let state = self.manager.source_discovery_state(&reference).await?;
            if let Some(retry) = state.as_ref().and_then(|state| state.next_retry_at) {
                next = next.min((retry - now).to_std().unwrap_or(Duration::ZERO));
            }
            let Some(initial_scan) =
                should_discover(state.as_ref(), now, routine, force || self.refetch)
            else {
                continue;
            };
            let scheduled = ScheduledSource {
                source: source.clone(),
                reference,
            };
            if initial_scan {
                initial.push_back(scheduled);
            } else {
                rescans.push_back(scheduled);
            }
        }
        initial.extend(rescans);
        self.discovery = DiscoverySchedule {
            sources: initial,
            routine,
            can_complete,
            next_check: Instant::now() + next,
        };
        Ok(())
    }
}

/// Returns whether this is initial work; `None` means the source is not due.
pub(super) fn should_discover(
    state: Option<&SourceDiscoveryState>,
    now: DateTime<Utc>,
    routine: bool,
    force: bool,
) -> Option<bool> {
    let Some(state) = state else {
        return Some(true);
    };
    if force {
        return Some(state.last_completed_at.is_none());
    }
    if state.next_retry_at.is_some_and(|retry| retry > now) {
        return None;
    }
    if state.needs_discovery(now) || routine {
        return Some(state.last_completed_at.is_none());
    }
    None
}

/// Future timestamps count as "just ran", rather than overflowing a duration.
pub(super) fn discovery_delay(
    last: DateTime<Utc>,
    now: DateTime<Utc>,
    interval: Duration,
) -> Duration {
    interval.saturating_sub((now - last).to_std().unwrap_or(Duration::ZERO))
}
