use chrono::{DateTime, TimeZone, Utc};
use wasm_package_manager::Reference;
use wasm_package_manager::storage::SourceDiscoveryState;

use super::schedule::{discovery_delay, should_discover};
use super::*;
use crate::config::{PackageKind, PackageSource};

fn now() -> DateTime<Utc> {
    Utc.timestamp_opt(1_800_000_000, 0)
        .single()
        .expect("valid date")
}

fn source(name: &str) -> PackageSource {
    PackageSource {
        registry: "example.test/owner".into(),
        repository: name.into(),
        namespace: "example".into(),
        name: name.into(),
        kind: PackageKind::Interface,
    }
}

fn reference(name: &str) -> Reference {
    format!("example.test/owner/{name}")
        .parse()
        .expect("reference")
}

async fn indexer(packages: Vec<PackageSource>) -> (tempfile::TempDir, Indexer) {
    let dir = tempfile::tempdir().expect("tempdir");
    let manager = Manager::open_at(dir.path())
        .await
        .expect("open isolated manager");
    let config = Config {
        sync_interval: 86_400,
        bind: "127.0.0.1:0".into(),
        packages,
    };
    (dir, Indexer::new(config, manager))
}

#[test]
fn daily_threshold_overrides_and_clock_skew() {
    for seconds in [600, 3_600, 86_400, 172_800] {
        let interval = Duration::from_secs(seconds);
        let end = now() + chrono::Duration::from_std(interval).expect("duration");
        assert_eq!(
            discovery_delay(now(), end - chrono::Duration::seconds(1), interval),
            Duration::from_secs(1)
        );
        assert_eq!(discovery_delay(now(), end, interval), Duration::ZERO);
        assert_eq!(
            discovery_delay(now(), end + chrono::Duration::seconds(1), interval),
            Duration::ZERO
        );
        assert_eq!(discovery_delay(end, now(), interval), interval);
    }
}

#[test]
fn retry_is_independent_of_routine_and_not_source_readiness() {
    assert_eq!(should_discover(None, now(), false, false), Some(true));
    let mut state = SourceDiscoveryState {
        last_completed_at: Some(now()),
        next_retry_at: None,
        failure_count: 0,
        last_error: None,
    };
    assert_eq!(should_discover(Some(&state), now(), false, false), None);
    assert_eq!(
        should_discover(Some(&state), now(), true, false),
        Some(false)
    );
    state.next_retry_at = Some(now() + chrono::Duration::minutes(1));
    assert_eq!(should_discover(Some(&state), now(), true, false), None);
    assert_eq!(
        should_discover(
            Some(&state),
            now() + chrono::Duration::minutes(1),
            false,
            false
        ),
        Some(false)
    );
    assert_eq!(
        should_discover(Some(&state), now(), true, true),
        Some(false)
    );
}

#[tokio::test]
async fn persisted_watermark_blocks_known_sources_but_not_new_or_stub_sources() {
    let (dir, first) = indexer(vec![source("known"), source("new"), source("stub")]).await;
    first
        .manager
        .record_index_discovery()
        .await
        .expect("routine watermark");
    first
        .manager
        .record_source_discovery_success(&reference("known"), Utc::now())
        .await
        .expect("source scan");
    first
        .manager
        .add_known_package("example.test", "owner/stub", None, None)
        .await
        .expect("source row only");
    let last = first
        .manager
        .last_index_discovery_at()
        .await
        .expect("query")
        .expect("time");
    let config = first.config.clone();
    drop(first);
    let manager = Manager::open_at(dir.path())
        .await
        .expect("reopen persisted database");
    let mut restarted = Indexer::new(config, manager);

    restarted
        .schedule_discovery(last + chrono::Duration::hours(1), false)
        .await
        .expect("schedule");
    assert!(!restarted.discovery.routine);
    let names: Vec<_> = restarted
        .discovery
        .sources
        .iter()
        .map(|scheduled| scheduled.source.name.as_str())
        .collect();
    assert_eq!(names, ["new", "stub"]);
    restarted
        .schedule_discovery(last + chrono::Duration::hours(24), false)
        .await
        .expect("daily schedule");
    assert!(restarted.discovery.routine);
    let names: Vec<_> = restarted
        .discovery
        .sources
        .iter()
        .map(|scheduled| scheduled.source.name.as_str())
        .collect();
    assert_eq!(
        names,
        ["new", "stub", "known"],
        "bootstrap precedes routine rescans"
    );
}

#[tokio::test]
async fn duplicate_coordinates_schedule_only_once_and_do_not_advance_routine_time() {
    let (_dir, mut indexer) = indexer(vec![source("new"), source("new")]).await;
    indexer
        .manager
        .record_index_discovery()
        .await
        .expect("watermark");
    let before = indexer
        .manager
        .last_index_discovery_at()
        .await
        .expect("last");
    indexer
        .schedule_discovery(Utc::now(), false)
        .await
        .expect("schedule");
    assert_eq!(indexer.discovery.sources.len(), 1);
    let source = indexer
        .discovery
        .sources
        .pop_front()
        .expect("initial source");
    indexer
        .record_discovery_result(&source, Ok(()), Utc::now())
        .await;
    indexer
        .schedule_discovery(Utc::now(), false)
        .await
        .expect("reschedule");
    assert!(indexer.discovery.sources.is_empty());
    assert_eq!(
        indexer
            .manager
            .last_index_discovery_at()
            .await
            .expect("last"),
        before
    );
    assert!(
        indexer
            .manager
            .get_known_package("example.test", "owner/new")
            .await
            .expect("lookup")
            .is_none(),
        "a scheduled scan does not fabricate a usable release"
    );
}

#[tokio::test]
async fn failed_source_retries_without_delaying_a_healthy_source_or_daily_sweep() {
    let (_dir, mut indexer) = indexer(vec![source("bad"), source("good")]).await;
    indexer
        .manager
        .record_index_discovery()
        .await
        .expect("watermark");
    let time = Utc::now();
    indexer
        .schedule_discovery(time, false)
        .await
        .expect("schedule");
    let bad = indexer.discovery.sources.pop_front().expect("bad source");
    indexer
        .record_discovery_result(&bad, Err(anyhow::anyhow!("upstream unavailable")), time)
        .await;
    let good = indexer
        .discovery
        .sources
        .pop_front()
        .expect("healthy source not blocked");
    indexer.record_discovery_result(&good, Ok(()), time).await;
    indexer
        .schedule_discovery(time + chrono::Duration::seconds(59), false)
        .await
        .expect("before retry");
    assert!(indexer.discovery.sources.is_empty());
    indexer
        .schedule_discovery(time + chrono::Duration::seconds(60), false)
        .await
        .expect("retry due");
    assert!(!indexer.discovery.routine);
    assert_eq!(indexer.discovery.sources.len(), 1);
    assert_eq!(
        indexer
            .discovery
            .sources
            .front()
            .expect("retry")
            .source
            .name,
        "bad"
    );
}

#[tokio::test]
async fn backfill_has_its_own_persisted_hourly_clock() {
    let (_dir, mut indexer) = indexer(Vec::new()).await;
    indexer
        .manager
        .record_config_backfill_at(now())
        .await
        .expect("backfill time");
    indexer
        .prepare_backfill(now() + chrono::Duration::seconds(3_599))
        .await;
    assert!(indexer.backfill.is_none());
    indexer.next_backfill_check = Instant::now();
    indexer
        .prepare_backfill(now() + chrono::Duration::hours(1))
        .await;
    assert!(indexer.backfill.is_some());
    assert_eq!(indexer.backfill_interval(), Duration::from_hours(1));
    indexer.config.sync_interval = 600;
    assert_eq!(indexer.backfill_interval(), Duration::from_mins(10));
}

#[tokio::test]
async fn empty_queue_steps_do_not_postpone_scheduled_discovery() {
    let (_dir, mut indexer) = indexer(Vec::new()).await;
    tokio::time::pause();
    let next = Instant::now() + LEADER_RETRY_INTERVAL;
    indexer.discovery.next_check = next;
    for _ in 0..3 {
        assert!(!indexer.discover_next().await);
        tokio::time::advance(Duration::from_secs(20)).await;
    }
    assert_eq!(indexer.discovery.next_check, next);
    assert!(Instant::now() >= indexer.discovery.next_check);
}

#[tokio::test]
async fn bounded_discovery_failure_does_not_block_the_next_source() {
    let (_dir, mut indexer) = indexer(vec![source("slow"), source("healthy")]).await;
    indexer
        .manager
        .record_index_discovery()
        .await
        .expect("watermark");
    indexer
        .schedule_discovery(Utc::now(), false)
        .await
        .expect("schedule");
    assert!(
        indexer
            .discover_next_with(async |_, _, _| {
                // SQLite uses blocking I/O: resume before persistence, even when the
                // stub future is cancelled by the virtual timeout.
                struct ResumeClock;
                impl Drop for ResumeClock {
                    fn drop(&mut self) {
                        tokio::time::resume();
                    }
                }
                tokio::time::pause();
                let _clock = ResumeClock;
                tokio::time::advance(DISCOVERY_TIMEOUT).await;
                std::future::pending::<anyhow::Result<()>>().await
            })
            .await
    );
    let failure = indexer
        .manager
        .source_discovery_state(&reference("slow"))
        .await
        .expect("state")
        .expect("failure persisted");
    assert!(failure.last_completed_at.is_none());
    assert!(failure.next_retry_at.is_some());
    assert!(failure.last_error.expect("error").contains("timed out"));
    assert!(
        indexer
            .discover_next_with(async |_, source, refetch| {
                assert_eq!(source.source.name, "healthy");
                assert!(!refetch);
                Ok(())
            })
            .await
    );
    assert!(
        indexer
            .manager
            .source_discovery_state(&reference("healthy"))
            .await
            .expect("state")
            .expect("scan")
            .last_completed_at
            .is_some()
    );
}

#[tokio::test]
async fn recent_sweep_does_not_delay_initial_history_or_queue_pickup() {
    use wasm_package_manager::manager::TaskOutcome;

    let (_dir, mut indexer) = indexer(vec![source("new")]).await;
    indexer
        .manager
        .record_index_discovery()
        .await
        .expect("watermark");
    let watermark = indexer
        .manager
        .last_index_discovery_at()
        .await
        .expect("last");
    indexer.prepare_work(Utc::now()).await;
    assert!(!indexer.discovery.routine);

    // The stub upstream schedules its entire history, not only its latest tag.
    assert!(
        indexer
            .discover_next_with(async |manager, source, _| {
                for tag in ["1.0.0", "1.1.0", "2.0.0"] {
                    manager
                        .notify_new_version(
                            source.reference.registry(),
                            source.reference.repository(),
                            tag,
                        )
                        .await?;
                }
                Ok(())
            })
            .await
    );
    let status = indexer.manager.get_queue_status().await.expect("queue");
    assert_eq!(status.pending, 3);
    assert_eq!(status.completed, 0, "discovered is not ingested");

    assert!(
        indexer
            .process_queue_step_with(async |manager| {
                assert_eq!(manager.get_queue_status().await?.pending, 3);
                Ok(TaskOutcome::Succeeded)
            })
            .await,
        "worker runs independently of routine discovery"
    );
    assert_eq!(
        indexer
            .manager
            .last_index_discovery_at()
            .await
            .expect("last"),
        watermark
    );
}

#[tokio::test]
async fn queue_errors_back_off_without_suppressing_individual_task_failures() {
    use wasm_package_manager::manager::TaskOutcome;

    let (_dir, mut indexer) = indexer(Vec::new()).await;
    for _ in 0..4 {
        assert!(
            indexer
                .process_queue_step_with(async |_| { Err(anyhow::anyhow!("database unavailable")) })
                .await
        );
    }
    assert!(
        !indexer
            .process_queue_step_with(async |_| { Err(anyhow::anyhow!("database unavailable")) })
            .await
    );
    assert!(indexer.queue_paused_until > Instant::now());
    tokio::time::pause();
    tokio::time::advance(LEADER_RETRY_INTERVAL).await;
    tokio::time::resume();
    assert!(
        indexer
            .process_queue_step_with(async |_| Ok(TaskOutcome::Failed))
            .await
    );
    assert_eq!(indexer.queue_errors, 0);
    assert!(
        indexer
            .process_queue_step_with(async |_| Ok(TaskOutcome::Succeeded))
            .await
    );
}

#[tokio::test]
async fn lease_loss_discards_incomplete_passes_without_recording_completion() {
    let (_dir, mut indexer) = indexer(vec![source("first"), source("second")]).await;
    indexer.update_leadership(true);
    indexer.prepare_work(Utc::now()).await;
    assert!(indexer.discovery.routine);
    assert!(indexer.backfill.is_some());
    let first = indexer.discovery.sources.pop_front().expect("first source");
    indexer
        .record_discovery_result(&first, Ok(()), Utc::now())
        .await;

    indexer.update_leadership(false);
    assert!(indexer.discovery.is_idle());
    assert!(indexer.backfill.is_none());
    assert!(
        indexer
            .manager
            .last_index_discovery_at()
            .await
            .expect("last")
            .is_none()
    );
    assert!(
        indexer
            .manager
            .last_config_backfill_at()
            .await
            .expect("backfill")
            .is_none()
    );

    indexer.update_leadership(true);
    indexer.prepare_work(Utc::now()).await;
    assert_eq!(
        indexer
            .discovery
            .sources
            .front()
            .expect("unfinished source")
            .source
            .name,
        "second"
    );
    assert!(
        indexer.discovery.routine,
        "the interrupted routine pass is still due"
    );
}

#[tokio::test]
async fn explicit_sync_bypasses_discovery_backoff_without_enabling_refetch() {
    let (_dir, mut indexer) = indexer(vec![source("retry")]).await;
    let time = Utc::now();
    indexer
        .manager
        .record_source_discovery_failure(&reference("retry"), time, "temporary failure")
        .await
        .expect("record retry");
    indexer
        .schedule_discovery(time, false)
        .await
        .expect("routine plan");
    assert!(indexer.discovery.sources.is_empty());
    indexer
        .schedule_discovery(time, true)
        .await
        .expect("explicit sync");
    assert_eq!(indexer.discovery.sources.len(), 1);
    assert!(!indexer.refetch, "force discovery is not force downloading");
}

#[tokio::test]
async fn completed_refetch_attempt_is_not_repeated_on_state_write_failure() {
    let (_dir, mut indexer) = indexer(Vec::new()).await;
    indexer.refetch = true;
    indexer.discovery.routine = true;
    indexer.discovery.can_complete = false;
    assert!(!indexer.discover_next().await);
    assert!(
        !indexer.refetch,
        "explicit refetch applies only to its first pass"
    );
    assert!(
        indexer
            .manager
            .last_index_discovery_at()
            .await
            .expect("last")
            .is_none()
    );
}

#[tokio::test]
async fn invalid_configured_reference_does_not_suppress_other_sources() {
    let (_dir, mut indexer) = indexer(vec![source("invalid space"), source("valid")]).await;
    indexer
        .schedule_discovery(Utc::now(), false)
        .await
        .expect("schedule valid sources");
    assert_eq!(indexer.discovery.sources.len(), 1);
    assert_eq!(
        indexer
            .discovery
            .sources
            .front()
            .expect("valid source")
            .source
            .name,
        "valid"
    );
    assert!(
        !indexer.discovery.can_complete,
        "invalid configuration remains visible"
    );
}
