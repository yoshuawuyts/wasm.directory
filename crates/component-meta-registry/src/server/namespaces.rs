//! Paginated namespace discovery and exact namespace package listings.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::response::IntoResponse;
use axum::{Extension, Json};

use super::{AppError, AppState, ListParams, clamp_limit};

pub(super) async fn list(
    State(manager): State<AppState>,
    Extension(registered): Extension<Arc<Vec<String>>>,
    Query(params): Query<ListParams>,
) -> Result<impl IntoResponse, AppError> {
    let manager = manager.read().await;
    Ok(Json(
        manager
            .list_namespaces(&registered, params.offset, clamp_limit(params.limit))
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
