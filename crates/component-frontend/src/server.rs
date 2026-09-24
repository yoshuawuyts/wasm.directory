//! Adapt the frontend router's responses to the WASI HTTP transport.

use axum::http::{Method, header};
use wstd::http::{Body, Request, Response, Result};

/// Handle a request while keeping HEAD responses compatible with WASI HTTP.
pub(crate) async fn serve(request: Request<Body>) -> Result<Response<Body>> {
    let is_head = request.method() == Method::HEAD;
    let mut response = wstd_axum::serve(request, crate::app()).await?;
    if is_head {
        // WASI checks Content-Length against transmitted bytes, even for HEAD.
        // Omit this optional header after Axum has stripped the response body.
        response.headers_mut().remove(header::CONTENT_LENGTH);
    }
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

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
