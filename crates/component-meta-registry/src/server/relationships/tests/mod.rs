mod fixtures;
mod pagination;
mod tags;

use axum::http::StatusCode;
use sea_orm::ConnectionTrait;
use wasm_meta_registry_types::{
    DependentPackage, MatchingWorld, PackageKind, RelationshipPage, RelationshipTargetError,
};

use fixtures::Fixture;

const ENDPOINTS: [&str; 3] = ["dependents", "imported-by", "exported-by"];

async fn assert_json_error(response: reqwest::Response, status: StatusCode) {
    assert_eq!(response.status(), status);
    assert!(
        response
            .headers()
            .get("content-type")
            .expect("content type")
            .to_str()
            .expect("content type text")
            .starts_with("application/json")
    );
    let body: serde_json::Value = response.json().await.expect("error JSON");
    assert!(
        body.get("error")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|error| !error.is_empty()),
        "{body}"
    );
}

#[tokio::test]
async fn relationships_return_typed_matching_packages_and_worlds() {
    let fixture = Fixture::new().await;
    let repo = fixture.repository("test:source", "component").await;
    for (version, package) in [
        ("1.9.0", "wasi:io"),
        ("1.10.0_build.7", "wasi:io"),
        ("2.0.0", "wasi:clocks"),
    ] {
        let release = fixture.release(repo, "root:component", version).await;
        fixture.dependency(&release, package).await;
        let world = fixture.world(&release, "root", version).await;
        fixture.member(world, package, "streams", true).await;
        fixture.member(world, package, "poll", false).await;
    }
    let consumer = fixture.repository("test:consumer", "interface").await;
    let consumer = fixture.release(consumer, "test:consumer", "1.0.0").await;
    fixture.dependency(&consumer, "test:source").await;

    let first = fixture
        .get("/v1/relationships/dependents?package=wasi%3Aio&offset=0&limit=1")
        .await;
    assert_eq!(first.status(), StatusCode::OK);
    let first: RelationshipPage<DependentPackage> = first.json().await.expect("dependent page");
    assert_eq!(first.total, Some(2));
    assert!(first.has_next);
    assert_eq!(
        first.results.first().expect("consumer").package.repository,
        "test/consumer"
    );
    let last = fixture
        .get("/v1/relationships/dependents?package=wasi%3Aio&offset=1&limit=1")
        .await;
    assert_eq!(last.status(), StatusCode::OK);
    let last: RelationshipPage<DependentPackage> = last.json().await.expect("last dependent page");
    assert!(!last.has_next);
    assert_eq!((last.offset, last.limit), (1, 1));
    let source = last.results.first().expect("source");
    assert_eq!(source.version, "1.10.0_build.7");
    assert_eq!(source.package.kind, Some(PackageKind::Component));
    assert_eq!(source.package.tags.first().expect("latest tag"), "2.0.0");

    for (endpoint, interface) in [("imported-by", "streams"), ("exported-by", "poll")] {
        let response = fixture
            .get(&format!(
                "/v1/relationships/{endpoint}?package=wasi%3Aio&interface={interface}"
            ))
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        let page: RelationshipPage<MatchingWorld> = response.json().await.expect("world page");
        assert_eq!(page.total, Some(1));
        assert!(!page.has_next);
        let world = page.results.first().expect("matching world");
        assert_eq!(world.name, "root");
        assert_eq!(world.description.as_deref(), Some("1.10.0_build.7"));
        assert_eq!(world.version, "1.10.0_build.7");
        assert_eq!(world.package.kind, Some(PackageKind::Component));
        assert!(world.is_synthetic);
    }
    let wrong_direction = fixture
        .get("/v1/relationships/exported-by?package=wasi%3Aio&interface=streams")
        .await;
    let page: RelationshipPage<MatchingWorld> = wrong_direction
        .json()
        .await
        .expect("nonmatching export page");
    assert!(page.results.is_empty());
    assert_eq!(page.total, Some(0));
}

#[tokio::test]
async fn relationships_reject_malformed_targets_and_pagination_with_json_errors() {
    let fixture = Fixture::new().await;
    for endpoint in ENDPOINTS {
        for query in [
            "",
            "?package=",
            "?package=wasi",
            "?package=wasi%3Aio%400.2.0",
            "?package=wasi%3Aio%2Fstreams",
            "?package=Wasi%3Aio",
            "?package=wasi%3AIo",
            "?package=2wasi%3Aio",
            "?package=wasi%3A2io",
            "?package=wasi%3Afoo--bar",
            "?package=foo-2bar%3Aio",
            "?package=%27%20OR%201%3D1%20--",
            "?package=wasi%3Aio&package=test%3Aother",
            "?package=wasi%3Aio&offset=-1",
            "?package=wasi%3Aio&offset=4294967296",
            "?package=wasi%3Aio&limit=invalid",
            "?package=wasi%3Aio&limit=4294967296",
        ] {
            let response = fixture
                .get(&format!("/v1/relationships/{endpoint}{query}"))
                .await;
            assert_json_error(response, StatusCode::BAD_REQUEST).await;
        }
    }
    for endpoint in ["imported-by", "exported-by"] {
        for interface in [
            "",
            "streams%400.2.0",
            "io%2Fstreams",
            "%27%20OR%201%3D1",
            "Streams",
            "2streams",
            "foo--bar",
            "foo-2bar",
        ] {
            let response = fixture
                .get(&format!(
                    "/v1/relationships/{endpoint}?package=wasi%3Aio&interface={interface}"
                ))
                .await;
            assert_json_error(response, StatusCode::BAD_REQUEST).await;
        }
    }
    for interface in ["", "streams"] {
        let response = fixture
            .get(&format!(
                "/v1/relationships/dependents?package=wasi%3Aio&interface={interface}"
            ))
            .await;
        assert_json_error(response, StatusCode::BAD_REQUEST).await;
    }
}

#[tokio::test]
async fn relationships_distinguish_unclassified_synthetic_worlds_from_authored_roots() {
    let fixture = Fixture::new().await;
    for (identity, own_name) in [
        ("test:authored", "test:authored"),
        ("test:unclassified", "root:component"),
    ] {
        let repo = fixture.repository(identity, "interface").await;
        fixture
            .execute(
                "UPDATE oci_repository SET kind = NULL WHERE id = ?",
                vec![repo.into()],
            )
            .await;
        let release = fixture.release(repo, own_name, "1.0.0").await;
        let world = fixture.world(&release, "root", "A root world").await;
        fixture.member(world, "wasi:io", "streams", true).await;
        fixture.member(world, "wasi:io", "streams", false).await;
    }
    for endpoint in ["imported-by", "exported-by"] {
        let response = fixture
            .get(&format!("/v1/relationships/{endpoint}?package=wasi%3Aio"))
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        let page: RelationshipPage<MatchingWorld> = response.json().await.expect("world page");
        assert_eq!(page.total, Some(2));
        assert_eq!(page.results.len(), 2);
        let authored = &page.results[0];
        let synthetic = &page.results[1];
        assert_eq!(authored.package.wit_name.as_deref(), Some("authored"));
        assert_eq!(synthetic.package.wit_name.as_deref(), Some("unclassified"));
        assert_eq!(authored.name, "root");
        assert_eq!(synthetic.name, "root");
        assert!(authored.package.kind.is_none());
        assert!(synthetic.package.kind.is_none());
        assert!(!authored.is_synthetic);
        assert!(synthetic.is_synthetic);
    }
}

#[tokio::test]
async fn relationships_empty_pages_report_effective_pagination() {
    let fixture = Fixture::new().await;
    for endpoint in ENDPOINTS {
        for (query, limit) in [("", 20), ("&limit=0", 20), ("&limit=999999", 100)] {
            let response = fixture
                .get(&format!(
                    "/v1/relationships/{endpoint}?package=not%3Aindexed&offset=123{query}"
                ))
                .await;
            assert_eq!(response.status(), StatusCode::OK);
            let page: RelationshipPage<serde_json::Value> =
                response.json().await.expect("empty page");
            assert!(page.results.is_empty());
            assert_eq!(page.total, Some(0));
            assert_eq!(page.offset, 123);
            assert_eq!(page.limit, limit);
            assert!(!page.has_next);
        }
    }
}

#[tokio::test]
async fn relationships_propagate_database_failures_as_server_errors() {
    let fixture = Fixture::new().await;
    for (endpoint, table) in [
        ("dependents", "wit_package_dependency"),
        ("imported-by", "wit_world_import"),
        ("exported-by", "wit_world_export"),
    ] {
        fixture
            .db
            .execute_unprepared(&format!("DROP TABLE {table}"))
            .await
            .expect("break the selected relationship query");
        let response = fixture
            .get(&format!("/v1/relationships/{endpoint}?package=wasi%3Aio"))
            .await;
        assert_json_error(response, StatusCode::INTERNAL_SERVER_ERROR).await;
        let invalid = fixture
            .get(&format!("/v1/relationships/{endpoint}?package=invalid"))
            .await;
        assert_json_error(invalid, StatusCode::BAD_REQUEST).await;
    }
}

#[tokio::test]
async fn relationships_propagate_hydration_errors_as_server_errors() {
    let fixture = Fixture::new().await;
    fixture.matching_package("test:source", "1.0.0").await;
    fixture
        .db
        .execute_unprepared("UPDATE oci_repository SET created_at = 'invalid timestamp'")
        .await
        .expect("corrupt repository metadata");
    for endpoint in ENDPOINTS {
        let response = fixture
            .get(&format!("/v1/relationships/{endpoint}?package=wasi%3Aio"))
            .await;
        assert_json_error(response, StatusCode::INTERNAL_SERVER_ERROR).await;
    }
}

#[tokio::test]
async fn relationships_manager_validates_targets_before_querying() {
    let fixture = Fixture::new().await;
    fixture
        .db
        .execute_unprepared("DROP TABLE wit_package_dependency")
        .await
        .expect("break dependency query");
    let manager = fixture.state.read().await;
    let errors = [
        manager
            .list_dependents("wasi:io@0.2.0", 0, 100)
            .await
            .expect_err("invalid package"),
        manager
            .list_importing_worlds("wasi:io", Some("streams@0.2.0"), 0, 100)
            .await
            .expect_err("invalid interface"),
        manager
            .list_exporting_worlds("wasi", None, 0, 100)
            .await
            .expect_err("invalid package"),
    ];
    assert!(
        errors
            .iter()
            .all(|error| error.downcast_ref::<RelationshipTargetError>().is_some())
    );
}

#[tokio::test]
async fn relationships_leave_legacy_search_shapes_and_package_matching_unchanged() {
    let fixture = Fixture::new().await;
    fixture.matching_package("test:source", "1.0.0").await;
    for path in [
        "/v1/search?q=test",
        "/v1/search/by-import?interface=wasi%3Aio",
        "/v1/search/by-export?interface=wasi%3Aio",
    ] {
        let response = fixture.get(path).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body: serde_json::Value = response.json().await.expect("legacy JSON");
        let results = body.as_array().expect("legacy search remains an array");
        assert_eq!(results.len(), 1, "{path}");
        let package = results.first().expect("known package");
        assert!(package.get("registry").is_some());
        assert!(
            package.get("package").is_none(),
            "no relationship result wrapper"
        );
    }
    for endpoint in ["by-import", "by-export"] {
        let response = fixture
            .get(&format!(
                "/v1/search/{endpoint}?interface=wasi%3Aio%2Fstreams"
            ))
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body: serde_json::Value = response.json().await.expect("legacy member JSON");
        assert!(body.as_array().expect("legacy array").is_empty());
    }
}
