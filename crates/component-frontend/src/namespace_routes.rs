//! Namespace directory and publisher listing routes.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use wasm_meta_registry_client::{ApiError, RegistryClient};

use crate::{AllPackagesParams, pages, reserved};

pub(crate) async fn all(
    State(client): State<Arc<RegistryClient>>,
    Query(params): Query<AllPackagesParams>,
) -> Response {
    respond(
        "All Namespaces",
        "Unable to load namespaces",
        pages::namespaces::render(&client, params.offset, params.limit.clamp(1, 100)).await,
    )
}

pub(crate) async fn packages(
    State(client): State<Arc<RegistryClient>>,
    Path(namespace): Path<String>,
    Query(params): Query<AllPackagesParams>,
) -> Response {
    if reserved::is_reserved(&namespace) {
        return crate::not_found_response();
    }
    directory_packages(State(client), Path(namespace), Query(params)).await
}

/// Non-colliding namespace listing, reachable even for reserved namespace names.
pub(crate) async fn directory_packages(
    State(client): State<Arc<RegistryClient>>,
    Path(namespace): Path<String>,
    Query(params): Query<AllPackagesParams>,
) -> Response {
    respond(
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

fn respond(title: &str, error_message: &str, result: Result<String, ApiError>) -> Response {
    match result {
        Ok(html) => crate::with_cache_control(html, "public, max-age=60"),
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
