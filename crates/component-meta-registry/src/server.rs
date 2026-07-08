//! HTTP server with JSON API endpoints for package discovery.
//!
//! Provides search and listing endpoints backed by the `wasm-package-manager`
//! known packages database.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Json, Router, routing::get, routing::post};
use serde::Deserialize;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use wasm_package_manager::manager::Manager;

/// Shared application state wrapping a `Manager` in a `tokio::sync::RwLock`.
///
/// All `Manager` query/mutation methods take `&self` and `Manager` performs
/// its own internal synchronization (it is backed by a database connection
/// pool), so every handler — including the `notify_new_version` write path —
/// acquires a *read* lock and requests run concurrently. The `RwLock` (rather
/// than a bare `Arc<Manager>`) is kept so that a future `Manager` method
/// requiring exclusive `&mut self` access could take a write lock without
/// changing this state type. This replaces the previous `tokio::sync::Mutex`,
/// which serialized *every* request and limited throughput.
///
/// # Example
///
/// ```no_run
/// use component_meta_registry::server::AppState;
/// use wasm_package_manager::manager::Manager;
/// use std::sync::Arc;
///
/// # async fn example() -> anyhow::Result<()> {
/// let manager = Manager::open().await?;
/// let state: AppState = Arc::new(tokio::sync::RwLock::new(manager));
/// # Ok(())
/// # }
/// ```
pub type AppState = Arc<tokio::sync::RwLock<Manager>>;

/// Query parameters for search.
///
/// # Example
///
/// ```
/// use component_meta_registry::server::SearchParams;
///
/// let params = SearchParams {
///     q: "wasi".to_string(),
///     offset: 0,
///     limit: 20,
/// };
///
/// assert_eq!(params.q, "wasi");
/// ```
#[derive(Debug, Deserialize)]
pub struct SearchParams {
    /// Search query string.
    pub q: String,
    /// Pagination offset (default: 0).
    #[serde(default)]
    pub offset: u32,
    /// Pagination limit (default: 20, clamped to `MAX_LIMIT`).
    #[serde(default = "default_limit")]
    pub limit: u32,
}

/// Query parameters for listing packages.
///
/// # Example
///
/// ```
/// use component_meta_registry::server::ListParams;
///
/// let params = ListParams {
///     offset: 0,
///     limit: 50,
/// };
///
/// assert_eq!(params.limit, 50);
/// ```
#[derive(Debug, Deserialize)]
pub struct ListParams {
    /// Pagination offset (default: 0).
    #[serde(default)]
    pub offset: u32,
    /// Pagination limit (default: 20, clamped to `MAX_LIMIT`).
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_limit() -> u32 {
    20
}

/// Maximum number of results that may be requested in a single paginated
/// query. Requests for a larger `limit` are clamped down to this value to
/// bound memory and query cost.
pub const MAX_LIMIT: u32 = 100;

/// Clamp a requested pagination `limit` to the inclusive range
/// `1..=MAX_LIMIT`. A `limit` of `0` is treated as the default to avoid
/// degenerate empty queries.
#[must_use]
fn clamp_limit(limit: u32) -> u32 {
    if limit == 0 {
        return default_limit();
    }
    limit.min(MAX_LIMIT)
}

/// Maximum accepted length (in bytes) for a version `tag` in a notify request.
pub const MAX_TAG_LEN: usize = 128;

/// Maximum accepted length (in bytes) for a `registry` path segment.
pub const MAX_REGISTRY_LEN: usize = 256;

/// Maximum accepted length (in bytes) for a `repository` path segment.
pub const MAX_REPOSITORY_LEN: usize = 512;

/// Build the axum router with all API routes.
///
/// # Example
///
/// ```no_run
/// use component_meta_registry::router;
/// use wasm_package_manager::manager::Manager;
/// use std::sync::Arc;
///
/// # async fn example() -> anyhow::Result<()> {
/// let manager = Manager::open().await?;
/// let state = Arc::new(tokio::sync::RwLock::new(manager));
/// let app = router(state);
///
/// let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
/// axum::serve(listener, app).await?;
/// # Ok(())
/// # }
/// ```
pub fn router(state: AppState) -> Router {
    // Routes with explicit suffixes must be registered before the catch-all
    // wildcard `{*repository}` to avoid conflicts.  We achieve this by
    // nesting the version/detail routes under a separate "prefix" router
    // that axum matches first.
    let package_detail_routes =
        Router::new().route("/{registry}/{*repository}", get(get_package_detail_nested));

    let package_versions_routes = Router::new().route(
        "/{registry}/{*repository}",
        get(get_package_versions_nested),
    );

    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/search", get(search))
        .route("/v1/search/by-import", get(search_by_import))
        .route("/v1/search/by-export", get(search_by_export))
        .route("/v1/packages", get(list_packages))
        .route("/v1/packages/recent", get(list_recent_packages))
        .nest("/v1/packages/detail", package_detail_routes)
        .nest("/v1/packages/versions", package_versions_routes)
        .route(
            "/v1/packages/version/{registry}/{version}/{*repository}",
            get(get_package_version_reordered),
        )
        .route("/v1/packages/{registry}/{*repository}", get(get_package))
        .route("/v1/queue", get(get_queue_status))
        .route(
            "/v1/packages/notify/{registry}/{*repository}",
            post(notify_new_version),
        )
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Health check endpoint.
async fn health() -> impl IntoResponse {
    Json(serde_json::json!({ "status": "ok" }))
}

/// Fetch queue status.
async fn get_queue_status(State(manager): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let manager = manager.read().await;
    let status = manager.get_queue_status().await?;
    Ok(Json(status))
}

/// Search packages by query string.
async fn search(
    State(manager): State<AppState>,
    Query(params): Query<SearchParams>,
) -> Result<impl IntoResponse, AppError> {
    let limit = clamp_limit(params.limit);
    let manager = manager.read().await;
    let packages = manager
        .search_packages(&params.q, params.offset, limit)
        .await?;
    Ok(Json(packages))
}

/// List all known packages.
async fn list_packages(
    State(manager): State<AppState>,
    Query(params): Query<ListParams>,
) -> Result<impl IntoResponse, AppError> {
    let limit = clamp_limit(params.limit);
    let manager = manager.read().await;
    let packages = manager.list_known_packages(params.offset, limit).await?;
    Ok(Json(packages))
}

/// List recently updated known packages.
async fn list_recent_packages(
    State(manager): State<AppState>,
    Query(params): Query<ListParams>,
) -> Result<impl IntoResponse, AppError> {
    let limit = clamp_limit(params.limit);
    let manager = manager.read().await;
    let packages = manager
        .list_recent_known_packages(params.offset, limit)
        .await?;
    Ok(Json(packages))
}

/// Get a specific package by registry and repository.
async fn get_package(
    State(manager): State<AppState>,
    Path((registry, repository)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    // Wildcard captures include a leading `/`; strip it.
    let repository = repository.trim_start_matches('/');
    let manager = manager.read().await;
    match manager.get_known_package(&registry, repository).await? {
        Some(package) => Ok(Json(package).into_response()),
        None => Ok(StatusCode::NOT_FOUND.into_response()),
    }
}

/// Query parameters for interface-based search.
#[derive(Debug, Deserialize)]
pub struct InterfaceSearchParams {
    /// The interface to search for (e.g. `"wasi:io/streams"`).
    pub interface: String,
    /// Pagination offset (default: 0).
    #[serde(default)]
    pub offset: u32,
    /// Pagination limit (default: 20, clamped to `MAX_LIMIT`).
    #[serde(default = "default_limit")]
    pub limit: u32,
}

/// Search packages by imported interface.
// r[verify server.search.by-import]
async fn search_by_import(
    State(manager): State<AppState>,
    Query(params): Query<InterfaceSearchParams>,
) -> Result<impl IntoResponse, AppError> {
    let limit = clamp_limit(params.limit);
    let manager = manager.read().await;
    let packages = manager
        .search_packages_by_import(&params.interface, params.offset, limit)
        .await?;
    Ok(Json(packages))
}

/// Search packages by exported interface.
// r[verify server.search.by-export]
async fn search_by_export(
    State(manager): State<AppState>,
    Query(params): Query<InterfaceSearchParams>,
) -> Result<impl IntoResponse, AppError> {
    let limit = clamp_limit(params.limit);
    let manager = manager.read().await;
    let packages = manager
        .search_packages_by_export(&params.interface, params.offset, limit)
        .await?;
    Ok(Json(packages))
}

/// Get full package detail including all versions and metadata.
// r[verify server.detail]
async fn get_package_detail_nested(
    State(manager): State<AppState>,
    Path((registry, repository)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let repository = repository.trim_start_matches('/');
    let manager = manager.read().await;
    match manager.get_package_detail(&registry, repository).await? {
        Some(detail) => Ok(Json(detail).into_response()),
        None => Ok(StatusCode::NOT_FOUND.into_response()),
    }
}

/// List all versions of a package.
// r[verify server.versions.list]
async fn get_package_versions_nested(
    State(manager): State<AppState>,
    Path((registry, repository)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let repository = repository.trim_start_matches('/');
    let manager = manager.read().await;
    match manager.get_package_detail(&registry, repository).await? {
        Some(detail) => Ok(Json(detail.versions).into_response()),
        None => Ok(StatusCode::NOT_FOUND.into_response()),
    }
}

/// Get a specific version of a package by tag.
// r[verify server.versions.get]
async fn get_package_version_reordered(
    State(manager): State<AppState>,
    Path((registry, version, repository)): Path<(String, String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let repository = repository.trim_start_matches('/');
    let manager = manager.read().await;
    match manager
        .get_package_version(&registry, repository, &version)
        .await?
    {
        Some(ver) => Ok(Json(ver).into_response()),
        None => Ok(StatusCode::NOT_FOUND.into_response()),
    }
}

/// Query parameters for `POST /v1/packages/notify/...`.
#[derive(Debug, Deserialize)]
pub struct NotifyParams {
    /// Tag of the newly-published version (e.g. `"1.1.0"`).
    pub tag: String,
}

/// Validate the user-supplied inputs to `notify_new_version`.
///
/// Returns `Some(message)` describing the first validation failure, or `None`
/// if all inputs are within bounds. `tag` is expected to already be trimmed.
#[must_use]
fn validate_notify_input(registry: &str, repository: &str, tag: &str) -> Option<&'static str> {
    if tag.is_empty() {
        return Some("tag must not be empty");
    }
    if tag.len() > MAX_TAG_LEN {
        return Some("tag is too long");
    }
    if registry.trim().is_empty() {
        return Some("registry must not be empty");
    }
    if registry.len() > MAX_REGISTRY_LEN {
        return Some("registry is too long");
    }
    if repository.trim().is_empty() {
        return Some("repository must not be empty");
    }
    if repository.len() > MAX_REPOSITORY_LEN {
        return Some("repository is too long");
    }
    None
}

/// Notify the registry that a new version was just published, requesting it
/// be pulled as soon as possible.
///
/// Returns `202 Accepted` with a JSON `NotifyOutcome` body in both the
/// "enqueued" and "skipped" cases — the request itself was accepted; the
/// outcome describes what the registry decided to do with it.
///
/// To prevent abuse, the endpoint:
///
/// * Rejects empty tags with `400 Bad Request`.
/// * Rejects tags longer than [`MAX_TAG_LEN`] bytes, and `registry` /
///   `repository` segments that are empty or exceed [`MAX_REGISTRY_LEN`] /
///   [`MAX_REPOSITORY_LEN`] bytes, with `400 Bad Request`.
/// * Only accepts notifications for packages already known to this
///   registry (i.e. previously indexed). Notifications for unknown
///   packages return `404 Not Found` so the queue can't be flooded with
///   arbitrary `(registry, repository, tag)` triples.
/// * Enforces a freshness window (the same 1-hour cooldown used by the
///   periodic indexer). Repeated notifications for a tag that was just
///   pulled are returned as `{"status":"skipped"}`.
async fn notify_new_version(
    State(manager): State<AppState>,
    Path((registry, repository)): Path<(String, String)>,
    Query(params): Query<NotifyParams>,
) -> Result<axum::response::Response, AppError> {
    let repository = repository.trim_start_matches('/');
    let tag = params.tag.trim();
    if let Some(error) = validate_notify_input(&registry, repository, tag) {
        return Ok((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": error })),
        )
            .into_response());
    }

    let manager = manager.read().await;

    // Only allow notifications for packages we already know about. This
    // prevents arbitrary clients from filling the fetch queue with
    // unknown `(registry, repository, tag)` triples.
    if manager
        .get_known_package(&registry, repository)
        .await?
        .is_none()
    {
        return Ok((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "unknown package; only previously-indexed packages may be notified"
            })),
        )
            .into_response());
    }

    let outcome = manager
        .notify_new_version(&registry, repository, tag)
        .await?;
    Ok((StatusCode::ACCEPTED, Json(outcome)).into_response())
}

/// Application error type that converts to HTTP responses.
struct AppError(anyhow::Error);

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": self.0.to_string() })),
        )
            .into_response()
    }
}

impl<E: Into<anyhow::Error>> From<E> for AppError {
    fn from(err: E) -> Self {
        Self(err.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// Open a `Manager` backed by an isolated temporary data directory.
    ///
    /// Using a per-test temp dir keeps tests from sharing on-disk SQLite
    /// state (which breaks under parallel execution) and avoids writing
    /// into a developer's or CI user's real platform data directory. The
    /// returned `TempDir` must be kept alive for the duration of the test;
    /// dropping it deletes the database.
    async fn isolated_manager() -> (tempfile::TempDir, Manager) {
        let dir = tempfile::tempdir().expect("failed to create tempdir");
        let manager = Manager::open_at(dir.path())
            .await
            .expect("failed to open manager");
        (dir, manager)
    }

    // r[verify server.health]
    /// Verify the server starts, binds to a port, and responds to `/v1/health`.
    #[tokio::test]
    async fn server_starts_and_listens() {
        let (_data_dir, manager) = isolated_manager().await;
        let state = Arc::new(tokio::sync::RwLock::new(manager));
        let app = router(state);

        // Bind to port 0 so the OS assigns a random available port.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("failed to bind listener");
        let addr = listener.local_addr().expect("failed to get local addr");

        // Spawn the server in a background task.
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("server error");
        });

        // Hit the health endpoint.
        let url = format!("http://{addr}/v1/health");
        let resp = reqwest::get(&url).await.expect("request failed");
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = resp.json().await.expect("invalid json");
        assert_eq!(body, serde_json::json!({ "status": "ok" }));

        // Clean up.
        server.abort();
    }

    /// Verify the notify endpoint enqueues a pull task and is idempotent.
    /// A second notify for the same tag while the task is still pending
    /// should also return `enqueued` (the queue dedupes internally).
    #[tokio::test]
    async fn notify_endpoint_enqueues_pull_task() {
        use wasm_meta_registry_types::NotifyOutcome;

        let (_data_dir, manager) = isolated_manager().await;

        // Register the target as a known package up-front: the notify
        // endpoint rejects unknown packages with `404` to prevent
        // arbitrary clients from flooding the fetch queue.
        let registry = "example.test";
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let repository = format!("notify-test-{unique}");
        manager
            .add_known_package(registry, &repository, None, None)
            .await
            .expect("failed to register known package");

        let state = Arc::new(tokio::sync::RwLock::new(manager));
        let app = router(state);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("failed to bind listener");
        let addr = listener.local_addr().expect("failed to get local addr");

        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("server error");
        });

        let url =
            format!("http://{addr}/v1/packages/notify/{registry}/{repository}?tag=0.0.1-test");
        let client = reqwest::Client::new();
        let resp = client.post(&url).send().await.expect("request failed");
        assert_eq!(resp.status(), StatusCode::ACCEPTED);
        let outcome: NotifyOutcome = resp.json().await.expect("invalid json");
        assert_eq!(outcome, NotifyOutcome::Enqueued);

        // A second notify for the same tag while the task is still
        // pending must also return `enqueued` — the underlying queue
        // dedupes by `(registry, repository, tag)` so the request is
        // accepted and reported as enqueued without creating a duplicate
        // row.
        let resp = client
            .post(&url)
            .send()
            .await
            .expect("second request failed");
        assert_eq!(resp.status(), StatusCode::ACCEPTED);
        let outcome: NotifyOutcome = resp.json().await.expect("invalid json");
        assert_eq!(outcome, NotifyOutcome::Enqueued);

        server.abort();
    }

    /// `clamp_limit` enforces the upper bound and substitutes the default for
    /// a zero limit.
    #[test]
    fn clamp_limit_enforces_cap_and_default() {
        assert_eq!(clamp_limit(0), default_limit());
        assert_eq!(clamp_limit(20), 20);
        assert_eq!(clamp_limit(MAX_LIMIT), MAX_LIMIT);
        assert_eq!(clamp_limit(MAX_LIMIT + 1), MAX_LIMIT);
        assert_eq!(clamp_limit(u32::MAX), MAX_LIMIT);
    }

    /// `validate_notify_input` accepts well-formed input and rejects empty or
    /// oversized fields.
    #[test]
    fn validate_notify_input_bounds() {
        assert!(validate_notify_input("example.test", "owner/repo", "1.0.0").is_none());

        assert!(validate_notify_input("example.test", "owner/repo", "").is_some());
        let long_tag = "x".repeat(MAX_TAG_LEN + 1);
        assert!(validate_notify_input("example.test", "owner/repo", &long_tag).is_some());

        assert!(validate_notify_input("", "owner/repo", "1.0.0").is_some());
        let long_registry = "x".repeat(MAX_REGISTRY_LEN + 1);
        assert!(validate_notify_input(&long_registry, "owner/repo", "1.0.0").is_some());

        assert!(validate_notify_input("example.test", "", "1.0.0").is_some());
        let long_repository = "x".repeat(MAX_REPOSITORY_LEN + 1);
        assert!(validate_notify_input("example.test", &long_repository, "1.0.0").is_some());
    }

    /// A search request whose `limit` exceeds the cap is clamped rather than
    /// rejected: the request still succeeds with `200 OK`.
    #[tokio::test]
    async fn search_limit_above_cap_is_clamped() {
        let (_data_dir, manager) = isolated_manager().await;
        let state = Arc::new(tokio::sync::RwLock::new(manager));
        let app = router(state);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("failed to bind listener");
        let addr = listener.local_addr().expect("failed to get local addr");
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("server error");
        });

        let url = format!("http://{addr}/v1/search?q=wasi&limit=100000");
        let resp = reqwest::get(&url).await.expect("request failed");
        assert_eq!(resp.status(), StatusCode::OK);
        let body: serde_json::Value = resp.json().await.expect("invalid json");
        let results = body.as_array().expect("expected an array of results");
        assert!(results.len() as u32 <= MAX_LIMIT);

        server.abort();
    }

    /// A notify request with an empty tag is rejected with `400 Bad Request`.
    #[tokio::test]
    async fn notify_empty_tag_is_rejected() {
        let (_data_dir, manager) = isolated_manager().await;
        let state = Arc::new(tokio::sync::RwLock::new(manager));
        let app = router(state);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("failed to bind listener");
        let addr = listener.local_addr().expect("failed to get local addr");
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("server error");
        });

        let url = format!("http://{addr}/v1/packages/notify/example.test/owner/repo?tag=%20");
        let client = reqwest::Client::new();
        let resp = client.post(&url).send().await.expect("request failed");
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        server.abort();
    }

    /// A notify request with an oversized tag is rejected with `400 Bad
    /// Request`.
    #[tokio::test]
    async fn notify_oversized_tag_is_rejected() {
        let (_data_dir, manager) = isolated_manager().await;
        let state = Arc::new(tokio::sync::RwLock::new(manager));
        let app = router(state);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("failed to bind listener");
        let addr = listener.local_addr().expect("failed to get local addr");
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("server error");
        });

        let long_tag = "x".repeat(MAX_TAG_LEN + 1);
        let url =
            format!("http://{addr}/v1/packages/notify/example.test/owner/repo?tag={long_tag}");
        let client = reqwest::Client::new();
        let resp = client.post(&url).send().await.expect("request failed");
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        server.abort();
    }

    /// Multiple read-only endpoints can hold a read lock concurrently after the
    /// switch to `RwLock`. We fire several overlapping `/v1/health`-independent
    /// read requests and confirm they all succeed.
    #[tokio::test]
    async fn concurrent_reads_are_allowed() {
        let (_data_dir, manager) = isolated_manager().await;
        let state: AppState = Arc::new(tokio::sync::RwLock::new(manager));

        // Acquire a read guard and confirm a second read guard can be acquired
        // concurrently (a write lock would block here).
        let g1 = state.read().await;
        let g2 = state.read().await;
        let _ = (g1.get_queue_status().await, g2.get_queue_status().await);
        drop((g1, g2));

        let app = router(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("failed to bind listener");
        let addr = listener.local_addr().expect("failed to get local addr");
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("server error");
        });

        let mut handles = Vec::new();
        for _ in 0..8 {
            let url = format!("http://{addr}/v1/packages?limit=5");
            handles.push(tokio::spawn(async move {
                reqwest::get(&url).await.map(|r| r.status())
            }));
        }
        for handle in handles {
            let status = handle
                .await
                .expect("task panicked")
                .expect("request failed");
            assert_eq!(status, StatusCode::OK);
        }

        server.abort();
    }
}
