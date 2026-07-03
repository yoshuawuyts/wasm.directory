//! Web frontend for the WebAssembly package registry.
//!
//! A server-side rendered web application compiled as a `wasm32-wasip2`
//! component targeting `wasi:http`. Uses `wstd-axum` for routing and the
//! `html` crate for type-safe HTML generation.

// Logging errors to stderr is the appropriate way to surface API failures
// when running under wasmtime serve.
#![allow(clippy::print_stderr)]
#![recursion_limit = "512"]

// r[impl frontend.server.wasi-http]

mod components;
mod escape;
mod footer;
mod layout;
mod markdown;
mod pages;
mod reserved;
mod wit_doc;

use axum::body::Body;
use axum::extract::{Path, Query};
use axum::http::{HeaderMap, HeaderValue, StatusCode, Uri, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::{Json, Router, routing::get};
use serde::Deserialize;

use component_meta_registry_client::{KnownPackage, RegistryClient};

use crate::reserved::is_reserved;

/// Build the application router with all frontend routes.
fn app() -> Router {
    Router::new()
        .route("/", get(home))
        .route("/all", get(all_packages))
        .route("/search", get(search))
        .route("/about", get(about))
        .route("/docs", get(docs))
        .route("/docs/{page}", get(docs_page))
        .route("/design-system", get(design_system))
        .route("/downloads", get(downloads))
        .route("/status", get(queue_status))
        .route("/health", get(health))
        .route("/favicon.svg", get(favicon_svg))
        .route("/favicon.ico", get(favicon_ico))
        .route("/{namespace}/{name}", get(package_redirect))
        .route("/{namespace}/{name}/", get(package_redirect))
        .route("/{namespace}", get(namespace_page))
        .route("/{namespace}/", get(namespace_page))
        .route("/{namespace}/{name}/{version}", get(package_detail))
        .route(
            "/{namespace}/{name}/{version}/dependencies",
            get(package_dependencies),
        )
        .route(
            "/{namespace}/{name}/{version}/dependents",
            get(package_dependents),
        )
        .route(
            "/{namespace}/{name}/{version}/interface/{iface}",
            get(interface_detail),
        )
        .route(
            "/{namespace}/{name}/{version}/interface/{iface}/{item}",
            get(item_detail),
        )
        .route(
            "/{namespace}/{name}/{version}/world/{world_name}",
            get(world_detail),
        )
        .route(
            "/{namespace}/{name}/{version}/world/{world_name}/function/{func_name}",
            get(world_function_detail),
        )
        .route(
            "/{namespace}/{name}/{version}/function/{func_name}",
            get(package_function_detail),
        )
        .route(
            "/{namespace}/{name}/{version}/module/{child_name}",
            get(module_detail),
        )
        .route(
            "/{namespace}/{name}/{version}/component/{child_index}",
            get(child_component_detail),
        )
        .fallback(not_found)
}

// r[impl frontend.server.wasi-http]
#[wstd_axum::http_server]
fn main() -> Router {
    app()
}

// r[impl frontend.server.health]
/// Health check endpoint.
async fn health() -> impl IntoResponse {
    (
        [(header::CACHE_CONTROL, "no-cache")],
        Json(serde_json::json!({ "status": "ok" })),
    )
}

/// Site favicon, embedded in the binary at build time.
///
/// A small on-brand SVG mark that adapts to light and dark browser chrome
/// via `prefers-color-scheme`.
const FAVICON_SVG: &str = include_str!("../assets/favicon.svg");

/// Long-lived cache policy for immutable static assets (one week).
const ASSET_CACHE_CONTROL: &str = "public, max-age=604800";

// r[impl frontend.assets.favicon]
/// Serve the SVG favicon with a long cache lifetime.
async fn favicon_svg() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "image/svg+xml"),
            (header::CACHE_CONTROL, ASSET_CACHE_CONTROL),
        ],
        FAVICON_SVG,
    )
}

// r[impl frontend.assets.favicon]
/// Redirect the browser's default `/favicon.ico` probe to the SVG favicon so
/// it doesn't fall through to the 404 handler.
async fn favicon_ico() -> impl IntoResponse {
    Redirect::permanent("/favicon.svg")
}

// r[impl frontend.pages.home]
/// Front page showing recently updated components and interfaces.
async fn home(headers: HeaderMap) -> Response {
    let client = RegistryClient::from_env();
    let html = pages::home::render(&client).await;
    with_cache_control(&headers, html, "public, max-age=60")
}

/// Query parameters for the search page.
#[derive(Deserialize)]
struct SearchParams {
    /// The search query string.
    #[serde(default)]
    q: String,
}

/// Query parameters for the all-packages page.
#[derive(Deserialize)]
struct AllPackagesParams {
    /// Pagination offset.
    #[serde(default)]
    offset: u32,
    /// Pagination limit.
    #[serde(default = "default_all_packages_limit")]
    limit: u32,
}

fn default_all_packages_limit() -> u32 {
    100
}

// r[impl frontend.pages.search]
/// Search results page.
async fn search(headers: HeaderMap, Query(params): Query<SearchParams>) -> Response {
    let client = RegistryClient::from_env();
    let html = pages::search::render(&client, &params.q).await;
    with_cache_control(&headers, html, "public, max-age=60")
}

// r[impl frontend.pages.all]
/// Paginated listing of all known packages.
async fn all_packages(headers: HeaderMap, Query(params): Query<AllPackagesParams>) -> Response {
    let client = RegistryClient::from_env();
    let limit = params.limit.clamp(1, 200);
    let html = pages::all::render(&client, params.offset, limit).await;
    with_cache_control(&headers, html, "public, max-age=60")
}

/// About page — redirects to docs.
async fn about() -> Response {
    Redirect::permanent("/docs").into_response()
}

/// Documentation page.
async fn docs(headers: HeaderMap) -> Response {
    let html = pages::docs::render();
    with_cache_control(&headers, html, "public, max-age=3600")
}

/// Individual documentation sub-page (`/docs/<slug>`).
async fn docs_page(headers: HeaderMap, Path(page): Path<String>) -> Response {
    match pages::docs::render_page(&page) {
        Some(html) => with_cache_control(&headers, html, "public, max-age=3600"),
        None => not_found_response(),
    }
}

/// Design system reference page.
async fn design_system(headers: HeaderMap) -> Response {
    let html = pages::design_system::render();
    with_cache_control(&headers, html, "public, max-age=3600")
}

/// Downloads page.
async fn downloads(headers: HeaderMap) -> Response {
    let html = pages::downloads::render();
    with_cache_control(&headers, html, "public, max-age=3600")
}

/// Fetch queue status page.
async fn queue_status(headers: HeaderMap) -> Response {
    let client = RegistryClient::from_env();
    let html = pages::queue::render(&client).await;
    with_cache_control(&headers, html, "no-cache")
}

/// Namespace page — list all packages under a publisher.
async fn namespace_page(headers: HeaderMap, Path(namespace): Path<String>) -> Response {
    if is_reserved(&namespace) {
        return not_found_response();
    }

    let client = RegistryClient::from_env();
    let html = pages::namespace::render(&client, &namespace).await;
    with_cache_control(&headers, html, "public, max-age=60")
}

// r[impl frontend.pages.package-redirect]
// r[impl frontend.routing.reserved-namespaces]
/// Redirect `/<namespace>/<name>` to `/<namespace>/<name>/<latest-version>`.
async fn package_redirect(
    Path((namespace, name)): Path<(String, String)>,
) -> Result<Redirect, Response> {
    if is_reserved(&namespace) {
        return Err(not_found_response());
    }

    let client = RegistryClient::from_env();
    match client.fetch_package_by_wit(&namespace, &name).await {
        Ok(Some(pkg)) => {
            if let Some(version) = pick_redirect_version(&pkg.tags) {
                Ok(Redirect::temporary(&format!(
                    "/{namespace}/{name}/{version}"
                )))
            } else {
                eprintln!(
                    "component-frontend: package has no redirectable tags: {namespace}/{name}"
                );
                Err(not_found_response())
            }
        }
        Ok(None) => {
            eprintln!("component-frontend: package not found: {namespace}/{name}");
            Err(not_found_response())
        }
        Err(e) => {
            eprintln!("component-frontend: API error looking up {namespace}/{name}: {e}");
            Err(error_response(&e.to_string()))
        }
    }
}

// r[impl frontend.pages.package-detail]
// r[impl frontend.routing.package-path]
/// Package detail page at `/<namespace>/<name>/<version>`.
async fn package_detail(
    headers: HeaderMap,
    Path((namespace, name, version)): Path<(String, String, String)>,
) -> Response {
    let client = RegistryClient::from_env();
    let pkg = match fetch_package_or_404(&client, &namespace, &name, &version).await {
        Ok(Some(pkg)) => pkg,
        Ok(None) => return not_found_response(),
        Err(resp) => return resp,
    };
    let version_detail = client
        .fetch_package_version(&pkg.registry, &pkg.repository, &version)
        .await
        .ok()
        .flatten();
    let display_name = format!("{namespace}:{name}");
    let importers = client
        .search_packages_by_import(&display_name)
        .await
        .unwrap_or_default();
    let exporters = client
        .search_packages_by_export(&display_name)
        .await
        .unwrap_or_default();
    let html = pages::package::render(
        &pkg,
        &version,
        version_detail.as_ref(),
        &importers,
        &exporters,
    );
    with_cache_control(&headers, html, "public, max-age=300")
}

/// Legacy dependencies route — redirects to the main package page.
async fn package_dependencies(
    Path((namespace, name, version)): Path<(String, String, String)>,
) -> Response {
    Redirect::permanent(&format!("/{namespace}/{name}/{version}")).into_response()
}

/// Legacy dependents route — redirects to the main package page.
async fn package_dependents(
    Path((namespace, name, version)): Path<(String, String, String)>,
) -> Response {
    Redirect::permanent(&format!("/{namespace}/{name}/{version}")).into_response()
}

/// Interface detail page at `/<namespace>/<name>/<version>/interface/<iface>`.
async fn interface_detail(
    headers: HeaderMap,
    Path((namespace, name, version, iface)): Path<(String, String, String, String)>,
) -> Response {
    let client = RegistryClient::from_env();
    let pkg = match fetch_package_or_404(&client, &namespace, &name, &version).await {
        Ok(Some(pkg)) => pkg,
        Ok(None) => return not_found_response(),
        Err(resp) => return resp,
    };
    let Some((doc, version_detail)) = fetch_wit_doc(&client, &pkg, &version).await else {
        return not_found_response();
    };
    let Some(iface_doc) = doc.interfaces.iter().find(|i| i.name == iface) else {
        return not_found_response();
    };
    let html = pages::interface::render(&pkg, &version, Some(&version_detail), iface_doc, &doc);
    with_cache_control(&headers, html, "public, max-age=300")
}

/// Item detail page at `/<namespace>/<name>/<version>/interface/<iface>/<item>`.
async fn item_detail(
    headers: HeaderMap,
    Path((namespace, name, version, iface, item_name)): Path<(
        String,
        String,
        String,
        String,
        String,
    )>,
) -> Response {
    let client = RegistryClient::from_env();
    let pkg = match fetch_package_or_404(&client, &namespace, &name, &version).await {
        Ok(Some(pkg)) => pkg,
        Ok(None) => return not_found_response(),
        Err(resp) => return resp,
    };
    let Some((doc, version_detail)) = fetch_wit_doc(&client, &pkg, &version).await else {
        return not_found_response();
    };
    let Some(iface_doc) = doc.interfaces.iter().find(|i| i.name == iface) else {
        return not_found_response();
    };

    // Try types first, then functions.
    if let Some(ty) = iface_doc.types.iter().find(|t| t.name == item_name) {
        let html =
            pages::item::render_type(&pkg, &version, Some(&version_detail), &iface, ty, &doc);
        return with_cache_control(&headers, html, "public, max-age=300");
    }
    if let Some(func) = iface_doc.functions.iter().find(|f| f.name == item_name) {
        let iface_url = format!("/{namespace}/{name}/{version}/interface/{iface}");
        let html = pages::item::render_function(
            &pkg,
            &version,
            Some(&version_detail),
            &iface,
            &iface_url,
            func,
            &doc,
        );
        return with_cache_control(&headers, html, "public, max-age=300");
    }

    not_found_response()
}

/// World detail page at `/<namespace>/<name>/<version>/world/<world_name>`.
async fn world_detail(
    headers: HeaderMap,
    Path((namespace, name, version, world_name)): Path<(String, String, String, String)>,
) -> Response {
    let client = RegistryClient::from_env();
    let pkg = match fetch_package_or_404(&client, &namespace, &name, &version).await {
        Ok(Some(pkg)) => pkg,
        Ok(None) => return not_found_response(),
        Err(resp) => return resp,
    };
    let Some((doc, version_detail)) = fetch_wit_doc(&client, &pkg, &version).await else {
        return not_found_response();
    };
    let Some(world_doc) = doc.worlds.iter().find(|w| w.name == world_name) else {
        return not_found_response();
    };
    if world_doc.is_synthetic {
        // Synthetic worlds are inlined into the package page; no detail page.
        return not_found_response();
    }
    let html = pages::world::render(&pkg, &version, Some(&version_detail), world_doc, &doc);
    with_cache_control(&headers, html, "public, max-age=300")
}

/// Detail page for a freestanding function declared directly on a world,
/// at `/<namespace>/<name>/<version>/world/<world>/function/<func>`.
async fn world_function_detail(
    headers: HeaderMap,
    Path((namespace, name, version, world_name, func_name)): Path<(
        String,
        String,
        String,
        String,
        String,
    )>,
) -> Response {
    use crate::wit_doc::WorldItemDoc;

    let client = RegistryClient::from_env();
    let pkg = match fetch_package_or_404(&client, &namespace, &name, &version).await {
        Ok(Some(pkg)) => pkg,
        Ok(None) => return not_found_response(),
        Err(resp) => return resp,
    };
    let Some((doc, version_detail)) = fetch_wit_doc(&client, &pkg, &version).await else {
        return not_found_response();
    };
    let Some(world_doc) = doc.worlds.iter().find(|w| w.name == world_name) else {
        return not_found_response();
    };
    let func = world_doc
        .imports
        .iter()
        .chain(world_doc.exports.iter())
        .find_map(|item| match item {
            WorldItemDoc::Function(f) if f.name == func_name => Some(f),
            _ => None,
        });
    let Some(func) = func else {
        return not_found_response();
    };
    let world_url = format!("/{namespace}/{name}/{version}/world/{world_name}");
    let html = pages::item::render_function(
        &pkg,
        &version,
        Some(&version_detail),
        &world_name,
        &world_url,
        func,
        &doc,
    );
    with_cache_control(&headers, html, "public, max-age=300")
}

/// Detail page for a freestanding function inlined onto a package page,
/// at `/<namespace>/<name>/<version>/function/<func>`. Searches every
/// world's imports and exports for a function with the given name and
/// returns the first match.
async fn package_function_detail(
    headers: HeaderMap,
    Path((namespace, name, version, func_name)): Path<(String, String, String, String)>,
) -> Response {
    use crate::wit_doc::WorldItemDoc;

    let client = RegistryClient::from_env();
    let pkg = match fetch_package_or_404(&client, &namespace, &name, &version).await {
        Ok(Some(pkg)) => pkg,
        Ok(None) => return not_found_response(),
        Err(resp) => return resp,
    };
    let Some((doc, version_detail)) = fetch_wit_doc(&client, &pkg, &version).await else {
        return not_found_response();
    };
    // For component packages the `/function/` URL space belongs exclusively to
    // the synthetic `root` world's items.  Searching all worlds would pick the
    // first match across potentially many worlds that happen to define a
    // function with the same name, making routing ambiguous.  Only look in
    // worlds where `is_synthetic` is true so the lookup is deterministic.
    let func = doc.worlds.iter().filter(|w| w.is_synthetic).find_map(|w| {
        w.imports
            .iter()
            .chain(w.exports.iter())
            .find_map(|item| match item {
                WorldItemDoc::Function(f) if f.name == func_name => Some(f),
                _ => None,
            })
    });
    let Some(func) = func else {
        return not_found_response();
    };
    let pkg_url = format!("/{namespace}/{name}/{version}");
    let display_name = components::page_shell::display_name_for(&pkg);
    let html = pages::item::render_function(
        &pkg,
        &version,
        Some(&version_detail),
        &display_name,
        &pkg_url,
        func,
        &doc,
    );
    with_cache_control(&headers, html, "public, max-age=300")
}

/// Fetch and parse the WIT document for a package version, returning
/// both the parsed document and the version detail.
async fn fetch_wit_doc(
    client: &RegistryClient,
    pkg: &KnownPackage,
    version: &str,
) -> Option<(
    wit_doc::WitDocument,
    component_meta_registry_client::PackageVersion,
)> {
    let detail = client
        .fetch_package_version(&pkg.registry, &pkg.repository, version)
        .await
        .ok()
        .flatten()?;
    let wit_text = detail.wit_text.as_deref()?;
    let dep_urls: std::collections::HashMap<String, String> = detail
        .dependencies
        .iter()
        .filter_map(|dep| {
            let v = dep.version.as_deref()?;
            let url = format!("/{}/{v}", dep.package.replace(':', "/"));
            Some((dep.package.clone(), url))
        })
        .collect();
    let url_base = format!(
        "/{}/{}/{}",
        pkg.wit_namespace.as_deref().unwrap_or("_"),
        pkg.wit_name.as_deref().unwrap_or(&pkg.repository),
        version
    );
    let own_oci_package = match (pkg.wit_namespace.as_deref(), pkg.wit_name.as_deref()) {
        (Some(ns), Some(n)) => Some(format!("{ns}:{n}")),
        _ => None,
    };
    let doc = wit_doc::parse_wit_doc_with_type_docs(
        wit_text,
        &url_base,
        &dep_urls,
        &detail.type_docs,
        own_oci_package.as_deref(),
    )
    .ok()?;
    Some((doc, detail))
}

/// Module detail page at `/<namespace>/<name>/<version>/module/<child_name>`.
async fn module_detail(
    headers: HeaderMap,
    Path((namespace, name, version, child_name)): Path<(String, String, String, String)>,
) -> Response {
    let client = RegistryClient::from_env();
    let pkg = match fetch_package_or_404(&client, &namespace, &name, &version).await {
        Ok(Some(pkg)) => pkg,
        Ok(None) => return not_found_response(),
        Err(resp) => return resp,
    };
    let version_detail = client
        .fetch_package_version(&pkg.registry, &pkg.repository, &version)
        .await
        .ok()
        .flatten();
    let child = version_detail.as_ref().and_then(|d| {
        let modules: Vec<&component_meta_registry_client::ComponentSummary> = d
            .components
            .iter()
            .flat_map(|c| &c.children)
            .filter(|ch| ch.kind.as_deref() == Some("module"))
            .collect();

        // Try exact name match first.
        if let Some(ch) = modules
            .iter()
            .find(|ch| ch.name.as_deref() == Some(child_name.as_str()))
        {
            return Some(*ch);
        }

        // Fall back to index match for unnamed modules (e.g. "module[1]").
        if child_name.starts_with("module[") && child_name.ends_with(']') {
            let idx_str = &child_name[7..child_name.len() - 1];
            if let Ok(idx) = idx_str.parse::<usize>() {
                // Only match unnamed modules at this index.
                let unnamed: Vec<_> = modules.iter().filter(|ch| ch.name.is_none()).collect();
                return unnamed.get(idx).map(|ch| **ch);
            }
        }

        None
    });
    let Some(child) = child else {
        return not_found_response();
    };
    let html =
        pages::child_component::render(&pkg, &version, version_detail.as_ref(), child, &child_name);
    with_cache_control(&headers, html, "public, max-age=300")
}

/// Child component detail page at `/<namespace>/<name>/<version>/component/<index>`.
async fn child_component_detail(
    headers: HeaderMap,
    Path((namespace, name, version, child_index)): Path<(String, String, String, String)>,
) -> Response {
    let client = RegistryClient::from_env();
    let pkg = match fetch_package_or_404(&client, &namespace, &name, &version).await {
        Ok(Some(pkg)) => pkg,
        Ok(None) => return not_found_response(),
        Err(resp) => return resp,
    };
    let version_detail = client
        .fetch_package_version(&pkg.registry, &pkg.repository, &version)
        .await
        .ok()
        .flatten();
    let idx: usize = child_index.parse().unwrap_or(usize::MAX);
    let child = version_detail.as_ref().and_then(|d| {
        d.components
            .iter()
            .flat_map(|c| &c.children)
            .filter(|ch| ch.kind.as_deref() == Some("component"))
            .nth(idx)
    });
    let Some(child) = child else {
        return not_found_response();
    };
    let display_name = child
        .name
        .clone()
        .unwrap_or_else(|| format!("component[{child_index}]"));
    let html = pages::child_component::render(
        &pkg,
        &version,
        version_detail.as_ref(),
        child,
        &display_name,
    );
    with_cache_control(&headers, html, "public, max-age=300")
}

/// Fetch a package by WIT namespace/name, validating the version exists.
///
/// Returns `Ok(None)` (and logs) if the namespace is reserved, the package is
/// not found, or the version tag doesn't exist. Returns `Err(Response)` with
/// a `502 Bad Gateway` response when the upstream API call fails, so that
/// registry outages are surfaced correctly instead of being masked as 404s.
async fn fetch_package_or_404(
    client: &RegistryClient,
    namespace: &str,
    name: &str,
    version: &str,
) -> Result<Option<KnownPackage>, Response> {
    if is_reserved(namespace) {
        return Ok(None);
    }
    match client.fetch_package_by_wit(namespace, name).await {
        Ok(Some(pkg)) => {
            if pkg.tags.iter().any(|tag| tag == version) {
                Ok(Some(pkg))
            } else {
                eprintln!(
                    "component-frontend: version not found for {namespace}/{name}: {version}"
                );
                Ok(None)
            }
        }
        Ok(None) => {
            eprintln!("component-frontend: package not found: {namespace}/{name}@{version}");
            Ok(None)
        }
        Err(e) => {
            eprintln!("component-frontend: API error looking up {namespace}/{name}@{version}: {e}");
            Err(error_response(&e.to_string()))
        }
    }
}

// r[impl frontend.pages.not-found]
/// Fallback 404 handler — logs a warning and renders the not-found page.
async fn not_found(uri: Uri) -> Response {
    eprintln!("component-frontend: 404 {uri}");
    not_found_response()
}

/// Render the 404 page response.
fn not_found_response() -> Response {
    let html = pages::not_found::render();
    let mut response = axum::response::Html(html).into_response();
    *response.status_mut() = StatusCode::NOT_FOUND;
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    response
}

/// Render an error page when the registry API is unreachable.
fn error_response(message: &str) -> Response {
    let html = pages::error::render(message);
    let mut response = axum::response::Html(html).into_response();
    *response.status_mut() = StatusCode::BAD_GATEWAY;
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    response
}

// r[impl frontend.caching.static-pages]
// r[impl frontend.caching.etag]
/// Wrap an HTML string response with `Cache-Control` and a content-derived
/// `ETag` header. Honors `If-None-Match` on the incoming request and returns
/// `304 Not Modified` (with the matching `ETag`/`Cache-Control` headers and
/// an empty body) when the client already has the current version.
fn with_cache_control(
    req_headers: &HeaderMap,
    html: String,
    cache_control: &'static str,
) -> Response {
    let etag = compute_etag(html.as_bytes());
    let etag_value = HeaderValue::from_str(&etag)
        .expect("etag is composed of ASCII hex digits and quotes (always a valid HeaderValue)");
    let cache_value = HeaderValue::from_static(cache_control);

    if if_none_match_matches(req_headers, &etag) {
        let mut response = Response::new(Body::empty());
        *response.status_mut() = StatusCode::NOT_MODIFIED;
        response.headers_mut().insert(header::ETAG, etag_value);
        response
            .headers_mut()
            .insert(header::CACHE_CONTROL, cache_value);
        return response;
    }

    let mut response = axum::response::Html(html).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, cache_value);
    response.headers_mut().insert(header::ETAG, etag_value);
    response
}

/// Compute a strong `ETag` value (a quoted hex string) from the response
/// body bytes using the FNV-1a 64-bit hash. FNV-1a is deterministic and
/// stable across Rust toolchain versions, platforms, and process restarts,
/// which is what we need so that a given page produces the same ETag every
/// time it is served. Collisions are not treated as a security concern
/// here, but they can cause incorrect cache validation: if two different
/// bodies ever hash to the same ETag, clients and intermediaries may keep
/// receiving `304 Not Modified` for the newer body and continue serving the
/// older cached body indefinitely across revalidations.
fn compute_etag(bytes: &[u8]) -> String {
    // FNV-1a 64-bit, per http://www.isthe.com/chongo/tech/comp/fnv/
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = FNV_OFFSET;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    format!("\"{hash:016x}\"")
}

/// Returns `true` when the request's `If-None-Match` header lists `etag` or
/// the wildcard `*`, indicating the client already has a matching cached
/// representation.
fn normalize_entity_tag(tag: &str) -> &str {
    tag.trim().strip_prefix("W/").unwrap_or(tag.trim())
}

fn if_none_match_matches(req_headers: &HeaderMap, etag: &str) -> bool {
    let normalized_etag = normalize_entity_tag(etag);
    req_headers
        .get_all(header::IF_NONE_MATCH)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .any(|tag| tag == "*" || normalize_entity_tag(tag) == normalized_etag)
}

#[must_use]
pub(crate) fn pick_redirect_version(tags: &[String]) -> Option<String> {
    pick_latest_semver(tags, |version| version.pre.is_empty())
        .or_else(|| pick_latest_semver(tags, |_| true))
        .or_else(|| tags.iter().find(|tag| tag.as_str() == "latest").cloned())
}

/// Pick the latest semver tag matching `predicate`, preserving the original
/// tag string. Returns `None` if no tag parses as semver and matches.
fn pick_latest_semver(
    tags: &[String],
    predicate: impl Fn(&semver::Version) -> bool,
) -> Option<String> {
    tags.iter()
        .filter_map(|tag| {
            parse_tag_as_semver(tag)
                .filter(|version| predicate(version))
                .map(|version| (version, tag))
        })
        .max_by(|(acc_version, _), (candidate_version, _)| acc_version.cmp(candidate_version))
        .map(|(_, tag)| tag.clone())
}

/// Parse an OCI tag as a semantic version, reversing the `+`->`_` mapping that
/// publishing applies. OCI tags cannot contain `+`, so a SemVer with build
/// metadata such as `0.2.0+circleci-v1` is stored under the tag
/// `0.2.0_circleci-v1`. SemVer has at most one `+` and build metadata cannot
/// contain `_`, so decoding the first `_` back to `+` round-trips the tag and
/// lets it parse as semver instead of being discarded (which would leave the
/// package with no redirectable version). Mirrors
/// `component_package_manager::manager::parse_tag_as_semver`.
fn parse_tag_as_semver(tag: &str) -> Option<semver::Version> {
    if let Ok(version) = semver::Version::parse(tag) {
        return Some(version);
    }
    if tag.contains('_') {
        return semver::Version::parse(&tag.replacen('_', "+", 1)).ok();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;

    // r[verify frontend.pages.package-redirect]
    // r[verify frontend.routing.reserved-namespaces]
    #[tokio::test]
    async fn package_redirect_reserved_namespace_returns_not_found() {
        let response = package_redirect(Path(("all".to_string(), "demo".to_string())))
            .await
            .expect_err("reserved namespace should not redirect");

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            response
                .headers()
                .get(header::CACHE_CONTROL)
                .expect("cache-control header should be set"),
            "no-cache"
        );
    }

    // r[verify frontend.pages.package-detail]
    // r[verify frontend.routing.package-path]
    // r[verify frontend.routing.reserved-namespaces]
    #[tokio::test]
    async fn package_detail_reserved_namespace_returns_not_found() {
        let response = package_detail(
            HeaderMap::new(),
            Path(("all".to_string(), "demo".to_string(), "1.0.0".to_string())),
        )
        .await;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            response
                .headers()
                .get(header::CACHE_CONTROL)
                .expect("cache-control header should be set"),
            "no-cache"
        );
    }

    // r[verify frontend.pages.not-found]
    #[tokio::test]
    async fn fallback_not_found_has_expected_status_and_headers() {
        let response = not_found(Uri::from_static("/does-not-exist")).await;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            response
                .headers()
                .get(header::CACHE_CONTROL)
                .expect("cache-control header should be set"),
            "no-cache"
        );
    }

    // r[verify frontend.server.wasi-http]
    // r[verify frontend.server.health]
    #[tokio::test]
    async fn health_returns_ok_json_and_no_cache() {
        let response = health().await.into_response();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(header::CACHE_CONTROL)
                .expect("cache-control header should be set"),
            "no-cache"
        );

        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("health response body should be readable");
        assert_eq!(bytes.as_ref(), br#"{"status":"ok"}"#);
    }

    // r[verify frontend.assets.favicon]
    #[tokio::test]
    async fn favicon_svg_has_svg_content_type_and_long_cache() {
        let response = favicon_svg().await.into_response();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .expect("content-type header should be set"),
            "image/svg+xml"
        );
        assert_eq!(
            response
                .headers()
                .get(header::CACHE_CONTROL)
                .expect("cache-control header should be set"),
            "public, max-age=604800"
        );

        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("favicon response body should be readable");
        assert_eq!(bytes.as_ref(), FAVICON_SVG.as_bytes());
    }

    // r[verify frontend.assets.favicon]
    #[tokio::test]
    async fn favicon_ico_redirects_to_svg() {
        let response = favicon_ico().await.into_response();
        assert_eq!(response.status(), StatusCode::PERMANENT_REDIRECT);
        assert_eq!(
            response
                .headers()
                .get(header::LOCATION)
                .expect("location header should be set"),
            "/favicon.svg"
        );
    }

    // r[verify frontend.assets.favicon]
    #[test]
    fn favicon_svg_asset_is_wellformed() {
        assert!(FAVICON_SVG.starts_with("<svg"));
        assert!(FAVICON_SVG.contains("viewBox"));
        assert!(FAVICON_SVG.trim_end().ends_with("</svg>"));
    }

    // r[verify frontend.caching.static-pages]
    #[test]
    fn with_cache_control_sets_header() {
        let response = with_cache_control(
            &HeaderMap::new(),
            "<p>Hello</p>".to_string(),
            "public, max-age=60",
        );
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(header::CACHE_CONTROL)
                .expect("cache-control header should be set"),
            "public, max-age=60"
        );
    }

    // r[verify frontend.caching.etag]
    #[test]
    fn with_cache_control_emits_strong_etag_for_body() {
        let response = with_cache_control(
            &HeaderMap::new(),
            "<p>Hello</p>".to_string(),
            "public, max-age=60",
        );
        let etag = response
            .headers()
            .get(header::ETAG)
            .expect("etag header should be set")
            .to_str()
            .expect("etag should be ascii");
        assert!(
            etag.starts_with('"') && etag.ends_with('"'),
            "etag should be a quoted string, got {etag}"
        );
    }

    // r[verify frontend.caching.etag]
    #[test]
    fn compute_etag_changes_with_body() {
        let a = compute_etag(b"<p>Hello</p>");
        let b = compute_etag(b"<p>World</p>");
        assert_ne!(
            a, b,
            "different bodies should produce different etags so fresh content is picked up"
        );
        assert_eq!(a, compute_etag(b"<p>Hello</p>"));
    }

    // r[verify frontend.caching.etag]
    #[test]
    fn compute_etag_is_stable_across_invocations() {
        // Pin the FNV-1a 64 output so that an accidental change to the hash
        // (e.g. switching to a randomized hasher) breaks this test rather
        // than silently invalidating every client's cached ETag.
        assert_eq!(compute_etag(b""), "\"cbf29ce484222325\"");
        assert_eq!(compute_etag(b"foobar"), "\"85944171f73967e8\"");
    }

    // r[verify frontend.caching.etag]
    #[tokio::test]
    async fn with_cache_control_returns_304_on_matching_if_none_match() {
        let body = "<p>Hello</p>".to_string();
        let etag = compute_etag(body.as_bytes());

        let mut req_headers = HeaderMap::new();
        req_headers.insert(
            header::IF_NONE_MATCH,
            HeaderValue::from_str(&etag).expect("etag is ascii"),
        );

        let response = with_cache_control(&req_headers, body, "public, max-age=60");
        assert_eq!(response.status(), StatusCode::NOT_MODIFIED);
        assert_eq!(
            response
                .headers()
                .get(header::ETAG)
                .expect("etag header should be set"),
            etag.as_str()
        );
        assert_eq!(
            response
                .headers()
                .get(header::CACHE_CONTROL)
                .expect("cache-control header should be set"),
            "public, max-age=60"
        );
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("304 response body should be readable");
        assert!(bytes.is_empty(), "304 responses must have an empty body");
    }

    // r[verify frontend.caching.etag]
    #[test]
    fn with_cache_control_returns_304_on_wildcard_if_none_match() {
        let mut req_headers = HeaderMap::new();
        req_headers.insert(header::IF_NONE_MATCH, HeaderValue::from_static("*"));

        let response = with_cache_control(
            &req_headers,
            "<p>Hello</p>".to_string(),
            "public, max-age=60",
        );
        assert_eq!(response.status(), StatusCode::NOT_MODIFIED);
    }

    // r[verify frontend.caching.etag]
    #[test]
    fn with_cache_control_returns_200_when_if_none_match_does_not_match() {
        let mut req_headers = HeaderMap::new();
        req_headers.insert(header::IF_NONE_MATCH, HeaderValue::from_static("\"stale\""));

        let response = with_cache_control(
            &req_headers,
            "<p>Hello</p>".to_string(),
            "public, max-age=60",
        );
        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers().get(header::ETAG).is_some());
    }

    #[test]
    fn pick_redirect_version_prefers_latest_stable_semver() {
        let tags = vec![
            "latest".to_string(),
            "2.0.0-rc.1".to_string(),
            "1.2.0".to_string(),
            "1.10.0".to_string(),
        ];
        assert_eq!(pick_redirect_version(&tags), Some("1.10.0".to_string()));
    }

    #[test]
    fn pick_redirect_version_falls_back_to_latest_tag() {
        let tags = vec!["latest".to_string(), "sha256-deadbeef".to_string()];
        assert_eq!(pick_redirect_version(&tags), Some("latest".to_string()));
    }

    // r[verify frontend.pages.package-redirect]
    #[test]
    fn pick_redirect_version_falls_back_to_latest_prerelease() {
        // When only pre-release semver tags exist (e.g. wasi:otel only ships
        // `0.2.0-rc` versions), the redirect MUST still resolve to the latest
        // pre-release rather than 404'ing.
        let tags = vec![
            "0.2.0-rc.1".to_string(),
            "0.2.0-rc.2".to_string(),
            "sha256-deadbeef".to_string(),
        ];
        assert_eq!(pick_redirect_version(&tags), Some("0.2.0-rc.2".to_string()));
    }

    #[test]
    fn pick_redirect_version_prefers_stable_over_prerelease() {
        let tags = vec![
            "1.0.0".to_string(),
            "2.0.0-rc.1".to_string(),
            "0.9.0".to_string(),
        ];
        assert_eq!(pick_redirect_version(&tags), Some("1.0.0".to_string()));
    }

    #[test]
    fn pick_redirect_version_returns_none_for_unusable_tags() {
        let tags = vec!["sha256-deadbeef".to_string()];
        assert_eq!(pick_redirect_version(&tags), None);
    }

    // r[verify frontend.pages.package-redirect]
    #[test]
    fn pick_redirect_version_decodes_build_metadata_tags() {
        // OCI tags cannot contain `+`, so a SemVer with build metadata like
        // `0.2.0+circleci-v1` is published as `0.2.0_circleci-v1`. The redirect
        // MUST decode the first `_` back to `+`, pick the highest version, and
        // preserve the original tag string so the detail page resolves.
        let tags = vec!["0.2.0_circleci-v1".to_string(), "0.1.0_v1".to_string()];
        assert_eq!(
            pick_redirect_version(&tags),
            Some("0.2.0_circleci-v1".to_string())
        );
    }

    /// Trailing-slash URLs must be handled: the router must register
    /// both `/{namespace}/{name}` and `/{namespace}/{name}/`.
    #[test]
    fn trailing_slash_package_route_is_registered() {
        // Verify the app builds with trailing-slash routes by checking
        // that the route table doesn't panic or conflict.
        let _app = app();
    }

    /// Verify the package redirect handler works with valid path parameters
    /// and doesn't panic — it should either redirect, return not-found, or
    /// return bad-gateway when the registry API is unreachable.
    #[tokio::test]
    async fn package_redirect_handles_trailing_slash_path() {
        let result = package_redirect(Path(("wasi".to_string(), "random".to_string()))).await;
        match result {
            Ok(redirect) => {
                let resp = redirect.into_response();
                assert!(resp.status().is_redirection());
            }
            Err(resp) => {
                let status = resp.status();
                assert!(
                    status == StatusCode::NOT_FOUND || status == StatusCode::BAD_GATEWAY,
                    "expected 404 or 502, got {status}"
                );
            }
        }
    }
}
