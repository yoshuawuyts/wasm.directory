use axum::http::StatusCode;
use wasm_meta_registry_types::{DependentPackage, RelationshipPage};

use super::{ENDPOINTS, Fixture};

#[tokio::test]
async fn relationships_http_paginates_more_than_a_hundred_deduplicated_matches() {
    let fixture = Fixture::new().await;
    for index in (0..102).rev() {
        let release = fixture
            .matching_package(&format!("test:item-{index:03}"), "1.9.0")
            .await;
        fixture.tag(&release, "1.10.0_build.2").await;
        fixture.tag(&release, "latest").await;
        fixture.tag(&release, "sha256-example.sig").await;
    }
    fixture
        .matching_package("test:aaa-nonsemver", "latest")
        .await;
    fixture
        .matching_package("test:zzz-nonsemver", "latest")
        .await;
    for endpoint in ENDPOINTS {
        for (offset, length, has_next) in [
            (0, 100, true),
            (100, 2, false),
            (2, 100, false),
            (102, 0, false),
        ] {
            let response = fixture
                .get(&format!(
                    "/v1/relationships/{endpoint}?package=wasi%3Aio&offset={offset}&limit=999"
                ))
                .await;
            assert_eq!(response.status(), StatusCode::OK);
            let page: RelationshipPage<serde_json::Value> =
                response.json().await.expect("relationship page");
            assert_eq!(page.total, Some(102));
            assert_eq!(page.results.len(), length);
            assert_eq!(page.offset, offset);
            assert_eq!(page.limit, 100);
            assert_eq!(page.has_next, has_next);
            assert!(page.results.iter().all(|row| {
                row.get("version").and_then(serde_json::Value::as_str) == Some("1.10.0_build.2")
            }));
        }
    }
    let response = fixture
        .get("/v1/relationships/dependents?package=wasi%3Aio&offset=100&limit=100")
        .await;
    let page: RelationshipPage<DependentPackage> = response.json().await.expect("last page");
    assert_eq!(
        page.results
            .iter()
            .map(|entry| entry.package.repository.as_str())
            .collect::<Vec<_>>(),
        ["test/item-100", "test/item-101"]
    );
}
