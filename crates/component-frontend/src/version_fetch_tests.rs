use super::*;
use std::future::Future;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::time::{Duration, timeout};

const VERSION: &str = "1.0.0";
const MINIMAL_DETAIL: &str = r#"{"tag":"1.0.0","digest":"sha256:abc"}"#;

async fn with_version_response<F, Fut, T>(status: &str, body: &str, fetch: F) -> T
where
    F: FnOnce(RegistryClient, KnownPackage) -> Fut,
    Fut: Future<Output = T>,
{
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind selected-version registry fixture");
    let address = listener
        .local_addr()
        .expect("get selected-version registry fixture address");
    let client = RegistryClient::new(format!("http://{address}"));
    let pkg: KnownPackage = serde_json::from_value(serde_json::json!({
        "registry": "ghcr.io",
        "repository": "example/demo",
        "tags": [VERSION],
        "last_seen_at": "2026-09-01T00:00:00Z",
        "created_at": "2026-08-01T00:00:00Z",
        "wit_namespace": "example",
        "wit_name": "demo"
    }))
    .expect("deserialize selected-version package fixture");

    timeout(Duration::from_secs(10), async {
        let (result, ()) = tokio::join!(
            fetch(client, pkg),
            serve_version_response(listener, status, body)
        );
        result
    })
    .await
    .expect("selected-version registry fixture request should finish")
}

async fn serve_version_response(listener: TcpListener, status: &str, body: &str) {
    let (stream, _) = listener.accept().await.expect("accept version request");
    let mut stream = BufReader::new(stream);
    let mut request = String::new();
    stream
        .read_line(&mut request)
        .await
        .expect("read version request");
    assert_eq!(
        request.trim(),
        "GET /v1/packages/version/ghcr.io/1.0.0/example/demo HTTP/1.1"
    );
    loop {
        let mut line = String::new();
        stream
            .read_line(&mut line)
            .await
            .expect("read request header");
        if line == "\r\n" || line.is_empty() {
            break;
        }
    }
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream
        .get_mut()
        .write_all(response.as_bytes())
        .await
        .expect("write version response");
}

fn failed_responses() -> [(&'static str, &'static str); 4] {
    [
        ("503 Service Unavailable", r#"{"error":"unavailable"}"#),
        ("503 Service Unavailable", MINIMAL_DETAIL),
        ("200 OK", "not valid JSON"),
        ("200 OK", r#"{"digest":42}"#),
    ]
}

fn assert_bad_gateway(response: &Response) {
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    assert_eq!(
        response
            .headers()
            .get(header::CACHE_CONTROL)
            .expect("upstream failures must have cache-control"),
        "no-cache"
    );
}

#[tokio::test]
async fn version_fetch_failures_are_non_cacheable_bad_gateway() {
    for (status, body) in failed_responses() {
        let response = with_version_response(status, body, |client, pkg| async move {
            fetch_version(&client, &pkg, VERSION).await
        })
        .await
        .expect_err("HTTP and decoding failures must not be treated as missing detail");
        assert_bad_gateway(&response);
    }
}

#[tokio::test]
async fn wit_fetch_propagates_non_cacheable_bad_gateway() {
    for (status, body) in failed_responses() {
        let response = with_version_response(status, body, |client, pkg| async move {
            fetch_wit_doc(&client, &pkg, VERSION).await
        })
        .await
        .expect_err("WIT lookup must propagate upstream failures instead of returning absent");
        assert_bad_gateway(&response);
    }
}

#[tokio::test]
async fn missing_version_detail_remains_absent() {
    let detail = with_version_response("404 Not Found", "", |client, pkg| async move {
        fetch_version(&client, &pkg, VERSION).await
    })
    .await
    .expect("missing selected-version metadata is not an API failure");
    assert!(detail.is_none());
}

#[tokio::test]
async fn version_detail_does_not_require_timestamps_or_annotations() {
    let detail = with_version_response("200 OK", MINIMAL_DETAIL, |client, pkg| async move {
        fetch_version(&client, &pkg, VERSION).await
    })
    .await
    .expect("optional metadata must not be required")
    .expect("minimal selected-version metadata should be present");
    assert_eq!(detail.digest, "sha256:abc");
    assert!(detail.created_at.is_none());
    assert!(detail.synced_at.is_none());
    assert!(detail.annotations.is_none());
}

#[tokio::test]
async fn missing_or_unparseable_wit_remains_absent() {
    for (status, body) in [
        ("404 Not Found", ""),
        ("200 OK", MINIMAL_DETAIL),
        (
            "200 OK",
            r#"{"digest":"sha256:abc","wit_text":"not valid WIT"}"#,
        ),
    ] {
        let detail = with_version_response(status, body, |client, pkg| async move {
            fetch_wit_doc(&client, &pkg, VERSION).await
        })
        .await
        .expect("missing or unparseable WIT preserves not-found behavior");
        assert!(detail.is_none());
    }
}

#[tokio::test]
async fn valid_wit_does_not_require_timestamps_or_annotations() {
    let body = serde_json::json!({
        "digest": "sha256:abc",
        "wit_text": "package example:demo; interface api { ping: func(); }"
    })
    .to_string();
    let (doc, detail) = with_version_response("200 OK", &body, |client, pkg| async move {
        fetch_wit_doc(&client, &pkg, VERSION).await
    })
    .await
    .expect("valid WIT metadata should be accepted")
    .expect("valid WIT document should be present");
    assert_eq!(doc.interfaces.len(), 1);
    assert_eq!(doc.interfaces[0].name, "api");
    assert!(detail.created_at.is_none());
    assert!(detail.annotations.is_none());
}
