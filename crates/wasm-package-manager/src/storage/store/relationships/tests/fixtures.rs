use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use sea_orm::{ConnectOptions, Database};
use wasm_package_manager_migration::{Migrator, MigratorTrait};

use super::super::super::{
    Store, apply_sqlite_pragmas, insert_oci_layer, insert_wit_package_dependency, insert_wit_world,
    insert_wit_world_iface, upsert_oci_manifest, upsert_oci_repository_full, upsert_oci_tag,
    upsert_wit_package,
};
use crate::storage::{Migrations, StateInfo};

/// No environment configuration or filesystem storage is consulted.
pub(super) async fn fixture_store() -> Store {
    let mut options = ConnectOptions::new("sqlite::memory:");
    options.max_connections(1).sqlx_logging(false);
    let db = Database::connect(options).await.expect("connect SQLite");
    apply_sqlite_pragmas(&db).await.expect("configure SQLite");
    Migrator::up(&db, None).await.expect("apply migrations");
    let migrations = Migrations::snapshot(&db).await;
    Store {
        state_info: StateInfo::new_at(
            PathBuf::from("relationship-fixture"),
            PathBuf::from("relationship-fixture/config.toml"),
            &migrations,
            0,
            0,
        ),
        db,
        db_config: None,
    }
}

pub(super) struct Release {
    pub(super) repo_id: i64,
    pub(super) manifest_id: i64,
    pub(super) wit_id: i64,
    pub(super) digest: String,
}

impl Release {
    pub(super) async fn dependency(&self, store: &Store, package: &str, version: Option<&str>) {
        insert_wit_package_dependency(&store.db, self.wit_id, package, version)
            .await
            .expect("insert dependency declaration");
    }

    pub(super) async fn world(&self, store: &Store, name: &str, description: Option<&str>) -> i64 {
        insert_wit_world(&store.db, self.wit_id, name, description)
            .await
            .expect("insert world")
    }

    pub(super) async fn tag(&self, store: &Store, tag: &str) {
        upsert_oci_tag(&store.db, self.repo_id, tag, &self.digest)
            .await
            .expect("insert tag");
    }
}

pub(super) async fn member(
    store: &Store,
    world_id: i64,
    package: &str,
    interface: Option<&str>,
    version: Option<&str>,
    is_import: bool,
) {
    insert_wit_world_iface(&store.db, world_id, package, interface, version, is_import)
        .await
        .expect("insert world declaration");
}

pub(super) async fn repository(
    store: &Store,
    registry: &str,
    repository: &str,
    identity: Option<&str>,
    kind: Option<&str>,
) -> i64 {
    let identity = identity.map(|name| name.split_once(':').expect("namespace:name"));
    upsert_oci_repository_full(
        &store.db,
        registry,
        repository,
        identity.map(|(ns, _)| ns),
        identity.map(|(_, name)| name),
        kind,
    )
    .await
    .expect("insert repository")
}

pub(super) async fn seed_release(
    store: &Store,
    repo_id: i64,
    own_name: &str,
    wit_version: Option<&str>,
    tags: &[&str],
) -> Release {
    static NEXT_DIGEST: AtomicU64 = AtomicU64::new(1);
    let serial = NEXT_DIGEST.fetch_add(1, Ordering::Relaxed);
    let digest = format!("sha256:{repo_id}-{serial}");
    let (manifest_id, _) = upsert_oci_manifest(
        &store.db,
        repo_id,
        &digest,
        None,
        None,
        None,
        None,
        None,
        None,
        &HashMap::new(),
    )
    .await
    .expect("insert manifest");
    let layer = insert_oci_layer(
        &store.db,
        manifest_id,
        &format!("sha256:layer-{serial}"),
        None,
        None,
        0,
    )
    .await
    .expect("insert layer");
    let wit_id = upsert_wit_package(
        &store.db,
        own_name,
        wit_version,
        None,
        None,
        Some(manifest_id),
        Some(layer),
    )
    .await
    .expect("insert WIT package");
    let release = Release {
        repo_id,
        manifest_id,
        wit_id,
        digest,
    };
    for tag in tags {
        release.tag(store, tag).await;
    }
    release
}

pub(super) async fn package(
    store: &Store,
    name: &str,
    tag: &str,
    dependencies: &[&str],
) -> Release {
    let repo_id = repository(
        store,
        "registry.test",
        &name.replace(':', "/"),
        Some(name),
        Some("interface"),
    )
    .await;
    let release = seed_release(store, repo_id, name, Some(tag), &[tag]).await;
    for dependency in dependencies {
        release.dependency(store, dependency, Some("99.0.0")).await;
    }
    release
}
