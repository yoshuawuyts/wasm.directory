use std::collections::BTreeSet;

use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode, header};
use axum::response::Response;

use super::fixtures::WIT;
use super::navigation::{assert_source_destinations, package_destinations};
use super::registry::RegistryFixture;

const SOURCE: &str = "?registry=mirror.test&repository=mirrors%2Fhttp";
const DETAIL_PATHS: &[&str] = &[
    "",
    "/interface/types",
    "/interface/types/request",
    "/interface/types/response",
    "/interface/types/send",
    "/world/proxy",
    "/world/proxy/function/run",
    "/module/tool",
    "/component/0",
];

async fn html(response: Response) -> String {
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read detail HTML");
    String::from_utf8(body.to_vec()).expect("UTF-8 detail HTML")
}

#[tokio::test]
async fn every_detail_handler_resolves_the_selected_mirror_and_release() {
    let registry = RegistryFixture::new(Some(WIT)).await;
    for suffix in DETAIL_PATHS {
        let uri = format!("/example/http/0.1.0{suffix}{SOURCE}");
        let response = registry.request(Method::GET, &uri).await;
        assert_eq!(response.status(), StatusCode::OK, "{uri}");
        assert_source_destinations(&html(response).await, &["/example/http/0.1.0"]);
    }
    let requests = registry.requests();
    assert_eq!(requests.len(), DETAIL_PATHS.len() * 2);
    for pair in requests.chunks_exact(2) {
        assert_eq!(pair[0], "/v1/packages/mirror.test/mirrors/http");
        assert_eq!(
            pair[1],
            "/v1/packages/version/mirror.test/0.1.0/mirrors/http"
        );
    }
}

#[tokio::test]
async fn following_generated_navigation_never_falls_back_to_another_mirror() {
    let registry = RegistryFixture::new(Some(WIT)).await;
    let mut destinations = BTreeSet::new();
    for suffix in ["", "/interface/types", "/world/proxy"] {
        let uri = format!("/example/http/0.1.0{suffix}{SOURCE}");
        let response = registry.request(Method::GET, &uri).await;
        assert_eq!(response.status(), StatusCode::OK, "{uri}");
        let page = html(response).await;
        destinations.extend(
            package_destinations(&page)
                .into_iter()
                .map(|url| url.replace("&amp;", "&")),
        );
    }
    assert!(destinations.contains(&format!("/example/http/0.2.0{SOURCE}")));
    assert!(destinations.contains(&format!(
        "/example/http/0.1.0/world/proxy/function/run{SOURCE}"
    )));
    for uri in destinations {
        let response = registry.request(Method::GET, &uri).await;
        assert_eq!(response.status(), StatusCode::OK, "{uri}");
        assert_source_destinations(&html(response).await, &[]);
    }
    assert!(
        registry.requests().iter().all(|path| {
            path == "/v1/packages/mirror.test/mirrors/http"
                || path == "/v1/packages/version/mirror.test/0.1.0/mirrors/http"
                || path == "/v1/packages/version/mirror.test/0.2.0/mirrors/http"
        }),
        "generated navigation must not consult WIT-name search or another repository"
    );
}

#[tokio::test]
async fn synthetic_package_functions_keep_the_selected_mirror() {
    let registry = RegistryFixture::new(Some(
        "package root:component; world root { export run: func(); }",
    ))
    .await;
    for suffix in ["", "/function/run"] {
        let uri = format!("/example/http/0.1.0{suffix}{SOURCE}");
        let response = registry.request(Method::GET, &uri).await;
        assert_eq!(response.status(), StatusCode::OK, "{uri}");
        assert_source_destinations(
            &html(response).await,
            &["/example/http/0.1.0", "/example/http/0.1.0/function/run"],
        );
    }
    assert_eq!(registry.requests().len(), 4);
}

#[tokio::test]
async fn partial_source_parameters_are_rejected_by_all_detail_handlers() {
    let registry = RegistryFixture::new(Some(WIT)).await;
    for suffix in DETAIL_PATHS
        .iter()
        .copied()
        .chain(["/function/run", "/dependencies"])
    {
        let uri = format!("/example/http/0.1.0{suffix}?registry=mirror.test");
        let response = registry.request(Method::GET, &uri).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{uri}");
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-cache");
    }
    let response = registry
        .request(Method::GET, "/example/http?registry=mirror.test")
        .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(registry.requests().is_empty());
}

#[tokio::test]
async fn unavailable_sources_and_mismatched_versions_never_fall_back() {
    let registry = RegistryFixture::new(Some(WIT)).await;
    for suffix in DETAIL_PATHS {
        let uri =
            format!("/example/http/0.1.0{suffix}?registry=missing.test&repository=mirrors%2Fhttp");
        let response = registry.request(Method::GET, &uri).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{uri}");
    }
    for path in [
        "/example/http/9.0.0",
        "/example/http/9.0.0/interface/types",
        "/other/http/0.1.0",
    ] {
        let response = registry
            .request(Method::GET, &format!("{path}{SOURCE}"))
            .await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
    }
    assert!(
        !registry
            .requests()
            .iter()
            .any(|path| path.starts_with("/v1/search"))
    );
}

#[tokio::test]
async fn version_and_legacy_redirects_preserve_the_source() {
    let registry = RegistryFixture::new(Some(WIT)).await;
    for path in ["/example/http", "/example/http/"] {
        let response = registry
            .request(Method::GET, &format!("{path}{SOURCE}"))
            .await;
        assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
        assert_eq!(
            response.headers()[header::LOCATION],
            format!("/example/http/0.2.0{SOURCE}")
        );
    }
    let response = registry
        .request(
            Method::GET,
            &format!("/example/http/0.1.0/dependencies{SOURCE}"),
        )
        .await;
    assert_eq!(response.status(), StatusCode::PERMANENT_REDIRECT);
    assert_eq!(
        response.headers()[header::LOCATION],
        format!("/example/http/0.1.0{SOURCE}")
    );
    assert_eq!(registry.requests().len(), 2);
}

#[tokio::test]
async fn ordinary_name_lookup_pins_the_repository_it_resolves() {
    let registry = RegistryFixture::new(Some(WIT)).await;
    let response = registry.request(Method::GET, "/example/http").await;
    assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(
        response.headers()[header::LOCATION],
        "/example/http/9.0.0?registry=canonical.test&repository=canonical%2Fhttp"
    );
}

#[tokio::test]
async fn source_pinned_head_and_conditional_gets_preserve_cache_behavior() {
    let registry = RegistryFixture::new(Some(WIT)).await;
    let uri = format!("/example/http/0.1.0/interface/types{SOURCE}");
    let response = registry.request(Method::HEAD, &uri).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()[header::CACHE_CONTROL],
        "public, max-age=300"
    );
    let etag = response.headers()[header::ETAG].clone();
    assert!(html(response).await.is_empty());
    let response = registry
        .send(
            Request::builder()
                .uri(&uri)
                .header(header::IF_NONE_MATCH, etag.clone())
                .body(Body::empty())
                .expect("conditional detail request"),
        )
        .await;
    assert_eq!(response.status(), StatusCode::NOT_MODIFIED);
    assert_eq!(response.headers()[header::ETAG], etag);
    assert_eq!(
        response.headers()[header::CACHE_CONTROL],
        "public, max-age=300"
    );
    assert!(html(response).await.is_empty());
}
