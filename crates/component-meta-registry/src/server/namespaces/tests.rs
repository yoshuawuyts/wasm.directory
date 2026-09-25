use super::super::{MAX_LIMIT, router};
use axum::http::StatusCode;
use std::sync::Arc;
use wasm_package_manager::manager::Manager;

#[tokio::test]
async fn namespace_endpoints_return_pages_and_clamp_limits() {
    let dir = tempfile::tempdir().expect("namespace API data directory");
    let manager = Manager::open_at(dir.path())
        .await
        .expect("open namespace API manager");
    let app = router(Arc::new(tokio::sync::RwLock::new(manager)));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind namespace API");
    let address = listener.local_addr().expect("namespace API address");
    let server = tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("serve namespace API");
    });
    for path in ["/v1/namespaces", "/v1/namespaces/wasi/packages"] {
        for (query, limit) in [
            ("", 20),
            ("?limit=0", 20),
            ("?limit=1", 1),
            ("?limit=200&offset=100", MAX_LIMIT),
        ] {
            let response = reqwest::get(format!("http://{address}{path}{query}"))
                .await
                .expect("namespace API request");
            assert_eq!(response.status(), StatusCode::OK);
            let page: serde_json::Value = response.json().await.expect("namespace API JSON");
            assert_eq!(page["results"], serde_json::json!([]));
            assert_eq!(page["total"], 0);
            assert_eq!(page["limit"], limit);
            assert_eq!(page["has_next"], false);
            assert_eq!(
                page["offset"],
                if query.contains("offset") { 100 } else { 0 }
            );
        }
        let response = reqwest::get(format!("http://{address}{path}?offset=invalid"))
            .await
            .expect("invalid namespace request");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
    server.abort();
}
