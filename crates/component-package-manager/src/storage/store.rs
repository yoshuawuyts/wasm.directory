//! SeaORM-backed implementation of the package-manager metadata store.
//!
//! This is the single place in the crate that talks to the database. It owns
//! a [`sea_orm::DatabaseConnection`] and exposes a method-oriented API used by
//! [`crate::manager::Manager`] and friends.
//!
//! The schema is defined in [`component_package_manager_migration`]; entities
//! used here are re-imported from that crate.

// SeaORM 2.0-rc deprecates `Insert::do_nothing` in favour of
// `try_insert`/`on_conflict_do_nothing*`, but those have a different return
// shape that doesn't compose cleanly with our existing helpers. Re-evaluate
// when SeaORM 2.0 ships stable.
#![allow(deprecated)]
// `mod _foo {}` shims used as scratch space for bound traits get lifted out
// of statement position by clippy in this file; the patterns are intentional.
#![allow(clippy::items_after_statements)]

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use anyhow::Context;
use chrono::{DateTime, Utc};
use futures_concurrency::prelude::*;
use oci_client::{Reference, client::ImageData, manifest::OciImageManifest};
#[cfg(test)]
use sea_orm::ConnectOptions;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, Database, DatabaseConnection, DbBackend,
    EntityTrait, FromQueryResult, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set,
    Statement, TransactionTrait,
    sea_query::{Expr, OnConflict, SimpleExpr},
};
use tracing::warn;

use component_package_manager_migration::Migrator;
use component_package_manager_migration::MigratorTrait;
use component_package_manager_migration::entities::{
    component_target, fetch_queue, oci_layer, oci_layer_annotation, oci_manifest,
    oci_manifest_annotation, oci_referrer, oci_repository, oci_tag, sync_meta, wasm_component,
    wit_package, wit_package_dependency, wit_world, wit_world_export, wit_world_import,
};

use super::config::StateInfo;
use super::known_package::KnownPackageParams;
use super::models::Migrations;
use crate::oci::{InsertResult, RawImageEntry};
use crate::types::WitMetadata;
use crate::types::extract_wit_metadata;

// -- Public types --------------------------------------------------------

/// The kind of work a [`FetchTask`] represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FetchTaskKind {
    /// Download from the OCI registry and extract metadata.
    Pull,
    /// Re-derive WIT metadata from already-cached layers.
    Reindex,
}

impl From<&str> for FetchTaskKind {
    fn from(s: &str) -> Self {
        match s {
            "reindex" => Self::Reindex,
            _ => Self::Pull,
        }
    }
}

impl From<String> for FetchTaskKind {
    fn from(s: String) -> Self {
        Self::from(s.as_str())
    }
}

impl From<fetch_queue::FetchTask> for FetchTaskKind {
    fn from(t: fetch_queue::FetchTask) -> Self {
        match t {
            fetch_queue::FetchTask::Pull => Self::Pull,
            fetch_queue::FetchTask::Reindex => Self::Reindex,
        }
    }
}

/// A single unit of work dequeued from the fetch queue.
#[derive(Debug)]
pub struct FetchTask {
    /// Row id in the `fetch_queue` table.
    pub id: i64,
    /// OCI registry hostname.
    pub registry: String,
    /// OCI repository path.
    pub repository: String,
    /// Version tag.
    pub tag: String,
    /// What to do with this tag.
    pub kind: FetchTaskKind,
    /// How many times this task has been attempted so far.
    pub attempts: i64,
}

// -- Internal helpers ----------------------------------------------------

/// Calculate the total size of a directory recursively.
async fn dir_size(path: &Path) -> u64 {
    let mut total = 0u64;
    let mut stack = vec![path.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let Ok(mut entries) = tokio::fs::read_dir(&dir).await else {
            continue;
        };
        while let Ok(Some(entry)) = entries.next_entry().await {
            let Ok(metadata) = entry.metadata().await else {
                continue;
            };
            if metadata.is_dir() {
                stack.push(entry.path());
            } else {
                total += metadata.len();
            }
        }
    }
    total
}

/// Apply the SQLite-specific PRAGMAs that the legacy rusqlite path used.
async fn apply_sqlite_pragmas(db: &DatabaseConnection) -> anyhow::Result<()> {
    if !matches!(db.get_database_backend(), DbBackend::Sqlite) {
        return Ok(());
    }
    for pragma in [
        "PRAGMA foreign_keys = ON;",
        "PRAGMA journal_mode = WAL;",
        "PRAGMA synchronous = NORMAL;",
        "PRAGMA busy_timeout = 5000;",
    ] {
        db.execute_unprepared(pragma).await?;
    }
    Ok(())
}

/// Fixed advisory-lock key for serializing Postgres schema migrations.
///
/// This value was generated once from the ASCII bytes `b"cmpmigr!"` interpreted
/// as a big-endian signed 64-bit integer. It is intentionally hardcoded and
/// MUST NOT change, or different binaries could contend on different lock keys.
const POSTGRES_MIGRATION_ADVISORY_LOCK_KEY: i64 = 7_164_506_197_438_460_449;

const POSTGRES_MIGRATION_SET_TIMEOUT_SQL: &str =
    "SET lock_timeout = '60s'; SET statement_timeout = '60s';";

/// Run SeaORM migrations for Postgres while holding a session-scoped advisory
/// lock on a dedicated single-connection pool.
async fn run_postgres_migrations_with_advisory_lock(
    cfg: &super::db_config::DbConfig,
) -> anyhow::Result<()> {
    // Advisory locks are session-scoped, so lock/unlock + Migrator::up must
    // happen on the exact same physical connection. A one-connection pool
    // guarantees that.
    let mut migration_opts = cfg.to_connect_options();
    migration_opts.max_connections(1);
    migration_opts.min_connections(1);
    let db = Database::connect(migration_opts)
        .await
        .with_context(|| format!("failed to connect to database at {}", cfg.redacted_url()))?;

    db.execute_unprepared(POSTGRES_MIGRATION_SET_TIMEOUT_SQL)
        .await
        .context("failed to configure Postgres migration lock timeouts")?;

    let lock_stmt = Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT pg_advisory_lock($1);",
        [POSTGRES_MIGRATION_ADVISORY_LOCK_KEY.into()],
    );
    db.execute_raw(lock_stmt)
        .await
        .with_context(|| "failed to acquire Postgres migration advisory lock")?;

    let migration_result = Migrator::up(&db, None)
        .await
        .context("failed to run database migrations");
    let unlock_stmt = Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT pg_advisory_unlock($1);",
        [POSTGRES_MIGRATION_ADVISORY_LOCK_KEY.into()],
    );
    let unlock_result = db
        .execute_raw(unlock_stmt)
        .await
        .with_context(|| "failed to release Postgres migration advisory lock");

    if let Err(migration_err) = migration_result {
        unlock_result?;
        return Err(migration_err);
    }
    unlock_result?;
    Ok(())
}

/// Parse the OCI repository's `kind` text column into a [`PackageKind`].
fn parse_kind(s: Option<&str>) -> Option<component_meta_registry_types::PackageKind> {
    use component_meta_registry_types::PackageKind;
    match s {
        Some("component") => Some(PackageKind::Component),
        Some("interface") => Some(PackageKind::Interface),
        _ => None,
    }
}

/// Build a public [`KnownPackage`] from an `oci_repository` row, fetching its
/// tags and description.
async fn known_package_from_repo(
    db: &DatabaseConnection,
    repo: oci_repository::Model,
) -> anyhow::Result<super::known_package::KnownPackage> {
    let tags = fetch_repo_tags(db, repo.id).await?;
    let description = fetch_repo_description(db, repo.id).await?;
    Ok(super::known_package::KnownPackage {
        registry: repo.registry,
        repository: repo.repository,
        kind: parse_kind(repo.kind.as_deref()),
        description,
        tags,
        signature_tags: Vec::new(),
        attestation_tags: Vec::new(),
        last_seen_at: repo.updated_at.to_rfc3339(),
        created_at: repo.created_at.to_rfc3339(),
        wit_namespace: repo.wit_namespace,
        wit_name: repo.wit_name,
        dependencies: Vec::new(),
    })
}

/// Fetch a repository's tags from `oci_tag`, sorted by semver descending.
/// Only tags that parse as semver are returned (accepting an optional leading
/// `v` prefix, e.g. `1.2.3` or `v1.2.3`); tags like `latest` or `sha256-...`
/// are excluded.
async fn fetch_repo_tags(db: &DatabaseConnection, repo_id: i64) -> anyhow::Result<Vec<String>> {
    let rows = oci_tag::Entity::find()
        .filter(oci_tag::Column::OciRepositoryId.eq(repo_id))
        .all(db)
        .await?;
    let mut versioned: Vec<(semver::Version, String)> = rows
        .into_iter()
        .filter_map(|t| crate::manager::parse_tag_as_semver(&t.tag).map(|v| (v, t.tag)))
        .collect();
    versioned.sort_by(|(a, _), (b, _)| b.cmp(a));
    Ok(versioned.into_iter().map(|(_, t)| t).collect())
}

/// Fetch the first manifest description for a repository, if any.
async fn fetch_repo_description(
    db: &DatabaseConnection,
    repo_id: i64,
) -> anyhow::Result<Option<String>> {
    let row = oci_manifest::Entity::find()
        .filter(oci_manifest::Column::OciRepositoryId.eq(repo_id))
        .filter(oci_manifest::Column::OciDescription.is_not_null())
        .one(db)
        .await?;
    Ok(row.and_then(|m| m.oci_description))
}

impl Store {
    /// Build a [`PackageVersion`] from a manifest row + an optional tag.
    ///
    /// Populates annotations, layers, dependencies, referrers, `wit_text`,
    /// worlds, components, and cross-package `type_docs`.
    async fn build_package_version(
        &self,
        m: &oci_manifest::Model,
        tag: Option<String>,
    ) -> anyhow::Result<component_meta_registry_types::PackageVersion> {
        use component_meta_registry_types::{
            LayerInfo, OciAnnotations, PackageDependencyRef, PackageVersion, ReferrerSummary,
        };

        // Custom annotations (overflow keys).
        let custom_rows = oci_manifest_annotation::Entity::find()
            .filter(oci_manifest_annotation::Column::OciManifestId.eq(m.id))
            .order_by_asc(oci_manifest_annotation::Column::Key)
            .all(&self.db)
            .await?;
        let custom: Vec<component_meta_registry_types::AnnotationEntry> = custom_rows
            .into_iter()
            .map(|a| component_meta_registry_types::AnnotationEntry {
                key: a.key,
                value: a.value,
            })
            .collect();

        let has_annotations = m.oci_created.is_some()
            || m.oci_authors.is_some()
            || m.oci_url.is_some()
            || m.oci_documentation.is_some()
            || m.oci_source.is_some()
            || m.oci_version.is_some()
            || m.oci_revision.is_some()
            || m.oci_vendor.is_some()
            || m.oci_licenses.is_some()
            || m.oci_title.is_some()
            || m.oci_description.is_some()
            || !custom.is_empty();
        let annotations = if has_annotations {
            Some(OciAnnotations {
                created: m.oci_created.clone(),
                authors: m.oci_authors.clone(),
                url: m.oci_url.clone(),
                documentation: m.oci_documentation.clone(),
                source: m.oci_source.clone(),
                version: m.oci_version.clone(),
                revision: m.oci_revision.clone(),
                vendor: m.oci_vendor.clone(),
                licenses: m.oci_licenses.clone(),
                title: m.oci_title.clone(),
                description: m.oci_description.clone(),
                custom,
            })
        } else {
            None
        };

        // Dependencies via wit_package -> wit_package_dependency.
        let sql = "\
            SELECT DISTINCT wpd.declared_package AS declared_package, \
                   wpd.declared_version AS declared_version \
            FROM wit_package_dependency wpd \
            JOIN wit_package wp ON wpd.dependent_id = wp.id \
            WHERE wp.oci_manifest_id = ? \
            ORDER BY wpd.declared_package";
        let stmt =
            Statement::from_sql_and_values(self.db.get_database_backend(), sql, [m.id.into()]);
        #[derive(FromQueryResult)]
        struct DepRow {
            declared_package: String,
            declared_version: Option<String>,
        }
        let dep_rows = DepRow::find_by_statement(stmt).all(&self.db).await?;
        let dependencies: Vec<PackageDependencyRef> = dep_rows
            .into_iter()
            .map(|d| PackageDependencyRef {
                package: d.declared_package,
                version: d.declared_version,
            })
            .collect();

        // Referrers.
        let ref_rows = oci_referrer::Entity::find()
            .filter(oci_referrer::Column::SubjectManifestId.eq(m.id))
            .order_by_desc(oci_referrer::Column::CreatedAt)
            .all(&self.db)
            .await?;
        let mut referrers: Vec<ReferrerSummary> = Vec::with_capacity(ref_rows.len());
        for r in ref_rows {
            if let Some(rm) = oci_manifest::Entity::find_by_id(r.referrer_manifest_id)
                .one(&self.db)
                .await?
            {
                referrers.push(ReferrerSummary {
                    artifact_type: r.artifact_type,
                    digest: rm.digest,
                });
            }
        }

        // Layers (manifest descriptor + config + content layers).
        let mut layers: Vec<LayerInfo> = Vec::new();
        layers.push(LayerInfo {
            digest: m.digest.clone(),
            media_type: m.media_type.clone(),
            size_bytes: m.size_bytes,
        });
        if let Some(cfg_digest) = m.config_digest.as_deref() {
            layers.push(LayerInfo {
                digest: cfg_digest.to_string(),
                media_type: m.config_media_type.clone(),
                size_bytes: None,
            });
        }
        let content_layers = oci_layer::Entity::find()
            .filter(oci_layer::Column::OciManifestId.eq(m.id))
            .order_by_asc(oci_layer::Column::Position)
            .all(&self.db)
            .await?;
        for l in content_layers {
            layers.push(LayerInfo {
                digest: l.digest,
                media_type: l.media_type,
                size_bytes: l.size_bytes,
            });
        }

        // First WIT text for this manifest, if any.
        let wit_text = wit_package::Entity::find()
            .filter(wit_package::Column::OciManifestId.eq(m.id))
            .filter(wit_package::Column::WitText.is_not_null())
            .one(&self.db)
            .await?
            .and_then(|p| p.wit_text);

        // Worlds, components, and cross-package type docs (rich extraction).
        let worlds = self.get_wit_worlds_for_manifest(m.id).await?;
        let components = self
            .get_components_for_manifest(m.id, tag.as_deref())
            .await?;
        let type_docs = self.build_type_docs(&dependencies).await;

        Ok(PackageVersion {
            tag,
            digest: m.digest.clone(),
            size_bytes: m.size_bytes,
            created_at: m.oci_created.clone(),
            synced_at: Some(m.created_at.to_rfc3339()),
            annotations,
            worlds,
            components,
            dependencies,
            referrers,
            layers,
            wit_text,
            type_docs,
        })
    }

    /// Return all WIT worlds (with enriched imports/exports) for a manifest.
    async fn get_wit_worlds_for_manifest(
        &self,
        manifest_id: i64,
    ) -> anyhow::Result<Vec<component_meta_registry_types::WitWorldSummary>> {
        use component_meta_registry_types::WitWorldSummary;

        let pkg_ids: Vec<i64> = wit_package::Entity::find()
            .filter(wit_package::Column::OciManifestId.eq(manifest_id))
            .all(&self.db)
            .await?
            .into_iter()
            .map(|p| p.id)
            .collect();

        let mut worlds = Vec::new();
        for pkg_id in pkg_ids {
            let db_worlds = wit_world::Entity::find()
                .filter(wit_world::Column::WitPackageId.eq(pkg_id))
                .order_by_asc(wit_world::Column::Name)
                .all(&self.db)
                .await?;
            for w in db_worlds {
                let mut imports = self.get_world_imports(w.id).await?;
                let mut exports = self.get_world_exports(w.id).await?;
                self.enrich_iface_docs(&mut imports).await;
                self.enrich_iface_docs(&mut exports).await;
                worlds.push(WitWorldSummary {
                    name: w.name,
                    description: w.description,
                    imports,
                    exports,
                });
            }
        }
        Ok(worlds)
    }

    /// Return all imports for a WIT world.
    async fn get_world_imports(
        &self,
        world_id: i64,
    ) -> anyhow::Result<Vec<component_meta_registry_types::WitInterfaceRef>> {
        let rows = wit_world_import::Entity::find()
            .filter(wit_world_import::Column::WitWorldId.eq(world_id))
            .order_by_asc(wit_world_import::Column::DeclaredPackage)
            .all(&self.db)
            .await?;
        Ok(rows
            .into_iter()
            .map(|r| component_meta_registry_types::WitInterfaceRef {
                package: r.declared_package,
                interface: r.declared_interface,
                version: r.declared_version,
                docs: None,
                is_native: false,
            })
            .collect())
    }

    /// Return all exports for a WIT world.
    async fn get_world_exports(
        &self,
        world_id: i64,
    ) -> anyhow::Result<Vec<component_meta_registry_types::WitInterfaceRef>> {
        let rows = wit_world_export::Entity::find()
            .filter(wit_world_export::Column::WitWorldId.eq(world_id))
            .order_by_asc(wit_world_export::Column::DeclaredPackage)
            .all(&self.db)
            .await?;
        Ok(rows
            .into_iter()
            .map(|r| component_meta_registry_types::WitInterfaceRef {
                package: r.declared_package,
                interface: r.declared_interface,
                version: r.declared_version,
                docs: None,
                is_native: false,
            })
            .collect())
    }

    /// Return all Wasm components (with targets) found in the given manifest.
    ///
    /// `parent_version` is the package version tag this manifest is being
    /// served as; it is used to stamp a version onto native interface/world
    /// refs that lack one, so URLs to same-package items resolve correctly.
    async fn get_components_for_manifest(
        &self,
        manifest_id: i64,
        parent_version: Option<&str>,
    ) -> anyhow::Result<Vec<component_meta_registry_types::ComponentSummary>> {
        let components = wasm_component::Entity::find()
            .filter(wasm_component::Column::OciManifestId.eq(manifest_id))
            .order_by_asc(wasm_component::Column::Name)
            .all(&self.db)
            .await?;

        let parent_pkg = parent_pkg_for_manifest(&self.db, manifest_id).await;

        let mut result = Vec::new();
        for component in components {
            let summary = self
                .component_to_summary(component, parent_version, parent_pkg.as_deref())
                .await?;
            result.push(summary);
        }
        Ok(result)
    }

    /// Build a [`ComponentSummary`](component_meta_registry_types::ComponentSummary)
    /// from a stored `wasm_component` row plus its targets.
    async fn component_to_summary(
        &self,
        component: wasm_component::Model,
        parent_version: Option<&str>,
        parent_pkg: Option<&str>,
    ) -> anyhow::Result<component_meta_registry_types::ComponentSummary> {
        use component_meta_registry_types::{ComponentSummary, ComponentTargetRef};

        let targets = component_target::Entity::find()
            .filter(component_target::Column::WasmComponentId.eq(component.id))
            .all(&self.db)
            .await?;
        let target_refs: Vec<ComponentTargetRef> = targets
            .into_iter()
            .map(|t| ComponentTargetRef {
                version: if t.is_native_package && t.declared_version.is_none() {
                    parent_version.map(str::to_owned)
                } else {
                    t.declared_version
                },
                package: t.declared_package,
                world: t.declared_world,
                is_native: t.is_native_package,
            })
            .collect();

        // Try to deserialize the full ComponentSummary tree from JSON. If
        // available, merge in the DB-stored targets. Otherwise fall back to
        // a basic summary.
        let mut summary: ComponentSummary = component
            .producers_json
            .as_deref()
            .and_then(|json| serde_json::from_str(json).ok())
            .unwrap_or_else(|| ComponentSummary {
                name: component.name.clone(),
                description: component.description.clone(),
                targets: vec![],
                producers: vec![],
                kind: None,
                size_bytes: None,
                range_start: None,
                range_end: None,
                languages: vec![],
                children: vec![],
                source: None,
                homepage: None,
                licenses: None,
                authors: None,
                revision: None,
                component_version: None,
                bill_of_materials: vec![],
                imports: vec![],
                exports: vec![],
            });

        summary.targets = target_refs;

        // Enrich imports/exports with docs from stored WIT packages.
        self.enrich_iface_docs(&mut summary.imports).await;
        self.enrich_iface_docs(&mut summary.exports).await;

        // Mark interfaces as native when their package matches the parent
        // OCI repository's WIT package. Recurses into children so nested
        // components are flagged consistently. Native refs also inherit the
        // parent's version when they lack one, so generated URLs include the
        // version segment.
        if let Some(pkg) = parent_pkg {
            mark_native_iface_refs(&mut summary, pkg, parent_version);
        }

        Ok(summary)
    }

    /// Enrich `WitInterfaceRef` entries with docs from stored WIT packages.
    ///
    /// For each import/export that has a package name and version but no docs,
    /// look up the stored `wit_text` for that WIT package, parse it, and
    /// extract the interface's doc comment.
    async fn enrich_iface_docs(&self, refs: &mut [component_meta_registry_types::WitInterfaceRef]) {
        for iface_ref in refs.iter_mut() {
            if iface_ref.docs.is_some() {
                continue;
            }
            let Some(iface_name) = iface_ref.interface.clone() else {
                continue;
            };

            let Some(wit_text) = self
                .latest_wit_text(&iface_ref.package, iface_ref.version.as_deref())
                .await
            else {
                continue;
            };

            if let Ok(pkg) = wit_parser::UnresolvedPackageGroup::parse("lookup.wit", &wit_text) {
                for (_, iface) in &pkg.main.interfaces {
                    if iface.name.as_deref() == Some(iface_name.as_str())
                        && let Some(doc) = &iface.docs.contents
                    {
                        iface_ref.docs = Some(first_doc_sentence(doc));
                    }
                }
            }
        }
    }

    /// Look up the most relevant stored `wit_text` for a package, preferring
    /// an exact version match and falling back to the most recently inserted.
    async fn latest_wit_text(&self, package: &str, version: Option<&str>) -> Option<String> {
        let rows = wit_package::Entity::find()
            .filter(wit_package::Column::PackageName.eq(package))
            .filter(wit_package::Column::WitText.is_not_null())
            .order_by_desc(wit_package::Column::Id)
            .all(&self.db)
            .await
            .ok()?;

        let want = version.unwrap_or("");
        rows.iter()
            .find(|p| p.version.as_deref().unwrap_or("") == want)
            .or_else(|| rows.first())
            .and_then(|p| p.wit_text.clone())
    }

    /// Build a map of cross-package type documentation.
    ///
    /// For each dependency that has stored WIT text, parse it and extract
    /// docs for all types in all interfaces. Returns a map from
    /// `"package/interface/type"` → doc string.
    async fn build_type_docs(
        &self,
        deps: &[component_meta_registry_types::PackageDependencyRef],
    ) -> HashMap<String, String> {
        let mut result = HashMap::new();

        for dep in deps {
            let Some(wit_text) = self
                .latest_wit_text(&dep.package, dep.version.as_deref())
                .await
            else {
                continue;
            };
            let Ok(pkg) = wit_parser::UnresolvedPackageGroup::parse("dep.wit", &wit_text) else {
                continue;
            };

            for (_, iface) in &pkg.main.interfaces {
                let Some(iface_name) = &iface.name else {
                    continue;
                };
                for (type_name, type_id) in &iface.types {
                    if let Some(type_def) = pkg.main.types.get(*type_id)
                        && let Some(docs) = &type_def.docs.contents
                        && !docs.is_empty()
                    {
                        let key = format!("{}/{iface_name}/{type_name}", dep.package);
                        result.insert(key, first_doc_sentence(docs));
                    }
                }
            }
        }
        result
    }
}

impl Store {
    /// Search known packages joined through wit_world_{import|export}.
    async fn search_known_packages_by_iface(
        &self,
        interface: &str,
        offset: u32,
        limit: u32,
        is_import: bool,
    ) -> anyhow::Result<Vec<super::known_package::KnownPackage>> {
        let join_table = if is_import {
            "wit_world_import"
        } else {
            "wit_world_export"
        };
        let sql = format!(
            "SELECT DISTINCT r.id AS id, r.registry AS registry, r.repository AS repository, \
             r.created_at AS created_at, r.updated_at AS updated_at, \
             r.wit_namespace AS wit_namespace, r.wit_name AS wit_name, r.kind AS kind \
             FROM oci_repository r \
             JOIN oci_manifest m ON m.oci_repository_id = r.id \
             JOIN wit_package wp ON wp.oci_manifest_id = m.id \
             JOIN wit_world ww ON ww.wit_package_id = wp.id \
             JOIN {join_table} wi ON wi.wit_world_id = ww.id \
             WHERE wi.declared_package = ? \
             ORDER BY r.repository ASC, r.registry ASC \
             LIMIT ? OFFSET ?"
        );
        let backend = self.db.get_database_backend();
        let stmt = Statement::from_sql_and_values(
            backend,
            &sql,
            [
                interface.into(),
                i64::from(limit).into(),
                i64::from(offset).into(),
            ],
        );
        let rows = oci_repository::Model::find_by_statement(stmt)
            .all(&self.db)
            .await?;
        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            let pkg = known_package_from_repo(&self.db, r).await?;
            if !pkg.tags.is_empty() {
                out.push(pkg);
            }
        }
        Ok(out)
    }
}

/// Split `"namespace:name@version"` into `("namespace:name", Some("version"))`.
fn split_package_version(raw: &str) -> (&str, Option<&str>) {
    if let Some(at) = raw.rfind('@') {
        (&raw[..at], Some(&raw[at + 1..]))
    } else {
        (raw, None)
    }
}

/// Upsert a `wit_package` row keyed by (package_name, version, oci_layer_id).
async fn upsert_wit_package(
    db: &DatabaseConnection,
    package_name: &str,
    version: Option<&str>,
    description: Option<&str>,
    wit_text: Option<&str>,
    oci_manifest_id: Option<i64>,
    oci_layer_id: Option<i64>,
) -> anyhow::Result<i64> {
    // Match against the legacy unique index:
    //   (package_name, COALESCE(version,''), COALESCE(oci_layer_id, -1))
    if let Some(existing) = wit_package::Entity::find()
        .filter(wit_package::Column::PackageName.eq(package_name))
        .filter(match version {
            Some(v) => wit_package::Column::Version.eq(v),
            None => wit_package::Column::Version.is_null(),
        })
        .filter(match oci_layer_id {
            Some(id) => wit_package::Column::OciLayerId.eq(id),
            None => wit_package::Column::OciLayerId.is_null(),
        })
        .one(db)
        .await?
    {
        // Best-effort fill of any missing fields.
        let mut am: wit_package::ActiveModel = existing.clone().into();
        let mut changed = false;
        if existing.description.is_none() && description.is_some() {
            am.description = Set(description.map(str::to_owned));
            changed = true;
        }
        if existing.wit_text.is_none() && wit_text.is_some() {
            am.wit_text = Set(wit_text.map(str::to_owned));
            changed = true;
        }
        if existing.oci_manifest_id.is_none() && oci_manifest_id.is_some() {
            am.oci_manifest_id = Set(oci_manifest_id);
            changed = true;
        }
        if changed {
            am.update(db).await?;
        }
        return Ok(existing.id);
    }

    let am = wit_package::ActiveModel {
        package_name: Set(package_name.to_owned()),
        version: Set(version.map(str::to_owned)),
        description: Set(description.map(str::to_owned)),
        wit_text: Set(wit_text.map(str::to_owned)),
        oci_manifest_id: Set(oci_manifest_id),
        oci_layer_id: Set(oci_layer_id),
        ..Default::default()
    };
    let res = wit_package::Entity::insert(am).exec(db).await?;
    Ok(res.last_insert_id)
}

/// Insert a `wit_world` row (idempotent on (wit_package_id, name)).
async fn insert_wit_world(
    db: &DatabaseConnection,
    wit_package_id: i64,
    name: &str,
    description: Option<&str>,
) -> anyhow::Result<i64> {
    let am = wit_world::ActiveModel {
        wit_package_id: Set(wit_package_id),
        name: Set(name.to_owned()),
        description: Set(description.map(str::to_owned)),
        ..Default::default()
    };
    wit_world::Entity::insert(am)
        .on_conflict(
            OnConflict::columns([wit_world::Column::WitPackageId, wit_world::Column::Name])
                .do_nothing()
                .to_owned(),
        )
        .do_nothing()
        .exec(db)
        .await?;
    let row = wit_world::Entity::find()
        .filter(wit_world::Column::WitPackageId.eq(wit_package_id))
        .filter(wit_world::Column::Name.eq(name))
        .one(db)
        .await?
        .context("wit_world row missing after insert")?;
    Ok(row.id)
}

/// Insert a wit_world_import or wit_world_export row (idempotent).
async fn insert_wit_world_iface(
    db: &DatabaseConnection,
    wit_world_id: i64,
    declared_package: &str,
    declared_interface: Option<&str>,
    declared_version: Option<&str>,
    is_import: bool,
) -> anyhow::Result<()> {
    // The unique indexes on these tables are expression-based (use
    // COALESCE on nullable columns), so SQLite can't match them via
    // ON CONFLICT(columns). Do a manual find-then-insert instead.
    if is_import {
        let existing = wit_world_import::Entity::find()
            .filter(wit_world_import::Column::WitWorldId.eq(wit_world_id))
            .filter(wit_world_import::Column::DeclaredPackage.eq(declared_package))
            .filter(match declared_interface {
                Some(v) => wit_world_import::Column::DeclaredInterface.eq(v),
                None => wit_world_import::Column::DeclaredInterface.is_null(),
            })
            .filter(match declared_version {
                Some(v) => wit_world_import::Column::DeclaredVersion.eq(v),
                None => wit_world_import::Column::DeclaredVersion.is_null(),
            })
            .one(db)
            .await?;
        if existing.is_some() {
            return Ok(());
        }
        let am = wit_world_import::ActiveModel {
            wit_world_id: Set(wit_world_id),
            declared_package: Set(declared_package.to_owned()),
            declared_interface: Set(declared_interface.map(str::to_owned)),
            declared_version: Set(declared_version.map(str::to_owned)),
            ..Default::default()
        };
        wit_world_import::Entity::insert(am).exec(db).await?;
    } else {
        let existing = wit_world_export::Entity::find()
            .filter(wit_world_export::Column::WitWorldId.eq(wit_world_id))
            .filter(wit_world_export::Column::DeclaredPackage.eq(declared_package))
            .filter(match declared_interface {
                Some(v) => wit_world_export::Column::DeclaredInterface.eq(v),
                None => wit_world_export::Column::DeclaredInterface.is_null(),
            })
            .filter(match declared_version {
                Some(v) => wit_world_export::Column::DeclaredVersion.eq(v),
                None => wit_world_export::Column::DeclaredVersion.is_null(),
            })
            .one(db)
            .await?;
        if existing.is_some() {
            return Ok(());
        }
        let am = wit_world_export::ActiveModel {
            wit_world_id: Set(wit_world_id),
            declared_package: Set(declared_package.to_owned()),
            declared_interface: Set(declared_interface.map(str::to_owned)),
            declared_version: Set(declared_version.map(str::to_owned)),
            ..Default::default()
        };
        wit_world_export::Entity::insert(am).exec(db).await?;
    }
    Ok(())
}

/// Insert a wit_package_dependency row (idempotent).
async fn insert_wit_package_dependency(
    db: &DatabaseConnection,
    dependent_id: i64,
    declared_package: &str,
    declared_version: Option<&str>,
) -> anyhow::Result<()> {
    // Unique index on this table uses COALESCE(declared_version, ''),
    // which SQLite won't accept as an ON CONFLICT target. Find-then-insert.
    let existing = wit_package_dependency::Entity::find()
        .filter(wit_package_dependency::Column::DependentId.eq(dependent_id))
        .filter(wit_package_dependency::Column::DeclaredPackage.eq(declared_package))
        .filter(match declared_version {
            Some(v) => wit_package_dependency::Column::DeclaredVersion.eq(v),
            None => wit_package_dependency::Column::DeclaredVersion.is_null(),
        })
        .one(db)
        .await?;
    if existing.is_some() {
        return Ok(());
    }
    let am = wit_package_dependency::ActiveModel {
        dependent_id: Set(dependent_id),
        declared_package: Set(declared_package.to_owned()),
        declared_version: Set(declared_version.map(str::to_owned)),
        ..Default::default()
    };
    wit_package_dependency::Entity::insert(am).exec(db).await?;
    Ok(())
}

/// Insert a `wasm_component` row, returning its id.
///
/// The unique index on (`oci_manifest_id`, COALESCE(`oci_layer_id`, -1)) is
/// expression-based, so we do a manual find-then-insert.
async fn insert_wasm_component(
    db: &DatabaseConnection,
    oci_manifest_id: i64,
    oci_layer_id: Option<i64>,
    name: Option<&str>,
    description: Option<&str>,
    producers_json: Option<&str>,
) -> anyhow::Result<i64> {
    if let Some(existing) = wasm_component::Entity::find()
        .filter(wasm_component::Column::OciManifestId.eq(oci_manifest_id))
        .filter(match oci_layer_id {
            Some(id) => wasm_component::Column::OciLayerId.eq(id),
            None => wasm_component::Column::OciLayerId.is_null(),
        })
        .one(db)
        .await?
    {
        return Ok(existing.id);
    }
    let am = wasm_component::ActiveModel {
        oci_manifest_id: Set(oci_manifest_id),
        oci_layer_id: Set(oci_layer_id),
        name: Set(name.map(str::to_owned)),
        description: Set(description.map(str::to_owned)),
        producers_json: Set(producers_json.map(str::to_owned)),
        ..Default::default()
    };
    let res = wasm_component::Entity::insert(am).exec(db).await?;
    Ok(res.last_insert_id)
}

/// Insert a `component_target` row (idempotent).
///
/// The unique index uses COALESCE(`declared_version`, ''), so we do a manual
/// find-then-insert.
async fn insert_component_target(
    db: &DatabaseConnection,
    wasm_component_id: i64,
    declared_package: &str,
    declared_world: &str,
    declared_version: Option<&str>,
    wit_world_id: Option<i64>,
    is_native_package: bool,
) -> anyhow::Result<()> {
    let existing = component_target::Entity::find()
        .filter(component_target::Column::WasmComponentId.eq(wasm_component_id))
        .filter(component_target::Column::DeclaredPackage.eq(declared_package))
        .filter(component_target::Column::DeclaredWorld.eq(declared_world))
        .filter(match declared_version {
            Some(v) => component_target::Column::DeclaredVersion.eq(v),
            None => component_target::Column::DeclaredVersion.is_null(),
        })
        .one(db)
        .await?;
    if existing.is_some() {
        return Ok(());
    }
    let am = component_target::ActiveModel {
        wasm_component_id: Set(wasm_component_id),
        declared_package: Set(declared_package.to_owned()),
        declared_world: Set(declared_world.to_owned()),
        declared_version: Set(declared_version.map(str::to_owned)),
        wit_world_id: Set(wit_world_id),
        is_native_package: Set(is_native_package),
        ..Default::default()
    };
    component_target::Entity::insert(am).exec(db).await?;
    Ok(())
}

/// Return the parent package name (`"namespace:name"`) for a manifest, derived
/// from its OCI repository's WIT namespace/name.
async fn parent_pkg_for_manifest(db: &DatabaseConnection, manifest_id: i64) -> Option<String> {
    let manifest = oci_manifest::Entity::find_by_id(manifest_id)
        .one(db)
        .await
        .ok()
        .flatten()?;
    let repo = oci_repository::Entity::find_by_id(manifest.oci_repository_id)
        .one(db)
        .await
        .ok()
        .flatten()?;
    repo.wit_namespace
        .zip(repo.wit_name)
        .map(|(ns, n)| format!("{ns}:{n}"))
}

/// Mark every `WitInterfaceRef` in the component's imports/exports (and
/// recursively in children) as `is_native` when its package equals the parent
/// component's own package (`wit_namespace:wit_name`). Native refs inherit
/// `parent_version` when they lack their own.
fn mark_native_iface_refs(
    summary: &mut component_meta_registry_types::ComponentSummary,
    parent_pkg: &str,
    parent_version: Option<&str>,
) {
    for iface in summary.imports.iter_mut().chain(summary.exports.iter_mut()) {
        if iface.package == parent_pkg {
            iface.is_native = true;
            if iface.version.is_none() {
                iface.version = parent_version.map(str::to_owned);
            }
        }
    }
    for child in &mut summary.children {
        mark_native_iface_refs(child, parent_pkg, parent_version);
    }
}

/// Extract component-level metadata using `wasm-metadata`.
///
/// Returns `(name, description, metadata_json)` where `metadata_json` is the
/// full `ComponentSummary` tree serialized as JSON (producers, children, kind,
/// size, languages). Targets are not included — they come from the DB.
fn extract_component_metadata(
    wasm_bytes: &[u8],
) -> (Option<String>, Option<String>, Option<String>) {
    let Ok(payload) = wasm_metadata::Payload::from_binary(wasm_bytes) else {
        return (None, None, None);
    };

    // Decode WIT once for the full component — used by all children.
    let root_wit = wit_parser::decoding::decode(wasm_bytes).ok();

    let summary = payload_to_summary(&payload, wasm_bytes, root_wit.as_ref());
    let name = summary.name.clone();
    let description = summary.description.clone();
    let json = serde_json::to_string(&summary).ok();

    (name, description, json)
}

/// Convert a `wasm_metadata::Payload` tree into a `ComponentSummary` tree.
///
/// `parent_bytes` is the full wasm binary; child byte ranges are sliced from
/// it to extract WIT imports/exports via `wit-parser`. `root_wit` is the
/// decoded WIT from the full parent binary, used to resolve inner component
/// names back to WIT-qualified names.
fn payload_to_summary(
    payload: &wasm_metadata::Payload,
    parent_bytes: &[u8],
    root_wit: Option<&wit_parser::decoding::DecodedWasm>,
) -> component_meta_registry_types::ComponentSummary {
    use component_meta_registry_types::{BomEntry, ComponentSummary, ProducerEntry};

    let meta = payload.metadata();

    let (kind, children) = match payload {
        wasm_metadata::Payload::Component { children, .. } => (
            "component",
            children
                .iter()
                .map(|c| payload_to_summary(c, parent_bytes, root_wit))
                .collect(),
        ),
        wasm_metadata::Payload::Module(_) => ("module", vec![]),
    };

    let producers: Vec<ProducerEntry> = meta
        .producers
        .iter()
        .flat_map(|p| {
            p.iter().flat_map(|(field, pairs)| {
                pairs
                    .iter()
                    .map(|(tool, ver)| ProducerEntry {
                        field: field.clone(),
                        name: tool.clone(),
                        version: ver.clone(),
                    })
                    .collect::<Vec<_>>()
            })
        })
        .collect();

    let languages: Vec<String> = producers
        .iter()
        .filter(|e| e.field == "language")
        .map(|e| e.name.clone())
        .collect();

    #[allow(clippy::cast_sign_loss)]
    let size_bytes = {
        let r = &meta.range;
        let size = r.end.saturating_sub(r.start);
        if size > 0 { Some(size as u64) } else { None }
    };

    #[allow(clippy::cast_sign_loss)]
    let (range_start, range_end) = {
        let r = &meta.range;
        (Some(r.start as u64), Some(r.end as u64))
    };

    let bill_of_materials: Vec<BomEntry> = meta
        .dependencies
        .as_ref()
        .map(|deps| {
            deps.version_info()
                .packages
                .iter()
                .map(|p| BomEntry {
                    name: p.name.clone(),
                    version: p.version.to_string(),
                    source: Some(String::from(p.source.clone())),
                })
                .collect()
        })
        .unwrap_or_default();

    // Extract WIT imports/exports from this component's byte range.
    let (imports, exports) = extract_wit_imports_exports(parent_bytes, &meta.range, root_wit);

    ComponentSummary {
        name: meta.name.clone(),
        description: meta.description.as_ref().map(ToString::to_string),
        targets: vec![],
        producers,
        kind: Some(kind.to_string()),
        size_bytes,
        range_start,
        range_end,
        languages,
        children,
        source: meta.source.as_ref().map(ToString::to_string),
        homepage: meta.homepage.as_ref().map(ToString::to_string),
        licenses: meta.licenses.as_ref().map(ToString::to_string),
        authors: meta.authors.as_ref().map(ToString::to_string),
        revision: meta.revision.as_ref().map(ToString::to_string),
        component_version: meta.version.as_ref().map(ToString::to_string),
        bill_of_materials,
        imports,
        exports,
    }
}

/// Extract WIT imports and exports from a wasm binary at the given byte range.
///
/// First tries `wit-parser` for a full decode (works for self-contained
/// components). Falls back to using the root component's decoded WIT to
/// provide the correct WIT-qualified names. If neither works, uses
/// `wasmparser` to read raw component import/export names.
fn extract_wit_imports_exports(
    parent_bytes: &[u8],
    range: &std::ops::Range<usize>,
    root_wit: Option<&wit_parser::decoding::DecodedWasm>,
) -> (
    Vec<component_meta_registry_types::WitInterfaceRef>,
    Vec<component_meta_registry_types::WitInterfaceRef>,
) {
    let Some(bytes) = parent_bytes.get(range.start..range.end) else {
        return (vec![], vec![]);
    };

    // Try wit-parser on the child's own bytes (works for self-contained components).
    if let Ok(decoded) = wit_parser::decoding::decode(bytes) {
        let resolve = decoded.resolve();
        if let wit_parser::decoding::DecodedWasm::Component(_, world_id) = &decoded
            && let Some(world) = resolve.worlds.get(*world_id)
        {
            let imports = world
                .imports
                .keys()
                .filter_map(|k| world_key_to_iface_ref(resolve, k))
                .collect();
            let exports = world
                .exports
                .keys()
                .filter_map(|k| world_key_to_iface_ref(resolve, k))
                .collect();
            return (imports, exports);
        }
    }

    // For inner components that can't be decoded standalone, use the root
    // component's already-decoded WIT world. The root world's imports/exports
    // are the WIT-level view of what the inner component implements.
    if let Some(decoded) = root_wit {
        let resolve = decoded.resolve();
        if let wit_parser::decoding::DecodedWasm::Component(_, world_id) = decoded
            && let Some(world) = resolve.worlds.get(*world_id)
        {
            let imports = world
                .imports
                .keys()
                .filter_map(|k| world_key_to_iface_ref(resolve, k))
                .collect();
            let exports = world
                .exports
                .keys()
                .filter_map(|k| world_key_to_iface_ref(resolve, k))
                .collect();
            return (imports, exports);
        }
    }

    // Last resort: raw wasmparser names.
    extract_wit_imports_exports_wasmparser(bytes)
}

/// Use `wasmparser` to extract component import/export names from raw bytes.
///
/// This handles inner components that can't be decoded by `wit-parser` because
/// they reference types from the outer component scope.
fn extract_wit_imports_exports_wasmparser(
    bytes: &[u8],
) -> (
    Vec<component_meta_registry_types::WitInterfaceRef>,
    Vec<component_meta_registry_types::WitInterfaceRef>,
) {
    use wasmparser::{Parser, Payload};

    let mut imports = Vec::new();
    let mut exports = Vec::new();

    for payload in Parser::new(0).parse_all(bytes) {
        let Ok(payload) = payload else { continue };
        match payload {
            Payload::ComponentImportSection(reader) => {
                for import in reader {
                    let Ok(import) = import else { continue };
                    imports.push(parse_component_extern_name(import.name.name));
                }
            }
            Payload::ComponentExportSection(reader) => {
                for export in reader {
                    let Ok(export) = export else { continue };
                    exports.push(parse_component_extern_name(export.name.name));
                }
            }
            _ => {}
        }
    }

    (imports, exports)
}

/// Parse a component extern name like `"wasi:http/types@0.2.3"` into a
/// `WitInterfaceRef`.
fn parse_component_extern_name(name: &str) -> component_meta_registry_types::WitInterfaceRef {
    // Component extern names follow the pattern:
    //   "namespace:package/interface@version"
    //   "namespace:package@version"
    //   or just a plain name
    if !name.contains(':') {
        return component_meta_registry_types::WitInterfaceRef {
            package: name.to_string(),
            interface: None,
            version: None,
            docs: None,
            is_native: false,
        };
    }

    let (name_part, version) = match name.rsplit_once('@') {
        Some((n, v)) => (n, Some(v.to_string())),
        None => (name, None),
    };

    let (package, interface) = match name_part.split_once('/') {
        Some((pkg, iface)) => (pkg.to_string(), Some(iface.to_string())),
        None => (name_part.to_string(), None),
    };

    component_meta_registry_types::WitInterfaceRef {
        package,
        interface,
        version,
        docs: None,
        is_native: false,
    }
}

/// Convert a `wit_parser::WorldKey` to a `WitInterfaceRef`, including docs.
fn world_key_to_iface_ref(
    resolve: &wit_parser::Resolve,
    key: &wit_parser::WorldKey,
) -> Option<component_meta_registry_types::WitInterfaceRef> {
    match key {
        wit_parser::WorldKey::Name(name) => Some(component_meta_registry_types::WitInterfaceRef {
            package: name.clone(),
            interface: None,
            version: None,
            docs: None,
            is_native: false,
        }),
        wit_parser::WorldKey::Interface(id) => {
            let iface = resolve.interfaces.get(*id)?;
            let pkg_id = iface.package?;
            let pkg = resolve.packages.get(pkg_id)?;
            let docs = iface.docs.contents.as_deref().map(first_doc_sentence);
            Some(component_meta_registry_types::WitInterfaceRef {
                package: format!("{}:{}", pkg.name.namespace, pkg.name.name),
                interface: iface.name.clone(),
                version: pkg.name.version.as_ref().map(ToString::to_string),
                docs,
                is_native: false,
            })
        }
    }
}

/// Extract the first sentence from a doc comment.
fn first_doc_sentence(text: &str) -> String {
    text.split_once("\n\n").map_or_else(
        || text.trim().to_owned(),
        |(first, _)| first.trim().to_owned(),
    )
}

/// Best-effort: fill in `wit_world_import.resolved_package_id` for imports
/// belonging to the given wit_package_id.
async fn resolve_import_foreign_keys(
    db: &DatabaseConnection,
    wit_package_id: i64,
) -> anyhow::Result<()> {
    let sql = "\
        UPDATE wit_world_import \
        SET resolved_package_id = ( \
            SELECT wi.id FROM wit_package wi \
            WHERE wi.package_name = wit_world_import.declared_package \
              AND COALESCE(wi.version, '') = COALESCE(wit_world_import.declared_version, '') \
            LIMIT 1 \
        ) \
        WHERE wit_world_id IN (SELECT id FROM wit_world WHERE wit_package_id = ?) \
          AND resolved_package_id IS NULL";
    db.execute_raw(Statement::from_sql_and_values(
        db.get_database_backend(),
        sql,
        [wit_package_id.into()],
    ))
    .await?;
    Ok(())
}

async fn resolve_export_foreign_keys(
    db: &DatabaseConnection,
    wit_package_id: i64,
) -> anyhow::Result<()> {
    let sql = "\
        UPDATE wit_world_export \
        SET resolved_package_id = ( \
            SELECT wi.id FROM wit_package wi \
            WHERE wi.package_name = wit_world_export.declared_package \
              AND COALESCE(wi.version, '') = COALESCE(wit_world_export.declared_version, '') \
            LIMIT 1 \
        ) \
        WHERE wit_world_id IN (SELECT id FROM wit_world WHERE wit_package_id = ?) \
          AND resolved_package_id IS NULL";
    db.execute_raw(Statement::from_sql_and_values(
        db.get_database_backend(),
        sql,
        [wit_package_id.into()],
    ))
    .await?;
    Ok(())
}

async fn resolve_dependency_foreign_keys(
    db: &DatabaseConnection,
    wit_package_id: i64,
) -> anyhow::Result<()> {
    let sql = "\
        UPDATE wit_package_dependency \
        SET resolved_package_id = ( \
            SELECT wi.id FROM wit_package wi \
            WHERE wi.package_name = wit_package_dependency.declared_package \
              AND COALESCE(wi.version, '') = COALESCE(wit_package_dependency.declared_version, '') \
            LIMIT 1 \
        ) \
        WHERE dependent_id = ? \
          AND resolved_package_id IS NULL";
    db.execute_raw(Statement::from_sql_and_values(
        db.get_database_backend(),
        sql,
        [wit_package_id.into()],
    ))
    .await?;
    Ok(())
}

async fn resolve_component_target_foreign_keys(
    db: &DatabaseConnection,
    manifest_id: i64,
) -> anyhow::Result<()> {
    let sql = "\
        UPDATE component_target \
        SET wit_world_id = ( \
            SELECT ww.id FROM wit_world ww \
            JOIN wit_package wi ON ww.wit_package_id = wi.id \
            WHERE wi.package_name = component_target.declared_package \
              AND COALESCE(wi.version, '') = COALESCE(component_target.declared_version, '') \
              AND ww.name = component_target.declared_world \
            LIMIT 1 \
        ) \
        WHERE wasm_component_id IN ( \
            SELECT id FROM wasm_component WHERE oci_manifest_id = ? \
        ) \
        AND wit_world_id IS NULL";
    db.execute_raw(Statement::from_sql_and_values(
        db.get_database_backend(),
        sql,
        [manifest_id.into()],
    ))
    .await?;
    Ok(())
}

/// Well-known OCI annotation keys that map onto dedicated columns.
const WELL_KNOWN_ANNOTATIONS: &[(&str, &str)] = &[
    ("org.opencontainers.image.created", "oci_created"),
    ("org.opencontainers.image.authors", "oci_authors"),
    ("org.opencontainers.image.url", "oci_url"),
    (
        "org.opencontainers.image.documentation",
        "oci_documentation",
    ),
    ("org.opencontainers.image.source", "oci_source"),
    ("org.opencontainers.image.version", "oci_version"),
    ("org.opencontainers.image.revision", "oci_revision"),
    ("org.opencontainers.image.vendor", "oci_vendor"),
    ("org.opencontainers.image.licenses", "oci_licenses"),
    ("org.opencontainers.image.ref.name", "oci_ref_name"),
    ("org.opencontainers.image.title", "oci_title"),
    ("org.opencontainers.image.description", "oci_description"),
    ("org.opencontainers.image.base.digest", "oci_base_digest"),
    ("org.opencontainers.image.base.name", "oci_base_name"),
];

/// Upsert an `oci_manifest` row, COALESCE-preserving existing non-NULL
/// columns when the incoming data has gaps. Returns `(manifest_id, was_inserted)`.
#[allow(clippy::too_many_arguments)]
async fn upsert_oci_manifest(
    db: &DatabaseConnection,
    oci_repository_id: i64,
    digest: &str,
    media_type: Option<&str>,
    raw_json: Option<&str>,
    size_bytes: Option<i64>,
    artifact_type: Option<&str>,
    config_media_type: Option<&str>,
    config_digest: Option<&str>,
    annotations: &HashMap<String, String>,
) -> anyhow::Result<(i64, bool)> {
    let ann_key_to_col: HashMap<&str, &str> = WELL_KNOWN_ANNOTATIONS.iter().copied().collect();
    let mut well_known: HashMap<&str, &str> = HashMap::new();
    let mut extra: Vec<(String, String)> = Vec::new();
    for (k, v) in annotations {
        match ann_key_to_col.get(k.as_str()) {
            Some(&col) => {
                well_known.insert(col, v.as_str());
            }
            None => extra.push((k.clone(), v.clone())),
        }
    }

    let already_exists = oci_manifest::Entity::find()
        .filter(oci_manifest::Column::OciRepositoryId.eq(oci_repository_id))
        .filter(oci_manifest::Column::Digest.eq(digest))
        .one(db)
        .await?
        .is_some();

    let mut am = oci_manifest::ActiveModel {
        oci_repository_id: Set(oci_repository_id),
        digest: Set(digest.to_owned()),
        media_type: Set(media_type.map(str::to_owned)),
        raw_json: Set(raw_json.map(str::to_owned)),
        size_bytes: Set(size_bytes),
        artifact_type: Set(artifact_type.map(str::to_owned)),
        config_media_type: Set(config_media_type.map(str::to_owned)),
        config_digest: Set(config_digest.map(str::to_owned)),
        ..Default::default()
    };
    am.oci_created = Set(well_known.get("oci_created").map(|s| (*s).to_owned()));
    am.oci_authors = Set(well_known.get("oci_authors").map(|s| (*s).to_owned()));
    am.oci_url = Set(well_known.get("oci_url").map(|s| (*s).to_owned()));
    am.oci_documentation = Set(well_known.get("oci_documentation").map(|s| (*s).to_owned()));
    am.oci_source = Set(well_known.get("oci_source").map(|s| (*s).to_owned()));
    am.oci_version = Set(well_known.get("oci_version").map(|s| (*s).to_owned()));
    am.oci_revision = Set(well_known.get("oci_revision").map(|s| (*s).to_owned()));
    am.oci_vendor = Set(well_known.get("oci_vendor").map(|s| (*s).to_owned()));
    am.oci_licenses = Set(well_known.get("oci_licenses").map(|s| (*s).to_owned()));
    am.oci_ref_name = Set(well_known.get("oci_ref_name").map(|s| (*s).to_owned()));
    am.oci_title = Set(well_known.get("oci_title").map(|s| (*s).to_owned()));
    am.oci_description = Set(well_known.get("oci_description").map(|s| (*s).to_owned()));
    am.oci_base_digest = Set(well_known.get("oci_base_digest").map(|s| (*s).to_owned()));
    am.oci_base_name = Set(well_known.get("oci_base_name").map(|s| (*s).to_owned()));

    // Conflict resolution: keep existing non-NULL values when incoming is NULL.
    // SeaORM doesn't support COALESCE in `value()` against the row name without
    // raw expr, so we drop down to Expr::cust per column.
    let coalesce_cols = [
        "media_type",
        "raw_json",
        "size_bytes",
        "artifact_type",
        "config_media_type",
        "config_digest",
        "oci_created",
        "oci_authors",
        "oci_url",
        "oci_documentation",
        "oci_source",
        "oci_version",
        "oci_revision",
        "oci_vendor",
        "oci_licenses",
        "oci_ref_name",
        "oci_title",
        "oci_description",
        "oci_base_digest",
        "oci_base_name",
    ];
    let mut on_conflict = OnConflict::columns([
        oci_manifest::Column::OciRepositoryId,
        oci_manifest::Column::Digest,
    ])
    .clone();
    for col in coalesce_cols {
        let expr = format!("COALESCE(excluded.{col}, oci_manifest.{col})");
        on_conflict.value(sea_orm::sea_query::Alias::new(col), Expr::cust(expr));
    }
    oci_manifest::Entity::insert(am)
        .on_conflict(on_conflict)
        .exec(db)
        .await?;

    let row = oci_manifest::Entity::find()
        .filter(oci_manifest::Column::OciRepositoryId.eq(oci_repository_id))
        .filter(oci_manifest::Column::Digest.eq(digest))
        .one(db)
        .await?
        .context("oci_manifest row missing after upsert")?;
    let manifest_id = row.id;

    for (k, v) in extra {
        let am = oci_manifest_annotation::ActiveModel {
            oci_manifest_id: Set(manifest_id),
            key: Set(k.clone()),
            value: Set(v.clone()),
            ..Default::default()
        };
        oci_manifest_annotation::Entity::insert(am)
            .on_conflict(
                OnConflict::columns([
                    oci_manifest_annotation::Column::OciManifestId,
                    oci_manifest_annotation::Column::Key,
                ])
                .update_column(oci_manifest_annotation::Column::Value)
                .to_owned(),
            )
            .exec(db)
            .await?;
    }

    Ok((manifest_id, !already_exists))
}

/// Upsert an `oci_tag` row pointing at `manifest_digest`.
async fn upsert_oci_tag(
    db: &DatabaseConnection,
    oci_repository_id: i64,
    tag: &str,
    manifest_digest: &str,
) -> anyhow::Result<()> {
    let am = oci_tag::ActiveModel {
        oci_repository_id: Set(oci_repository_id),
        manifest_digest: Set(manifest_digest.to_owned()),
        tag: Set(tag.to_owned()),
        ..Default::default()
    };
    oci_tag::Entity::insert(am)
        .on_conflict(
            OnConflict::columns([oci_tag::Column::OciRepositoryId, oci_tag::Column::Tag])
                .update_column(oci_tag::Column::ManifestDigest)
                .to_owned(),
        )
        .exec(db)
        .await?;
    Ok(())
}

/// Insert (idempotently) an `oci_layer` row, returning its id.
async fn insert_oci_layer(
    db: &DatabaseConnection,
    oci_manifest_id: i64,
    digest: &str,
    media_type: Option<&str>,
    size_bytes: Option<i64>,
    position: i32,
) -> anyhow::Result<i64> {
    let am = oci_layer::ActiveModel {
        oci_manifest_id: Set(oci_manifest_id),
        digest: Set(digest.to_owned()),
        media_type: Set(media_type.map(str::to_owned)),
        size_bytes: Set(size_bytes),
        position: Set(i64::from(position)),
        ..Default::default()
    };
    oci_layer::Entity::insert(am)
        .on_conflict(
            OnConflict::columns([oci_layer::Column::OciManifestId, oci_layer::Column::Digest])
                .do_nothing()
                .to_owned(),
        )
        .do_nothing()
        .exec(db)
        .await?;
    let row = oci_layer::Entity::find()
        .filter(oci_layer::Column::OciManifestId.eq(oci_manifest_id))
        .filter(oci_layer::Column::Digest.eq(digest))
        .one(db)
        .await?
        .context("oci_layer row missing after insert")?;
    Ok(row.id)
}

/// Insert a layer-level annotation (idempotent).
async fn insert_oci_layer_annotation(
    db: &DatabaseConnection,
    oci_layer_id: i64,
    key: &str,
    value: &str,
) -> anyhow::Result<()> {
    let am = oci_layer_annotation::ActiveModel {
        oci_layer_id: Set(oci_layer_id),
        key: Set(key.to_owned()),
        value: Set(value.to_owned()),
        ..Default::default()
    };
    oci_layer_annotation::Entity::insert(am)
        .on_conflict(
            OnConflict::columns([
                oci_layer_annotation::Column::OciLayerId,
                oci_layer_annotation::Column::Key,
            ])
            .update_column(oci_layer_annotation::Column::Value)
            .to_owned(),
        )
        .exec(db)
        .await?;
    Ok(())
}

/// Insert an `oci_referrer` edge (idempotent).
async fn insert_oci_referrer(
    db: &DatabaseConnection,
    subject_manifest_id: i64,
    referrer_manifest_id: i64,
    artifact_type: &str,
) -> anyhow::Result<()> {
    let am = oci_referrer::ActiveModel {
        subject_manifest_id: Set(subject_manifest_id),
        referrer_manifest_id: Set(referrer_manifest_id),
        artifact_type: Set(artifact_type.to_owned()),
        ..Default::default()
    };
    oci_referrer::Entity::insert(am)
        .on_conflict(
            OnConflict::columns([
                oci_referrer::Column::SubjectManifestId,
                oci_referrer::Column::ReferrerManifestId,
            ])
            .do_nothing()
            .to_owned(),
        )
        .do_nothing()
        .exec(db)
        .await?;
    Ok(())
}

/// Upsert an `oci_repository` row, optionally filling in WIT metadata and
/// `kind`. Returns the row id.
async fn upsert_oci_repository_full(
    db: &DatabaseConnection,
    registry: &str,
    repository: &str,
    wit_namespace: Option<&str>,
    wit_name: Option<&str>,
    kind: Option<&str>,
) -> anyhow::Result<i64> {
    let am = oci_repository::ActiveModel {
        registry: Set(registry.to_owned()),
        repository: Set(repository.to_owned()),
        wit_namespace: Set(wit_namespace.map(str::to_owned)),
        wit_name: Set(wit_name.map(str::to_owned)),
        kind: Set(kind.map(str::to_owned)),
        ..Default::default()
    };
    // ON CONFLICT(registry, repository) DO UPDATE SET wit_*/kind = COALESCE(excluded.*, table.*)
    oci_repository::Entity::insert(am)
        .on_conflict(
            OnConflict::columns([
                oci_repository::Column::Registry,
                oci_repository::Column::Repository,
            ])
            .value(
                oci_repository::Column::WitNamespace,
                Expr::cust("COALESCE(excluded.wit_namespace, oci_repository.wit_namespace)"),
            )
            .value(
                oci_repository::Column::WitName,
                Expr::cust("COALESCE(excluded.wit_name, oci_repository.wit_name)"),
            )
            .value(
                oci_repository::Column::Kind,
                Expr::cust("COALESCE(excluded.kind, oci_repository.kind)"),
            )
            .to_owned(),
        )
        .exec(db)
        .await?;
    let row = oci_repository::Entity::find()
        .filter(oci_repository::Column::Registry.eq(registry))
        .filter(oci_repository::Column::Repository.eq(repository))
        .one(db)
        .await?
        .context("oci_repository row missing after upsert")?;
    Ok(row.id)
}

/// Convert a `fetch_queue` row into the public `QueueTask` shape.
fn into_queue_task(row: fetch_queue::Model) -> component_meta_registry_types::QueueTask {
    let task_str = match row.task {
        fetch_queue::FetchTask::Pull => "pull",
        fetch_queue::FetchTask::Reindex => "reindex",
    };
    let status_str = match row.status {
        fetch_queue::FetchStatus::Pending => "pending",
        fetch_queue::FetchStatus::InProgress => "in_progress",
        fetch_queue::FetchStatus::Completed => "completed",
        fetch_queue::FetchStatus::Failed => "failed",
    };
    component_meta_registry_types::QueueTask {
        registry: row.registry,
        repository: row.repository,
        tag: row.tag,
        task: task_str.to_owned(),
        status: status_str.to_owned(),
        priority: row.priority,
        attempts: row.attempts,
        max_attempts: row.max_attempts,
        last_error: row.last_error,
        created_at: row.created_at.to_rfc3339(),
        updated_at: row.updated_at.to_rfc3339(),
    }
}

// -- Store ---------------------------------------------------------------

/// Handle to the metadata database used by the package manager.
#[derive(Debug)]
pub(crate) struct Store {
    pub(crate) state_info: StateInfo,
    db: DatabaseConnection,
}

impl Store {
    /// Open the store in the platform's default data directory.
    pub(crate) async fn open() -> anyhow::Result<Self> {
        let data_dir = dirs::data_local_dir()
            .context("No local data dir known for the current OS")?
            .join("wasm");
        let config_file = crate::xdg_config_home()
            .context("Could not determine config directory (set $XDG_CONFIG_HOME or $HOME)")?
            .join("wasm")
            .join("config.toml");
        Self::open_inner(data_dir, config_file).await
    }

    /// Open the store at a custom data directory.
    pub(crate) async fn open_at(data_dir: impl Into<std::path::PathBuf>) -> anyhow::Result<Self> {
        let data_dir = data_dir.into();
        let config_file = data_dir.join("config.toml");
        Self::open_inner(data_dir, config_file).await
    }

    /// Shared implementation of `open` / `open_at`.
    async fn open_inner(
        data_dir: std::path::PathBuf,
        config_file: std::path::PathBuf,
    ) -> anyhow::Result<Self> {
        let store_dir = data_dir.join("store");
        let db_dir = data_dir.join("db");
        // Bumped from `metadata.db3` to `metadata-v2.db3` as part of the SeaORM
        // port — schema is incompatible with old rusqlite-managed bookkeeping.
        let metadata_file = db_dir.join("metadata-v2.db3");

        let a = tokio::fs::create_dir_all(&data_dir);
        let b = tokio::fs::create_dir_all(&store_dir);
        let c = tokio::fs::create_dir_all(&db_dir);
        let _ = (a, b, c)
            .try_join()
            .await
            .context("Could not create config directories on disk")?;

        let cfg = super::db_config::DbConfig::from_env(&metadata_file)?;
        let db = Database::connect(cfg.to_connect_options())
            .await
            .with_context(|| format!("failed to connect to database at {}", cfg.redacted_url()))?;
        apply_sqlite_pragmas(&db).await?;

        match db.get_database_backend() {
            DbBackend::Sqlite => {
                // SQLite is single-user; auto-migrate matches the legacy
                // rusqlite behaviour.
                Migrator::up(&db, None)
                    .await
                    .context("failed to run database migrations")?;
            }
            DbBackend::Postgres => {
                run_postgres_migrations_with_advisory_lock(&cfg).await?;
            }
            other => {
                anyhow::bail!(
                    "unsupported database backend {:?} for {} (expected sqlite:// or postgres://)",
                    other,
                    cfg.redacted_url()
                );
            }
        }

        let migration_info = Migrations::snapshot(&db).await;
        let store_size = dir_size(&store_dir).await;
        // For SQLite we report the on-disk file size; for Postgres we have
        // no such number locally, so leave it at 0.
        let metadata_size = match cfg.backend {
            super::db_config::Backend::Sqlite => tokio::fs::metadata(&metadata_file)
                .await
                .map_or(0, |m| m.len()),
            super::db_config::Backend::Postgres => 0,
        };
        let state_info = StateInfo::new_at(
            data_dir,
            config_file,
            &migration_info,
            store_size,
            metadata_size,
        );

        Ok(Self { state_info, db })
    }

    /// Build a Store backed by an in-memory SQLite database with all
    /// migrations applied. Used by tests.
    #[cfg(test)]
    pub(crate) async fn open_in_memory() -> anyhow::Result<Self> {
        let mut opts = ConnectOptions::new("sqlite::memory:");
        opts.sqlx_logging(false);
        let db = Database::connect(opts).await?;
        apply_sqlite_pragmas(&db).await?;
        Migrator::up(&db, None).await?;

        let tmp = tempfile::tempdir()?.keep();
        let migration_info = Migrations::snapshot(&db).await;
        let state_info =
            StateInfo::new_at(tmp.clone(), tmp.join("config.toml"), &migration_info, 0, 0);
        Ok(Self { state_info, db })
    }

    /// Test-only accessor for the underlying SeaORM database connection.
    ///
    /// Used by sibling test modules (e.g. the resolver smoke tests) that
    /// need to seed entity rows directly without going through the
    /// still-stubbed high-level insert APIs.
    #[cfg(test)]
    pub(crate) fn db(&self) -> &DatabaseConnection {
        &self.db
    }

    // ---- TODO: methods below are stubbed; will be filled in incrementally.

    /// Extract WIT metadata from wasm bytes and persist the resulting
    /// `wit_package`/`wit_world`/`wasm_component`/`component_target` rows.
    ///
    /// Best-effort: errors are logged and the call returns. Failures don't
    /// roll back the surrounding insert.
    async fn try_extract_wit_package(
        &self,
        manifest_id: i64,
        layer_id: Option<i64>,
        wasm_bytes: &[u8],
    ) {
        let Some(metadata) = extract_wit_metadata(wasm_bytes) else {
            return;
        };
        let Some(raw_name) = metadata.package_name.as_deref() else {
            return;
        };
        let (package_name, version) = split_package_version(raw_name);

        let wit_package_id = match upsert_wit_package(
            &self.db,
            package_name,
            version,
            None,
            Some(&metadata.wit_text),
            Some(manifest_id),
            layer_id,
        )
        .await
        {
            Ok(id) => id,
            Err(e) => {
                warn!(
                    "Failed to insert WIT package for manifest {}: {}",
                    manifest_id, e
                );
                return;
            }
        };

        let mut world_ids: HashMap<String, i64> = HashMap::new();
        for world in &metadata.worlds {
            let wit_world_id =
                match insert_wit_world(&self.db, wit_package_id, &world.name, None).await {
                    Ok(id) => id,
                    Err(e) => {
                        warn!("Failed to insert WIT world '{}': {}", world.name, e);
                        continue;
                    }
                };
            world_ids.insert(world.name.clone(), wit_world_id);

            for item in &world.imports {
                if let Err(e) = insert_wit_world_iface(
                    &self.db,
                    wit_world_id,
                    &item.package,
                    item.interface.as_deref(),
                    item.version.as_deref(),
                    /* is_import */ true,
                )
                .await
                {
                    warn!("Failed to insert WIT world import: {}", e);
                }
            }
            for item in &world.exports {
                if let Err(e) = insert_wit_world_iface(
                    &self.db,
                    wit_world_id,
                    &item.package,
                    item.interface.as_deref(),
                    item.version.as_deref(),
                    /* is_import */ false,
                )
                .await
                {
                    warn!("Failed to insert WIT world export: {}", e);
                }
            }
        }

        for dep in &metadata.dependencies {
            if let Err(e) = insert_wit_package_dependency(
                &self.db,
                wit_package_id,
                &dep.package,
                dep.version.as_deref(),
            )
            .await
            {
                warn!("Failed to insert WIT package dependency: {}", e);
            }
        }

        // For compiled components, create wasm_component and component_target
        // rows so the detail view can render core .wasm modules and targets.
        if metadata.is_component {
            self.extract_wasm_component(
                manifest_id,
                layer_id,
                wasm_bytes,
                package_name,
                version,
                &world_ids,
                &metadata,
            )
            .await;
        }

        // Best-effort cross-package FK resolution.
        let _ = resolve_import_foreign_keys(&self.db, wit_package_id).await;
        let _ = resolve_export_foreign_keys(&self.db, wit_package_id).await;
        let _ = resolve_dependency_foreign_keys(&self.db, wit_package_id).await;
        let _ = resolve_component_target_foreign_keys(&self.db, manifest_id).await;
    }

    /// Extract and persist a `wasm_component` row plus its `component_target`
    /// rows for a compiled component.
    #[allow(clippy::too_many_arguments)]
    async fn extract_wasm_component(
        &self,
        manifest_id: i64,
        layer_id: Option<i64>,
        wasm_bytes: &[u8],
        package_name: &str,
        version: Option<&str>,
        world_ids: &HashMap<String, i64>,
        metadata: &WitMetadata,
    ) {
        let (comp_name, comp_desc, producers_json) = extract_component_metadata(wasm_bytes);

        let component_id = match insert_wasm_component(
            &self.db,
            manifest_id,
            layer_id,
            comp_name.as_deref(),
            comp_desc.as_deref(),
            producers_json.as_deref(),
        )
        .await
        {
            Ok(id) => id,
            Err(e) => {
                warn!("Failed to insert wasm_component: {}", e);
                return;
            }
        };

        let parent_package = parent_pkg_for_manifest(&self.db, manifest_id).await;
        for world in &metadata.worlds {
            let wit_world_id = world_ids.get(&world.name).copied();
            let is_native = parent_package.as_deref() == Some(package_name);
            if let Err(e) = insert_component_target(
                &self.db,
                component_id,
                package_name,
                &world.name,
                version,
                wit_world_id,
                is_native,
            )
            .await
            {
                warn!("Failed to insert component_target: {}", e);
            }
        }
    }

    pub(crate) async fn insert(
        &self,
        reference: &Reference,
        image: ImageData,
    ) -> anyhow::Result<(
        InsertResult,
        Option<String>,
        Option<OciImageManifest>,
        Option<i64>,
    )> {
        let digest = reference.digest().map(str::to_owned).or(image.digest);
        let manifest_str = serde_json::to_string(&image.manifest)?;
        let size_on_disk: u64 = image
            .layers
            .iter()
            // `usize` byte length always fits in `u64` on supported platforms.
            .map(|l| u64::try_from(l.data.len()).unwrap_or(u64::MAX))
            .sum();

        let repo_id = upsert_oci_repository_full(
            &self.db,
            reference.registry(),
            reference.repository(),
            None,
            None,
            None,
        )
        .await?;

        let annotations: HashMap<String, String> = image
            .manifest
            .as_ref()
            .and_then(|m| m.annotations.clone())
            .unwrap_or_default()
            .into_iter()
            .collect();

        let (manifest_id, was_inserted) = upsert_oci_manifest(
            &self.db,
            repo_id,
            digest.as_deref().unwrap_or("unknown"),
            image
                .manifest
                .as_ref()
                .and_then(|m| m.media_type.as_deref()),
            Some(&manifest_str),
            Some(crate::convert::size_to_i64(size_on_disk)?),
            image
                .manifest
                .as_ref()
                .and_then(|m| m.artifact_type.as_deref()),
            image
                .manifest
                .as_ref()
                .map(|m| m.config.media_type.as_str()),
            image.manifest.as_ref().map(|m| m.config.digest.as_str()),
            &annotations,
        )
        .await?;

        let result = if was_inserted {
            InsertResult::Inserted
        } else {
            InsertResult::AlreadyExists
        };

        if let Some(tag) = reference.tag()
            && let Some(d) = digest.as_deref()
        {
            upsert_oci_tag(&self.db, repo_id, tag, d).await?;
        }

        let manifest = image.manifest.clone();

        // Store layers when the manifest is newly inserted, or when the
        // existing manifest has no layers yet (e.g. created as a referrer
        // placeholder).
        let needs_layers = was_inserted
            || oci_layer::Entity::find()
                .filter(oci_layer::Column::OciManifestId.eq(manifest_id))
                .count(&self.db)
                .await?
                == 0;

        if needs_layers && let Some(ref manifest) = image.manifest {
            for (idx, layer) in image.layers.iter().enumerate() {
                let cache = self.state_info.store_dir();
                let fallback_key = reference.whole().clone();
                let layer_digest = manifest
                    .layers
                    .get(idx)
                    .map_or(fallback_key.as_str(), |l| l.digest.as_str());
                let layer_media_type = manifest.layers.get(idx).map(|l| l.media_type.as_str());
                let layer_size = manifest.layers.get(idx).map(|l| l.size);
                let data = &layer.data;
                let _integrity = cacache::write(&cache, layer_digest, data).await?;

                let layer_id = insert_oci_layer(
                    &self.db,
                    manifest_id,
                    layer_digest,
                    layer_media_type,
                    layer_size.map(|s| s.max(0)),
                    crate::convert::index_to_i32(idx)?,
                )
                .await?;

                if let Some(descriptor) = manifest.layers.get(idx)
                    && let Some(ref annotations) = descriptor.annotations
                {
                    for (key, value) in annotations {
                        if let Err(e) =
                            insert_oci_layer_annotation(&self.db, layer_id, key, value).await
                        {
                            warn!("Failed to insert layer annotation '{}': {}", key, e);
                        }
                    }
                }

                self.try_extract_wit_package(manifest_id, Some(layer_id), data)
                    .await;
            }
        }
        let manifest_id_opt = if result == InsertResult::Inserted {
            Some(manifest_id)
        } else {
            None
        };
        Ok((result, digest, manifest, manifest_id_opt))
    }

    pub(crate) async fn insert_metadata(
        &self,
        reference: &Reference,
        digest: Option<&str>,
        manifest: &OciImageManifest,
        size_on_disk: u64,
    ) -> anyhow::Result<(InsertResult, Option<i64>)> {
        let manifest_str = serde_json::to_string(manifest)?;
        let repo_id = upsert_oci_repository_full(
            &self.db,
            reference.registry(),
            reference.repository(),
            None,
            None,
            None,
        )
        .await?;

        let annotations: HashMap<String, String> = manifest
            .annotations
            .clone()
            .unwrap_or_default()
            .into_iter()
            .collect();

        let (manifest_id, was_inserted) = upsert_oci_manifest(
            &self.db,
            repo_id,
            digest.unwrap_or("unknown"),
            manifest.media_type.as_deref(),
            Some(&manifest_str),
            Some(crate::convert::size_to_i64(size_on_disk)?),
            manifest.artifact_type.as_deref(),
            Some(manifest.config.media_type.as_str()),
            Some(manifest.config.digest.as_str()),
            &annotations,
        )
        .await?;

        let result = if was_inserted {
            InsertResult::Inserted
        } else {
            InsertResult::AlreadyExists
        };

        if let Some(tag) = reference.tag()
            && let Some(d) = digest
        {
            upsert_oci_tag(&self.db, repo_id, tag, d).await?;
        }

        if result == InsertResult::Inserted {
            Ok((result, Some(manifest_id)))
        } else {
            Ok((result, None))
        }
    }

    pub(crate) async fn insert_layer(
        &self,
        layer_digest: &str,
        data: &[u8],
        manifest_id: Option<i64>,
        media_type: Option<&str>,
        position: i32,
        layer_annotations: Option<&BTreeMap<String, String>>,
    ) -> anyhow::Result<()> {
        let cache = self.state_info.store_dir();
        let _integrity = cacache::write(&cache, layer_digest, data).await?;

        let Some(manifest_id) = manifest_id else {
            return Ok(());
        };

        let layer_id = insert_oci_layer(
            &self.db,
            manifest_id,
            layer_digest,
            media_type,
            Some(crate::convert::len_to_i64(data.len())?),
            position,
        )
        .await?;

        if let Some(annotations) = layer_annotations {
            for (key, value) in annotations {
                if let Err(e) = insert_oci_layer_annotation(&self.db, layer_id, key, value).await {
                    warn!("Failed to insert layer annotation '{}': {}", key, e);
                }
            }
        }

        self.try_extract_wit_package(manifest_id, Some(layer_id), data)
            .await;
        Ok(())
    }

    pub(crate) async fn store_referrer(
        &self,
        subject_manifest_id: i64,
        registry: &str,
        repository: &str,
        referrer_digest: &str,
        artifact_type: &str,
    ) -> anyhow::Result<()> {
        let repo_id =
            upsert_oci_repository_full(&self.db, registry, repository, None, None, None).await?;
        let (referrer_manifest_id, _) = upsert_oci_manifest(
            &self.db,
            repo_id,
            referrer_digest,
            None,
            None,
            None,
            Some(artifact_type),
            None,
            None,
            &HashMap::new(),
        )
        .await?;
        insert_oci_referrer(
            &self.db,
            subject_manifest_id,
            referrer_manifest_id,
            artifact_type,
        )
        .await?;
        Ok(())
    }

    pub(crate) async fn reindex_wit_packages(&self) -> anyhow::Result<u64> {
        // Re-derive WIT metadata for every wit_package that has a cached
        // layer. The legacy implementation considered two sources:
        //   1. wit_package rows whose oci_layer_id is set
        //   2. wit_package rows with only oci_manifest_id (legacy data),
        //      resolved via the manifest's first wasm layer.
        let sql = "\
            SELECT wp.id AS wit_id, wp.oci_manifest_id AS manifest_id, \
                   ol.id AS layer_id, ol.digest AS digest \
            FROM wit_package wp \
            JOIN oci_layer ol ON ol.id = wp.oci_layer_id \
            UNION ALL \
            SELECT wp.id, wp.oci_manifest_id, ol.id, ol.digest \
            FROM wit_package wp \
            JOIN oci_layer ol ON ol.oci_manifest_id = wp.oci_manifest_id \
            WHERE wp.oci_layer_id IS NULL \
              AND wp.oci_manifest_id IS NOT NULL \
              AND ol.position = ( \
                  SELECT MIN(ol2.position) FROM oci_layer ol2 \
                  WHERE ol2.oci_manifest_id = wp.oci_manifest_id \
              )";
        #[derive(FromQueryResult)]
        struct Row {
            wit_id: i64,
            manifest_id: Option<i64>,
            layer_id: i64,
            digest: String,
        }
        let rows =
            Row::find_by_statement(Statement::from_string(self.db.get_database_backend(), sql))
                .all(&self.db)
                .await?;

        let store_dir = self.state_info.store_dir().to_path_buf();
        let mut reindexed = 0u64;
        for r in rows {
            let Some(manifest_id) = r.manifest_id else {
                continue;
            };
            let bytes = match cacache::read(&store_dir, &r.digest).await {
                Ok(b) => b,
                Err(e) => {
                    warn!("reindex: failed to read layer {} from cache: {e}", r.digest);
                    continue;
                }
            };
            // Delete and re-extract under a transaction.
            let txn = self.db.begin().await?;
            if let Err(e) = wit_package::Entity::delete_by_id(r.wit_id).exec(&txn).await {
                warn!("reindex: failed to delete wit_package {}: {e}", r.wit_id);
                let _ = txn.rollback().await;
                continue;
            }
            if let Err(e) = wasm_component::Entity::delete_many()
                .filter(wasm_component::Column::OciManifestId.eq(manifest_id))
                .exec(&txn)
                .await
            {
                warn!("reindex: failed to delete wasm_component for manifest {manifest_id}: {e}");
                let _ = txn.rollback().await;
                continue;
            }
            txn.commit().await?;
            self.try_extract_wit_package(manifest_id, Some(r.layer_id), &bytes)
                .await;
            reindexed += 1;
        }
        Ok(reindexed)
    }

    pub(crate) async fn list_all(&self) -> anyhow::Result<Vec<RawImageEntry>> {
        // Return all manifests with cached raw_json, joined to their repositories
        // and to a representative tag. Maps to the legacy SQL:
        //   SELECT m.id, r.registry, r.repository, m.digest, m.raw_json,
        //          m.size_bytes,
        //          (SELECT t.tag FROM oci_tag t
        //           WHERE t.oci_repository_id = r.id AND t.manifest_digest = m.digest
        //           ORDER BY t.updated_at DESC LIMIT 1) AS tag
        //   FROM oci_manifest m JOIN oci_repository r ON m.oci_repository_id = r.id
        //   WHERE m.raw_json IS NOT NULL
        //   ORDER BY r.repository ASC, r.registry ASC
        let manifests = oci_manifest::Entity::find()
            .filter(oci_manifest::Column::RawJson.is_not_null())
            .find_also_related(oci_repository::Entity)
            .all(&self.db)
            .await?;

        let mut entries: Vec<RawImageEntry> = Vec::with_capacity(manifests.len());
        for (m, repo_opt) in manifests {
            let Some(repo) = repo_opt else { continue };
            let Some(json) = m.raw_json.as_deref() else {
                continue;
            };
            let manifest = match serde_json::from_str::<OciImageManifest>(json) {
                Ok(m) => m,
                Err(e) => {
                    warn!(
                        "Skipping manifest {} in {}/{}: {}",
                        m.digest, repo.registry, repo.repository, e
                    );
                    continue;
                }
            };
            // Most-recent tag pointing at this manifest.
            let tag = oci_tag::Entity::find()
                .filter(oci_tag::Column::OciRepositoryId.eq(repo.id))
                .filter(oci_tag::Column::ManifestDigest.eq(&m.digest))
                .order_by_desc(oci_tag::Column::UpdatedAt)
                .one(&self.db)
                .await?
                .map(|t| t.tag);

            entries.push(RawImageEntry {
                id: m.id,
                ref_registry: repo.registry,
                ref_repository: repo.repository,
                ref_mirror_registry: None,
                ref_tag: tag,
                ref_digest: Some(m.digest),
                manifest,
                // `size_bytes` is stored as a non-negative count, so reading it
                // back as `u64` cannot realistically truncate; default to 0.
                size_on_disk: u64::try_from(m.size_bytes.unwrap_or(0)).unwrap_or(0),
            });
        }
        // Sort by repository, then registry (matches legacy ORDER BY).
        entries.sort_by(|a, b| {
            a.ref_repository
                .cmp(&b.ref_repository)
                .then_with(|| a.ref_registry.cmp(&b.ref_registry))
        });
        Ok(entries)
    }

    pub(crate) async fn delete(&self, reference: &Reference) -> anyhow::Result<bool> {
        // Find the repository.
        let Some(repo) = oci_repository::Entity::find()
            .filter(oci_repository::Column::Registry.eq(reference.registry()))
            .filter(oci_repository::Column::Repository.eq(reference.repository()))
            .one(&self.db)
            .await?
        else {
            return Ok(false);
        };
        let repo_id = repo.id;

        // Resolve which manifests to delete based on the reference shape.
        let manifests_to_delete: Vec<oci_manifest::Model> =
            match (reference.tag(), reference.digest()) {
                (Some(tag), Some(digest)) => {
                    if let Some(t) = oci_tag::Entity::find()
                        .filter(oci_tag::Column::OciRepositoryId.eq(repo_id))
                        .filter(oci_tag::Column::Tag.eq(tag))
                        .one(&self.db)
                        .await?
                        && t.manifest_digest == digest
                    {
                        oci_manifest::Entity::find()
                            .filter(oci_manifest::Column::OciRepositoryId.eq(repo_id))
                            .filter(oci_manifest::Column::Digest.eq(digest))
                            .all(&self.db)
                            .await?
                    } else {
                        Vec::new()
                    }
                }
                (Some(tag), None) => {
                    if let Some(t) = oci_tag::Entity::find()
                        .filter(oci_tag::Column::OciRepositoryId.eq(repo_id))
                        .filter(oci_tag::Column::Tag.eq(tag))
                        .one(&self.db)
                        .await?
                    {
                        oci_manifest::Entity::find()
                            .filter(oci_manifest::Column::OciRepositoryId.eq(repo_id))
                            .filter(oci_manifest::Column::Digest.eq(&t.manifest_digest))
                            .all(&self.db)
                            .await?
                    } else {
                        Vec::new()
                    }
                }
                (None, Some(digest)) => {
                    oci_manifest::Entity::find()
                        .filter(oci_manifest::Column::OciRepositoryId.eq(repo_id))
                        .filter(oci_manifest::Column::Digest.eq(digest))
                        .all(&self.db)
                        .await?
                }
                (None, None) => {
                    oci_manifest::Entity::find()
                        .filter(oci_manifest::Column::OciRepositoryId.eq(repo_id))
                        .all(&self.db)
                        .await?
                }
            };

        if manifests_to_delete.is_empty() {
            return Ok(false);
        }

        let mut layer_digests: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut manifest_ids: Vec<i64> = Vec::new();
        for manifest in &manifests_to_delete {
            manifest_ids.push(manifest.id);
            let layers = oci_layer::Entity::find()
                .filter(oci_layer::Column::OciManifestId.eq(manifest.id))
                .all(&self.db)
                .await?;
            for l in layers {
                layer_digests.insert(l.digest);
            }
        }

        // Layers retained by other manifests in the same repo (not being
        // deleted): their digests are still needed.
        let all_manifests = oci_manifest::Entity::find()
            .filter(oci_manifest::Column::OciRepositoryId.eq(repo_id))
            .all(&self.db)
            .await?;
        let mut retained_digests: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        for other in &all_manifests {
            if manifest_ids.contains(&other.id) {
                continue;
            }
            let other_layers = oci_layer::Entity::find()
                .filter(oci_layer::Column::OciManifestId.eq(other.id))
                .all(&self.db)
                .await?;
            for l in other_layers {
                retained_digests.insert(l.digest);
            }
        }
        let orphaned = crate::oci::compute_orphaned_layers(&layer_digests, &retained_digests);
        for layer_digest in &orphaned {
            let _ = cacache::remove(self.state_info.store_dir(), layer_digest).await;
        }

        for manifest in &manifests_to_delete {
            oci_manifest::Entity::delete_by_id(manifest.id)
                .exec(&self.db)
                .await?;
        }
        Ok(true)
    }

    pub(crate) async fn search_known_packages(
        &self,
        query: &str,
        offset: u32,
        limit: u32,
    ) -> anyhow::Result<Vec<super::known_package::KnownPackage>> {
        let pat = format!("%{query}%");
        let rows = oci_repository::Entity::find()
            .filter(
                sea_orm::Condition::any()
                    .add(oci_repository::Column::Registry.like(&pat))
                    .add(oci_repository::Column::Repository.like(&pat))
                    .add(oci_repository::Column::WitNamespace.like(&pat))
                    .add(oci_repository::Column::WitName.like(&pat)),
            )
            .order_by_asc(oci_repository::Column::Repository)
            .order_by_asc(oci_repository::Column::Registry)
            .offset(u64::from(offset))
            .limit(u64::from(limit))
            .all(&self.db)
            .await?;
        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            let pkg = known_package_from_repo(&self.db, r).await?;
            if !pkg.tags.is_empty() {
                out.push(pkg);
            }
        }
        Ok(out)
    }

    pub(crate) async fn search_known_packages_by_import(
        &self,
        interface: &str,
        offset: u32,
        limit: u32,
    ) -> anyhow::Result<Vec<super::known_package::KnownPackage>> {
        self.search_known_packages_by_iface(interface, offset, limit, true)
            .await
    }

    pub(crate) async fn search_known_packages_by_export(
        &self,
        interface: &str,
        offset: u32,
        limit: u32,
    ) -> anyhow::Result<Vec<super::known_package::KnownPackage>> {
        self.search_known_packages_by_iface(interface, offset, limit, false)
            .await
    }

    pub(crate) async fn list_known_packages(
        &self,
        offset: u32,
        limit: u32,
    ) -> anyhow::Result<Vec<super::known_package::KnownPackage>> {
        let rows = oci_repository::Entity::find()
            .order_by_asc(oci_repository::Column::Repository)
            .order_by_asc(oci_repository::Column::Registry)
            .offset(u64::from(offset))
            .limit(u64::from(limit))
            .all(&self.db)
            .await?;
        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            let pkg = known_package_from_repo(&self.db, r).await?;
            if !pkg.tags.is_empty() {
                out.push(pkg);
            }
        }
        Ok(out)
    }

    pub(crate) async fn list_recent_known_packages(
        &self,
        offset: u32,
        limit: u32,
    ) -> anyhow::Result<Vec<super::known_package::KnownPackage>> {
        let rows = oci_repository::Entity::find()
            .order_by_desc(oci_repository::Column::UpdatedAt)
            .offset(u64::from(offset))
            .limit(u64::from(limit))
            .all(&self.db)
            .await?;
        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            let pkg = known_package_from_repo(&self.db, r).await?;
            if !pkg.tags.is_empty() {
                out.push(pkg);
            }
        }
        Ok(out)
    }

    pub(crate) async fn get_known_package(
        &self,
        registry: &str,
        repository: &str,
    ) -> anyhow::Result<Option<super::known_package::KnownPackage>> {
        let row = oci_repository::Entity::find()
            .filter(oci_repository::Column::Registry.eq(registry))
            .filter(oci_repository::Column::Repository.eq(repository))
            .one(&self.db)
            .await?;
        match row {
            Some(r) => Ok(Some(known_package_from_repo(&self.db, r).await?)),
            None => Ok(None),
        }
    }

    pub(crate) async fn add_known_package(
        &self,
        registry: &str,
        repository: &str,
        tag: Option<&str>,
        description: Option<&str>,
    ) -> anyhow::Result<()> {
        self.add_known_package_with_params(&KnownPackageParams {
            registry,
            repository,
            tag,
            description,
            wit_namespace: None,
            wit_name: None,
            kind: None,
        })
        .await
    }

    pub(crate) async fn add_known_package_with_params(
        &self,
        params: &KnownPackageParams<'_>,
    ) -> anyhow::Result<()> {
        let kind_str = params.kind.map(|k| k.to_string());
        let repo_id = upsert_oci_repository_full(
            &self.db,
            params.registry,
            params.repository,
            params.wit_namespace,
            params.wit_name,
            kind_str.as_deref(),
        )
        .await?;

        // Optionally fill in a description on the most recent description-less
        // manifest. Best-effort: if there's no manifest yet, this is a no-op.
        if let Some(desc) = params.description {
            let candidate = oci_manifest::Entity::find()
                .filter(oci_manifest::Column::OciRepositoryId.eq(repo_id))
                .filter(oci_manifest::Column::OciDescription.is_null())
                .order_by_desc(oci_manifest::Column::CreatedAt)
                .one(&self.db)
                .await?;
            if let Some(m) = candidate {
                let mut am: oci_manifest::ActiveModel = m.into();
                am.oci_description = Set(Some(desc.to_owned()));
                if let Err(e) = am.update(&self.db).await {
                    warn!("Failed to update description for repo {repo_id}: {e}");
                }
            }
        }

        // The legacy implementation deliberately did NOT write a tag here —
        // tag→digest mappings are only authoritative after a real pull.
        let _ = params.tag;
        Ok(())
    }

    pub(crate) async fn is_tag_fresh(
        &self,
        registry: &str,
        repository: &str,
        tag: &str,
        max_age_secs: u64,
    ) -> bool {
        // A freshness window in seconds; saturating to `i64::MAX` only widens
        // the window, which is harmless for the freshness check below.
        let max_age = i64::try_from(max_age_secs).unwrap_or(i64::MAX);
        let cutoff = Utc::now() - chrono::Duration::seconds(max_age);
        // Tag is "fresh" iff:
        //   * the tag exists in this repo
        //   * the tag was updated >= cutoff
        //   * the manifest it points at has at least one cached layer
        async fn inner(
            this: &Store,
            registry: &str,
            repository: &str,
            tag: &str,
            cutoff: DateTime<Utc>,
        ) -> anyhow::Result<bool> {
            let Some(repo) = oci_repository::Entity::find()
                .filter(oci_repository::Column::Registry.eq(registry))
                .filter(oci_repository::Column::Repository.eq(repository))
                .one(&this.db)
                .await?
            else {
                return Ok(false);
            };
            let Some(t) = oci_tag::Entity::find()
                .filter(oci_tag::Column::OciRepositoryId.eq(repo.id))
                .filter(oci_tag::Column::Tag.eq(tag))
                .filter(oci_tag::Column::UpdatedAt.gte(cutoff))
                .one(&this.db)
                .await?
            else {
                return Ok(false);
            };
            let Some(manifest) = oci_manifest::Entity::find()
                .filter(oci_manifest::Column::OciRepositoryId.eq(repo.id))
                .filter(oci_manifest::Column::Digest.eq(&t.manifest_digest))
                .one(&this.db)
                .await?
            else {
                return Ok(false);
            };
            let layer_count = oci_layer::Entity::find()
                .filter(oci_layer::Column::OciManifestId.eq(manifest.id))
                .count(&this.db)
                .await?;
            Ok(layer_count > 0)
        }
        inner(self, registry, repository, tag, cutoff)
            .await
            .unwrap_or(false)
    }

    // ---- Fetch queue --------------------------------------------------

    pub(crate) async fn enqueue_pull(
        &self,
        registry: &str,
        repository: &str,
        tag: &str,
        priority: i32,
    ) -> anyhow::Result<()> {
        // INSERT … ON CONFLICT(registry, repository, tag, task) DO NOTHING
        let am = fetch_queue::ActiveModel {
            registry: Set(registry.to_owned()),
            repository: Set(repository.to_owned()),
            tag: Set(tag.to_owned()),
            task: Set(fetch_queue::FetchTask::Pull),
            priority: Set(priority),
            ..Default::default()
        };
        fetch_queue::Entity::insert(am)
            .on_conflict(
                OnConflict::columns([
                    fetch_queue::Column::Registry,
                    fetch_queue::Column::Repository,
                    fetch_queue::Column::Tag,
                    fetch_queue::Column::Task,
                ])
                .do_nothing()
                .to_owned(),
            )
            .do_nothing()
            .exec(&self.db)
            .await?;
        Ok(())
    }

    pub(crate) async fn record_completed(
        &self,
        registry: &str,
        repository: &str,
        tag: &str,
    ) -> anyhow::Result<()> {
        let am = fetch_queue::ActiveModel {
            registry: Set(registry.to_owned()),
            repository: Set(repository.to_owned()),
            tag: Set(tag.to_owned()),
            task: Set(fetch_queue::FetchTask::Pull),
            status: Set(fetch_queue::FetchStatus::Completed),
            ..Default::default()
        };
        fetch_queue::Entity::insert(am)
            .on_conflict(
                OnConflict::columns([
                    fetch_queue::Column::Registry,
                    fetch_queue::Column::Repository,
                    fetch_queue::Column::Tag,
                    fetch_queue::Column::Task,
                ])
                .do_nothing()
                .to_owned(),
            )
            .do_nothing()
            .exec(&self.db)
            .await?;
        Ok(())
    }

    #[allow(dead_code)]
    pub(crate) async fn enqueue_reindex(
        &self,
        registry: &str,
        repository: &str,
        tag: &str,
    ) -> anyhow::Result<()> {
        let am = fetch_queue::ActiveModel {
            registry: Set(registry.to_owned()),
            repository: Set(repository.to_owned()),
            tag: Set(tag.to_owned()),
            task: Set(fetch_queue::FetchTask::Reindex),
            ..Default::default()
        };
        fetch_queue::Entity::insert(am)
            .on_conflict(
                OnConflict::columns([
                    fetch_queue::Column::Registry,
                    fetch_queue::Column::Repository,
                    fetch_queue::Column::Tag,
                    fetch_queue::Column::Task,
                ])
                .do_nothing()
                .to_owned(),
            )
            .do_nothing()
            .exec(&self.db)
            .await?;
        Ok(())
    }

    pub(crate) async fn enqueue_reindex_all(&self) -> anyhow::Result<u64> {
        // Bulk INSERT … SELECT to enqueue reindex tasks for every tag that
        // has cached layers. Same filter rules as legacy: skip 'latest',
        // signature tags (sha256-*), and bare 40-char hex commit SHAs.
        let sql = "\
            INSERT INTO fetch_queue (registry, repository, tag, task) \
            SELECT r.registry, r.repository, t.tag, 'reindex' \
            FROM oci_tag t \
            JOIN oci_repository r ON r.id = t.oci_repository_id \
            JOIN oci_manifest m ON m.oci_repository_id = r.id \
                                AND m.digest = t.manifest_digest \
            JOIN oci_layer l ON l.oci_manifest_id = m.id \
            WHERE t.tag != 'latest' \
              AND t.tag NOT LIKE 'sha256-%' \
              AND NOT (length(t.tag) = 40 \
                       AND t.tag GLOB '[0-9a-f]*' \
                       AND t.tag NOT GLOB '*[^0-9a-f]*') \
            GROUP BY r.registry, r.repository, t.tag \
            ON CONFLICT(registry, repository, tag, task) DO NOTHING";
        let result = self
            .db
            .execute_raw(Statement::from_string(self.db.get_database_backend(), sql))
            .await?;
        Ok(result.rows_affected())
    }

    pub(crate) async fn seed_completed_from_tags(&self) -> anyhow::Result<u64> {
        // Same filter as enqueue_reindex_all but inserts 'completed' rows so
        // history shows tags that pre-date the queue.
        let sql = "\
            INSERT INTO fetch_queue (registry, repository, tag, task, status, created_at, updated_at) \
            SELECT r.registry, r.repository, t.tag, 'pull', 'completed', t.created_at, t.updated_at \
            FROM oci_tag t \
            JOIN oci_repository r ON r.id = t.oci_repository_id \
            JOIN oci_manifest m ON m.oci_repository_id = r.id \
                                AND m.digest = t.manifest_digest \
            JOIN oci_layer l ON l.oci_manifest_id = m.id \
            WHERE t.tag != 'latest' \
              AND t.tag NOT LIKE 'sha256-%' \
              AND NOT (length(t.tag) = 40 \
                       AND t.tag GLOB '[0-9a-f]*' \
                       AND t.tag NOT GLOB '*[^0-9a-f]*') \
            GROUP BY r.registry, r.repository, t.tag \
            ON CONFLICT(registry, repository, tag, task) DO NOTHING";
        let result = self
            .db
            .execute_raw(Statement::from_string(self.db.get_database_backend(), sql))
            .await?;
        Ok(result.rows_affected())
    }

    pub(crate) async fn enqueue_refetch(
        &self,
        registry: &str,
        repository: &str,
        tag: &str,
        priority: i32,
    ) -> anyhow::Result<()> {
        // INSERT … ON CONFLICT DO UPDATE SET status='pending', priority=excluded.priority,
        //   attempts=0, last_error=NULL
        let am = fetch_queue::ActiveModel {
            registry: Set(registry.to_owned()),
            repository: Set(repository.to_owned()),
            tag: Set(tag.to_owned()),
            task: Set(fetch_queue::FetchTask::Pull),
            priority: Set(priority),
            ..Default::default()
        };
        fetch_queue::Entity::insert(am)
            .on_conflict(
                OnConflict::columns([
                    fetch_queue::Column::Registry,
                    fetch_queue::Column::Repository,
                    fetch_queue::Column::Tag,
                    fetch_queue::Column::Task,
                ])
                .values([
                    (
                        fetch_queue::Column::Status,
                        fetch_queue::FetchStatus::Pending.into(),
                    ),
                    (fetch_queue::Column::Priority, priority.into()),
                    (fetch_queue::Column::Attempts, 0i32.into()),
                    (
                        fetch_queue::Column::LastError,
                        SimpleExpr::Value(sea_orm::Value::String(None)),
                    ),
                ])
                .to_owned(),
            )
            .exec(&self.db)
            .await?;
        Ok(())
    }

    pub(crate) async fn dequeue_next(&self) -> anyhow::Result<Option<FetchTask>> {
        // Atomic claim: SELECT one pending row, mark it in_progress, return it.
        // Wrap in a transaction so concurrent dequeues don't double-claim.
        // SQLite serializes writes anyway; on Postgres this would benefit from
        // FOR UPDATE SKIP LOCKED, which we can add later if multi-worker
        // contention becomes an issue.
        let txn = self.db.begin().await?;
        let candidate = fetch_queue::Entity::find()
            .filter(fetch_queue::Column::Status.eq(fetch_queue::FetchStatus::Pending))
            .order_by_asc(fetch_queue::Column::Priority)
            .order_by_asc(fetch_queue::Column::CreatedAt)
            .one(&txn)
            .await?;
        let Some(row) = candidate else {
            txn.commit().await?;
            return Ok(None);
        };
        let task = FetchTask {
            id: row.id,
            registry: row.registry.clone(),
            repository: row.repository.clone(),
            tag: row.tag.clone(),
            kind: FetchTaskKind::from(row.task),
            attempts: i64::from(row.attempts),
        };
        let mut am: fetch_queue::ActiveModel = row.into();
        am.status = Set(fetch_queue::FetchStatus::InProgress);
        am.update(&txn).await?;
        txn.commit().await?;
        Ok(Some(task))
    }

    pub(crate) async fn complete_task(&self, task_id: i64) -> anyhow::Result<()> {
        fetch_queue::Entity::update_many()
            .col_expr(
                fetch_queue::Column::Status,
                Expr::value(fetch_queue::FetchStatus::Completed),
            )
            .col_expr(
                fetch_queue::Column::LastError,
                SimpleExpr::Value(sea_orm::Value::String(None)),
            )
            .filter(fetch_queue::Column::Id.eq(task_id))
            .exec(&self.db)
            .await?;
        Ok(())
    }

    pub(crate) async fn fail_task(&self, task_id: i64, error: &str) -> anyhow::Result<()> {
        // Read-modify-write inside a transaction: SeaORM's update builder
        // doesn't ergonomically express `attempts = attempts + 1` together
        // with a CASE-derived status, so we fetch the row first.
        let txn = self.db.begin().await?;
        if let Some(row) = fetch_queue::Entity::find_by_id(task_id).one(&txn).await? {
            let new_attempts = row.attempts + 1;
            let new_status = if new_attempts >= row.max_attempts {
                fetch_queue::FetchStatus::Failed
            } else {
                fetch_queue::FetchStatus::Pending
            };
            let mut am: fetch_queue::ActiveModel = row.into();
            am.attempts = Set(new_attempts);
            am.last_error = Set(Some(error.to_owned()));
            am.status = Set(new_status);
            am.update(&txn).await?;
        }
        txn.commit().await?;
        Ok(())
    }

    pub(crate) async fn pending_count(&self) -> anyhow::Result<u64> {
        let n = fetch_queue::Entity::find()
            .filter(fetch_queue::Column::Status.eq(fetch_queue::FetchStatus::Pending))
            .count(&self.db)
            .await?;
        Ok(n)
    }

    pub(crate) async fn get_queue_status(
        &self,
    ) -> anyhow::Result<component_meta_registry_types::QueueStatus> {
        use component_meta_registry_types::{QueueStatus, QueueTask};

        let mut pending = 0u64;
        let mut in_progress = 0u64;
        let mut completed = 0u64;
        let mut failed = 0u64;
        for status in [
            fetch_queue::FetchStatus::Pending,
            fetch_queue::FetchStatus::InProgress,
            fetch_queue::FetchStatus::Completed,
            fetch_queue::FetchStatus::Failed,
        ] {
            let n = fetch_queue::Entity::find()
                .filter(fetch_queue::Column::Status.eq(status))
                .count(&self.db)
                .await?;
            match status {
                fetch_queue::FetchStatus::Pending => pending = n,
                fetch_queue::FetchStatus::InProgress => in_progress = n,
                fetch_queue::FetchStatus::Completed => completed = n,
                fetch_queue::FetchStatus::Failed => failed = n,
            }
        }

        let active_rows = fetch_queue::Entity::find()
            .filter(fetch_queue::Column::Status.is_in([
                fetch_queue::FetchStatus::Pending,
                fetch_queue::FetchStatus::InProgress,
            ]))
            // ORDER BY: in_progress before pending, then priority asc, then
            // created_at asc. SeaORM doesn't have a built-in CASE order, so
            // emit it as a custom expr.
            .order_by_asc(Expr::cust(
                "CASE status WHEN 'in_progress' THEN 0 ELSE 1 END",
            ))
            .order_by_asc(fetch_queue::Column::Priority)
            .order_by_asc(fetch_queue::Column::CreatedAt)
            .all(&self.db)
            .await?;
        let active: Vec<QueueTask> = active_rows.into_iter().map(into_queue_task).collect();

        let history_rows = fetch_queue::Entity::find()
            .filter(fetch_queue::Column::Status.is_in([
                fetch_queue::FetchStatus::Completed,
                fetch_queue::FetchStatus::Failed,
            ]))
            .order_by_desc(fetch_queue::Column::UpdatedAt)
            .order_by_asc(fetch_queue::Column::Repository)
            .order_by_desc(fetch_queue::Column::Tag)
            .limit(50)
            .all(&self.db)
            .await?;
        let history: Vec<QueueTask> = history_rows.into_iter().map(into_queue_task).collect();

        Ok(QueueStatus {
            pending,
            in_progress,
            completed,
            failed,
            active,
            history,
        })
    }

    pub(crate) async fn reindex_tag(
        &self,
        registry: &str,
        repository: &str,
        tag: &str,
    ) -> anyhow::Result<()> {
        // Find the manifest id for this tag.
        let sql = "\
            SELECT m.id AS id FROM oci_tag t \
            JOIN oci_repository r ON r.id = t.oci_repository_id \
            JOIN oci_manifest m ON m.oci_repository_id = r.id \
                                AND m.digest = t.manifest_digest \
            WHERE r.registry = ? AND r.repository = ? AND t.tag = ? LIMIT 1";
        #[derive(FromQueryResult)]
        struct IdRow {
            id: i64,
        }
        let row = IdRow::find_by_statement(Statement::from_sql_and_values(
            self.db.get_database_backend(),
            sql,
            [registry.into(), repository.into(), tag.into()],
        ))
        .one(&self.db)
        .await?;
        let Some(IdRow { id: manifest_id }) = row else {
            anyhow::bail!("no manifest found for {registry}/{repository}:{tag}");
        };

        // Find the first wasm layer.
        let layer = oci_layer::Entity::find()
            .filter(oci_layer::Column::OciManifestId.eq(manifest_id))
            .order_by_asc(oci_layer::Column::Position)
            .one(&self.db)
            .await?;
        let Some(layer) = layer else {
            anyhow::bail!("no layers found for manifest of {registry}/{repository}:{tag}");
        };

        let store_dir = self.state_info.store_dir().to_path_buf();
        let bytes = cacache::read(&store_dir, &layer.digest).await?;

        // Wrap the delete + re-extract in a transaction so a mid-flight
        // failure leaves the previously-indexed WIT data intact.
        let txn = self.db.begin().await?;
        wit_package::Entity::delete_many()
            .filter(wit_package::Column::OciManifestId.eq(manifest_id))
            .exec(&txn)
            .await?;
        wasm_component::Entity::delete_many()
            .filter(wasm_component::Column::OciManifestId.eq(manifest_id))
            .exec(&txn)
            .await?;
        // Note: we extract via `self` (the outer connection), not the txn,
        // so the helper's own writes still need to be folded into this txn.
        // For simplicity we commit the deletes first; a follow-up could push
        // the extraction into the same transaction.
        txn.commit().await?;
        self.try_extract_wit_package(manifest_id, Some(layer.id), &bytes)
            .await;
        Ok(())
    }

    // ---- WIT helpers --------------------------------------------------

    #[allow(dead_code)]
    pub(crate) async fn list_wit_packages(&self) -> anyhow::Result<Vec<wit_package::Model>> {
        Ok(wit_package::Entity::find().all(&self.db).await?)
    }

    pub(crate) async fn list_wit_packages_with_components(
        &self,
    ) -> anyhow::Result<Vec<(wit_package::Model, String)>> {
        // Return every wit_package along with a synthetic OCI reference
        // "<registry>/<repository>:<latest_tag>" derived from its provenance.
        // Packages without a backing OCI manifest are skipped.
        let pkgs = wit_package::Entity::find()
            .filter(wit_package::Column::OciManifestId.is_not_null())
            .all(&self.db)
            .await?;
        let mut out = Vec::with_capacity(pkgs.len());
        for pkg in pkgs {
            let Some(manifest_id) = pkg.oci_manifest_id else {
                continue;
            };
            let Some(manifest) = oci_manifest::Entity::find_by_id(manifest_id)
                .one(&self.db)
                .await?
            else {
                continue;
            };
            let Some(repo) = oci_repository::Entity::find_by_id(manifest.oci_repository_id)
                .one(&self.db)
                .await?
            else {
                continue;
            };
            let tag_row = oci_tag::Entity::find()
                .filter(oci_tag::Column::OciRepositoryId.eq(repo.id))
                .filter(oci_tag::Column::ManifestDigest.eq(&manifest.digest))
                .order_by_desc(oci_tag::Column::UpdatedAt)
                .one(&self.db)
                .await?;
            let tag = tag_row.map_or_else(|| "latest".to_string(), |t| t.tag);
            let reference = format!("{}/{}:{}", repo.registry, repo.repository, tag);
            out.push((pkg, reference));
        }
        Ok(out)
    }

    pub(crate) async fn find_oci_reference_by_wit_name(
        &self,
        package_name: &str,
        version: Option<&str>,
    ) -> anyhow::Result<Option<(String, String)>> {
        // Find a wit_package row by (name, version) and join out to the
        // owning oci_manifest → oci_repository.
        let mut q = wit_package::Entity::find()
            .filter(wit_package::Column::PackageName.eq(package_name))
            .filter(wit_package::Column::OciManifestId.is_not_null());
        if let Some(v) = version {
            q = q.filter(wit_package::Column::Version.eq(v));
        }
        let pkg = q
            .order_by_desc(wit_package::Column::Id)
            .one(&self.db)
            .await?;
        let Some(pkg) = pkg else { return Ok(None) };
        let Some(manifest_id) = pkg.oci_manifest_id else {
            return Ok(None);
        };
        let manifest = oci_manifest::Entity::find_by_id(manifest_id)
            .one(&self.db)
            .await?;
        let Some(manifest) = manifest else {
            return Ok(None);
        };
        let repo = oci_repository::Entity::find_by_id(manifest.oci_repository_id)
            .one(&self.db)
            .await?;
        Ok(repo.map(|r| (r.registry, r.repository)))
    }

    pub(crate) async fn search_known_package_by_wit_name(
        &self,
        wit_name: &str,
    ) -> anyhow::Result<Option<super::known_package::KnownPackage>> {
        let Some((namespace, name)) = wit_name.split_once(':') else {
            return Ok(None);
        };
        // Exact lookup first.
        let by_columns = oci_repository::Entity::find()
            .filter(oci_repository::Column::WitNamespace.eq(namespace))
            .filter(oci_repository::Column::WitName.eq(name))
            .order_by_desc(oci_repository::Column::UpdatedAt)
            .one(&self.db)
            .await?;
        if let Some(repo) = by_columns {
            return Ok(Some(known_package_from_repo(&self.db, repo).await?));
        }
        // Fuzzy fallback: match repository column.
        let pattern = wit_name.replace(':', "/");
        let like_pat = format!("%{pattern}%");
        let by_repo = oci_repository::Entity::find()
            .filter(oci_repository::Column::Repository.like(&like_pat))
            .order_by_desc(oci_repository::Column::UpdatedAt)
            .one(&self.db)
            .await?;
        match by_repo {
            Some(r) => Ok(Some(known_package_from_repo(&self.db, r).await?)),
            None => Ok(None),
        }
    }

    // ---- _sync_meta ---------------------------------------------------

    #[allow(dead_code)]
    pub(crate) async fn get_sync_meta(&self, key: &str) -> anyhow::Result<Option<String>> {
        let row = sync_meta::Entity::find_by_id(key.to_owned())
            .one(&self.db)
            .await?;
        Ok(row.map(|r| r.value))
    }

    #[allow(dead_code)]
    pub(crate) async fn set_sync_meta(&self, key: &str, value: &str) -> anyhow::Result<()> {
        let am = sync_meta::ActiveModel {
            key: Set(key.to_owned()),
            value: Set(value.to_owned()),
        };
        sync_meta::Entity::insert(am)
            .on_conflict(
                OnConflict::column(sync_meta::Column::Key)
                    .update_column(sync_meta::Column::Value)
                    .to_owned(),
            )
            .exec(&self.db)
            .await?;
        Ok(())
    }

    pub(crate) async fn get_package_dependencies(
        &self,
        registry: &str,
        repository: &str,
    ) -> anyhow::Result<Vec<component_meta_registry_types::PackageDependencyRef>> {
        // Two cases (matches legacy SQL):
        //   1. Pulled: dependencies via wit_package -> oci_manifest (latest)
        //   2. Stub: wit_package row with oci_manifest_id IS NULL whose
        //      package_name matches "<wit_namespace>:<wit_name>".
        let sql = "\
            SELECT DISTINCT wpd.declared_package AS declared_package, \
                   wpd.declared_version AS declared_version \
            FROM wit_package_dependency wpd \
            JOIN wit_package wp ON wpd.dependent_id = wp.id \
            WHERE \
              wp.oci_manifest_id = ( \
                SELECT om.id FROM oci_manifest om \
                JOIN oci_repository repo ON om.oci_repository_id = repo.id \
                WHERE repo.registry = ? AND repo.repository = ? \
                ORDER BY om.id DESC LIMIT 1 \
              ) \
              OR ( \
                wp.oci_manifest_id IS NULL \
                AND wp.package_name = ( \
                  SELECT repo.wit_namespace || ':' || repo.wit_name \
                  FROM oci_repository repo \
                  WHERE repo.registry = ? AND repo.repository = ? \
                    AND repo.wit_namespace IS NOT NULL \
                    AND repo.wit_name IS NOT NULL \
                  LIMIT 1 \
                ) \
              ) \
            ORDER BY wpd.declared_package";
        let stmt = Statement::from_sql_and_values(
            self.db.get_database_backend(),
            sql,
            [
                registry.into(),
                repository.into(),
                registry.into(),
                repository.into(),
            ],
        );
        #[derive(FromQueryResult)]
        struct Row {
            declared_package: String,
            declared_version: Option<String>,
        }
        let rows = Row::find_by_statement(stmt).all(&self.db).await?;
        Ok(rows
            .into_iter()
            .map(|r| component_meta_registry_types::PackageDependencyRef {
                package: r.declared_package,
                version: r.declared_version,
            })
            .collect())
    }

    pub(crate) async fn get_package_dependencies_by_name(
        &self,
        package_name: &str,
        version: Option<&str>,
    ) -> anyhow::Result<Vec<component_meta_registry_types::PackageDependencyRef>> {
        // Pick the canonical wit_package row: prefer pulled (oci_manifest_id
        // IS NOT NULL) over stubs, then newest id. Then load its deps.
        let sql = "\
            SELECT wpd.declared_package AS declared_package, \
                   wpd.declared_version AS declared_version \
            FROM wit_package_dependency wpd \
            WHERE wpd.dependent_id = ( \
                SELECT id FROM wit_package \
                WHERE package_name = ? \
                  AND COALESCE(version, '') = COALESCE(?, '') \
                ORDER BY (oci_manifest_id IS NOT NULL) DESC, id DESC \
                LIMIT 1 \
            ) \
            ORDER BY wpd.declared_package";
        let stmt = Statement::from_sql_and_values(
            self.db.get_database_backend(),
            sql,
            [package_name.into(), version.unwrap_or("").into()],
        );
        #[derive(FromQueryResult)]
        struct Row {
            declared_package: String,
            declared_version: Option<String>,
        }
        let rows = Row::find_by_statement(stmt).all(&self.db).await?;
        Ok(rows
            .into_iter()
            .map(|r| component_meta_registry_types::PackageDependencyRef {
                package: r.declared_package,
                version: r.declared_version,
            })
            .collect())
    }

    pub(crate) async fn list_wit_package_versions(
        &self,
        package_name: &str,
    ) -> anyhow::Result<Vec<String>> {
        // SELECT DISTINCT version FROM wit_package WHERE package_name = ?
        //   AND version IS NOT NULL ORDER BY id DESC
        let rows = wit_package::Entity::find()
            .filter(wit_package::Column::PackageName.eq(package_name))
            .filter(wit_package::Column::Version.is_not_null())
            .order_by_desc(wit_package::Column::Id)
            .all(&self.db)
            .await?;
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            if let Some(v) = r.version
                && seen.insert(v.clone())
            {
                out.push(v);
            }
        }
        Ok(out)
    }

    #[cfg(feature = "http-sync")]
    pub(crate) async fn upsert_package_dependencies_from_sync(
        &self,
        package_name: &str,
        version: Option<&str>,
        dependencies: &[component_meta_registry_types::PackageDependencyRef],
    ) -> anyhow::Result<()> {
        // Find or insert a wit_package row anchored to (package_name, version).
        let existing = wit_package::Entity::find()
            .filter(wit_package::Column::PackageName.eq(package_name))
            .filter(match version {
                Some(v) => wit_package::Column::Version.eq(v),
                None => wit_package::Column::Version.is_null(),
            })
            // prefer a pulled row over a stub.
            .order_by_desc(Expr::cust(
                "CASE WHEN oci_manifest_id IS NOT NULL THEN 1 ELSE 0 END",
            ))
            .order_by_desc(wit_package::Column::Id)
            .one(&self.db)
            .await?;
        let pkg_id = if let Some(row) = existing {
            row.id
        } else {
            let am = wit_package::ActiveModel {
                package_name: Set(package_name.to_owned()),
                version: Set(version.map(str::to_owned)),
                ..Default::default()
            };
            let res = wit_package::Entity::insert(am).exec(&self.db).await?;
            res.last_insert_id
        };

        for dep in dependencies {
            if let Err(e) = insert_wit_package_dependency(
                &self.db,
                pkg_id,
                &dep.package,
                dep.version.as_deref(),
            )
            .await
            {
                warn!(
                    "Failed to insert synced dependency {} → {}: {}",
                    package_name, dep.package, e
                );
            }
        }
        Ok(())
    }

    pub(crate) async fn get_package_versions(
        &self,
        registry: &str,
        repository: &str,
    ) -> anyhow::Result<Vec<component_meta_registry_types::PackageVersion>> {
        let Some(repo) = oci_repository::Entity::find()
            .filter(oci_repository::Column::Registry.eq(registry))
            .filter(oci_repository::Column::Repository.eq(repository))
            .one(&self.db)
            .await?
        else {
            return Ok(Vec::new());
        };
        let manifests = oci_manifest::Entity::find()
            .filter(oci_manifest::Column::OciRepositoryId.eq(repo.id))
            .order_by_desc(oci_manifest::Column::Id)
            .all(&self.db)
            .await?;
        let mut out = Vec::with_capacity(manifests.len());
        for m in manifests {
            let tag = oci_tag::Entity::find()
                .filter(oci_tag::Column::OciRepositoryId.eq(repo.id))
                .filter(oci_tag::Column::ManifestDigest.eq(&m.digest))
                .order_by_desc(oci_tag::Column::Id)
                .one(&self.db)
                .await?
                .map(|t| t.tag);
            out.push(self.build_package_version(&m, tag).await?);
        }
        Ok(out)
    }

    pub(crate) async fn get_package_version(
        &self,
        registry: &str,
        repository: &str,
        version_tag: &str,
    ) -> anyhow::Result<Option<component_meta_registry_types::PackageVersion>> {
        let Some(repo) = oci_repository::Entity::find()
            .filter(oci_repository::Column::Registry.eq(registry))
            .filter(oci_repository::Column::Repository.eq(repository))
            .one(&self.db)
            .await?
        else {
            return Ok(None);
        };
        let Some(t) = oci_tag::Entity::find()
            .filter(oci_tag::Column::OciRepositoryId.eq(repo.id))
            .filter(oci_tag::Column::Tag.eq(version_tag))
            .one(&self.db)
            .await?
        else {
            return Ok(None);
        };
        let Some(m) = oci_manifest::Entity::find()
            .filter(oci_manifest::Column::OciRepositoryId.eq(repo.id))
            .filter(oci_manifest::Column::Digest.eq(&t.manifest_digest))
            .one(&self.db)
            .await?
        else {
            return Ok(None);
        };
        Ok(Some(
            self.build_package_version(&m, Some(version_tag.to_owned()))
                .await?,
        ))
    }

    pub(crate) async fn get_package_detail(
        &self,
        registry: &str,
        repository: &str,
    ) -> anyhow::Result<Option<component_meta_registry_types::PackageDetail>> {
        let Some(repo) = oci_repository::Entity::find()
            .filter(oci_repository::Column::Registry.eq(registry))
            .filter(oci_repository::Column::Repository.eq(repository))
            .one(&self.db)
            .await?
        else {
            return Ok(None);
        };
        let kind = parse_kind(repo.kind.as_deref());
        let description = self
            .get_known_package(registry, repository)
            .await?
            .and_then(|pkg| pkg.description);
        let versions = self.get_package_versions(registry, repository).await?;
        Ok(Some(component_meta_registry_types::PackageDetail {
            registry: registry.to_string(),
            repository: repository.to_string(),
            kind,
            description,
            wit_namespace: repo.wit_namespace,
            wit_name: repo.wit_name,
            versions,
        }))
    }
}

#[cfg(test)]
mod smoke_tests {
    use super::*;

    #[tokio::test]
    async fn open_in_memory_runs_migrations() {
        let store = Store::open_in_memory().await.expect("open in-memory store");
        assert!(store.state_info.migration_total() > 0);
        assert_eq!(
            store.state_info.migration_current(),
            store.state_info.migration_total()
        );
    }

    #[tokio::test]
    async fn sync_meta_round_trip() {
        let store = Store::open_in_memory().await.unwrap();
        assert_eq!(store.get_sync_meta("foo").await.unwrap(), None);
        store.set_sync_meta("foo", "bar").await.unwrap();
        assert_eq!(
            store.get_sync_meta("foo").await.unwrap().as_deref(),
            Some("bar")
        );
        store.set_sync_meta("foo", "baz").await.unwrap();
        assert_eq!(
            store.get_sync_meta("foo").await.unwrap().as_deref(),
            Some("baz")
        );
    }

    #[tokio::test]
    async fn known_packages_basic() {
        let store = Store::open_in_memory().await.unwrap();
        assert!(store.list_known_packages(0, 10).await.unwrap().is_empty());
        store
            .add_known_package("ghcr.io", "user/repo", None, Some("hello"))
            .await
            .unwrap();
        // list_known_packages filters out packages with no semver tags, so
        // insert a manifest + tag to mimic a real ingested package.
        let repo_id =
            upsert_oci_repository_full(&store.db, "ghcr.io", "user/repo", None, None, None)
                .await
                .unwrap();
        let digest = "sha256:deadbeef";
        upsert_oci_manifest(
            &store.db,
            repo_id,
            digest,
            None,
            None,
            None,
            None,
            None,
            None,
            &HashMap::new(),
        )
        .await
        .unwrap();
        upsert_oci_tag(&store.db, repo_id, "1.0.0", digest)
            .await
            .unwrap();
        let pkgs = store.list_known_packages(0, 10).await.unwrap();
        assert_eq!(pkgs.len(), 1);
        assert_eq!(pkgs[0].registry, "ghcr.io");
        assert_eq!(pkgs[0].repository, "user/repo");
        let pkg = store
            .get_known_package("ghcr.io", "user/repo")
            .await
            .unwrap();
        assert!(pkg.is_some());
    }

    #[tokio::test]
    async fn fetch_queue_pull_complete_roundtrip() {
        let store = Store::open_in_memory().await.unwrap();
        store
            .enqueue_pull("ghcr.io", "user/repo", "1.0.0", 0)
            .await
            .unwrap();
        assert_eq!(store.pending_count().await.unwrap(), 1);
        let task = store
            .dequeue_next()
            .await
            .unwrap()
            .expect("task should dequeue");
        assert_eq!(task.tag, "1.0.0");
        assert_eq!(store.pending_count().await.unwrap(), 0);
        store.complete_task(task.id).await.unwrap();
        let status = store.get_queue_status().await.unwrap();
        assert_eq!(status.completed, 1);
    }

    #[tokio::test]
    async fn fetch_queue_fail_then_pending_again() {
        let store = Store::open_in_memory().await.unwrap();
        store
            .enqueue_pull("ghcr.io", "user/repo", "1.0.0", 0)
            .await
            .unwrap();
        let task = store.dequeue_next().await.unwrap().unwrap();
        store.fail_task(task.id, "oops").await.unwrap();
        assert_eq!(store.pending_count().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn db_config_redacts_password() {
        use crate::storage::redact_url;
        assert_eq!(
            redact_url("postgres://alice:secret@db.example.com/wasm"),
            "postgres://alice:[REDACTED]@db.example.com/wasm"
        );
    }

    #[tokio::test]
    async fn open_at_runs_sqlite_migrations_on_disk() {
        // Exercises the on-disk `Store::open_at` -> `open_inner` path, which
        // is otherwise only reached via `Manager::open_at` in production
        // code. Confirms the SQLite auto-migration arm runs to completion
        // and produces a fully-applied migration snapshot.
        let tmp = tempfile::tempdir().expect("create temp data dir");
        let store = Store::open_at(tmp.path().to_path_buf())
            .await
            .expect("Store::open_at should succeed for sqlite default");
        let snapshot = Migrations::snapshot(store.db()).await;
        assert!(snapshot.total > 0);
        assert_eq!(snapshot.current, snapshot.total);
        assert!(tmp.path().join("db").join("metadata-v2.db3").exists());
    }

    #[tokio::test]
    async fn postgres_concurrent_open_succeeds() {
        let Ok(url) = std::env::var("COMPONENT_DATABASE_URL") else {
            return;
        };
        let lower = url.to_ascii_lowercase();
        if !(lower.starts_with("postgres:") || lower.starts_with("postgresql:")) {
            return;
        }

        let data_dir_a = tempfile::tempdir().expect("create temp dir A");
        let data_dir_b = tempfile::tempdir().expect("create temp dir B");
        let (a, b) = tokio::join!(
            Store::open_at(data_dir_a.path().to_path_buf()),
            Store::open_at(data_dir_b.path().to_path_buf())
        );
        let store_a = a.expect("first concurrent Store::open_at should succeed");
        let store_b = b.expect("second concurrent Store::open_at should succeed");

        let snapshot_a = Migrations::snapshot(store_a.db()).await;
        let snapshot_b = Migrations::snapshot(store_b.db()).await;
        assert_eq!(snapshot_a.current, snapshot_a.total);
        assert_eq!(snapshot_b.current, snapshot_b.total);
        assert_eq!(snapshot_a.current, snapshot_b.current);
    }
}

#[cfg(test)]
mod component_tests {
    use super::*;
    use component_meta_registry_types::{ComponentSummary, WitInterfaceRef};

    #[test]
    fn parse_component_extern_name_full() {
        let r = parse_component_extern_name("wasi:http/types@0.2.3");
        assert_eq!(r.package, "wasi:http");
        assert_eq!(r.interface.as_deref(), Some("types"));
        assert_eq!(r.version.as_deref(), Some("0.2.3"));
    }

    #[test]
    fn parse_component_extern_name_no_interface() {
        let r = parse_component_extern_name("wasi:http@0.2.3");
        assert_eq!(r.package, "wasi:http");
        assert_eq!(r.interface, None);
        assert_eq!(r.version.as_deref(), Some("0.2.3"));
    }

    #[test]
    fn parse_component_extern_name_no_version() {
        let r = parse_component_extern_name("wasi:http/types");
        assert_eq!(r.package, "wasi:http");
        assert_eq!(r.interface.as_deref(), Some("types"));
        assert_eq!(r.version, None);
    }

    #[test]
    fn parse_component_extern_name_plain() {
        let r = parse_component_extern_name("my-import");
        assert_eq!(r.package, "my-import");
        assert_eq!(r.interface, None);
        assert_eq!(r.version, None);
    }

    #[test]
    fn extract_wit_imports_exports_returns_empty_for_non_wasm() {
        let (imports, exports) = extract_wit_imports_exports(b"not wasm", &(0..8), None);
        assert!(imports.is_empty());
        assert!(exports.is_empty());
    }

    #[test]
    fn extract_wit_imports_exports_returns_empty_for_out_of_range() {
        let (imports, exports) = extract_wit_imports_exports(b"short", &(0..100), None);
        assert!(imports.is_empty());
        assert!(exports.is_empty());
    }

    #[test]
    fn extract_component_metadata_returns_none_for_invalid_bytes() {
        let (name, desc, json) = extract_component_metadata(b"not wasm");
        assert!(name.is_none());
        assert!(desc.is_none());
        assert!(json.is_none());
    }

    #[test]
    fn mark_native_iface_refs_flags_and_inherits_version() {
        let mut summary = sample_summary();
        summary.imports = vec![
            iface_ref("ns:pkg", None),
            iface_ref("other:dep", Some("1.0.0")),
        ];
        mark_native_iface_refs(&mut summary, "ns:pkg", Some("0.2.0"));
        assert!(summary.imports[0].is_native);
        assert_eq!(summary.imports[0].version.as_deref(), Some("0.2.0"));
        assert!(!summary.imports[1].is_native);
        assert_eq!(summary.imports[1].version.as_deref(), Some("1.0.0"));
    }

    fn iface_ref(package: &str, version: Option<&str>) -> WitInterfaceRef {
        WitInterfaceRef {
            package: package.to_string(),
            interface: Some("iface".to_string()),
            version: version.map(str::to_owned),
            docs: None,
            is_native: false,
        }
    }

    fn sample_summary() -> ComponentSummary {
        ComponentSummary {
            name: Some("root".to_string()),
            description: None,
            targets: vec![],
            producers: vec![],
            kind: Some("component".to_string()),
            size_bytes: Some(123),
            range_start: Some(0),
            range_end: Some(123),
            languages: vec![],
            children: vec![ComponentSummary {
                name: Some("main.wasm".to_string()),
                description: None,
                targets: vec![],
                producers: vec![],
                kind: Some("module".to_string()),
                size_bytes: Some(64),
                range_start: Some(0),
                range_end: Some(64),
                languages: vec!["Rust".to_string()],
                children: vec![],
                source: None,
                homepage: None,
                licenses: None,
                authors: None,
                revision: None,
                component_version: None,
                bill_of_materials: vec![],
                imports: vec![],
                exports: vec![],
            }],
            source: None,
            homepage: None,
            licenses: None,
            authors: None,
            revision: None,
            component_version: None,
            bill_of_materials: vec![],
            imports: vec![],
            exports: vec![],
        }
    }

    async fn insert_manifest(store: &Store) -> i64 {
        let repo = oci_repository::ActiveModel {
            registry: Set("ghcr.io".to_string()),
            repository: Set("test/comp".to_string()),
            created_at: Set(Utc::now()),
            updated_at: Set(Utc::now()),
            wit_namespace: Set(Some("ns".to_string())),
            wit_name: Set(Some("pkg".to_string())),
            kind: Set(Some("component".to_string())),
            ..Default::default()
        };
        let repo_id = oci_repository::Entity::insert(repo)
            .exec(store.db())
            .await
            .unwrap()
            .last_insert_id;
        let manifest = oci_manifest::ActiveModel {
            oci_repository_id: Set(repo_id),
            digest: Set("sha256:deadbeef".to_string()),
            created_at: Set(Utc::now()),
            ..Default::default()
        };
        oci_manifest::Entity::insert(manifest)
            .exec(store.db())
            .await
            .unwrap()
            .last_insert_id
    }

    #[tokio::test]
    async fn get_components_for_manifest_empty_for_wit_only() {
        let store = Store::open_in_memory().await.unwrap();
        let manifest_id = insert_manifest(&store).await;
        let components = store
            .get_components_for_manifest(manifest_id, None)
            .await
            .unwrap();
        assert!(components.is_empty());
    }

    #[tokio::test]
    async fn get_components_for_manifest_returns_module_children_and_targets() {
        let store = Store::open_in_memory().await.unwrap();
        let manifest_id = insert_manifest(&store).await;

        let producers_json = serde_json::to_string(&sample_summary()).unwrap();
        let component_id = insert_wasm_component(
            store.db(),
            manifest_id,
            None,
            Some("root"),
            None,
            Some(&producers_json),
        )
        .await
        .unwrap();

        // A native target with no declared version should inherit the tag.
        insert_component_target(
            store.db(),
            component_id,
            "ns:pkg",
            "the-world",
            None,
            None,
            true,
        )
        .await
        .unwrap();

        let components = store
            .get_components_for_manifest(manifest_id, Some("0.2.0"))
            .await
            .unwrap();
        assert_eq!(components.len(), 1);
        let comp = &components[0];

        let modules: Vec<_> = comp
            .children
            .iter()
            .filter(|c| c.kind.as_deref() == Some("module"))
            .collect();
        assert_eq!(modules.len(), 1, "core .wasm module child should render");
        assert_eq!(modules[0].name.as_deref(), Some("main.wasm"));

        assert_eq!(comp.targets.len(), 1);
        assert!(comp.targets[0].is_native);
        assert_eq!(comp.targets[0].version.as_deref(), Some("0.2.0"));
    }

    #[tokio::test]
    async fn insert_wasm_component_is_idempotent() {
        let store = Store::open_in_memory().await.unwrap();
        let manifest_id = insert_manifest(&store).await;
        let a = insert_wasm_component(store.db(), manifest_id, None, Some("root"), None, None)
            .await
            .unwrap();
        let b = insert_wasm_component(store.db(), manifest_id, None, Some("root"), None, None)
            .await
            .unwrap();
        assert_eq!(a, b, "re-extracting must not create duplicate rows");
        let count = wasm_component::Entity::find()
            .filter(wasm_component::Column::OciManifestId.eq(manifest_id))
            .count(store.db())
            .await
            .unwrap();
        assert_eq!(count, 1);
    }
}
