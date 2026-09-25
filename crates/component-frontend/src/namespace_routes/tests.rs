use super::*;
use axum::body::{Body, to_bytes};
use axum::http::Request;
use tower::ServiceExt;

use crate::relationship_routes::tests::registry_response;

const PAGE: &str = r#"{"results":[{"name":"wasi","packages":12}],"total":1,"offset":0,"limit":100,"has_next":false}"#;
const EMPTY_PAGE: &str = r#"{"results":[],"total":0,"offset":100,"limit":100,"has_next":false}"#;

async fn request(client: RegistryClient, path: &str) -> Response {
    crate::app_with_client(client)
        .oneshot(
            Request::builder()
                .uri(path)
                .body(Body::empty())
                .expect("namespace page request"),
        )
        .await
        .expect("namespace page response")
}

async fn body(response: Response) -> String {
    String::from_utf8(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("page body")
            .to_vec(),
    )
    .expect("page UTF-8")
}

#[tokio::test]
async fn directory_routes_use_the_namespace_api_and_support_conditional_requests() {
    for path in ["/namespaces", "/namespaces/"] {
        let (client, upstream) = registry_response("200 OK", PAGE).await;
        let response = request(client, path).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()[header::CACHE_CONTROL],
            "public, max-age=60"
        );
        let etag = response.headers()[header::ETAG].clone();
        let html = body(response).await;
        assert!(html.contains("showing 1 of 1 result"));
        assert!(html.contains("href=\"/wasi\""));
        assert_eq!(
            upstream.await.expect("upstream request").trim(),
            "GET /v1/namespaces?offset=0&limit=100 HTTP/1.1"
        );

        let (client, upstream) = registry_response("200 OK", PAGE).await;
        let response = crate::app_with_client(client)
            .oneshot(
                Request::builder()
                    .uri(path)
                    .header(header::IF_NONE_MATCH, etag)
                    .body(Body::empty())
                    .expect("conditional request"),
            )
            .await
            .expect("conditional response");
        assert_eq!(response.status(), StatusCode::NOT_MODIFIED);
        upstream.await.expect("conditional upstream request");
    }
}

#[tokio::test]
async fn namespace_pages_negotiate_compression_before_revalidation() {
    use std::io::Read;

    for path in ["/namespaces", "/wasi"] {
        let (client, upstream) = registry_response("200 OK", EMPTY_PAGE).await;
        let identity = request(client, path).await;
        let etag = identity.headers()[header::ETAG].clone();
        assert!(etag.to_str().expect("ETag text").starts_with("W/"));
        let original = body(identity).await;
        upstream.await.expect("identity request");

        for (encoding, validator, expected_status) in [
            ("gzip", "\"stale\"", StatusCode::OK),
            (
                "gzip",
                etag.to_str().expect("ETag text"),
                StatusCode::NOT_MODIFIED,
            ),
            (
                "identity;q=0, gzip;q=0, br;q=0",
                "*",
                StatusCode::NOT_ACCEPTABLE,
            ),
        ] {
            let (client, upstream) = registry_response("200 OK", EMPTY_PAGE).await;
            let response = crate::app_with_client(client)
                .oneshot(
                    Request::builder()
                        .uri(path)
                        .header(header::ACCEPT_ENCODING, encoding)
                        .header(header::IF_NONE_MATCH, validator)
                        .body(Body::empty())
                        .expect("negotiated namespace request"),
                )
                .await
                .expect("negotiated namespace response");
            assert_eq!(response.status(), expected_status);
            match expected_status {
                StatusCode::NOT_ACCEPTABLE => {
                    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
                    assert!(!response.headers().contains_key(header::ETAG));
                }
                _ => {
                    assert_eq!(response.headers()[header::CONTENT_ENCODING], "gzip");
                    assert_eq!(response.headers()[header::ETAG], etag);
                }
            }
            let bytes = to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("response bytes");
            if expected_status == StatusCode::OK {
                let mut decoded = String::new();
                flate2::read::GzDecoder::new(bytes.as_ref())
                    .read_to_string(&mut decoded)
                    .expect("decode namespace HTML");
                assert_eq!(decoded, original);
            } else {
                assert!(bytes.is_empty());
            }
            upstream.await.expect("negotiated upstream request");
        }
    }
}

#[tokio::test]
async fn directory_and_namespace_pages_preserve_offsets_and_cap_limits() {
    for (path, expected) in [
        ("/namespaces", "/v1/namespaces"),
        ("/wasi", "/v1/namespaces/wasi/packages"),
        ("/wasi/", "/v1/namespaces/wasi/packages"),
        ("/a%26b", "/v1/namespaces/a%26b/packages"),
    ] {
        for (requested, effective) in [(0, 1), (2, 2), (200, 100), (u32::MAX, 100)] {
            let (client, upstream) = registry_response("200 OK", EMPTY_PAGE).await;
            let response = request(client, &format!("{path}?offset=100&limit={requested}")).await;
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(
                upstream.await.expect("upstream request").trim(),
                format!("GET {expected}?offset=100&limit={effective} HTTP/1.1")
            );
        }
    }
}

#[tokio::test]
async fn namespace_destination_renders_packages_and_real_pagination() {
    let packages = crate::components::ds::package_row::tests::packages();
    let payload = serde_json::json!({
        "results": packages,
        "total": 25,
        "offset": 20,
        "limit": 4,
        "has_next": true
    });
    let (client, upstream) = registry_response("200 OK", &payload.to_string()).await;
    let response = request(client, "/example?offset=20&limit=4").await;
    assert_eq!(response.status(), StatusCode::OK);
    let html = body(response).await;
    crate::components::ds::package_row::tests::assert_listing(&html, &packages);
    assert!(html.contains("showing 4 of 25 results"));
    assert!(html.contains("href=\"/example?offset=16&limit=4\""));
    assert!(html.contains("href=\"/example?offset=24&limit=4\""));
    upstream.await.expect("namespace request");
}

#[tokio::test]
async fn registered_empty_and_pending_namespace_destinations_are_valid_empty_pages() {
    for name in ["empty", "pending"] {
        let payload = r#"{"results":[],"total":0,"offset":0,"limit":100,"has_next":false}"#;
        let (client, upstream) = registry_response("200 OK", payload).await;
        let response = request(client, &format!("/{name}")).await;
        assert_eq!(response.status(), StatusCode::OK);
        let html = body(response).await;
        assert!(html.contains("No indexed packages found under this namespace yet."));
        assert!(html.contains("showing 0 of 0 results"));
        upstream.await.expect("empty namespace request");
    }
}

#[tokio::test]
async fn failures_are_visible_uncached_errors_not_empty_results() {
    for path in ["/namespaces", "/wasi"] {
        for (status, payload) in [
            ("503 Service Unavailable", EMPTY_PAGE),
            ("404 Not Found", EMPTY_PAGE),
            ("200 OK", "not JSON"),
        ] {
            let (client, upstream) = registry_response(status, payload).await;
            let response = request(client, path).await;
            assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
            assert_eq!(response.headers()[header::CACHE_CONTROL], "no-cache");
            assert!(!response.headers().contains_key(header::ETAG));
            let html = body(response).await;
            assert!(html.contains("Unable to load"));
            assert!(!html.contains("No namespaces have been registered"));
            assert!(!html.contains("No indexed packages found"));
            assert!(!html.contains("showing 0"));
            upstream.await.expect("failed upstream request");
        }
    }
}

#[tokio::test]
async fn invalid_pagination_and_reserved_package_paths_do_not_fetch() {
    for (path, status) in [
        ("/namespaces?offset=-1", StatusCode::BAD_REQUEST),
        ("/namespaces?limit=no", StatusCode::BAD_REQUEST),
        ("/wasi?offset=4294967296", StatusCode::BAD_REQUEST),
        ("/namespaces/pkg", StatusCode::NOT_FOUND),
        ("/namespaces/pkg/1.0.0", StatusCode::NOT_FOUND),
    ] {
        let response = request(RegistryClient::new("http://127.0.0.1:1"), path).await;
        assert_eq!(response.status(), status, "{path}");
    }
}
