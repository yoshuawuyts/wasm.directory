//! Landing-page highlight queries: new packages, recent releases, and
//! popular (most depended-upon) packages.

use std::collections::HashMap;

use sea_orm::{EntityTrait, FromQueryResult, QueryOrder, QuerySelect, Statement};
use wasm_meta_registry_types::{KnownPackage, PackageRelease, PopularPackage};
use wasm_package_manager_migration::entities::{oci_repository, oci_tag};

use super::{Store, bind_placeholders, known_package_from_repo};

/// Number of rows scanned per round-trip while collecting highlights.
const SCAN_BATCH: u64 = 100;

impl Store {
    /// List packages ordered by when they were first indexed, newest first.
    ///
    /// Packages without any semver tags (e.g. not-yet-pulled stubs) are
    /// skipped *before* pagination, so `offset` and `limit` count only
    /// packages that are actually returned.
    pub(crate) async fn list_new_known_packages(
        &self,
        offset: u32,
        limit: u32,
    ) -> anyhow::Result<Vec<KnownPackage>> {
        let limit = usize::try_from(limit).unwrap_or(usize::MAX);
        let mut to_skip = usize::try_from(offset).unwrap_or(usize::MAX);
        let mut out = Vec::new();
        let mut scanned = 0u64;
        while out.len() < limit {
            let rows = oci_repository::Entity::find()
                .order_by_desc(oci_repository::Column::CreatedAt)
                .order_by_desc(oci_repository::Column::Id)
                .offset(scanned)
                .limit(SCAN_BATCH)
                .all(&self.db)
                .await?;
            if rows.is_empty() {
                break;
            }
            scanned += SCAN_BATCH;
            for r in rows {
                if out.len() >= limit {
                    break;
                }
                let pkg = known_package_from_repo(&self.db, r).await?;
                if pkg.tags.is_empty() {
                    continue;
                }
                if to_skip > 0 {
                    to_skip -= 1;
                    continue;
                }
                out.push(pkg);
            }
        }
        Ok(out)
    }

    /// List the most recently indexed semver releases across all packages,
    /// newest first, one entry per `(package, version)`.
    pub(crate) async fn list_recent_releases(
        &self,
        limit: u32,
    ) -> anyhow::Result<Vec<PackageRelease>> {
        let limit = usize::try_from(limit).unwrap_or(usize::MAX);
        let mut out = Vec::new();
        let mut repos: HashMap<i64, Option<KnownPackage>> = HashMap::new();
        let mut offset = 0u64;
        while out.len() < limit {
            let tags = oci_tag::Entity::find()
                .order_by_desc(oci_tag::Column::CreatedAt)
                .order_by_desc(oci_tag::Column::Id)
                .offset(offset)
                .limit(SCAN_BATCH)
                .all(&self.db)
                .await?;
            if tags.is_empty() {
                break;
            }
            offset += SCAN_BATCH;
            for tag in tags {
                if out.len() >= limit {
                    break;
                }
                if let Some(release) = self.release_from_tag(tag, &mut repos).await? {
                    out.push(release);
                }
            }
        }
        Ok(out)
    }

    /// Turn a tag row into a release, skipping non-semver tags and tags whose
    /// repository is missing. Repository lookups are memoized in `repos`.
    async fn release_from_tag(
        &self,
        tag: oci_tag::Model,
        repos: &mut HashMap<i64, Option<KnownPackage>>,
    ) -> anyhow::Result<Option<PackageRelease>> {
        if crate::manager::parse_tag_as_semver(&tag.tag).is_none() {
            return Ok(None);
        }
        let id = tag.oci_repository_id;
        let package = match repos.get(&id).cloned() {
            Some(package) => package,
            None => self.load_package(id, repos).await?,
        };
        let Some(package) = package else {
            return Ok(None);
        };
        Ok(Some(PackageRelease {
            package,
            version: tag.tag,
            released_at: tag.created_at.to_rfc3339(),
        }))
    }

    /// Load the package for repository `id` and memoize it in `repos`.
    async fn load_package(
        &self,
        id: i64,
        repos: &mut HashMap<i64, Option<KnownPackage>>,
    ) -> anyhow::Result<Option<KnownPackage>> {
        let row = oci_repository::Entity::find_by_id(id).one(&self.db).await?;
        let package = match row {
            Some(repo) => Some(known_package_from_repo(&self.db, repo).await?),
            None => None,
        };
        repos.insert(id, package.clone());
        Ok(package)
    }

    /// List packages ranked by how many distinct *other* indexed
    /// repositories declare them as a WIT dependency.
    ///
    /// Only repositories with at least one tag are ranked; any whose tags
    /// are all non-semver are additionally dropped from the page.
    pub(crate) async fn list_popular_known_packages(
        &self,
        offset: u32,
        limit: u32,
    ) -> anyhow::Result<Vec<PopularPackage>> {
        let sql = "\
            SELECT repo.id AS repo_id, \
                   COUNT(DISTINCT dependent_repo.id) AS dependents \
            FROM wit_package_dependency wpd \
            JOIN wit_package wp ON wpd.dependent_id = wp.id \
            JOIN oci_manifest om ON wp.oci_manifest_id = om.id \
            JOIN oci_repository dependent_repo ON om.oci_repository_id = dependent_repo.id \
            JOIN oci_repository repo \
              ON repo.wit_namespace || ':' || repo.wit_name = wpd.declared_package \
            WHERE dependent_repo.id <> repo.id \
              AND EXISTS ( \
                SELECT 1 FROM oci_tag t WHERE t.oci_repository_id = repo.id \
              ) \
            GROUP BY repo.id, repo.repository \
            ORDER BY dependents DESC, repo.repository ASC \
            LIMIT ? OFFSET ?";
        let backend = self.db.get_database_backend();
        let stmt = Statement::from_sql_and_values(
            backend,
            bind_placeholders(backend, sql),
            [i64::from(limit).into(), i64::from(offset).into()],
        );
        #[derive(FromQueryResult)]
        struct Row {
            repo_id: i64,
            dependents: i64,
        }
        let rows = Row::find_by_statement(stmt).all(&self.db).await?;
        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let repo = oci_repository::Entity::find_by_id(row.repo_id)
                .one(&self.db)
                .await?;
            let Some(repo) = repo else { continue };
            let package = known_package_from_repo(&self.db, repo).await?;
            if package.tags.is_empty() {
                continue;
            }
            out.push(PopularPackage {
                package,
                dependents: u64::try_from(row.dependents).unwrap_or(0),
            });
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::super::{
        Store, insert_wit_package_dependency, upsert_oci_manifest, upsert_oci_repository_full,
        upsert_oci_tag, upsert_wit_package,
    };

    /// Insert a repository with a single manifest carrying `tags`. Returns
    /// `(repo_id, manifest_id)`.
    async fn seed_repo(store: &Store, ns: &str, name: &str, tags: &[&str]) -> (i64, i64) {
        let repository = format!("{ns}/{name}");
        let repo_id = upsert_oci_repository_full(
            &store.db,
            "ghcr.io",
            &repository,
            Some(ns),
            Some(name),
            None,
        )
        .await
        .expect("upsert repo");
        let digest = format!("sha256:{ns}{name}");
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
        .expect("upsert manifest");
        for tag in tags {
            upsert_oci_tag(&store.db, repo_id, tag, &digest)
                .await
                .expect("upsert tag");
        }
        (repo_id, manifest_id)
    }

    /// Record that the package `own_name` in `manifest_id` depends on
    /// `declared`.
    async fn seed_dependency(store: &Store, manifest_id: i64, own_name: &str, declared: &str) {
        let wp_id = upsert_wit_package(
            &store.db,
            own_name,
            Some("1.0.0"),
            None,
            None,
            Some(manifest_id),
            None,
        )
        .await
        .expect("upsert wit package");
        insert_wit_package_dependency(&store.db, wp_id, declared, Some("0.2.0"))
            .await
            .expect("insert dependency");
    }

    #[tokio::test]
    async fn new_packages_are_ordered_newest_first_and_skip_tagless() {
        let store = Store::open_in_memory().await.expect("open store");
        seed_repo(&store, "a", "first", &["1.0.0"]).await;
        seed_repo(&store, "b", "untagged", &[]).await;
        seed_repo(&store, "c", "second", &["0.1.0"]).await;

        let pkgs = store.list_new_known_packages(0, 10).await.expect("query");
        let names: Vec<_> = pkgs.iter().map(|p| p.repository.as_str()).collect();
        assert_eq!(names, ["c/second", "a/first"]);

        // A tag-less repo at the top must not eat into the page.
        seed_repo(&store, "d", "newest-untagged", &[]).await;
        let first = store.list_new_known_packages(0, 1).await.expect("query");
        assert_eq!(first[0].repository, "c/second");
        let second = store.list_new_known_packages(1, 1).await.expect("query");
        assert_eq!(second[0].repository, "a/first");
    }

    #[tokio::test]
    async fn recent_releases_list_each_semver_tag_newest_first() {
        let store = Store::open_in_memory().await.expect("open store");
        seed_repo(&store, "a", "one", &["1.0.0", "latest", "1.1.0"]).await;
        seed_repo(&store, "b", "two", &["0.1.0"]).await;

        let releases = store.list_recent_releases(10).await.expect("query");
        let got: Vec<_> = releases
            .iter()
            .map(|r| (r.package.repository.as_str(), r.version.as_str()))
            .collect();
        assert_eq!(
            got,
            [("b/two", "0.1.0"), ("a/one", "1.1.0"), ("a/one", "1.0.0")]
        );

        let limited = store.list_recent_releases(2).await.expect("query");
        assert_eq!(limited.len(), 2);
    }

    #[tokio::test]
    async fn popular_packages_count_distinct_other_dependents() {
        let store = Store::open_in_memory().await.expect("open store");
        seed_repo(&store, "wasi", "io", &["0.2.0"]).await;
        let (_, clocks_manifest) = seed_repo(&store, "wasi", "clocks", &["0.2.0"]).await;
        let (_, http_manifest) = seed_repo(&store, "wasi", "http", &["0.2.0"]).await;
        let (_, app_manifest) = seed_repo(&store, "ba", "app", &["1.0.0"]).await;
        seed_repo(&store, "ghost", "untagged", &[]).await;

        // wasi:io is used by clocks, http, and app; wasi:clocks by http only.
        seed_dependency(&store, clocks_manifest, "wasi:clocks", "wasi:io").await;
        seed_dependency(&store, http_manifest, "wasi:http", "wasi:io").await;
        seed_dependency(&store, http_manifest, "wasi:http", "wasi:clocks").await;
        seed_dependency(&store, app_manifest, "ba:app", "wasi:io").await;
        // A second edge from the same repository only counts once.
        seed_dependency(&store, app_manifest, "ba:app-extra", "wasi:io").await;
        // Tag-less targets are dropped; self-dependencies don't count.
        seed_dependency(&store, app_manifest, "ba:app", "ghost:untagged").await;
        seed_dependency(&store, app_manifest, "ba:app", "ba:app").await;

        let popular = store
            .list_popular_known_packages(0, 10)
            .await
            .expect("query");
        let got: Vec<_> = popular
            .iter()
            .map(|p| (p.package.repository.as_str(), p.dependents))
            .collect();
        assert_eq!(got, [("wasi/io", 3), ("wasi/clocks", 1)]);
    }

    /// The popularity query is hand-written SQL; make sure it also runs on
    /// Postgres when `COMPONENT_DATABASE_URL` points at a test database.
    #[tokio::test]
    async fn popular_packages_query_runs_postgres() {
        let Ok(url) = std::env::var("COMPONENT_DATABASE_URL") else {
            return;
        };
        let lower = url.to_ascii_lowercase();
        if !(lower.starts_with("postgres:") || lower.starts_with("postgresql:")) {
            return;
        }
        let data_dir = tempfile::tempdir().expect("create temp dir");
        let store = Store::open_at(data_dir.path().to_path_buf())
            .await
            .expect("open Postgres store");

        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let ns = format!("pgpopular{unique}");
        seed_repo(&store, &ns, "target", &["1.0.0"]).await;
        let (_, user_manifest) = seed_repo(&store, &ns, "user", &["1.0.0"]).await;
        seed_dependency(
            &store,
            user_manifest,
            &format!("{ns}:user"),
            &format!("{ns}:target"),
        )
        .await;

        let popular = store
            .list_popular_known_packages(0, 100)
            .await
            .expect("popular query must run on Postgres");
        let target = popular
            .iter()
            .find(|p| p.package.repository == format!("{ns}/target"))
            .expect("seeded package should be ranked");
        assert_eq!(target.dependents, 1);
        store
            .list_recent_releases(10)
            .await
            .expect("releases query must run on Postgres");
        store
            .list_new_known_packages(0, 10)
            .await
            .expect("new packages query must run on Postgres");
    }
}
