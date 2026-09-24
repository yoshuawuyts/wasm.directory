//! Embedded WebAssembly favicons, served without a registry API dependency.

use axum::http::header;
use axum::response::IntoResponse;

const SVG: &[u8] = include_bytes!("../assets/favicon.svg");
const ICO: &[u8] = include_bytes!("../assets/favicon.ico");

/// Serve the scalable favicon.
pub(crate) async fn svg() -> impl IntoResponse {
    icon_response("image/svg+xml", SVG)
}

/// Serve the multi-resolution fallback favicon.
pub(crate) async fn ico() -> impl IntoResponse {
    icon_response("image/vnd.microsoft.icon", ICO)
}

fn icon_response(content_type: &'static str, bytes: &'static [u8]) -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, "public, max-age=86400"),
        ],
        bytes,
    )
}

#[cfg(test)]
mod tests {
    use axum::body::{Body, to_bytes};
    use axum::http::{Method, Request, StatusCode};
    use tower::ServiceExt;

    use super::*;

    #[tokio::test]
    async fn favicon_routes_serve_embedded_images() {
        for (path, content_type, expected_body) in [
            ("/favicon.svg", "image/svg+xml", SVG),
            ("/favicon.ico", "image/vnd.microsoft.icon", ICO),
        ] {
            for method in [Method::GET, Method::HEAD] {
                let request = Request::builder()
                    .method(method.clone())
                    .uri(path)
                    .body(Body::empty())
                    .expect("favicon request should be valid");
                let response = crate::app()
                    .oneshot(request)
                    .await
                    .expect("favicon route should respond");

                assert_eq!(response.status(), StatusCode::OK, "{method} {path}");
                assert_eq!(response.headers()[header::CONTENT_TYPE], content_type);
                assert_eq!(
                    response.headers()[header::CACHE_CONTROL],
                    "public, max-age=86400"
                );
                let body = to_bytes(response.into_body(), expected_body.len())
                    .await
                    .expect("favicon body should be readable");
                let expected = match method {
                    Method::HEAD => &[][..],
                    _ => expected_body,
                };
                assert_eq!(body.as_ref(), expected, "{method} {path}");
            }
        }
    }
}
