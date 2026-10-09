use super::*;
use crate::{config::Config, oci::Client, storage::Store};

async fn manager() -> Manager {
    let store = Store::open_in_memory().await.expect("open test store");
    std::fs::remove_dir_all(store.state_info.data_dir()).expect("clean unused test directory");
    let config = Config::default();
    Manager {
        client: Client::new(config.clone()),
        store,
        config,
        offline: true,
    }
}

#[tokio::test]
async fn source_discovery_manager_forwards_without_network_access() {
    let manager = manager().await;
    let reference: Reference = "ghcr.io/example/component:1.0.0"
        .parse()
        .expect("reference");
    let now = DateTime::from_timestamp(1_800_000_000, 0).expect("timestamp");
    assert!(
        manager
            .source_discovery_state(&reference)
            .await
            .expect("read state")
            .is_none()
    );
    manager
        .record_source_discovery_failure(&reference, now, "unavailable")
        .await
        .expect("record failure");
    assert_eq!(
        manager
            .source_discovery_state(&reference)
            .await
            .expect("read state")
            .expect("failure state")
            .failure_count,
        1
    );
    manager
        .record_source_discovery_success(&reference, now)
        .await
        .expect("record success");
    assert_eq!(
        manager
            .source_discovery_state(&reference)
            .await
            .expect("read state"),
        Some(SourceDiscoveryState {
            last_completed_at: Some(now),
            ..SourceDiscoveryState::default()
        })
    );
}

#[tokio::test]
async fn config_backfill_watermark_is_typed_and_independent() {
    let manager = manager().await;
    let now = DateTime::from_timestamp(1_800_000_000, 123_456_789).expect("timestamp");
    assert_eq!(manager.last_config_backfill_at().await.expect("read"), None);
    manager
        .store
        .set_sync_meta(LAST_CONFIG_BACKFILL_KEY, "not a timestamp")
        .await
        .expect("store malformed metadata");
    assert_eq!(manager.last_config_backfill_at().await.expect("read"), None);
    manager
        .store
        .set_sync_meta("indexer_last_discovery_at", &now.to_rfc3339())
        .await
        .expect("seed global discovery watermark");
    let completed = now + chrono::Duration::minutes(10);
    manager
        .record_config_backfill_at(completed)
        .await
        .expect("record config backfill");
    assert_eq!(
        manager.last_config_backfill_at().await.expect("read"),
        Some(completed)
    );
    assert_eq!(
        manager.last_index_discovery_at().await.expect("read"),
        Some(now)
    );
    manager
        .record_index_discovery()
        .await
        .expect("record global");
    assert_eq!(
        manager.last_config_backfill_at().await.expect("read"),
        Some(completed)
    );
    manager
        .store
        .set_sync_meta(LAST_CONFIG_BACKFILL_KEY, "2026-09-25T02:00:00+02:00")
        .await
        .expect("store timezone metadata");
    assert_eq!(
        manager
            .last_config_backfill_at()
            .await
            .expect("read")
            .expect("timestamp")
            .to_rfc3339(),
        "2026-09-25T00:00:00+00:00"
    );
}
