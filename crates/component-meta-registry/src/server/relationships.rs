//! Fixed relationship queries, separate from the existing package search API.

#[cfg(test)]
mod tests;

use axum::extract::{Query, State, rejection::QueryRejection};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Json, Router, routing::get};
use serde::Deserialize;
use wasm_meta_registry_types::RelationshipTarget;

use super::{AppError, AppState, clamp_limit, default_limit};

#[derive(Deserialize)]
struct RelationshipParams {
    package: String,
    interface: Option<String>,
    #[serde(default)]
    offset: u32,
    #[serde(default = "default_limit")]
    limit: u32,
}

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/v1/relationships/dependents", get(dependents))
        .route("/v1/relationships/imported-by", get(imported_by))
        .route("/v1/relationships/exported-by", get(exported_by))
}

fn validate(
    query: Result<Query<RelationshipParams>, QueryRejection>,
    allow_interface: bool,
) -> Result<(RelationshipTarget, u32, u32), String> {
    let Query(params) = query.map_err(|error| error.body_text())?;
    if !allow_interface && params.interface.is_some() {
        return Err("The dependents query does not accept an interface parameter.".to_owned());
    }
    let target = RelationshipTarget::new(&params.package, params.interface.as_deref())
        .map_err(|error| error.to_string())?;
    Ok((target, params.offset, clamp_limit(params.limit)))
}

fn bad_request(error: impl Into<String>) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({ "error": error.into() })),
    )
        .into_response()
}

async fn dependents(
    State(manager): State<AppState>,
    query: Result<Query<RelationshipParams>, QueryRejection>,
) -> Result<Response, AppError> {
    let (target, offset, limit) = match validate(query, false) {
        Ok(params) => params,
        Err(error) => return Ok(bad_request(error)),
    };
    let page = manager
        .read()
        .await
        .list_dependents(target.package(), offset, limit)
        .await?;
    Ok(Json(page).into_response())
}

async fn imported_by(
    State(manager): State<AppState>,
    query: Result<Query<RelationshipParams>, QueryRejection>,
) -> Result<Response, AppError> {
    let (target, offset, limit) = match validate(query, true) {
        Ok(params) => params,
        Err(error) => return Ok(bad_request(error)),
    };
    let page = manager
        .read()
        .await
        .list_importing_worlds(target.package(), target.interface(), offset, limit)
        .await?;
    Ok(Json(page).into_response())
}

async fn exported_by(
    State(manager): State<AppState>,
    query: Result<Query<RelationshipParams>, QueryRejection>,
) -> Result<Response, AppError> {
    let (target, offset, limit) = match validate(query, true) {
        Ok(params) => params,
        Err(error) => return Ok(bad_request(error)),
    };
    let page = manager
        .read()
        .await
        .list_exporting_worlds(target.package(), target.interface(), offset, limit)
        .await?;
    Ok(Json(page).into_response())
}
