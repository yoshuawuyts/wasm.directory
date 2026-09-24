use std::pin::Pin;
use std::time::Duration;

use bytes::Bytes;
use http_body_util::{BodyExt as _, Empty, Full};
use wasmtime_wasi_http::{Error, RequestOptions, WasiBody, WasiHttpHooks as _, io::TokioIo};

use super::NativeCertHooks;

fn request(uri: &str) -> http::Request<WasiBody> {
    http::Request::builder()
        .uri(uri)
        .body(Empty::<Bytes>::new().map_err(Error::from).boxed_unsync())
        .expect("build test request")
}

#[tokio::test]
async fn shared_hook_rejects_missing_authority() {
    let result = Pin::from(NativeCertHooks.send_request(
        request("/relative"),
        None,
        Box::new(async { Ok(()) }),
    ))
    .await;
    assert!(matches!(result, Err(Error::HttpRequestUriInvalid)));
}

#[tokio::test]
async fn shared_hook_sends_request_and_streams_response() {
    tokio::time::timeout(Duration::from_secs(5), round_trip())
        .await
        .expect("HTTP round trip should not hang");
}

async fn round_trip() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind test HTTP listener");
    let addr = listener.local_addr().expect("read listener address");
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept test connection");
        hyper::server::conn::http1::Builder::new()
            .serve_connection(
                TokioIo::new(stream),
                hyper::service::service_fn(
                    |req: http::Request<hyper::body::Incoming>| async move {
                        assert_eq!(req.uri().to_string(), "/hello?test=yes");
                        Ok::<_, std::convert::Infallible>(
                            http::Response::builder()
                                .header(http::header::CONNECTION, "close")
                                .body(Full::new(Bytes::from_static(b"native hooks")))
                                .expect("build test response"),
                        )
                    },
                ),
            )
            .await
            .expect("serve test response");
    });

    let (response, io) = Pin::from(NativeCertHooks.send_request(
        request(&format!("http://{addr}/hello?test=yes")),
        None,
        Box::new(async { Ok(()) }),
    ))
    .await
    .expect("send through shared native-cert hook");
    assert_eq!(response.status(), http::StatusCode::OK);
    let (body, io_result) = tokio::join!(response.into_body().collect(), Pin::from(io));
    assert_eq!(
        body.expect("read response body").to_bytes(),
        b"native hooks"[..]
    );
    io_result.expect("drive connection to completion");
    server.await.expect("join test server");
}

#[tokio::test]
async fn shared_hook_enforces_first_byte_timeout() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind test HTTP listener");
    let addr = listener.local_addr().expect("read listener address");
    let server = tokio::spawn(async move {
        let (_stream, _) = listener.accept().await.expect("accept test connection");
        std::future::pending::<()>().await;
    });
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        Pin::from(NativeCertHooks.send_request(
            request(&format!("http://{addr}/")),
            Some(RequestOptions {
                first_byte_timeout: Some(Duration::from_millis(10)),
                ..RequestOptions::default()
            }),
            Box::new(async { Ok(()) }),
        )),
    )
    .await;
    server.abort();
    assert!(matches!(result, Ok(Err(Error::ConnectionReadTimeout))));
}
