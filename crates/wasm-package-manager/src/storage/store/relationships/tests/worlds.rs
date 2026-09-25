use sea_orm::{ActiveModelTrait, Set};
use wasm_meta_registry_types::PackageKind;
use wasm_package_manager_migration::entities::wasm_component;

use super::super::super::{insert_oci_layer, insert_wit_world, upsert_wit_package};
use super::fixtures::member;
use super::{fixture_store, package, repository, seed_release, target};

#[tokio::test]
async fn relationship_worlds_match_exact_packages_members_and_direction() {
    let store = fixture_store().await;
    let alpha = package(&store, "test:alpha", "1.0.0", &[]).await;
    let run = alpha.world(&store, "run", Some("Run alpha")).await;
    member(&store, run, "wasi:io", Some("streams"), Some("0.2.0"), true).await;
    member(&store, run, "wasi:io", Some("streams"), Some("0.3.0"), true).await;
    member(&store, run, "wasi:io", Some("poll"), None, true).await;
    let serve = alpha.world(&store, "serve", None).await;
    member(&store, serve, "wasi:io", Some("poll"), None, true).await;
    let beta = package(&store, "test:beta", "1.0.0", &[]).await;
    let run_beta = beta.world(&store, "run", None).await;
    member(
        &store,
        run_beta,
        "wasi:io",
        Some("streams-extra"),
        None,
        true,
    )
    .await;
    let anonymous = beta.world(&store, "whole-package", None).await;
    member(&store, anonymous, "wasi:io", None, None, true).await;
    let gamma = package(&store, "test:gamma", "1.0.0", &[]).await;
    let run_gamma = gamma.world(&store, "run", None).await;
    member(&store, run_gamma, "wasi:io", Some("streams"), None, false).await;
    member(
        &store,
        run_gamma,
        "wasi:io-extra",
        Some("streams"),
        None,
        true,
    )
    .await;
    member(&store, run_gamma, "wasi:xio", Some("streams"), None, true).await;
    package(&store, "test:transitive", "1.0.0", &["test:alpha"]).await;

    let all = store
        .list_importing_worlds(&target("wasi:io", None), 0, 100)
        .await
        .expect("package imports");
    let worlds: Vec<_> = all
        .results
        .iter()
        .map(|entry| (entry.package.repository.as_str(), entry.name.as_str()))
        .collect();
    assert_eq!(
        worlds,
        [
            ("test/alpha", "run"),
            ("test/alpha", "serve"),
            ("test/beta", "run"),
            ("test/beta", "whole-package")
        ]
    );
    assert_eq!(all.total, Some(4));
    let exact = store
        .list_importing_worlds(&target("wasi:io", Some("streams")), 0, 100)
        .await
        .expect("exact imports");
    assert_eq!(exact.total, Some(1));
    assert_eq!(
        exact
            .results
            .first()
            .expect("run world")
            .description
            .as_deref(),
        Some("Run alpha")
    );
    let exports = store
        .list_exporting_worlds(&target("wasi:io", Some("streams")), 0, 100)
        .await
        .expect("exact exports");
    assert_eq!(exports.total, Some(1));
    assert_eq!(
        exports
            .results
            .first()
            .expect("export world")
            .package
            .repository,
        "test/gamma"
    );
    let missing = store
        .list_importing_worlds(&target("wasi:io", Some("stream")), 0, 100)
        .await
        .expect("nonmatching member");
    assert!(missing.results.is_empty());
    assert_eq!(missing.total, Some(0));
}

#[tokio::test]
async fn relationship_worlds_deduplicate_matching_versions_tags_and_mirrors() {
    let store = fixture_store().await;
    for registry in ["z.registry.test", "a.registry.test"] {
        let repo = repository(
            &store,
            registry,
            "mirrored/source",
            Some("test:source"),
            Some("interface"),
        )
        .await;
        for (version, matches) in [("1.9.0", true), ("1.10.0_build.7", true), ("2.0.0", false)] {
            let release =
                seed_release(&store, repo, "test:source", Some(version), &[version]).await;
            let world = release.world(&store, "run", Some(version)).await;
            let package = if matches { "wasi:io" } else { "wasi:clocks" };
            member(&store, world, package, Some("streams"), None, true).await;
            member(&store, world, package, Some("poll"), None, true).await;
            member(&store, world, package, Some("streams"), None, false).await;
        }
    }
    for interface in [None, Some("streams")] {
        let query = target("wasi:io", interface);
        let imports = store
            .list_importing_worlds(&query, 0, 1)
            .await
            .expect("imports");
        let exports = store
            .list_exporting_worlds(&query, 0, 1)
            .await
            .expect("exports");
        for page in [imports, exports] {
            assert_eq!(page.total, Some(1));
            assert!(
                !page.has_next,
                "mirrors and duplicate members are not extra pages"
            );
            let world = page.results.first().expect("one world");
            assert_eq!(
                world.package.registry, "a.registry.test",
                "stable mirror tie"
            );
            assert_eq!(world.version, "1.10.0_build.7");
            assert!(world.package.tags.contains(&world.version));
            assert_eq!(world.description.as_deref(), Some("1.10.0_build.7"));
            assert_eq!(world.package.tags.first().expect("latest tag"), "2.0.0");
        }
    }
}

#[tokio::test]
async fn relationship_worlds_keep_compiled_sources_and_unregistered_oci_sources_distinct() {
    let store = fixture_store().await;
    for name in [
        "one",
        "two",
        "unregistered-one",
        "unregistered-two",
        "unknown-kind",
    ] {
        let registered = ["one", "two", "unknown-kind"].contains(&name);
        let identity = registered.then(|| format!("test:{name}"));
        let kind = (name != "unknown-kind").then_some("component");
        let repo = repository(
            &store,
            "registry.test",
            &format!("compiled/{name}"),
            identity.as_deref(),
            kind,
        )
        .await;
        let release = seed_release(&store, repo, "root:component", None, &["1.0.0"]).await;
        wasm_component::ActiveModel {
            oci_manifest_id: Set(release.manifest_id),
            ..Default::default()
        }
        .insert(&store.db)
        .await
        .expect("mark compiled component");
        release.dependency(&store, "wasi:io", None).await;
        let world = release.world(&store, "root", None).await;
        member(&store, world, "wasi:io", Some("streams"), None, true).await;
    }
    let query = target("wasi:io", None);
    let worlds = store
        .list_importing_worlds(&query, 0, 100)
        .await
        .expect("worlds");
    let dependents = store
        .list_dependents(&query, 0, 100)
        .await
        .expect("dependents");
    assert_eq!(worlds.total, Some(5));
    assert_eq!(dependents.total, Some(5));
    assert!(worlds.results.iter().all(|world| world.name == "root"));
    assert!(worlds.results.iter().all(|world| world.is_synthetic));
    let unclassified = worlds
        .results
        .iter()
        .find(|world| world.package.repository == "compiled/unknown-kind")
        .expect("unclassified component");
    assert!(unclassified.package.kind.is_none());
    assert_eq!(unclassified.package.wit_namespace.as_deref(), Some("test"));
    assert_eq!(
        worlds
            .results
            .first()
            .expect("registered component")
            .package
            .kind,
        Some(PackageKind::Component)
    );
    assert_eq!(
        worlds
            .results
            .iter()
            .map(|world| world.package.repository.as_str())
            .collect::<Vec<_>>(),
        [
            "compiled/one",
            "compiled/two",
            "compiled/unknown-kind",
            "compiled/unregistered-one",
            "compiled/unregistered-two",
        ]
    );
}

#[tokio::test]
async fn relationship_worlds_do_not_treat_authored_root_worlds_as_synthetic() {
    let store = fixture_store().await;
    let release = package(&store, "test:authored", "1.0.0", &["wasi:io"]).await;
    let world = release.world(&store, "root", None).await;
    for is_import in [true, false] {
        member(&store, world, "wasi:io", Some("streams"), None, is_import).await;
    }
    let query = target("wasi:io", Some("streams"));
    let imports = store
        .list_importing_worlds(&query, 0, 100)
        .await
        .expect("imports");
    let exports = store
        .list_exporting_worlds(&query, 0, 100)
        .await
        .expect("exports");
    for page in [imports, exports] {
        assert_eq!(page.total, Some(1));
        let world = page.results.first().expect("authored world");
        assert_eq!(world.name, "root");
        assert!(!world.is_synthetic);
    }
}

#[tokio::test]
async fn relationship_worlds_use_real_extracted_wit_names_when_registration_is_absent() {
    let store = fixture_store().await;
    for registry in ["a.registry.test", "b.registry.test"] {
        let repo = repository(&store, registry, "wit/source", None, Some("interface")).await;
        let release = seed_release(&store, repo, "test:source", None, &["1.0.0"]).await;
        release.dependency(&store, "wasi:io", None).await;
        let world = release.world(&store, "run", None).await;
        member(&store, world, "wasi:io", Some("streams"), None, true).await;
    }
    package(&store, "test:consumer", "1.0.0", &["test:source"]).await;
    let query = target("wasi:io", None);
    let worlds = store
        .list_importing_worlds(&query, 0, 100)
        .await
        .expect("worlds");
    let dependents = store
        .list_dependents(&query, 0, 100)
        .await
        .expect("dependents");
    assert_eq!(worlds.total, Some(1), "real WIT mirrors have one identity");
    assert_eq!(
        dependents.total,
        Some(2),
        "the real source participates in traversal"
    );
}

#[tokio::test]
async fn relationship_worlds_deduplicate_metadata_rows_within_one_manifest() {
    let store = fixture_store().await;
    let release = package(&store, "test:source", "1.0.0", &[]).await;
    release.tag(&store, "1.1.0").await;
    let original = release
        .world(&store, "run", Some("First indexed world"))
        .await;
    member(&store, original, "wasi:io", Some("streams"), None, true).await;
    let duplicate_layer = insert_oci_layer(
        &store.db,
        release.manifest_id,
        "sha256:duplicate-metadata",
        None,
        None,
        1,
    )
    .await
    .expect("duplicate WIT layer");
    let duplicate_package = upsert_wit_package(
        &store.db,
        "test:source",
        Some("1.0.0"),
        None,
        None,
        Some(release.manifest_id),
        Some(duplicate_layer),
    )
    .await
    .expect("duplicate package metadata");
    let duplicate = insert_wit_world(&store.db, duplicate_package, "run", None)
        .await
        .expect("duplicate world metadata");
    member(&store, duplicate, "wasi:io", Some("poll"), None, true).await;
    let page = store
        .list_importing_worlds(&target("wasi:io", None), 0, 1)
        .await
        .expect("deduplicate world rows");
    assert_eq!(page.total, Some(1));
    assert!(!page.has_next);
    let world = page.results.first().expect("single world");
    assert_eq!(world.version, "1.1.0");
    assert_eq!(world.description.as_deref(), Some("First indexed world"));
}
