//! Durable progress for discovering a configured OCI source's tags.

use chrono::{DateTime, Utc};

/// Repository-wide discovery progress, not ingestion or package readiness.
///
/// Completion means that tags were fully enumerated and durably enqueued.
/// Fetch tasks may still be pending or failed, and no usable OCI repository
/// or package needs to exist yet.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourceDiscoveryState {
    /// Last successful enumeration and durable enqueue of the source's tags.
    pub last_completed_at: Option<DateTime<Utc>>,
    /// Earliest retry after a discovery failure.
    pub next_retry_at: Option<DateTime<Utc>>,
    /// Consecutive discovery failures, reset on success and saturated at `u32::MAX`.
    pub failure_count: u32,
    /// Error from the most recent failed discovery, cleared on success.
    pub last_error: Option<String>,
}

impl SourceDiscoveryState {
    /// Whether discovery has never completed or a failed attempt is due for retry.
    ///
    /// A retry deadline takes precedence over any previous successful discovery.
    /// Periodic refreshes of completed sources are scheduled separately.
    #[must_use]
    pub fn needs_discovery(&self, now: DateTime<Utc>) -> bool {
        match self.next_retry_at {
            Some(next_retry_at) => now >= next_retry_at,
            None => self.last_completed_at.is_none(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn source_discovery_retry_deadline_takes_precedence_over_completion() {
        let now = DateTime::from_timestamp(1_800_000_000, 0).expect("valid timestamp");
        let mut state = SourceDiscoveryState::default();
        assert!(state.needs_discovery(now));
        state.next_retry_at = Some(now);
        for completed in [None, Some(now - Duration::hours(1))] {
            state.last_completed_at = completed;
            assert!(!state.needs_discovery(now - Duration::nanoseconds(1)));
            assert!(state.needs_discovery(now));
            assert!(state.needs_discovery(now + Duration::nanoseconds(1)));
        }
        state.next_retry_at = None;
        assert!(!state.needs_discovery(now));
    }
}
