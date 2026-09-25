//! Namespace directory and publisher listing routes.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use wasm_meta_registry_client::{ApiError, RegistryClient};

use crate::{AllPackagesParams, pages, reserved};

pub(crate) async fn all(
    State(client): State<Arc<RegistryClient>>,
    headers: HeaderMap,
    Query(params): Query<AllPackagesParams>,
) -> Response {
    respond(
        &headers,
        "All Namespaces",
        "Unable to load namespaces",
        pages::namespaces::render(&client, params.offset, params.limit.clamp(1, 100)).await,
    )
}

pub(crate) async fn packages(
    State(client): State<Arc<RegistryClient>>,
    headers: HeaderMap,
    Path(namespace): Path<String>,
    Query(params): Query<AllPackagesParams>,
) -> Response {
    if reserved::is_reserved(&namespace) {
        return crate::not_found_response();
    }
    respond(
        &headers,
        &namespace,
        "Unable to load packages",
        pages::namespace::render(
            &client,
            &namespace,
            params.offset,
            params.limit.clamp(1, 100),
        )
        .await,
    )
}

fn respond(
    headers: &HeaderMap,
    title: &str,
    error_message: &str,
    result: Result<String, ApiError>,
) -> Response {
    match result {
        Ok(html) => crate::with_cache_control(headers, html, "public, max-age=60"),
        Err(error) => {
            eprintln!("component-frontend: {title} lookup failed: {error}");
            (
                StatusCode::BAD_GATEWAY,
                [(header::CACHE_CONTROL, "no-cache")],
                Html(pages::namespaces::render_error(title, error_message)),
            )
                .into_response()
        }
    }
}

#[cfg(all(test, not(all(target_os = "wasi", target_env = "p2"))))]
mod tests;
