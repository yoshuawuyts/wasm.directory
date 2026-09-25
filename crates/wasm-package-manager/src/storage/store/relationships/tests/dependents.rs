use sea_orm::{EntityTrait, PaginatorTrait};
use wasm_meta_registry_types::PackageKind;
use wasm_package_manager_migration::entities::wit_package_dependency;

use super::super::super::upsert_wit_package;
use super::{fixture_store, package, repository, seed_release, target};

#[tokio::test]
async fn relationship_dependents_follow_chains_diamonds_and_cycles_without_self() {
    let store = fixture_store().await;
    package(&store, "wasi:io", "0.2.0", &["wasi:io", "test:d"]).await;
    let a = package(&store, "test:a", "1.0.0", &["wasi:io"]).await;
    a.dependency(&store, "wasi:io", Some("0.1.0")).await;
    package(&store, "test:b", "1.0.0", &["test:a", "test:b"]).await;
    package(&store, "test:c", "1.0.0", &["test:a"]).await;
    package(&store, "test:d", "1.0.0", &["test:b", "test:c"]).await;
    package(
        &store,
        "test:unrelated",
        "1.0.0",
        &["wasi:iota", "wasi:io-extra"],
    )
    .await;
    package(&store, "test:x", "1.0.0", &["test:y"]).await;
    package(&store, "test:y", "1.0.0", &["test:x"]).await;
    let page = store
        .list_dependents(&target("wasi:io", None), 0, 100)
        .await
        .expect("query reverse closure");
    let names: Vec<_> = page
        .results
        .iter()
        .map(|entry| entry.package.repository.as_str())
        .collect();
    assert_eq!(names, ["test/a", "test/b", "test/c", "test/d"]);
    assert_eq!(page.total, Some(4));
    assert!(!page.has_next);
    let edges = wit_package_dependency::Entity::find()
        .all(&store.db)
        .await
        .expect("read edges");
    assert!(edges.iter().all(|edge| edge.resolved_package_id.is_none()));
}

#[tokio::test]
async fn relationship_dependents_match_unindexed_targets_and_unattached_intermediates() {
    let store = fixture_store().await;
    let intermediate = upsert_wit_package(
        &store.db,
        "test:intermediate",
        Some("1.0.0"),
        None,
        None,
        None,
        None,
    )
    .await
    .expect("insert unattached WIT package");
    super::super::super::insert_wit_package_dependency(
        &store.db,
        intermediate,
        "wasi:io",
        Some("999.0.0"),
    )
    .await
    .expect("insert unresolved intermediate edge");
    package(&store, "test:source", "1.0.0", &["test:intermediate"]).await;
    let page = store
        .list_dependents(&target("wasi:io", None), 0, 100)
        .await
        .expect("query");
    assert_eq!(page.total, Some(1));
    assert_eq!(
        page.results
            .first()
            .expect("source match")
            .package
            .repository,
        "test/source"
    );
    let empty = store
        .list_dependents(&target("wasi:missing", None), 0, 100)
        .await
        .expect("unknown target is valid");
    assert!(empty.results.is_empty());
    assert_eq!(empty.total, Some(0));
    assert!(!empty.has_next);
}

#[tokio::test]
async fn relationship_dependents_keep_the_highest_matching_manifest_not_latest_repo_tag() {
    let store = fixture_store().await;
    let old = package(&store, "test:source", "1.9.0", &["wasi:io"]).await;
    let matching = seed_release(
        &store,
        old.repo_id,
        "test:source",
        Some("1.10.0+build.7"),
        &["1.10.0_build.7", "1.10.0-rc.1", "latest", "sha256-abc.sig"],
    )
    .await;
    matching.dependency(&store, "wasi:io", None).await;
    package(&store, "test:source", "2.0.0", &["wasi:clocks"]).await;
    package(&store, "test:transitive", "1.0.0", &["test:source"]).await;
    package(&store, "test:transitive", "3.0.0", &[]).await;
    let page = store
        .list_dependents(&target("wasi:io", None), 0, 10)
        .await
        .expect("query");
    let versions: Vec<_> = page
        .results
        .iter()
        .map(|entry| (entry.package.repository.as_str(), entry.version.as_str()))
        .collect();
    assert_eq!(
        versions,
        [
            ("test/source", "1.10.0_build.7"),
            ("test/transitive", "1.0.0")
        ]
    );
    assert!(
        page.results
            .iter()
            .all(|entry| entry.package.tags.contains(&entry.version)),
        "matching OCI tags must survive exact repository tag validation"
    );
    assert_eq!(
        page.results
            .first()
            .expect("source")
            .package
            .tags
            .first()
            .expect("latest tag"),
        "2.0.0",
        "matching version must not overwrite the ordinary package tag list"
    );
}

#[tokio::test]
async fn relationship_dependents_prefer_registered_component_identities() {
    let store = fixture_store().await;
    for (name, dependency) in [("one", "wasi:io"), ("two", "wasi:clocks")] {
        let repo = repository(
            &store,
            "registry.test",
            &format!("compiled/{name}"),
            Some(&format!("test:{name}")),
            Some("component"),
        )
        .await;
        let release = seed_release(&store, repo, "root:component", None, &["1.0.0"]).await;
        release.dependency(&store, dependency, None).await;
    }
    package(&store, "test:consumer", "1.0.0", &["test:one"]).await;
    package(&store, "test:unrelated", "1.0.0", &["test:two"]).await;
    let page = store
        .list_dependents(&target("wasi:io", None), 0, 100)
        .await
        .expect("query registered sources");
    let names: Vec<_> = page
        .results
        .iter()
        .map(|entry| entry.package.wit_name.as_deref())
        .collect();
    assert_eq!(names, [Some("consumer"), Some("one")]);
    assert_eq!(
        page.results.last().expect("component").package.kind,
        Some(PackageKind::Component)
    );
    let component_target = store
        .list_dependents(&target("test:one", None), 0, 100)
        .await
        .expect("query component target");
    assert_eq!(component_target.total, Some(1));
    assert_eq!(
        component_target
            .results
            .first()
            .expect("consumer")
            .package
            .wit_name
            .as_deref(),
        Some("consumer")
    );
}

#[tokio::test]
async fn relationship_dependents_traverse_more_than_a_hundred_levels() {
    let store = fixture_store().await;
    let mut dependency = "wasi:io".to_owned();
    for index in 0..128 {
        let name = format!("test:node-{index:03}");
        package(&store, &name, "1.0.0", &[&dependency]).await;
        dependency = name;
    }
    assert_eq!(
        wit_package_dependency::Entity::find()
            .count(&store.db)
            .await
            .expect("count graph edges"),
        128
    );
    let last = store
        .list_dependents(&target("wasi:io", None), 127, 1)
        .await
        .expect("query deep reverse closure");
    assert_eq!(last.total, Some(128));
    assert!(!last.has_next);
    assert_eq!(
        last.results.last().expect("deep match").package.repository,
        "test/node-127"
    );
}
