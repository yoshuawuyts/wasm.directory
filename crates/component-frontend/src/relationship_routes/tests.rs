use super::*;
use axum::body::{Body, to_bytes};
use axum::http::Request;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tower::ServiceExt;

pub(crate) async fn registry_response(
    status: &str,
    body: &str,
) -> (RegistryClient, tokio::task::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind registry fixture");
    let address = listener.local_addr().expect("fixture address");
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let task = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept registry request");
        let mut stream = BufReader::new(stream);
        let mut request = String::new();
        stream.read_line(&mut request).await.expect("read request");
        stream
            .get_mut()
            .write_all(response.as_bytes())
            .await
            .expect("write fixture");
        request
    });
    (RegistryClient::new(format!("http://{address}")), task)
}

#[tokio::test]
async fn malformed_queries_are_visible_uncached_client_errors() {
    for path in [
        "/search/dependents",
        "/search/dependents?package=wasi%3Aio&interface=streams",
        "/search/imported-by?package=wasi%3Aio%400.2.0",
        "/search/exported-by?package=wasi%3Aio&interface=",
        "/search/imported-by?package=wasi%3Aio&offset=invalid",
        "/search/dependents?package=Wasi%3Aio",
        "/search/dependents?package=2wasi%3Aio",
        "/search/imported-by?package=wasi%3Afoo--bar",
        "/search/imported-by?package=wasi%3Aio&interface=Streams",
        "/search/exported-by?package=wasi%3Aio&interface=foo-2bar",
        "/search/exported-by?package=wasi%3Aio&interface=foo--bar",
    ] {
        let response = crate::app()
            .oneshot(
                Request::builder()
                    .uri(path)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("route response");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{path}");
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-cache");
        let html = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body");
        assert!(String::from_utf8_lossy(&html).contains("Invalid relationship query"));
    }
}

#[tokio::test]
async fn each_handler_calls_only_its_fixed_query_and_caps_page_size() {
    for relation in [
        Relationship::Dependents,
        Relationship::ImportedBy,
        Relationship::ExportedBy,
    ] {
        let (client, request) = registry_response(
            "200 OK",
            r#"{"results":[],"total":0,"offset":100,"limit":100,"has_next":false}"#,
        )
        .await;
        let interface = (relation != Relationship::Dependents).then(|| "streams".to_owned());
        let response = handle(
            &client,
            relation,
            Ok(Query(RelationshipParams {
                package: "wasi:io".to_owned(),
                interface,
                offset: 100,
                limit: 200,
            })),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()[header::CACHE_CONTROL],
            "public, max-age=60"
        );
        let request = request.await.expect("fixture request");
        assert!(request.starts_with(&format!("GET /v1/relationships/{}?", relation.slug())));
        assert!(request.contains("package=wasi%3Aio"));
        assert!(request.contains("&offset=100&limit=100 "));
        assert_eq!(
            request.contains("&interface=streams"),
            relation != Relationship::Dependents
        );
    }
}

#[tokio::test]
async fn upstream_failures_are_uncached_errors_not_empty_pages() {
    let (client, request) = registry_response(
        "503 Service Unavailable",
        r#"{"results":[],"total":0,"offset":0,"limit":100,"has_next":false}"#,
    )
    .await;
    let response = handle(
        &client,
        Relationship::Dependents,
        Ok(Query(RelationshipParams {
            package: "wasi:io".to_owned(),
            interface: None,
            offset: 0,
            limit: 100,
        })),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-cache");
    assert!(!response.headers().contains_key(header::ETAG));
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("error body");
    let html = String::from_utf8_lossy(&body);
    assert!(html.contains("Unable to load relationship results"));
    assert!(html.contains("The registry could not complete this lookup."));
    assert!(!html.contains("503 Service Unavailable"));
    assert!(!html.contains("No dependents found"));
    request.await.expect("fixture finished");
}

#[tokio::test]
async fn legacy_dependents_redirect_to_versionless_relationships() {
    let response = crate::app()
        .oneshot(
            Request::builder()
                .uri("/wasi/io/0.2.0/dependents")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("redirect response");
    assert_eq!(response.status(), StatusCode::PERMANENT_REDIRECT);
    assert_eq!(
        response.headers()[header::LOCATION],
        "/search/dependents?package=wasi%3Aio"
    );
}
