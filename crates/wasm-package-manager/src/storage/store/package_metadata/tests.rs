use std::collections::HashMap;

use chrono::{DateTime, Utc};
use sea_orm::sea_query::Expr;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};
use wasm_meta_registry_types::KnownPackage;
use wasm_package_manager_migration::entities::{oci_manifest, oci_repository, oci_tag};

use super::super::{
    Store, insert_wit_package_dependency, insert_wit_world, insert_wit_world_iface,
    upsert_oci_manifest, upsert_oci_repository_full, upsert_oci_tag, upsert_wit_package,
};

async fn repo(store: &Store, path: &str, identity: Option<(&str, &str)>) -> i64 {
    upsert_oci_repository_full(
        &store.db,
        "ghcr.io",
        path,
        identity.map(|(ns, _)| ns),
        identity.map(|(_, name)| name),
        None,
    )
    .await
    .expect("insert repository")
}

async fn release(store: &Store, id: i64, tag: &str, published: Option<&str>) -> i64 {
    let digest = format!("sha256:{id}-{tag}");
    let annotations: HashMap<_, _> = published
        .map(|time| {
            (
                "org.opencontainers.image.created".to_owned(),
                time.to_owned(),
            )
        })
        .into_iter()
        .collect();
    let (manifest, _) = upsert_oci_manifest(
        &store.db,
        id,
        &digest,
        None,
        None,
        None,
        None,
        None,
        None,
        &annotations,
    )
    .await
    .expect("insert manifest");
    upsert_oci_tag(&store.db, id, tag, &digest)
        .await
        .expect("insert tag");
    manifest
}

async fn dependency(store: &Store, manifest: i64, name: &str, target: &str) -> i64 {
    let wit = upsert_wit_package(
        &store.db,
        name,
        Some(&format!("1.0.{manifest}")),
        None,
        None,
        Some(manifest),
        None,
    )
    .await
    .expect("insert WIT package");
    for version in ["0.1.0", "0.2.0"] {
        insert_wit_package_dependency(&store.db, wit, target, Some(version))
            .await
            .expect("insert dependency edge");
    }
    wit
}

async fn package(store: &Store, path: &str) -> KnownPackage {
    store
        .get_known_package("ghcr.io", path)
        .await
        .expect("load package")
        .expect("known repository")
}

fn at(time: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(time)
        .expect("valid test timestamp")
        .with_timezone(&Utc)
}

async fn index_times(store: &Store, id: i64, tag: &str, manifest: &str) {
    oci_tag::Entity::update_many()
        .col_expr(oci_tag::Column::CreatedAt, Expr::value(at(tag)))
        .filter(oci_tag::Column::OciRepositoryId.eq(id))
        .exec(&store.db)
        .await
        .expect("set tag indexing time");
    oci_manifest::Entity::update_many()
        .col_expr(oci_manifest::Column::CreatedAt, Expr::value(at(manifest)))
        .filter(oci_manifest::Column::OciRepositoryId.eq(id))
        .exec(&store.db)
        .await
        .expect("set manifest indexing time");
}

#[tokio::test]
async fn dependents_deduplicate_edges_versions_and_match_popularity() {
    let store = Store::open_in_memory().await.expect("open store");
    let target = repo(&store, "wasi/io", Some(("wasi", "io"))).await;
    let target_manifest = release(&store, target, "0.2.0", None).await;
    dependency(&store, target_manifest, "wasi:io", "wasi:io").await;
    for name in ["one", "two"] {
        let consumer = repo(&store, &format!("app/{name}"), Some(("app", name))).await;
        for tag in ["1.0.0", "2.0.0"] {
            let manifest = release(&store, consumer, tag, None).await;
            dependency(&store, manifest, &format!("app:{name}"), "wasi:io").await;
            dependency(&store, manifest, &format!("extra:{name}"), "wasi:io").await;
        }
    }
    assert_eq!(package(&store, "wasi/io").await.dependents, Some(2));
    let popular = store
        .list_popular_known_packages(0, 10)
        .await
        .expect("popular");
    let target = popular.first().expect("popular target");
    assert_eq!(target.dependents, 2);
    assert_eq!(target.package.dependents, Some(target.dependents));
}

#[tokio::test]
async fn mirror_counts_use_other_repository_ids_not_other_identities() {
    let store = Store::open_in_memory().await.expect("open store");
    let primary = repo(&store, "wasi/io", Some(("wasi", "io"))).await;
    let manifest = release(&store, primary, "0.2.0", None).await;
    dependency(&store, manifest, "wasi:io", "wasi:io").await;
    assert_eq!(package(&store, "wasi/io").await.dependents, Some(0));

    let mirror = repo(&store, "mirror/io", Some(("wasi", "io"))).await;
    assert_eq!(package(&store, "wasi/io").await.dependents, Some(0));
    let manifest = release(&store, mirror, "0.3.0", None).await;
    assert_eq!(package(&store, "wasi/io").await.dependents, Some(1));
    dependency(&store, manifest, "mirror:io", "wasi:io").await;
    for path in ["wasi/io", "mirror/io"] {
        assert_eq!(package(&store, path).await.dependents, Some(2));
    }
    let popular = store
        .list_popular_known_packages(0, 10)
        .await
        .expect("popular");
    assert_eq!(popular.len(), 1);
    let mirror = popular.first().expect("popular mirror");
    assert_eq!(mirror.package.repository, "mirror/io");
    assert_eq!(mirror.dependents, 2);
    assert_eq!(mirror.package.dependents, Some(2));
}

#[tokio::test]
async fn zero_unknown_and_partial_identities_remain_distinct() {
    let store = Store::open_in_memory().await.expect("open store");
    let known = repo(&store, "known/zero", Some(("known", "zero"))).await;
    release(&store, known, "1.0.0", None).await;
    let unknown = repo(&store, "unknown/package", None).await;
    release(&store, unknown, "1.0.0", None).await;
    assert_eq!(package(&store, "known/zero").await.dependents, Some(0));
    assert_eq!(package(&store, "unknown/package").await.dependents, None);
    oci_repository::Entity::update_many()
        .col_expr(oci_repository::Column::WitNamespace, Expr::value("partial"))
        .filter(oci_repository::Column::Id.eq(unknown))
        .exec(&store.db)
        .await
        .expect("set partial identity");
    assert_eq!(package(&store, "unknown/package").await.dependents, None);
}

#[tokio::test]
async fn latest_publication_is_repository_local_not_highest_version_or_nonsemver() {
    let store = Store::open_in_memory().await.expect("open store");
    let primary = repo(&store, "wasi/io", Some(("wasi", "io"))).await;
    release(&store, primary, "2.0.0", Some("2000-01-01T00:00:00Z")).await;
    release(&store, primary, "1.0.1", Some("2001-01-01T00:00:00Z")).await;
    release(&store, primary, "latest", Some("2005-01-01T00:00:00Z")).await;
    let mirror = repo(&store, "mirror/io", Some(("wasi", "io"))).await;
    release(&store, mirror, "3.0.0", Some("2010-01-01T00:00:00Z")).await;
    let primary = package(&store, "wasi/io").await;
    assert_eq!(primary.tags, ["2.0.0", "1.0.1"]);
    assert_eq!(
        primary.latest_release_at.as_deref(),
        Some("2001-01-01T00:00:00+00:00")
    );
    let mirror = package(&store, "mirror/io").await;
    assert_eq!(
        mirror.latest_release_at.as_deref(),
        Some("2010-01-01T00:00:00+00:00")
    );
}

#[tokio::test]
async fn release_time_precedence_fallback_and_future_cap() {
    let store = Store::open_in_memory().await.expect("open store");
    let cases = [
        (
            "annotation",
            Some(" 2000-01-01T01:00:00+01:00 "),
            Some("2001-01-01T00:00:00Z"),
            "2000-01-01T00:00:00+00:00",
        ),
        (
            "config",
            None,
            Some("2001-01-01T00:00:00Z"),
            "2001-01-01T00:00:00+00:00",
        ),
        (
            "invalid-annotation",
            Some("bad"),
            Some("2001-01-01T00:00:00Z"),
            "2001-01-01T00:00:00+00:00",
        ),
        ("missing", None, None, "2002-01-01T00:00:00+00:00"),
        (
            "invalid-both",
            Some("bad"),
            Some(""),
            "2002-01-01T00:00:00+00:00",
        ),
        (
            "future",
            Some("2999-01-01T00:00:00Z"),
            Some("2001-01-01T00:00:00Z"),
            "2002-01-01T00:00:00+00:00",
        ),
        (
            "future-config",
            None,
            Some("2999-01-01T00:00:00Z"),
            "2002-01-01T00:00:00+00:00",
        ),
    ];
    for (name, annotation, config, expected) in cases {
        let path = format!("bulk/{name}");
        let id = repo(&store, &path, None).await;
        let manifest = release(&store, id, "1.0.0", annotation).await;
        if let Some(config) = config {
            store
                .set_manifest_config_created(manifest, config)
                .await
                .expect("config time");
        }
        index_times(&store, id, "2003-01-01T00:00:00Z", "2002-01-01T00:00:00Z").await;
        assert_eq!(
            package(&store, &path).await.latest_release_at.as_deref(),
            Some(expected),
            "{name}"
        );
    }
    // All are debuts, including entries beyond highlight publisher caps.
    assert_eq!(
        store.list_known_packages(0, 100).await.expect("list").len(),
        cases.len()
    );
}

#[tokio::test]
async fn release_fallback_uses_earliest_tag_and_ignores_nonsemver() {
    let store = Store::open_in_memory().await.expect("open store");
    let id = repo(&store, "early-tag", None).await;
    release(&store, id, "1.0.0", None).await;
    index_times(&store, id, "2001-01-01T00:00:00Z", "2002-01-01T00:00:00Z").await;
    assert_eq!(
        package(&store, "early-tag")
            .await
            .latest_release_at
            .as_deref(),
        Some("2001-01-01T00:00:00+00:00")
    );
    let id = repo(&store, "no-semver", Some(("no", "semver"))).await;
    release(&store, id, "latest", None).await;
    assert_eq!(package(&store, "no-semver").await.latest_release_at, None);
}

#[tokio::test]
async fn rescans_and_recreated_tags_do_not_change_release_age() {
    let store = Store::open_in_memory().await.expect("open store");
    let id = repo(&store, "stable", None).await;
    release(&store, id, "1.0.0", None).await;
    index_times(&store, id, "2000-01-01T00:00:00Z", "2000-01-01T00:00:00Z").await;
    let before = package(&store, "stable").await;
    oci_tag::Entity::delete_many()
        .filter(oci_tag::Column::OciRepositoryId.eq(id))
        .exec(&store.db)
        .await
        .expect("remove tag");
    assert_eq!(repo(&store, "stable", None).await, id);
    release(&store, id, "1.0.0", None).await;
    let after = package(&store, "stable").await;
    assert_eq!(after.latest_release_at, before.latest_release_at);
    assert_eq!(
        after.latest_release_at.as_deref(),
        Some("2000-01-01T00:00:00+00:00")
    );
}

#[tokio::test]
async fn all_shared_paths_include_metadata_and_preserve_pages() {
    let store = Store::open_in_memory().await.expect("open store");
    let id = repo(&store, "a/io", Some(("wasi", "io"))).await;
    let manifest = release(&store, id, "1.0.0", Some("2000-01-01T00:00:00Z")).await;
    let wit = dependency(&store, manifest, "wasi:io", "wasi:io").await;
    let world = insert_wit_world(&store.db, wit, "test", None)
        .await
        .expect("world");
    for is_import in [true, false] {
        insert_wit_world_iface(&store.db, world, "wasi:io", None, None, is_import)
            .await
            .expect("world interface");
    }
    let consumer = repo(&store, "z/app", Some(("z", "app"))).await;
    let manifest = release(&store, consumer, "1.0.0", None).await;
    dependency(&store, manifest, "z:app", "wasi:io").await;
    let expected = package(&store, "a/io").await;
    assert_eq!(expected.dependents, Some(1));
    let lists = [
        store
            .search_known_packages("wasi", 0, 1)
            .await
            .expect("search"),
        store.list_known_packages(0, 1).await.expect("list"),
        store
            .list_recent_known_packages(0, 10)
            .await
            .expect("recent"),
        store
            .search_known_packages_by_import("wasi:io", 0, 1)
            .await
            .expect("imports"),
        store
            .search_known_packages_by_export("wasi:io", 0, 1)
            .await
            .expect("exports"),
    ];
    for list in lists {
        let found = list
            .iter()
            .find(|p| p.repository == "a/io")
            .expect("target in list");
        assert_eq!(found.dependents, expected.dependents);
        assert_eq!(found.latest_release_at, expected.latest_release_at);
    }
    let exact = store
        .search_known_package_by_wit_name("wasi:io")
        .await
        .expect("WIT lookup")
        .expect("exact");
    let fallback = store
        .search_known_package_by_wit_name("a:io")
        .await
        .expect("fallback lookup")
        .expect("fallback");
    for found in [exact, fallback] {
        assert_eq!(found.dependents, expected.dependents);
        assert_eq!(found.latest_release_at, expected.latest_release_at);
    }
    let second = store.list_known_packages(1, 1).await.expect("second page");
    assert_eq!(second.first().expect("second").repository, "z/app");
    assert!(
        store
            .search_known_packages("", 2, 1)
            .await
            .expect("end")
            .is_empty()
    );
}

#[tokio::test]
async fn page_enrichment_does_not_read_unselected_release_rows() {
    let store = Store::open_in_memory().await.expect("open store");
    let selected = repo(&store, "a/selected", Some(("a", "selected"))).await;
    release(&store, selected, "1.0.0", Some("2000-01-01T00:00:00Z")).await;
    let unrelated = repo(&store, "z/unrelated", None).await;
    release(&store, unrelated, "1.0.0", None).await;
    // A bad timestamp outside the page must not even be decoded. An unscoped
    // release query would fail while loading this unrelated repository.
    oci_tag::Entity::update_many()
        .col_expr(oci_tag::Column::CreatedAt, Expr::value("not a timestamp"))
        .filter(oci_tag::Column::OciRepositoryId.eq(unrelated))
        .exec(&store.db)
        .await
        .expect("corrupt unselected timestamp");
    let page = store
        .list_known_packages(0, 1)
        .await
        .expect("selected page");
    assert_eq!(
        page.first()
            .expect("selected package")
            .latest_release_at
            .as_deref(),
        Some("2000-01-01T00:00:00+00:00")
    );
    assert!(store.list_known_packages(1, 1).await.is_err());
}

#[tokio::test]
async fn metadata_load_chunks_large_pages() {
    let store = Store::open_in_memory().await.expect("open store");
    let id = repo(&store, "sample/io", Some(("sample", "io"))).await;
    release(&store, id, "1.0.0", Some("2000-01-01T00:00:00Z")).await;
    let model = oci_repository::Entity::find_by_id(id)
        .one(&store.db)
        .await
        .expect("load repository")
        .expect("sample repository");
    let mut repos = vec![model.clone()];
    for index in 0..super::BATCH_SIZE {
        let mut extra = model.clone();
        extra.wit_name = Some(format!("extra-{index}"));
        repos.push(extra);
    }
    let metadata = super::PackageMetadata::load(&store.db, &repos)
        .await
        .expect("multiple metadata batches");
    assert_eq!(metadata.dependents(&model), Some(0));
    assert_eq!(
        metadata.latest_release_at(id).as_deref(),
        Some("2000-01-01T00:00:00+00:00")
    );
    let empty = super::PackageMetadata::load(&store.db, &[])
        .await
        .expect("empty page");
    assert!(empty.dependents.is_empty());
    assert!(empty.latest_releases.is_empty());
}

#[tokio::test]
async fn metadata_errors_propagate_instead_of_becoming_zero_or_missing() {
    let store = Store::open_in_memory().await.expect("open store");
    let id = repo(&store, "known", Some(("known", "package"))).await;
    release(&store, id, "1.0.0", None).await;
    store
        .db
        .execute_unprepared("DROP TABLE wit_package_dependency")
        .await
        .expect("drop dependency table");
    assert!(store.list_known_packages(0, 10).await.is_err());
    assert!(store.get_known_package("ghcr.io", "known").await.is_err());

    let store = Store::open_in_memory().await.expect("open store");
    let id = repo(&store, "unknown", None).await;
    release(&store, id, "1.0.0", None).await;
    store
        .db
        .execute_unprepared(
            "ALTER TABLE oci_manifest RENAME COLUMN config_created TO missing_config_created",
        )
        .await
        .expect("break release query");
    assert!(store.list_known_packages(0, 10).await.is_err());
}
