use std::collections::HashMap;

use chrono::{DateTime, Utc};
use sea_orm::{ActiveModelTrait, Set};
use wasm_package_manager_migration::entities::oci_manifest;

use super::super::{Store, upsert_oci_manifest, upsert_oci_repository_full, upsert_oci_tag};
use super::publisher_time;

const REPOSITORY: &str = "andreiltd/componentize-qjs/componentize-qjs-runtime";
const INDEXED: &str = "2026-09-24T18:51:34.577639+00:00";

fn at(value: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(value)
        .expect("valid fixture timestamp")
        .with_timezone(&Utc)
}

async fn repository(store: &Store) -> i64 {
    upsert_oci_repository_full(
        &store.db,
        "ghcr.io",
        REPOSITORY,
        Some("andreiltd"),
        Some("componentize-qjs-runtime"),
        None,
    )
    .await
    .expect("seed repository")
}

async fn seed_version(
    store: &Store,
    repo: i64,
    tag: &str,
    digest: &str,
    annotation: Option<&str>,
    config: Option<&str>,
) {
    let annotations: HashMap<String, String> = annotation
        .map(|value| {
            (
                "org.opencontainers.image.created".to_owned(),
                value.to_owned(),
            )
        })
        .into_iter()
        .collect();
    let (id, _) = upsert_oci_manifest(
        &store.db,
        repo,
        digest,
        None,
        None,
        None,
        None,
        None,
        None,
        &annotations,
    )
    .await
    .expect("seed manifest");
    oci_manifest::ActiveModel {
        id: Set(id),
        created_at: Set(at(INDEXED)),
        config_created: Set(config.map(str::to_owned)),
        ..Default::default()
    }
    .update(&store.db)
    .await
    .expect("record config and fixed index time");
    upsert_oci_tag(&store.db, repo, tag, digest)
        .await
        .expect("seed tag");
}

#[test]
fn publisher_parser_has_no_indexing_fallback_or_cap() {
    assert_eq!(publisher_time([None, None]), None);
    assert_eq!(publisher_time([Some("garbage"), Some(" ")]), None);
    assert_eq!(
        publisher_time([Some(" \t2026-09-16T12:42:09.820719295+02:00\n"), None]),
        Some(at("2026-09-16T10:42:09.820719295Z"))
    );
    assert_eq!(
        publisher_time([Some("2030-01-01T00:00:00Z"), None]),
        Some(at("2030-01-01T00:00:00Z")),
        "ranking and publication callers apply their own future-date policy"
    );
}

#[tokio::test]
async fn selected_version_uses_its_own_real_config_timestamp() {
    let store = Store::open_in_memory().await.expect("open isolated store");
    let repo = repository(&store).await;
    // Public immutable GHCR manifest/config metadata, observed for the exact
    // requested repository; these are not indexing or latest-package dates.
    let versions = [
        (
            "0.4.5",
            "sha256:f9b8a1f133b7f9eb888a8e5d30073d8e0f820107e77850bcae403a25fc6942f1",
            "2026-09-16T10:42:09.820719295Z",
        ),
        (
            "0.4.4",
            "sha256:b9c9d698e5f5f9e712e88e2304bff1f6e18b6e1d09ecb1f3beb21f1279d7b7f0",
            "2026-09-02T15:11:52.178946721Z",
        ),
    ];
    for (tag, digest, created) in versions {
        seed_version(&store, repo, tag, digest, None, Some(created)).await;
    }
    for (tag, digest, created) in versions {
        let version = store
            .get_package_version("ghcr.io", REPOSITORY, tag)
            .await
            .expect("fetch selected version")
            .expect("version exists");
        assert_eq!(version.tag.as_deref(), Some(tag));
        assert_eq!(version.digest, digest);
        assert_eq!(version.created_at, Some(at(created).to_rfc3339()));
        assert_eq!(version.synced_at.as_deref(), Some(INDEXED));
        assert!(
            version.annotations.is_none(),
            "do not fabricate an annotation"
        );

        let json = serde_json::to_value(&version).expect("serialize API version");
        assert_eq!(json["created_at"], at(created).to_rfc3339());
        let decoded: wasm_meta_registry_types::PackageVersion =
            serde_json::from_value(json).expect("decode existing wire shape");
        assert_eq!(decoded.created_at, version.created_at);
    }
}

#[tokio::test]
async fn selected_publication_rejects_invalid_values_without_indexing_fallback() {
    let store = Store::open_in_memory().await.expect("open isolated store");
    let repo = repository(&store).await;
    let annotation = "2026-09-16T12:42:09.5+02:00";
    let config = "2026-09-02T15:11:52.178946721Z";
    let future = "2030-01-01T00:00:00Z";
    let cases = [
        (Some(annotation), Some(config), Some(annotation)),
        (Some("broken"), Some(config), Some(config)),
        (Some(future), Some(config), Some(config)),
        (None, Some(config), Some(config)),
        (Some(""), Some(" \t"), None),
        (Some("日本語"), Some("broken"), None),
        (Some(future), Some(future), None),
        (None, None, None),
    ];
    for (index, (annotation, config, expected)) in cases.into_iter().enumerate() {
        let tag = format!("1.0.{index}");
        seed_version(
            &store,
            repo,
            &tag,
            &format!("sha256:test{index}"),
            annotation,
            config,
        )
        .await;
        let version = store
            .get_package_version("ghcr.io", REPOSITORY, &tag)
            .await
            .expect("fetch version")
            .expect("version exists");
        assert_eq!(
            version.created_at,
            expected.map(|value| at(value).to_rfc3339())
        );
        assert_eq!(version.synced_at.as_deref(), Some(INDEXED));
        assert_eq!(
            version
                .annotations
                .as_ref()
                .and_then(|value| value.created.as_deref()),
            annotation,
            "raw annotation provenance stays unchanged"
        );
        if expected.is_none() {
            let json = serde_json::to_value(&version).expect("serialize");
            assert!(
                json.get("created_at").is_none(),
                "omit unavailable wire date"
            );
        }
    }
}
