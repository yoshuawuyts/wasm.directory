use std::collections::HashMap;

use sea_orm::{ConnectionTrait, DbBackend, Statement};
use wasm_meta_registry_types::RelationshipPage;

use super::super::super::{upsert_oci_manifest, upsert_oci_tag};
use super::fixtures::member;
use super::{Store, fixture_store, package, repository, seed_release, target};

async fn matching_world(store: &Store, release: &super::fixtures::Release) {
    let world = release.world(store, "run", None).await;
    for is_import in [true, false] {
        member(
            store,
            world,
            "wasi:io",
            Some("streams"),
            Some("0.2.0"),
            is_import,
        )
        .await;
        member(
            store,
            world,
            "wasi:io",
            Some("streams"),
            Some("0.3.0"),
            is_import,
        )
        .await;
        member(store, world, "wasi:io", Some("poll"), None, is_import).await;
    }
}

fn assert_page<T>(
    page: &RelationshipPage<T>,
    offset: u32,
    limit: u32,
    length: usize,
    total: u64,
    has_next: bool,
) {
    assert_eq!(page.offset, offset);
    assert_eq!(page.limit, limit);
    assert_eq!(page.results.len(), length);
    assert_eq!(page.total, Some(total));
    assert_eq!(page.has_next, has_next);
}

#[tokio::test]
async fn relationship_pagination_filters_before_counting_offsets_and_next_page() {
    let store = fixture_store().await;
    for index in (0..102).rev() {
        let release = package(
            &store,
            &format!("test:item-{index:03}"),
            "1.9.0",
            &["wasi:io"],
        )
        .await;
        release.tag(&store, "1.10.0_build.2").await;
        release.tag(&store, "latest").await;
        release.tag(&store, "sha256-abc.sig").await;
        matching_world(&store, &release).await;
    }
    for name in ["test:aaa-nonsemver", "test:zzz-nonsemver"] {
        let release = package(&store, name, "latest", &["wasi:io"]).await;
        release.tag(&store, "sha256-only.sig").await;
        matching_world(&store, &release).await;
    }
    let mirror_repo = repository(
        &store,
        "mirror.test",
        "mirror/item",
        Some("test:item-000"),
        Some("interface"),
    )
    .await;
    let mirror = seed_release(&store, mirror_repo, "test:item-000", None, &["1.0.0"]).await;
    mirror.dependency(&store, "wasi:io", None).await;
    matching_world(&store, &mirror).await;

    let query = target("wasi:io", None);
    for (offset, limit, length, has_next) in [
        (0, 100, 100, true),
        (100, 100, 2, false),
        (2, 100, 100, false),
        (101, 1, 1, false),
        (102, 100, 0, false),
        (u32::MAX, u32::MAX, 0, false),
    ] {
        let dependents = store
            .list_dependents(&query, offset, limit)
            .await
            .expect("dependents");
        let imports = store
            .list_importing_worlds(&query, offset, limit)
            .await
            .expect("imports");
        let exports = store
            .list_exporting_worlds(&query, offset, limit)
            .await
            .expect("exports");
        assert_page(&dependents, offset, limit, length, 102, has_next);
        assert_page(&imports, offset, limit, length, 102, has_next);
        assert_page(&exports, offset, limit, length, 102, has_next);
        let expected: Vec<_> = (offset..102)
            .take(length)
            .map(|index| format!("test/item-{index:03}"))
            .collect();
        assert_eq!(
            dependents
                .results
                .iter()
                .map(|entry| entry.package.repository.clone())
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(
            imports
                .results
                .iter()
                .map(|entry| entry.package.repository.clone())
                .collect::<Vec<_>>(),
            expected
        );
        assert!(
            dependents
                .results
                .iter()
                .all(|entry| entry.version == "1.10.0_build.2")
        );
        assert!(
            imports
                .results
                .iter()
                .all(|entry| entry.version == "1.10.0_build.2")
        );
    }
}

#[tokio::test]
async fn relationship_tags_must_belong_to_the_matching_repository_and_manifest() {
    let store = fixture_store().await;
    let matching = package(&store, "test:source", "latest", &["wasi:io"]).await;
    matching_world(&store, &matching).await;
    package(&store, "test:source", "2.0.0", &["wasi:clocks"]).await;
    upsert_oci_manifest(
        &store.db,
        matching.repo_id,
        "sha256:unindexed",
        None,
        None,
        None,
        None,
        None,
        None,
        &HashMap::new(),
    )
    .await
    .expect("manifest without extracted WIT metadata");
    upsert_oci_tag(&store.db, matching.repo_id, "9.0.0", "sha256:unindexed")
        .await
        .expect("insert unextracted tag");
    let unrelated = repository(&store, "other.test", "source", Some("test:other"), None).await;
    upsert_oci_manifest(
        &store.db,
        unrelated,
        &matching.digest,
        None,
        None,
        None,
        None,
        None,
        None,
        &HashMap::new(),
    )
    .await
    .expect("same digest in another repository");
    upsert_oci_tag(&store.db, unrelated, "99.0.0", &matching.digest)
        .await
        .expect("another repository's semver tag");
    let query = target("wasi:io", None);
    let dependents = store
        .list_dependents(&query, 0, 100)
        .await
        .expect("dependents");
    let imports = store
        .list_importing_worlds(&query, 0, 100)
        .await
        .expect("imports");
    let exports = store
        .list_exporting_worlds(&query, 0, 100)
        .await
        .expect("exports");
    assert_page(&dependents, 0, 100, 0, 0, false);
    assert_page(&imports, 0, 100, 0, 0, false);
    assert_page(&exports, 0, 100, 0, 0, false);
}

#[tokio::test]
async fn relationship_metadata_is_hydrated_only_for_the_selected_page_and_errors_propagate() {
    let store = fixture_store().await;
    let first = package(&store, "test:a", "1.0.0", &["wasi:io"]).await;
    matching_world(&store, &first).await;
    let last = package(&store, "test:z", "1.0.0", &["wasi:io"]).await;
    matching_world(&store, &last).await;
    store
        .db
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "UPDATE oci_repository SET created_at = 'not-a-timestamp' WHERE id = ?",
            [last.repo_id.into()],
        ))
        .await
        .expect("corrupt off-page repository metadata");
    let query = target("wasi:io", None);
    let first = store
        .list_dependents(&query, 0, 1)
        .await
        .expect("first page");
    let imports = store
        .list_importing_worlds(&query, 0, 1)
        .await
        .expect("first import page");
    let exports = store
        .list_exporting_worlds(&query, 0, 1)
        .await
        .expect("first export page");
    assert_page(&first, 0, 1, 1, 2, true);
    assert_page(&imports, 0, 1, 1, 2, true);
    assert_page(&exports, 0, 1, 1, 2, true);
    assert!(store.list_dependents(&query, 1, 1).await.is_err());
    assert!(store.list_importing_worlds(&query, 1, 1).await.is_err());
    assert!(store.list_exporting_worlds(&query, 1, 1).await.is_err());
    let beyond = store
        .list_dependents(&query, 2, 1)
        .await
        .expect("out of range");
    assert_page(&beyond, 2, 1, 0, 2, false);
}
