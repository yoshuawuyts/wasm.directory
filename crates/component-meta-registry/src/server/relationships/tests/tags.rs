use axum::http::StatusCode;
use wasm_meta_registry_types::{DependentPackage, KnownPackage, PackageVersion, RelationshipPage};

use super::{ENDPOINTS, Fixture};

#[tokio::test]
async fn relationships_preserve_raw_oci_tags_through_exact_repository_and_manifest_lookup() {
    const RAW_TAG: &str = "1.10.0_build.7";
    let fixture = Fixture::new().await;
    let matching = fixture.matching_package("test:source", RAW_TAG).await;
    fixture.tag(&matching, "1.9.0").await;
    fixture.tag(&matching, "v99.0.0").await;
    let latest = fixture
        .release(matching.repo_id, "test:source", "2.0.0")
        .await;
    fixture.dependency(&latest, "wasi:clocks").await;
    let world = fixture.world(&latest, "run", "No longer matches").await;
    fixture.member(world, "wasi:clocks", "streams", true).await;
    fixture.member(world, "wasi:clocks", "streams", false).await;

    let response = fixture.get("/v1/packages/registry.test/test/source").await;
    assert_eq!(response.status(), StatusCode::OK);
    let repository: KnownPackage = response.json().await.expect("exact repository");
    assert_eq!(repository.tags.first().map(String::as_str), Some("2.0.0"));
    assert!(!repository.tags.iter().any(|tag| tag.starts_with('v')));

    for endpoint in ENDPOINTS {
        let response = fixture
            .get(&format!("/v1/relationships/{endpoint}?package=wasi%3Aio"))
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        // DependentPackage reads the package/version fields common to all three
        // result types; world-specific fields are covered by the world tests.
        let page: RelationshipPage<DependentPackage> = response
            .json()
            .await
            .expect("selected relationship release");
        assert_eq!(page.total, Some(1));
        assert!(!page.has_next);
        let selected = page.results.first().expect("matching release");
        assert_eq!(selected.version, RAW_TAG);
        assert_eq!(selected.package.registry, repository.registry);
        assert_eq!(selected.package.repository, repository.repository);
        assert!(selected.package.tags.contains(&selected.version));
        assert!(repository.tags.contains(&selected.version));

        let response = fixture
            .get(&format!(
                "/v1/packages/version/{}/{}/{}",
                selected.package.registry, selected.version, selected.package.repository
            ))
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        let version: PackageVersion = response.json().await.expect("exact matching manifest");
        assert_eq!(version.tag.as_deref(), Some(RAW_TAG));
        assert_eq!(version.digest, matching.digest);
    }
}

#[tokio::test]
async fn relationships_share_existing_leading_v_tag_eligibility_with_package_lookup() {
    let fixture = Fixture::new().await;
    fixture
        .matching_package("test:v-prefixed", "v1.10.0_build.7")
        .await;
    let response = fixture
        .get("/v1/packages/registry.test/test/v-prefixed")
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let repository: KnownPackage = response.json().await.expect("exact repository");
    assert!(repository.tags.is_empty());

    for endpoint in ENDPOINTS {
        let response = fixture
            .get(&format!("/v1/relationships/{endpoint}?package=wasi%3Aio"))
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        let page: RelationshipPage<serde_json::Value> =
            response.json().await.expect("ineligible release page");
        assert!(page.results.is_empty());
        assert_eq!(page.total, Some(0));
        assert!(!page.has_next);
    }
}
