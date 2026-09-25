use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement, Value,
};
use tokio::task::JoinHandle;
use wasm_package_manager::manager::Manager;

use crate::server::{AppState, router};

pub(super) struct Fixture {
    pub(super) db: DatabaseConnection,
    pub(super) state: AppState,
    client: reqwest::Client,
    base_url: String,
    server: JoinHandle<()>,
    data_dir: PathBuf,
}

impl Fixture {
    pub(super) async fn new() -> Self {
        static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);
        assert!(
            std::env::var_os("COMPONENT_DATABASE_URL").is_none(),
            "unset COMPONENT_DATABASE_URL before running isolated relationship HTTP tests"
        );
        let id = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let data_dir = PathBuf::from("target/relationship-fixtures")
            .join(format!("{}-{id}", std::process::id()));
        assert!(!data_dir.exists(), "fixture directory must be new");
        let manager = Manager::open_at(&data_dir)
            .await
            .expect("open isolated manager");
        let mut options = ConnectOptions::new(format!(
            "sqlite://{}?mode=rw",
            data_dir.join("db/metadata-v2.db3").display()
        ));
        options.max_connections(1).sqlx_logging(false);
        let db = Database::connect(options)
            .await
            .expect("open fixture connection");
        db.execute_unprepared("PRAGMA foreign_keys = ON")
            .await
            .expect("enable fixture foreign keys");
        let state = Arc::new(tokio::sync::RwLock::new(manager));
        let app = router(Arc::clone(&state));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind fixture server");
        let address = listener.local_addr().expect("fixture address");
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve fixture");
        });
        Self {
            db,
            state,
            client: reqwest::Client::builder()
                .no_proxy()
                .build()
                .expect("HTTP client"),
            base_url: format!("http://{address}"),
            server,
            data_dir,
        }
    }

    pub(super) async fn get(&self, path: &str) -> reqwest::Response {
        self.client
            .get(format!("{}{path}", self.base_url))
            .send()
            .await
            .expect("local HTTP request")
    }

    pub(super) async fn execute(&self, sql: &str, values: Vec<Value>) -> i64 {
        let result = self
            .db
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                sql,
                values,
            ))
            .await
            .expect("execute fixture SQL");
        i64::try_from(result.last_insert_id()).expect("fixture row ID")
    }

    pub(super) async fn repository(&self, identity: &str, kind: &str) -> i64 {
        let (namespace, name) = identity.split_once(':').expect("namespace:name");
        self.execute(
            "INSERT INTO oci_repository \
             (registry, repository, wit_namespace, wit_name, kind) VALUES (?, ?, ?, ?, ?)",
            vec![
                "registry.test".into(),
                identity.replace(':', "/").into(),
                namespace.into(),
                name.into(),
                kind.into(),
            ],
        )
        .await
    }

    pub(super) async fn release(&self, repo_id: i64, own_name: &str, tag: &str) -> Release {
        let digest = format!("sha256:{repo_id}-{tag}");
        let manifest_id = self
            .execute(
                "INSERT INTO oci_manifest (oci_repository_id, digest) VALUES (?, ?)",
                vec![repo_id.into(), digest.clone().into()],
            )
            .await;
        let layer_id = self
            .execute(
                "INSERT INTO oci_layer (oci_manifest_id, digest, position) VALUES (?, ?, 0)",
                vec![
                    manifest_id.into(),
                    format!("sha256:layer-{manifest_id}").into(),
                ],
            )
            .await;
        let wit_id = self
            .execute(
                "INSERT INTO wit_package (package_name, version, oci_manifest_id, oci_layer_id) \
                 VALUES (?, ?, ?, ?)",
                vec![
                    own_name.into(),
                    tag.replace('_', "+").into(),
                    manifest_id.into(),
                    layer_id.into(),
                ],
            )
            .await;
        let release = Release {
            repo_id,
            wit_id,
            digest,
        };
        self.tag(&release, tag).await;
        release
    }

    pub(super) async fn tag(&self, release: &Release, tag: &str) {
        self.execute(
            "INSERT INTO oci_tag (oci_repository_id, manifest_digest, tag) VALUES (?, ?, ?)",
            vec![
                release.repo_id.into(),
                release.digest.clone().into(),
                tag.into(),
            ],
        )
        .await;
    }

    pub(super) async fn dependency(&self, release: &Release, target: &str) {
        self.execute(
            "INSERT INTO wit_package_dependency (dependent_id, declared_package, declared_version) \
             VALUES (?, ?, '99.0.0')",
            vec![release.wit_id.into(), target.into()],
        )
        .await;
    }

    pub(super) async fn world(&self, release: &Release, name: &str, description: &str) -> i64 {
        self.execute(
            "INSERT INTO wit_world (wit_package_id, name, description) VALUES (?, ?, ?)",
            vec![release.wit_id.into(), name.into(), description.into()],
        )
        .await
    }

    pub(super) async fn member(&self, world: i64, package: &str, interface: &str, is_import: bool) {
        let table = if is_import {
            "wit_world_import"
        } else {
            "wit_world_export"
        };
        self.execute(
            &format!(
                "INSERT INTO {table} (wit_world_id, declared_package, declared_interface) \
                 VALUES (?, ?, ?)"
            ),
            vec![world.into(), package.into(), interface.into()],
        )
        .await;
    }

    pub(super) async fn matching_package(&self, identity: &str, tag: &str) -> Release {
        let repo = self.repository(identity, "interface").await;
        let release = self.release(repo, identity, tag).await;
        self.dependency(&release, "wasi:io").await;
        let world = self.world(&release, "run", "A matching world").await;
        self.member(world, "wasi:io", "streams", true).await;
        self.member(world, "wasi:io", "poll", true).await;
        self.member(world, "wasi:io", "streams", false).await;
        release
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
        let _ = std::fs::remove_dir_all(&self.data_dir);
    }
}

pub(super) struct Release {
    pub(super) repo_id: i64,
    wit_id: i64,
    pub(super) digest: String,
}
