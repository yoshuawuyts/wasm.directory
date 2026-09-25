//! Adapt the frontend router's responses to the WASI HTTP transport.

use axum::http::{Method, StatusCode, header};
use wstd::http::{Body, Request, Response, Result};

/// Handle a request while keeping bodyless responses compatible with WASI HTTP.
pub(crate) async fn serve(request: Request<Body>) -> Result<Response<Body>> {
    serve_with_router(request, crate::app()).await
}

/// Apply the same transport adaptation to an explicitly supplied router.
pub(crate) async fn serve_with_router(
    request: Request<Body>,
    router: axum::Router,
) -> Result<Response<Body>> {
    let is_head = request.method() == Method::HEAD;
    let mut response = wstd_axum::serve(request, router).await?;
    if is_head
        || response.status().is_informational()
        || matches!(
            response.status(),
            StatusCode::NO_CONTENT | StatusCode::NOT_MODIFIED
        )
    {
        // WASI checks Content-Length against transmitted bytes, even for HEAD.
        // Axum can also infer zero for a 304; that is not the selected GET's
        // length. Omit it after routing, including statuses where it is forbidden.
        response.headers_mut().remove(header::CONTENT_LENGTH);
    }
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn encoded_head_responses_preserve_metadata_and_never_emit_compressor_bytes() {
        for encoding in ["gzip", "br"] {
            for path in ["/docs", "/robots.txt", crate::tailwind::PATH] {
                let get = Request::builder()
                    .uri(path)
                    .header(header::ACCEPT_ENCODING, encoding)
                    .body(Body::empty())
                    .expect("GET request");
                let get_response = serve(get).await.expect("GET response");
                assert_eq!(get_response.headers()[header::CONTENT_ENCODING], encoding);
                let head = Request::builder()
                    .method(Method::HEAD)
                    .uri(path)
                    .header(header::ACCEPT_ENCODING, encoding)
                    .body(Body::empty())
                    .expect("HEAD request");
                let mut head_response = serve(head).await.expect("HEAD response");
                assert_eq!(
                    head_response.headers(),
                    get_response.headers(),
                    "{encoding} {path}"
                );
                assert!(
                    head_response
                        .body_mut()
                        .contents()
                        .await
                        .expect("HEAD body")
                        .is_empty(),
                    "{encoding} {path}"
                );
            }
        }
    }

    #[tokio::test]
    async fn head_preserves_get_metadata_except_optional_content_length() {
        for path in [
            "/favicon.svg",
            "/favicon.ico",
            "/robots.txt",
            "/downloads",
            crate::tailwind::PATH,
            crate::tailwind::LICENSE_PATH,
        ] {
            let get = Request::builder()
                .uri(path)
                .body(Body::empty())
                .expect("GET request should be valid");
            let mut get_response = serve(get).await.expect("GET should succeed");
            let get_len = get_response
                .body_mut()
                .contents()
                .await
                .expect("GET body should be readable")
                .len();
            assert_eq!(
                get_response.headers()[header::CONTENT_LENGTH],
                get_len.to_string(),
                "{path}"
            );

            let head = Request::builder()
                .method(Method::HEAD)
                .uri(path)
                .body(Body::empty())
                .expect("HEAD request should be valid");
            let mut head_response = serve(head).await.expect("HEAD should succeed");
            assert_eq!(head_response.status(), get_response.status(), "{path}");
            let mut expected_headers = get_response.headers().clone();
            expected_headers.remove(header::CONTENT_LENGTH);
            assert_eq!(head_response.headers(), &expected_headers, "{path}");
            assert!(
                head_response
                    .body_mut()
                    .contents()
                    .await
                    .expect("HEAD body should be readable")
                    .is_empty(),
                "{path}"
            );
        }
    }
}
