//! Embedded CLI installers, served without a registry API dependency.

use axum::http::header;
use axum::response::IntoResponse;

const SHELL_SCRIPT: &[u8] = include_bytes!("../../../scripts/install.sh");
const WINDOWS_SCRIPT: &[u8] = include_bytes!("../../../scripts/install.ps1");

/// Serve the shared Linux and macOS shell installer.
pub(crate) async fn shell() -> impl IntoResponse {
    script_response(SHELL_SCRIPT)
}

/// Serve the Windows PowerShell installer.
pub(crate) async fn windows() -> impl IntoResponse {
    script_response(WINDOWS_SCRIPT)
}

fn script_response(bytes: &'static [u8]) -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "text/plain; charset=utf-8"),
            (header::CONTENT_DISPOSITION, "inline"),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
            (header::CACHE_CONTROL, "public, max-age=3600"),
        ],
        bytes,
    )
}

#[cfg(test)]
mod tests {
    use axum::http::{Method, Request, StatusCode};
    use wstd::http::{Body, Response};

    use super::*;

    const ROUTES: [(&str, &[u8]); 3] = [
        ("/install/linux", SHELL_SCRIPT),
        ("/install/macos", SHELL_SCRIPT),
        ("/install/windows", WINDOWS_SCRIPT),
    ];

    async fn response_for(method: Method, path: &str, user_agent: Option<&str>) -> Response<Body> {
        let mut request = Request::builder().method(method).uri(path);
        if let Some(user_agent) = user_agent {
            request = request.header(header::USER_AGENT, user_agent);
        }
        let request = request
            .body(Body::empty())
            .expect("installer request should be valid");
        crate::server::serve(request)
            .await
            .expect("installer route should respond without a registry backend")
    }

    #[tokio::test]
    async fn installer_routes_serve_exact_scripts_for_get_and_head() {
        for (path, expected_body) in ROUTES {
            for method in [Method::GET, Method::HEAD] {
                let mut response = response_for(method.clone(), path, None).await;

                assert_eq!(response.status(), StatusCode::OK, "{method} {path}");
                assert_eq!(
                    response.headers()[header::CONTENT_TYPE],
                    "text/plain; charset=utf-8"
                );
                assert_eq!(response.headers()[header::CONTENT_DISPOSITION], "inline");
                assert_eq!(
                    response.headers()[header::X_CONTENT_TYPE_OPTIONS],
                    "nosniff"
                );
                assert_eq!(
                    response.headers()[header::CACHE_CONTROL],
                    "public, max-age=3600"
                );
                match method {
                    Method::HEAD => {
                        assert!(!response.headers().contains_key(header::CONTENT_LENGTH));
                    }
                    _ => assert_eq!(
                        response.headers()[header::CONTENT_LENGTH],
                        expected_body.len().to_string()
                    ),
                }
                assert!(!response.headers().contains_key(header::LOCATION));

                let body = response
                    .body_mut()
                    .contents()
                    .await
                    .expect("installer body should be readable");
                let expected = match method {
                    Method::HEAD => &[][..],
                    _ => expected_body,
                };
                assert_eq!(body, expected, "{method} {path}");
            }
        }
    }

    #[tokio::test]
    async fn installer_routes_do_not_select_scripts_by_user_agent() {
        for (path, expected_body) in ROUTES {
            for user_agent in [
                "curl/8.0.0",
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) WindowsPowerShell/5.1",
            ] {
                let mut response = response_for(Method::GET, path, Some(user_agent)).await;
                assert_eq!(response.status(), StatusCode::OK, "{path} {user_agent}");
                let body = response
                    .body_mut()
                    .contents()
                    .await
                    .expect("installer body should be readable");
                assert_eq!(body, expected_body, "{path} {user_agent}");
            }
        }
    }

    #[tokio::test]
    async fn unsupported_install_paths_return_not_found_without_a_registry_backend() {
        for path in [
            "/install",
            "/install/",
            "/install/freebsd",
            "/install/linux/",
            "/install/linux/1.0.0",
        ] {
            for method in [Method::GET, Method::HEAD] {
                let response = response_for(method.clone(), path, None).await;
                assert_eq!(response.status(), StatusCode::NOT_FOUND, "{method} {path}");
                assert_eq!(response.headers()[header::CACHE_CONTROL], "no-cache");
                assert!(!response.headers().contains_key(header::LOCATION));
            }
        }
    }
}
