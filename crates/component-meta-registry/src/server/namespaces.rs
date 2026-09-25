//! Paginated namespace discovery and exact namespace package listings.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::response::IntoResponse;

use super::{AppError, AppState, ListParams, clamp_limit};

pub(super) async fn list(
    State(manager): State<AppState>,
    Query(params): Query<ListParams>,
) -> Result<impl IntoResponse, AppError> {
    let manager = manager.read().await;
    Ok(Json(
        manager
            .list_namespaces(params.offset, clamp_limit(params.limit))
            .await?,
    ))
}

pub(super) async fn packages(
    State(manager): State<AppState>,
    Path(namespace): Path<String>,
    Query(params): Query<ListParams>,
) -> Result<impl IntoResponse, AppError> {
    let manager = manager.read().await;
    Ok(Json(
        manager
            .list_namespace_packages(&namespace, params.offset, clamp_limit(params.limit))
            .await?,
    ))
}

#[cfg(test)]
mod tests;
