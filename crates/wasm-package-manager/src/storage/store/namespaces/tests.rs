use super::super::{upsert_oci_manifest, upsert_oci_repository_full, upsert_oci_tag};
use super::*;

async fn seed(
    store: &Store,
    registry: &str,
    repository: &str,
    namespace: Option<&str>,
    tags: &[&str],
) {
    let id = upsert_oci_repository_full(
        &store.db,
        registry,
        repository,
        namespace,
        Some("package"),
        None,
    )
    .await
    .expect("seed namespace repository");
    let digest = format!("sha256:{id:064x}");
    upsert_oci_manifest(
        &store.db,
        id,
        &digest,
        None,
        None,
        None,
        None,
        None,
        None,
        &HashMap::new(),
    )
    .await
    .expect("seed namespace manifest");
    for tag in tags {
        upsert_oci_tag(&store.db, id, tag, &digest)
            .await
            .expect("seed namespace tag");
    }
}

#[tokio::test]
async fn registered_namespaces_include_empty_and_pending_but_exclude_unregistered_names() {
    let store = Store::open_in_memory()
        .await
        .expect("open namespace test store");
    for (registry, repository, namespace, tags) in [
        (
            "ghcr.io",
            "zeta/one",
            Some("alpha"),
            vec!["1.0.0", "2.0.0", "latest"],
        ),
        ("mirror.test", "zeta/one", Some("alpha"), vec!["1.0.0"]),
        ("ghcr.io", "alpha/two", None, vec!["1.0.0_build"]),
        ("ghcr.io", "beta/one", None, vec!["0.1.0"]),
        ("ghcr.io", "solo", None, vec!["0.1.0"]),
        (
            "ghcr.io",
            "ignored/one",
            Some("ignored"),
            vec!["latest", "sha256-abc.sig", "nightly", "v1.0.0"],
        ),
        ("ghcr.io", "untagged/one", Some("untagged"), vec![]),
        ("ghcr.io", "empty/one", Some(""), vec!["0.1.0"]),
    ] {
        seed(&store, registry, repository, namespace, &tags).await;
    }

    let registered = [
        "pending", "alpha", "beta", "empty", "ignored", "untagged", "alpha",
    ]
    .map(str::to_owned);
    let page = store
        .list_namespaces(&registered, 0, 100)
        .await
        .expect("list namespaces");
    assert_eq!(
        page.results,
        [
            KnownNamespace {
                name: "alpha".into(),
                packages: 3
            },
            KnownNamespace {
                name: "beta".into(),
                packages: 1
            },
            KnownNamespace {
                name: "empty".into(),
                packages: 0
            },
            KnownNamespace {
                name: "ignored".into(),
                packages: 0
            },
            KnownNamespace {
                name: "pending".into(),
                packages: 0
            },
            KnownNamespace {
                name: "untagged".into(),
                packages: 0
            },
        ]
    );
    assert_eq!(page.total, 6);
    assert!(!page.has_next);
    assert_eq!(
        store
            .registry_stats()
            .await
            .expect("registry stats")
            .namespaces,
        3
    );
}

#[tokio::test]
async fn namespace_pagination_covers_all_registrations_without_requiring_indexed_packages() {
    let store = Store::open_in_memory()
        .await
        .expect("open namespace test store");
    let registered: Vec<_> = (0..205).rev().map(|n| format!("ns-{n:03}")).collect();
    for n in 0..2 {
        seed(
            &store,
            "ghcr.io",
            &format!("ns-{n:03}/pkg"),
            None,
            &["0.1.0"],
        )
        .await;
    }
    let mut names = Vec::new();
    for (offset, count, has_next) in [
        (0, 100, true),
        (100, 100, true),
        (200, 5, false),
        (300, 0, false),
    ] {
        let page = store
            .list_namespaces(&registered, offset, 100)
            .await
            .expect("list namespace page");
        assert_eq!(page.total, 205);
        assert_eq!(page.results.len(), count);
        assert_eq!(page.has_next, has_next);
        names.extend(page.results.into_iter().map(|ns| ns.name));
    }
    assert_eq!(
        names,
        (0..205).map(|n| format!("ns-{n:03}")).collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn namespace_packages_match_exactly_and_filter_before_pagination() {
    let store = Store::open_in_memory()
        .await
        .expect("open namespace test store");
    seed(
        &store,
        "ghcr.io",
        "wasi/00-unreleased",
        Some("wasi"),
        &["latest"],
    )
    .await;
    seed(&store, "ghcr.io", "another/http", Some("wasi"), &["0.2.0"]).await;
    seed(&store, "ghcr.io", "wasi/io", None, &["0.2.0"]).await;
    seed(
        &store,
        "ghcr.io",
        "wasi/not-wasi",
        Some("wasi-extra"),
        &["0.2.0"],
    )
    .await;
    seed(&store, "ghcr.io", "wasi-extra/other", None, &["0.2.0"]).await;

    for (offset, repository, has_next) in [(0, "another/http", true), (1, "wasi/io", false)] {
        let page = store
            .list_namespace_packages("wasi", offset, 1)
            .await
            .expect("namespace packages");
        assert_eq!(page.total, 2);
        assert_eq!(page.results.len(), 1);
        assert_eq!(page.results[0].repository, repository);
        assert_eq!(page.has_next, has_next);
    }
    let missing = store
        .list_namespace_packages("missing", 0, 100)
        .await
        .expect("missing namespace");
    assert_eq!(missing.total, 0);
    assert!(missing.results.is_empty());
}

#[tokio::test]
async fn empty_index_and_extreme_offsets_return_empty_pages() {
    let store = Store::open_in_memory()
        .await
        .expect("open namespace test store");
    let empty = store
        .list_namespaces(&[], 0, 100)
        .await
        .expect("empty namespaces");
    assert_eq!(empty.total, 0);
    assert!(empty.results.is_empty());
    assert!(!empty.has_next);
    seed(&store, "ghcr.io", "wasi/io", Some("wasi"), &["0.2.0"]).await;
    let registered = ["wasi".to_owned()];
    let end = store
        .list_namespaces(&registered, u32::MAX, 100)
        .await
        .expect("past end");
    assert_eq!(end.total, 1);
    assert!(end.results.is_empty());
    assert!(!end.has_next);
    let zero = store
        .list_namespaces(&registered, 0, 0)
        .await
        .expect("zero limit");
    assert_eq!(zero.limit, 1);
    assert_eq!(zero.results.len(), 1);
    assert!(!zero.has_next);
}

#[tokio::test]
async fn failed_counts_are_errors_not_fabricated_zeroes_for_registered_namespaces() {
    use sea_orm::ConnectionTrait;

    let store = Store::open_in_memory()
        .await
        .expect("open namespace test store");
    store
        .db
        .execute_unprepared("DROP TABLE oci_tag")
        .await
        .expect("break count query");
    assert!(
        store
            .list_namespaces(&["registered".into()], 0, 100)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn owner_fallback_matching_escapes_wildcards_and_is_case_sensitive() {
    let store = Store::open_in_memory()
        .await
        .expect("open namespace test store");
    for repository in ["a_b/one", "axb/two", "a%b/three", "A_B/four", "a_b"] {
        seed(&store, "ghcr.io", repository, None, &["1.0.0"]).await;
    }
    let packages = store
        .list_namespace_packages("a_b", 0, 100)
        .await
        .expect("namespace packages");
    let repositories: Vec<_> = packages
        .results
        .iter()
        .map(|pkg| pkg.repository.as_str())
        .collect();
    assert_eq!(repositories, ["a_b", "a_b/one"]);
    let registered = ["a_b", "a%b", "axb"].map(str::to_owned);
    let page = store
        .list_namespaces(&registered, 0, 100)
        .await
        .expect("namespace counts");
    let counts: Vec<_> = page
        .results
        .iter()
        .map(|ns| (ns.name.as_str(), ns.packages))
        .collect();
    assert_eq!(counts, [("a%b", 1), ("a_b", 2), ("axb", 1)]);
}
