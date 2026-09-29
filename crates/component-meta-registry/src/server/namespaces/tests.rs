use super::super::{MAX_LIMIT, router, router_with_namespaces};
use axum::http::StatusCode;
use std::sync::Arc;
use wasm_package_manager::manager::Manager;

async fn seed_index(dir: &std::path::Path) {
    use sea_orm::{ConnectionTrait, Database};

    let db = Database::connect(format!(
        "sqlite://{}?mode=rw",
        dir.join("db/metadata-v2.db3").display()
    ))
    .await
    .expect("open isolated fixture database");
    for (id, repository, namespace) in [
        (1, "populated/package", Some("populated")),
        (2, "fallback/package", None),
        (3, "unregistered/package", Some("unregistered")),
    ] {
        let namespace = namespace.map_or("NULL".into(), |name| format!("'{name}'"));
        db.execute_unprepared(&format!(
            "INSERT INTO oci_repository (id, registry, repository, wit_namespace, wit_name) \
             VALUES ({id}, 'registry.test', '{repository}', {namespace}, 'package');"
        ))
        .await
        .expect("seed indexed repository");
        db.execute_unprepared(&format!(
            "INSERT INTO oci_manifest (oci_repository_id, digest) VALUES ({id}, 'sha256:{id}');"
        ))
        .await
        .expect("seed manifest");
        db.execute_unprepared(&format!(
            "INSERT INTO oci_tag (oci_repository_id, manifest_digest, tag) \
             VALUES ({id}, 'sha256:{id}', '1.0.0');"
        ))
        .await
        .expect("seed release");
    }
    db.close().await.expect("close fixture writer");
}

fn registered_namespaces(dir: &std::path::Path) -> Vec<crate::registry_file::Namespace> {
    for name in ["populated", "empty", "pending"] {
        let package = if name == "empty" {
            ""
        } else {
            "\n[[interface]]\nname = 'package'\nrepository = 'package'\n"
        };
        std::fs::write(
            dir.join(format!("{name}.toml")),
            format!("[namespace]\nname = '{name}'\nregistry = 'registry.test/{name}'\n{package}"),
        )
        .expect("write registration");
    }
    crate::Config::from_registry_dir_with_namespaces(dir, 3600, "127.0.0.1:0".into())
        .expect("load registrations")
        .1
}

#[tokio::test]
async fn configured_membership_includes_empty_and_pending_namespaces_before_indexing() {
    let registrations = tempfile::tempdir().expect("registration directory");
    let namespaces = registered_namespaces(registrations.path());
    let data = tempfile::tempdir().expect("index directory");
    let manager = Manager::open_at(data.path()).await.expect("open index");
    seed_index(data.path()).await;
    let app = router_with_namespaces(Arc::new(tokio::sync::RwLock::new(manager)), &namespaces);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind API");
    let address = listener.local_addr().expect("API address");
    let server = tokio::spawn(async move { axum::serve(listener, app).await.expect("serve API") });
    for (offset, name, count, has_next) in [
        (0, "empty", 0, true),
        (1, "pending", 0, true),
        (2, "populated", 1, false),
    ] {
        let response = reqwest::get(format!(
            "http://{address}/v1/namespaces?offset={offset}&limit=1"
        ))
        .await
        .expect("fetch directory");
        assert_eq!(response.status(), StatusCode::OK);
        let page: serde_json::Value = response.json().await.expect("directory JSON");
        assert_eq!(
            page["results"],
            serde_json::json!([{"name":name,"packages":count}])
        );
        assert_eq!(page["total"], 3);
        assert_eq!(page["has_next"], has_next);
    }
    for name in ["empty", "pending"] {
        let response = reqwest::get(format!("http://{address}/v1/namespaces/{name}/packages"))
            .await
            .expect("fetch empty namespace");
        assert_eq!(response.status(), StatusCode::OK);
        let page: serde_json::Value = response.json().await.expect("package JSON");
        assert_eq!(page["results"], serde_json::json!([]));
        assert_eq!(page["total"], 0);
    }
    server.abort();
}

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
