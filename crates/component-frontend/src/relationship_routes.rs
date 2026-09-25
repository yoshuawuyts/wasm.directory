//! HTTP handlers for the three fixed relationship queries.

use axum::extract::{Query, rejection::QueryRejection};
use axum::http::{StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use serde::Deserialize;
use wasm_meta_registry_client::{RegistryClient, RelationshipTarget};

use crate::pages::relationships;
use crate::relationships::Relationship;

#[derive(Deserialize)]
pub(crate) struct RelationshipParams {
    package: String,
    interface: Option<String>,
    #[serde(default)]
    offset: u32,
    #[serde(default = "default_limit")]
    limit: u32,
}

fn default_limit() -> u32 {
    100
}

pub(crate) async fn dependents(
    query: Result<Query<RelationshipParams>, QueryRejection>,
) -> Response {
    handle(&RegistryClient::from_env(), Relationship::Dependents, query).await
}

pub(crate) async fn imported_by(
    query: Result<Query<RelationshipParams>, QueryRejection>,
) -> Response {
    handle(&RegistryClient::from_env(), Relationship::ImportedBy, query).await
}

pub(crate) async fn exported_by(
    query: Result<Query<RelationshipParams>, QueryRejection>,
) -> Response {
    handle(&RegistryClient::from_env(), Relationship::ExportedBy, query).await
}

async fn handle(
    client: &RegistryClient,
    relation: Relationship,
    query: Result<Query<RelationshipParams>, QueryRejection>,
) -> Response {
    let params = match query {
        Ok(Query(params)) => params,
        Err(error) => return invalid_query(&error.body_text()),
    };
    if relation == Relationship::Dependents && params.interface.is_some() {
        return invalid_query("Dependents requires a package, not an individual interface.");
    }
    let target = match RelationshipTarget::new(&params.package, params.interface.as_deref()) {
        Ok(target) => target,
        Err(error) => return invalid_query(&error.to_string()),
    };
    let limit = params.limit.clamp(1, 100);
    match relationships::render(client, relation, &target, params.offset, limit).await {
        Ok(html) => crate::with_cache_control(html, "public, max-age=60"),
        Err(error) => {
            eprintln!(
                "component-frontend: {} for {} failed: {error}",
                relation.slug(),
                target.package()
            );
            let html = relationships::render_error(
                relation,
                &target,
                "The registry could not complete this lookup.",
                params.offset,
                limit,
            );
            (
                StatusCode::BAD_GATEWAY,
                [(header::CACHE_CONTROL, "no-cache")],
                Html(html),
            )
                .into_response()
        }
    }
}

fn invalid_query(message: &str) -> Response {
    eprintln!("component-frontend: invalid relationship query: {message}");
    let body = html::text_content::Division::builder()
        .class("pt-8")
        .heading_1(|h| {
            h.class(crate::components::ds::typography::H1_CLASS)
                .text("Invalid relationship query")
        })
        .paragraph(|p| {
            p.class("mt-4 text-ink-700")
                .text(crate::escape::escape_html_text(message))
        })
        .paragraph(|p| {
            p.class("mt-4").anchor(|a| {
                a.href("/all")
                    .class("text-accent hover:underline")
                    .text("Browse all packages")
            })
        })
        .build();
    (
        StatusCode::BAD_REQUEST,
        [(header::CACHE_CONTROL, "no-cache")],
        Html(crate::layout::document_with_nav(
            "Invalid relationship query",
            &body.to_string(),
        )),
    )
        .into_response()
}

#[cfg(test)]
pub(crate) mod tests;
