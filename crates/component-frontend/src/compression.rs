//! Streaming content negotiation and conditional responses for the frontend.

use axum::Router;
use axum::body::Body;
use axum::extract::Request;
use axum::http::{Extensions, HeaderMap, Method, StatusCode, Version, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use tower_http::compression::{CompressionLayer, CompressionLevel};

/// Negotiate before evaluating validators, and before Axum strips HEAD bodies.
pub(crate) fn layer(router: Router) -> Router {
    router
        .layer(
            CompressionLayer::new()
                .quality(CompressionLevel::Precise(4))
                .compress_when(is_text_response),
        )
        .layer(middleware::from_fn(conditional_response))
}

fn is_text_response(status: StatusCode, _: Version, headers: &HeaderMap, _: &Extensions) -> bool {
    if status.is_informational()
        || matches!(
            status,
            StatusCode::NO_CONTENT | StatusCode::RESET_CONTENT | StatusCode::NOT_MODIFIED
        )
    {
        return false;
    }
    let Some(content_type) = headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
    else {
        return false;
    };
    let content_type = content_type.trim();
    (content_type.starts_with("text/") && content_type != "text/event-stream")
        || matches!(
            content_type,
            "application/json" | "application/javascript" | "image/svg+xml"
        )
}

async fn conditional_response(request: Request, next: Next) -> Response {
    let can_revalidate = matches!(*request.method(), Method::GET | Method::HEAD);
    let headers = request.headers().clone();
    let mut response = next.run(request).await;

    // Negotiation failures must not inherit the page's public cache policy,
    // validators, or body from tower-http's rejected representation.
    if response.status() == StatusCode::NOT_ACCEPTABLE {
        return (
            StatusCode::NOT_ACCEPTABLE,
            [
                (header::CACHE_CONTROL, "no-store"),
                (header::VARY, "accept-encoding"),
            ],
        )
            .into_response();
    }

    let matches = response
        .headers()
        .get(header::ETAG)
        .and_then(|etag| etag.to_str().ok())
        .is_some_and(|etag| crate::if_none_match_matches(&headers, etag));
    if can_revalidate && response.status() == StatusCode::OK && matches {
        *response.status_mut() = StatusCode::NOT_MODIFIED;
        *response.body_mut() = Body::empty();
        response.headers_mut().remove(header::CONTENT_LENGTH);
    }
    response
}

#[cfg(test)]
mod tests;
