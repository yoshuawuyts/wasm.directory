use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use sea_orm::{ConnectionTrait, EntityTrait, Statement};
use wasm_package_manager_migration::entities::oci_repository;

use super::super::super::{known_packages_from_repos, package_metadata::BATCH_SIZE};
use super::fixtures::member;
use super::{Store, fixture_store, package, seed_release, target};

fn count_queries(store: &mut Store) -> Arc<AtomicUsize> {
    let count = Arc::new(AtomicUsize::new(0));
    let callback_count = Arc::clone(&count);
    store.db.set_metric_callback(move |_| {
        callback_count.fetch_add(1, Ordering::Relaxed);
    });
    count
}

#[tokio::test]
async fn relationship_hydration_round_trips_do_not_grow_with_page_length() {
    let mut store = fixture_store().await;
    for index in 0..101 {
        let release = package(
            &store,
            &format!("test:package{index:03}"),
            "1.0.0",
            &["wasi:io"],
        )
        .await;
        let world = release.world(&store, "run", Some("Run a package")).await;
        member(&store, world, "wasi:io", Some("streams"), None, true).await;
        member(&store, world, "wasi:io", Some("streams"), None, false).await;
    }
    let queries = count_queries(&mut store);
    let target = target("wasi:io", None);
    let mut measurements = Vec::new();
    for limit in [1, 100] {
        queries.store(0, Ordering::Relaxed);
        let dependents = store
            .list_dependents(&target, 0, limit)
            .await
            .expect("dependents");
        let dependent_queries = queries.swap(0, Ordering::Relaxed);
        let imports = store
            .list_importing_worlds(&target, 0, limit)
            .await
            .expect("imports");
        let import_queries = queries.swap(0, Ordering::Relaxed);
        let exports = store
            .list_exporting_worlds(&target, 0, limit)
            .await
            .expect("exports");
        let export_queries = queries.swap(0, Ordering::Relaxed);
        assert_eq!(
            dependents.results.len(),
            usize::try_from(limit).expect("page length")
        );
        assert_eq!(imports.results.len(), dependents.results.len());
        assert_eq!(exports.results.len(), dependents.results.len());
        assert_eq!(imports.total, Some(101));
        let counts = [dependent_queries, import_queries, export_queries];
        assert!(
            counts.iter().all(|count| (1..=8).contains(count)),
            "{counts:?}"
        );
        measurements.push(counts);
    }
    assert_eq!(measurements[0], measurements[1]);
    assert_eq!(measurements[0], [6, 7, 7]);
}

#[tokio::test]
async fn relationship_hydration_reuses_package_data_across_worlds() {
    let mut store = fixture_store().await;
    let release = package(&store, "test:worlds", "1.10.0_build.2", &["wasi:io"]).await;
    release.tag(&store, "1.9.0").await;
    for index in (0..100).rev() {
        let name = format!("world{index:03}");
        let world = release.world(&store, &name, Some(&name)).await;
        member(&store, world, "wasi:io", Some("streams"), None, true).await;
    }
    let queries = count_queries(&mut store);
    let page = store
        .list_importing_worlds(&target("wasi:io", Some("streams")), 0, 100)
        .await
        .expect("world page");
    assert_eq!(queries.load(Ordering::Relaxed), 7);
    assert_eq!(page.results.len(), 100);
    assert_eq!(page.total, Some(100));
    for (index, world) in page.results.iter().enumerate() {
        assert_eq!(world.name, format!("world{index:03}"));
        assert_eq!(world.description.as_deref(), Some(world.name.as_str()));
        assert_eq!(world.version, "1.10.0_build.2");
        assert_eq!(world.package.repository, "test/worlds");
        assert_eq!(world.package.tags, ["1.10.0_build.2", "1.9.0"]);
        assert!(!world.is_synthetic);
    }
}

#[tokio::test]
async fn relationship_package_hydration_preserves_order_tags_and_descriptions() {
    let store = fixture_store().await;
    let first = package(&store, "test:first", "latest", &[]).await;
    let described = seed_release(
        &store,
        first.repo_id,
        "test:first",
        None,
        &["1.9.0", "1.10.0_build.2", "v99.0.0", "sha256-abc.sig"],
    )
    .await;
    set_description(&store, described.manifest_id, "First indexed description").await;
    let newer = seed_release(&store, first.repo_id, "test:first", None, &["2.0.0"]).await;
    set_description(&store, newer.manifest_id, "Later description").await;
    let second = package(&store, "test:second", "0.1.0", &[]).await;
    set_description(&store, second.manifest_id, "").await;
    let first = oci_repository::Entity::find_by_id(first.repo_id)
        .one(&store.db)
        .await
        .expect("repository query")
        .expect("first repository");
    let second = oci_repository::Entity::find_by_id(second.repo_id)
        .one(&store.db)
        .await
        .expect("repository query")
        .expect("second repository");
    let packages = known_packages_from_repos(&store.db, vec![second.clone(), first, second])
        .await
        .expect("batched package data");
    assert_eq!(packages.len(), 3);
    assert_eq!(packages[0].repository, "test/second");
    assert_eq!(packages[1].repository, "test/first");
    assert_eq!(packages[2].repository, "test/second");
    assert_eq!(packages[0].tags, ["0.1.0"]);
    assert_eq!(packages[2].tags, packages[0].tags);
    assert_eq!(packages[0].description.as_deref(), Some(""));
    assert_eq!(packages[2].description, packages[0].description);
    assert_eq!(packages[1].tags, ["2.0.0", "1.10.0_build.2", "1.9.0"]);
    assert_eq!(
        packages[1].description.as_deref(),
        Some("First indexed description")
    );
    assert_eq!(packages[1].dependents, Some(0));
    assert!(packages[1].latest_release_at.is_some());
}

#[tokio::test]
async fn relationship_package_hydration_chunks_large_repository_sets() {
    let mut store = fixture_store().await;
    for index in 0..=BATCH_SIZE {
        package(&store, &format!("test:package{index:03}"), "1.0.0", &[]).await;
    }
    let repos = oci_repository::Entity::find()
        .all(&store.db)
        .await
        .expect("fixture repositories");
    let queries = count_queries(&mut store);
    let packages = known_packages_from_repos(&store.db, repos)
        .await
        .expect("multi-batch metadata");
    assert_eq!(packages.len(), BATCH_SIZE + 1);
    assert_eq!(queries.load(Ordering::Relaxed), 8);
    assert!(packages.iter().all(|package| package.tags == ["1.0.0"]));
    assert!(packages.iter().all(|package| package.description.is_none()));
    assert!(
        packages
            .iter()
            .all(|package| package.latest_release_at.is_some())
    );
}

async fn set_description(store: &Store, manifest: i64, description: &str) {
    store
        .db
        .execute_raw(Statement::from_sql_and_values(
            store.db.get_database_backend(),
            "UPDATE oci_manifest SET oci_description = ? WHERE id = ?",
            [description.into(), manifest.into()],
        ))
        .await
        .expect("set manifest description");
}
