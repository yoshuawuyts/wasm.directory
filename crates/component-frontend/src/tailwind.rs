//! Embedded, pinned Tailwind browser runtime, independent of the registry API.

use axum::http::header;
use axum::response::IntoResponse;

/// Fingerprinted URL shared by the router and document head.
pub(crate) const PATH: &str = "/assets/tailwind-3.4.17-176e894661aa.js";

/// License and third-party attributions shipped with the runtime.
pub(crate) const LICENSE_PATH: &str = "/assets/tailwind-3.4.17-LICENSE.txt";

const SCRIPT: &[u8] = include_bytes!("../assets/tailwind-3.4.17-176e894661aa.js");
const LICENSE: &[u8] = include_bytes!("../assets/tailwind-3.4.17-LICENSE.txt");

/// Serve the unmodified Tailwind runtime with immutable caching.
pub(crate) async fn script() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "text/javascript; charset=utf-8"),
            (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
        ],
        [(header::LINK, format!("<{LICENSE_PATH}>; rel=\"license\""))],
        SCRIPT,
    )
}

/// Serve the accompanying license notices without a static-file directory.
pub(crate) async fn license() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "text/plain; charset=utf-8"),
            (header::CACHE_CONTROL, "public, max-age=86400"),
        ],
        LICENSE,
    )
}

#[cfg(test)]
mod tests {
    use axum::body::{Body, to_bytes};
    use axum::http::{Method, Request, StatusCode};
    use axum::response::Response;
    use tower::ServiceExt;

    use super::*;

    #[tokio::test]
    async fn tailwind_routes_serve_the_embedded_runtime_and_licenses() {
        for (path, content_type, cache_control, expected_body) in [
            (
                PATH,
                "text/javascript; charset=utf-8",
                "public, max-age=31536000, immutable",
                SCRIPT,
            ),
            (
                LICENSE_PATH,
                "text/plain; charset=utf-8",
                "public, max-age=86400",
                LICENSE,
            ),
        ] {
            for method in [Method::GET, Method::HEAD] {
                let response = request(method.clone(), path).await;

                assert_eq!(response.status(), StatusCode::OK, "{method} {path}");
                assert_eq!(response.headers()[header::CONTENT_TYPE], content_type);
                assert_eq!(response.headers()[header::CACHE_CONTROL], cache_control);
                assert_eq!(
                    response.headers()[header::CONTENT_LENGTH],
                    expected_body.len().to_string()
                );
                if path == PATH {
                    assert_eq!(
                        response.headers()[header::LINK],
                        format!("<{LICENSE_PATH}>; rel=\"license\"")
                    );
                }
                let body = to_bytes(response.into_body(), expected_body.len())
                    .await
                    .expect("Tailwind response body should be readable");
                let expected = match method {
                    Method::HEAD => &[][..],
                    _ => expected_body,
                };
                assert_eq!(body.as_ref(), expected, "{method} {path}");
            }
        }
    }

    #[tokio::test]
    async fn tailwind_route_rejects_unknown_assets_and_unsupported_methods() {
        for (method, path, status) in [
            (Method::POST, PATH, StatusCode::METHOD_NOT_ALLOWED),
            (Method::POST, LICENSE_PATH, StatusCode::METHOD_NOT_ALLOWED),
            (
                Method::GET,
                "/assets/tailwind-unknown.js",
                StatusCode::NOT_FOUND,
            ),
            (
                Method::HEAD,
                "/assets/tailwind-unknown.js",
                StatusCode::NOT_FOUND,
            ),
        ] {
            let response = request(method.clone(), path).await;
            assert_eq!(response.status(), status, "{method} {path}");
            assert!(!response.headers().contains_key(header::LOCATION));
        }
    }

    async fn request(method: Method, path: &str) -> Response {
        let request = Request::builder()
            .method(method)
            .uri(path)
            .body(Body::empty())
            .expect("Tailwind request should be valid");
        crate::app()
            .oneshot(request)
            .await
            .expect("Tailwind route should respond")
    }
}
