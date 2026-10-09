use std::path::Path;

use sea_orm::{ColumnTrait, ConnectOptions, Database, PaginatorTrait, QueryFilter};
use wasm_package_manager_migration::{
    Migrator, MigratorTrait,
    entities::{fetch_queue, oci_repository},
};

use super::*;
use crate::storage::{Migrations, StateInfo};

fn now() -> DateTime<Utc> {
    DateTime::from_timestamp(1_800_000_000, 123_456_789).expect("valid timestamp")
}

fn reference() -> Reference {
    "ghcr.io/example/component:1.0.0"
        .parse()
        .expect("valid reference")
}

async fn open_store(url: &str, path: &Path) -> Store {
    let mut options = ConnectOptions::new(url);
    options.max_connections(1).sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("connect test SQLite");
    super::super::apply_sqlite_pragmas(&db)
        .await
        .expect("set SQLite pragmas");
    Migrator::up(&db, None).await.expect("apply migrations");
    let migrations = Migrations::snapshot(&db).await;
    Store {
        state_info: StateInfo::new_at(path.to_owned(), path.join("config.toml"), &migrations, 0, 0),
        db,
        db_config: None,
    }
}

async fn memory_store() -> Store {
    open_store("sqlite::memory:", Path::new(".")).await
}

async fn state(store: &Store, reference: &Reference) -> SourceDiscoveryState {
    store
        .source_discovery_state(reference)
        .await
        .expect("read discovery")
        .expect("stored discovery")
}

#[tokio::test]
async fn canonical_source_identity_ignores_tags_and_digests() {
    let store = memory_store().await;
    let implicit: Reference = "busybox:1.0.0".parse().expect("implicit reference");
    let explicit: Reference = "docker.io/library/busybox:2.0.0"
        .parse()
        .expect("explicit reference");
    let digest: Reference = format!("docker.io/library/busybox@sha256:{}", "a".repeat(64))
        .parse()
        .expect("digest reference");
    assert_eq!(implicit.registry(), explicit.registry());
    assert_eq!(implicit.repository(), explicit.repository());
    assert!(
        store
            .source_discovery_state(&implicit)
            .await
            .expect("read empty state")
            .is_none()
    );
    store
        .record_source_discovery_success(&implicit, now())
        .await
        .expect("record success");
    store
        .record_source_discovery_failure(&explicit, now(), "unavailable")
        .await
        .expect("record failure");
    let saved = state(&store, &digest).await;
    assert_eq!(saved.last_completed_at, Some(now()));
    assert_eq!(saved.failure_count, 1);
    assert_eq!(
        source_discovery::Entity::find()
            .count(&store.db)
            .await
            .expect("count identities"),
        1
    );
    for other in [
        "ghcr.io/library/busybox:1.0.0",
        "docker.io/library/other:1.0.0",
    ] {
        store
            .record_source_discovery_success(&other.parse().expect("other reference"), now())
            .await
            .expect("record distinct source");
    }
    assert_eq!(
        source_discovery::Entity::find()
            .count(&store.db)
            .await
            .expect("count distinct identities"),
        3
    );
    assert_eq!(
        oci_repository::Entity::find()
            .count(&store.db)
            .await
            .expect("count repositories"),
        0,
        "discovery does not require or create an ingested repository"
    );
}

#[tokio::test]
async fn legacy_repository_and_queue_rows_do_not_prove_discovery() {
    let store = memory_store().await;
    let reference = reference();
    store
        .add_known_package(reference.registry(), reference.repository(), None, None)
        .await
        .expect("create legacy repository");
    store
        .record_completed(reference.registry(), reference.repository(), "1.0.0")
        .await
        .expect("create legacy completed task");
    assert!(
        store
            .source_discovery_state(&reference)
            .await
            .expect("read discovery")
            .is_none()
    );
}

#[tokio::test]
async fn failure_backoff_doubles_caps_and_preserves_completion() {
    let store = memory_store().await;
    let reference = reference();
    store
        .record_source_discovery_success(&reference, now())
        .await
        .expect("initial success");
    let mut attempt_at = now();
    for (attempt, seconds) in [60, 120, 240, 480, 960, 1920, 3600, 3600, 3600]
        .into_iter()
        .enumerate()
    {
        let error = format!("failure {attempt}");
        store
            .record_source_discovery_failure(&reference, attempt_at, &error)
            .await
            .expect("record failure");
        let saved = state(&store, &reference).await;
        let deadline = attempt_at + Duration::seconds(seconds);
        assert_eq!(
            saved.failure_count,
            u32::try_from(attempt + 1).expect("count")
        );
        assert_eq!(saved.last_completed_at, Some(now()));
        assert_eq!(saved.last_error.as_deref(), Some(error.as_str()));
        assert_eq!(saved.next_retry_at, Some(deadline));
        assert!(!saved.needs_discovery(deadline - Duration::nanoseconds(1)));
        assert!(saved.needs_discovery(deadline));
        attempt_at = deadline;
    }
}

#[tokio::test]
async fn success_clears_retry_state_without_completing_ingestion() {
    let store = memory_store().await;
    let reference = reference();
    store
        .record_source_discovery_failure(&reference, now(), "authentication failed")
        .await
        .expect("record first failure");
    assert_eq!(state(&store, &reference).await.last_completed_at, None);
    store
        .enqueue_pull(reference.registry(), reference.repository(), "1.0.0", 0)
        .await
        .expect("enqueue tag");
    store
        .record_source_discovery_success(&reference, now())
        .await
        .expect("record success");
    let saved = state(&store, &reference).await;
    assert_eq!(
        saved,
        SourceDiscoveryState {
            last_completed_at: Some(now()),
            ..SourceDiscoveryState::default()
        }
    );
    assert!(!saved.needs_discovery(now() + Duration::days(1)));
    let task = fetch_queue::Entity::find()
        .one(&store.db)
        .await
        .expect("read queue")
        .expect("pending task");
    assert_eq!(task.status, fetch_queue::FetchStatus::Pending);
    assert_eq!(
        oci_repository::Entity::find()
            .count(&store.db)
            .await
            .expect("count ingested repositories"),
        0
    );
    store
        .record_source_discovery_failure(&reference, now(), "failed again")
        .await
        .expect("new failure sequence");
    assert_eq!(state(&store, &reference).await.failure_count, 1);
}

#[tokio::test]
async fn failure_count_saturates_without_overflowing_retry() {
    let store = memory_store().await;
    let reference = reference();
    store
        .record_source_discovery_failure(&reference, now(), "initial failure")
        .await
        .expect("initial failure");
    source_discovery::Entity::update_many()
        .col_expr(
            source_discovery::Column::FailureCount,
            Expr::value(i64::from(u32::MAX)),
        )
        .exec(&store.db)
        .await
        .expect("seed maximum count");
    store
        .record_source_discovery_failure(&reference, now(), "still unavailable")
        .await
        .expect("record saturated failure");
    let saved = state(&store, &reference).await;
    assert_eq!(saved.failure_count, u32::MAX);
    assert_eq!(saved.next_retry_at, Some(now() + Duration::hours(1)));
}

#[tokio::test]
async fn failure_transaction_rolls_back_if_retry_date_overflows() {
    let store = memory_store().await;
    let reference = reference();
    store
        .record_source_discovery_success(&reference, now())
        .await
        .expect("initial success");
    assert!(
        store
            .record_source_discovery_failure(&reference, DateTime::<Utc>::MAX_UTC, "invalid clock")
            .await
            .is_err()
    );
    assert_eq!(
        state(&store, &reference).await,
        SourceDiscoveryState {
            last_completed_at: Some(now()),
            ..SourceDiscoveryState::default()
        }
    );
}

#[tokio::test]
async fn source_discovery_survives_restart_and_concurrent_writers() {
    let directory = tempfile::Builder::new()
        .prefix("source-discovery-")
        .tempdir_in(".")
        .expect("create local test directory");
    let url = format!(
        "sqlite://{}?mode=rwc",
        directory.path().join("discovery.sqlite").display()
    );
    let reference = reference();
    let store = open_store(&url, directory.path()).await;
    store
        .record_source_discovery_success(&reference, now())
        .await
        .expect("initial success");
    store.db.close().await.expect("close first connection");

    let first = open_store(&url, directory.path()).await;
    let second = open_store(&url, directory.path()).await;
    assert_eq!(
        state(&first, &reference).await.last_completed_at,
        Some(now())
    );
    let (a, b) = tokio::join!(
        first.record_source_discovery_failure(&reference, now(), "first writer"),
        second.record_source_discovery_failure(&reference, now(), "second writer")
    );
    a.expect("first concurrent failure");
    b.expect("second concurrent failure");
    let expected = state(&first, &reference).await;
    assert_eq!(expected.failure_count, 2);
    assert_eq!(expected.last_completed_at, Some(now()));
    assert_eq!(expected.next_retry_at, Some(now() + Duration::seconds(120)));
    assert!(expected.last_error.is_some());
    first.db.close().await.expect("close first writer");
    second.db.close().await.expect("close second writer");

    let reopened = open_store(&url, directory.path()).await;
    assert_eq!(state(&reopened, &reference).await, expected);
    reopened
        .record_source_discovery_failure(&reference, now(), "after restart")
        .await
        .expect("continue retry sequence");
    assert_eq!(state(&reopened, &reference).await.failure_count, 3);
    assert_eq!(
        source_discovery::Entity::find()
            .filter(source_discovery::Column::Registry.eq(reference.registry()))
            .count(&reopened.db)
            .await
            .expect("count durable identity"),
        1
    );
    reopened.db.close().await.expect("close reopened store");
}
