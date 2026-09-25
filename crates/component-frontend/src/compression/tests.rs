use std::io::Read;

use axum::body::to_bytes;
use axum::http::{HeaderValue, Request};
use axum::routing::get;
use tower::ServiceExt;

use super::*;

mod registry;

async fn request(
    router: Router,
    method: Method,
    path: &str,
    encoding: Option<&str>,
    validators: &[&str],
) -> Response {
    let mut request = Request::builder().method(method).uri(path);
    if let Some(encoding) = encoding {
        request = request.header(header::ACCEPT_ENCODING, encoding);
    }
    for validator in validators {
        request = request.header(header::IF_NONE_MATCH, *validator);
    }
    crate::server::serve_with_router(
        request
            .body(wstd::http::Body::empty())
            .expect("valid request"),
        router,
    )
    .await
    .expect("response")
    .map(|body| Body::new(body.into_boxed_body()))
}

async fn body(response: Response) -> Vec<u8> {
    to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read response body")
        .to_vec()
}

fn decode(encoding: &str, bytes: &[u8]) -> Vec<u8> {
    let mut decoded = Vec::new();
    match encoding {
        "gzip" => flate2::read::GzDecoder::new(bytes)
            .read_to_end(&mut decoded)
            .expect("decode gzip"),
        "br" => brotli::Decompressor::new(bytes, 4096)
            .read_to_end(&mut decoded)
            .expect("decode Brotli"),
        _ => panic!("unexpected encoding {encoding}"),
    };
    decoded
}

fn varies_on_encoding(headers: &HeaderMap) -> bool {
    headers
        .get_all(header::VARY)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .any(|value| value.trim().eq_ignore_ascii_case("accept-encoding"))
}

#[tokio::test]
async fn homepage_compression_round_trips_the_actual_rendered_html() {
    let registry = registry::Registry::new().await;
    let app = crate::app_with_client(registry.client());
    let identity = request(app.clone(), Method::GET, "/", Some("identity"), &[]).await;
    assert_eq!(identity.status(), StatusCode::OK);
    assert!(varies_on_encoding(identity.headers()));
    assert!(!identity.headers().contains_key(header::CONTENT_ENCODING));
    let etag = identity.headers()[header::ETAG].clone();
    let original = body(identity).await;
    let html = std::str::from_utf8(&original).expect("HTML is UTF-8");
    assert!(html.contains("<!DOCTYPE html>"));
    assert!(!html.contains("Registry offline"));

    for encoding in ["gzip", "br"] {
        let response = request(app.clone(), Method::GET, "/", Some(encoding), &[]).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CONTENT_ENCODING], encoding);
        assert_eq!(
            response.headers()[header::CONTENT_TYPE],
            "text/html; charset=utf-8"
        );
        assert_eq!(
            response.headers()[header::CACHE_CONTROL],
            "public, max-age=60"
        );
        assert_eq!(response.headers()[header::ETAG], etag);
        assert!(!response.headers().contains_key(header::CONTENT_LENGTH));
        assert!(varies_on_encoding(response.headers()));
        let encoded = body(response).await;
        assert!(
            encoded.len() < original.len(),
            "{encoding} should save bytes"
        );
        assert_eq!(decode(encoding, &encoded), original);
    }
}

#[tokio::test]
async fn negotiation_respects_preferences_exclusions_and_identity_fallback() {
    let original = body(request(crate::app(), Method::GET, "/docs", None, &[]).await).await;
    for (accept, expected) in [
        (None, None),
        (Some(""), None),
        (Some("identity"), None),
        (Some("compress, zstd, deflate"), None),
        (Some("br;q=0, gzip;q=0"), None),
        (Some("br;q=1.1, gzip;q=broken"), None),
        (Some("br;q=0.1234"), None),
        (Some("br;q=0.2, identity;q=0.8"), None),
        (Some("gzip"), Some("gzip")),
        (Some("br, gzip"), Some("br")),
        (Some("br;q=0.3, gzip;q=0.7"), Some("gzip")),
        (Some("br;q=0, gzip;q=0.5"), Some("gzip")),
        (Some("GZIP;Q=1.000"), Some("gzip")),
        (Some("*"), Some("br")),
        (Some("*;q=0.5, br;q=0"), Some("gzip")),
        (Some("*;q=0, gzip;q=1"), Some("gzip")),
        (Some("identity;q=0, br"), Some("br")),
    ] {
        let response = request(crate::app(), Method::GET, "/docs", accept, &[]).await;
        assert_eq!(response.status(), StatusCode::OK, "{accept:?}");
        assert!(varies_on_encoding(response.headers()), "{accept:?}");
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_ENCODING)
                .map(|v| v.to_str().expect("encoding")),
            expected,
            "{accept:?}"
        );
        let bytes = body(response).await;
        let decoded = match expected {
            Some(encoding) => decode(encoding, &bytes),
            None => bytes,
        };
        assert_eq!(decoded, original, "{accept:?}");
    }
}

#[tokio::test]
async fn multiple_accept_encoding_fields_are_combined() {
    let response = crate::app()
        .oneshot(
            Request::builder()
                .uri("/docs")
                .header(header::ACCEPT_ENCODING, "br;q=0.2")
                .header(header::ACCEPT_ENCODING, "gzip;q=0.8")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.headers()[header::CONTENT_ENCODING], "gzip");
    assert!(!decode("gzip", &body(response).await).is_empty());
}

// r[verify frontend.caching.etag]
#[tokio::test]
async fn conditional_requests_keep_negotiated_metadata_without_sending_a_body() {
    for encoding in ["identity", "gzip", "br"] {
        let response = request(crate::app(), Method::GET, "/docs", Some(encoding), &[]).await;
        let mut expected = response.headers().clone();
        expected.remove(header::CONTENT_LENGTH);
        let etag = expected
            .get(header::ETAG)
            .expect("etag header")
            .to_str()
            .expect("etag")
            .to_owned();
        assert!(
            etag.starts_with("W/"),
            "validators must not imply byte equality"
        );
        let strong = etag.strip_prefix("W/").expect("weak validator");
        let list = format!("\"stale\", {etag}");
        for validators in [
            vec![etag.as_str()],
            vec![strong],
            vec!["*"],
            vec!["\"stale\"", etag.as_str()],
            vec![list.as_str()],
        ] {
            for method in [Method::GET, Method::HEAD] {
                let response =
                    request(crate::app(), method, "/docs", Some(encoding), &validators).await;
                assert_eq!(response.status(), StatusCode::NOT_MODIFIED);
                assert_eq!(response.headers(), &expected);
                assert!(body(response).await.is_empty(), "{encoding} 304 body");
            }
        }
        let response = request(
            crate::app(),
            Method::GET,
            "/docs",
            Some(encoding),
            &["\"stale\""],
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert!(!body(response).await.is_empty());
    }
}

#[tokio::test]
async fn unacceptable_encoding_cannot_be_revalidated_or_cached_as_a_page() {
    for encoding in ["*;q=0", "identity;q=0", "br;q=0, gzip;q=0, identity;q=0"] {
        let response = request(crate::app(), Method::GET, "/docs", Some(encoding), &["*"]).await;
        assert_eq!(response.status(), StatusCode::NOT_ACCEPTABLE);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        assert!(!response.headers().contains_key(header::ETAG));
        assert!(!response.headers().contains_key(header::CONTENT_ENCODING));
        assert!(varies_on_encoding(response.headers()));
        assert!(body(response).await.is_empty());
    }
}

#[tokio::test]
async fn text_assets_compress_but_media_is_left_alone() {
    for path in [
        crate::tailwind::PATH,
        "/health",
        "/install/linux",
        "/robots.txt",
        "/favicon.svg",
        "/favicon.ico",
    ] {
        let identity = request(crate::app(), Method::GET, path, None, &[]).await;
        let content_type = identity.headers()[header::CONTENT_TYPE].clone();
        let original = body(identity).await;
        let response = request(crate::app(), Method::GET, path, Some("gzip"), &[]).await;
        assert_eq!(response.headers()[header::CONTENT_TYPE], content_type);
        if path == "/favicon.ico" {
            assert!(!response.headers().contains_key(header::CONTENT_ENCODING));
            assert_eq!(body(response).await, original);
        } else {
            assert_eq!(response.headers()[header::CONTENT_ENCODING], "gzip");
            assert_eq!(decode("gzip", &body(response).await), original);
        }
    }
}

#[tokio::test]
async fn empty_statuses_preencoded_data_and_ranges_are_not_compressed() {
    for (status, headers, bytes) in [
        (StatusCode::NO_CONTENT, HeaderMap::new(), b"".as_slice()),
        (StatusCode::RESET_CONTENT, HeaderMap::new(), b"".as_slice()),
        (StatusCode::NOT_MODIFIED, HeaderMap::new(), b"".as_slice()),
        (
            StatusCode::OK,
            HeaderMap::from_iter([(header::CONTENT_ENCODING, HeaderValue::from_static("gzip"))]),
            b"already encoded".as_slice(),
        ),
        (
            StatusCode::PARTIAL_CONTENT,
            HeaderMap::from_iter([(
                header::CONTENT_RANGE,
                HeaderValue::from_static("bytes 0-3/100"),
            )]),
            b"part".as_slice(),
        ),
    ] {
        let expected_headers = headers.clone();
        let app = layer(Router::new().route(
            "/",
            get(move || async move {
                (
                    status,
                    headers,
                    [(header::CONTENT_TYPE, "text/plain")],
                    bytes,
                )
            }),
        ));
        let response = request(app, Method::GET, "/", Some("br, gzip"), &[]).await;
        assert_eq!(response.status(), status);
        if matches!(status, StatusCode::NO_CONTENT | StatusCode::NOT_MODIFIED) {
            assert!(!response.headers().contains_key(header::CONTENT_LENGTH));
        }
        assert_eq!(
            response.headers().get(header::CONTENT_ENCODING),
            expected_headers.get(header::CONTENT_ENCODING)
        );
        assert_eq!(body(response).await, bytes);
    }
}

#[tokio::test]
async fn compression_preserves_other_vary_fields() {
    let app = layer(Router::new().route(
        "/",
        get(|| async { ([(header::VARY, "Origin")], "text to compress") }),
    ));
    let response = request(app, Method::GET, "/", Some("gzip"), &[]).await;
    assert!(varies_on_encoding(response.headers()));
    assert!(
        response
            .headers()
            .get_all(header::VARY)
            .iter()
            .any(|v| v == "Origin")
    );
    assert_eq!(decode("gzip", &body(response).await), b"text to compress");
}

#[tokio::test]
async fn streamed_body_errors_remain_errors_in_both_encodings() {
    for encoding in ["gzip", "br"] {
        let app = layer(Router::new().route(
            "/",
            get(|| async {
                let stream = tokio_stream::iter([
                    Ok("first chunk"),
                    Err(std::io::Error::other("fixture body failed")),
                ]);
                (
                    [(header::CONTENT_TYPE, "text/plain")],
                    Body::from_stream(stream),
                )
            }),
        ));
        let response = request(app, Method::GET, "/", Some(encoding), &[]).await;
        assert_eq!(response.headers()[header::CONTENT_ENCODING], encoding);
        assert!(to_bytes(response.into_body(), usize::MAX).await.is_err());
    }
}

#[tokio::test]
async fn fallback_html_compresses_without_becoming_cacheable_or_not_modified() {
    for encoding in ["gzip", "br"] {
        let response = request(
            crate::app(),
            Method::GET,
            "/docs/missing",
            Some(encoding),
            &["*"],
        )
        .await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-cache");
        assert!(!response.headers().contains_key(header::ETAG));
        assert_eq!(response.headers()[header::CONTENT_ENCODING], encoding);
        assert!(!decode(encoding, &body(response).await).is_empty());
    }
}

#[tokio::test]
async fn binary_media_and_event_streams_are_not_encoded() {
    for content_type in [
        "application/gzip",
        "application/zip",
        "video/mp4",
        "text/event-stream",
    ] {
        let app = layer(Router::new().route(
            "/",
            get(move || async move {
                (
                    [(header::CONTENT_TYPE, content_type)],
                    "unchanged response bytes",
                )
            }),
        ));
        let response = request(app, Method::GET, "/", Some("gzip, br"), &[]).await;
        assert!(!response.headers().contains_key(header::CONTENT_ENCODING));
        assert_eq!(body(response).await, b"unchanged response bytes");
    }
}

#[tokio::test]
async fn weak_validators_revalidate_across_encodings_but_not_changed_content() {
    let app = || {
        layer(Router::new().route(
            "/{text}",
            get(
                |axum::extract::Path(text): axum::extract::Path<String>| async move {
                    crate::with_cache_control(text, "public, max-age=60")
                },
            ),
        ))
    };
    let response = request(app(), Method::GET, "/original", Some("gzip"), &[]).await;
    let etag = response
        .headers()
        .get(header::ETAG)
        .expect("etag")
        .to_str()
        .expect("etag")
        .to_owned();
    for encoding in ["identity", "br"] {
        let response = request(app(), Method::GET, "/original", Some(encoding), &[&etag]).await;
        assert_eq!(response.status(), StatusCode::NOT_MODIFIED);
        assert!(body(response).await.is_empty());
        let response = request(app(), Method::GET, "/changed", Some(encoding), &[&etag]).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_ne!(response.headers()[header::ETAG], etag);
    }
}
