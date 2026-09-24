//! Landing-page highlight queries: new packages, recent releases, and
//! popular (most depended-upon) packages.

mod timeline;

use std::cmp::Reverse;

use sea_orm::{ColumnTrait, EntityTrait, FromQueryResult, QueryFilter, QueryOrder, Statement};
use wasm_meta_registry_types::{KnownPackage, NewPackage, PackageRelease, PopularPackage};
use wasm_package_manager_migration::entities::oci_repository;

use self::timeline::{
    PackageTimeline, RELEASE_ROWS_SQL, ReleaseRow, cap_per_publisher, package_timelines,
};
use super::{Store, known_package_from_repo};

/// Most entries one publisher may occupy in the new-packages and
/// recent-releases lists, so a bulk publish doesn't drown out everyone else.
const MAX_PER_PUBLISHER: usize = 2;

impl Store {
    /// List packages by when this registry first indexed them, newest
    /// first: fresh additions to the registry.
    ///
    /// A package's first-indexed time is the earliest time any of its
    /// semver releases was indexed; packages discovered together (e.g. in
    /// one sync) are ordered by when they were first published. Each package
    /// appears once, showing its latest tags alongside its first-indexed
    /// time, and each publisher (registry + owner) contributes at most
    /// [`MAX_PER_PUBLISHER`] packages. Packages without semver releases are
    /// skipped before pagination.
    pub(crate) async fn list_new_known_packages(
        &self,
        offset: u32,
        limit: u32,
    ) -> anyhow::Result<Vec<NewPackage>> {
        let mut timelines = self.package_timelines().await?;
        timelines.sort_by_key(|t| Reverse(t.discovery_key()));
        let offset = usize::try_from(offset).unwrap_or(usize::MAX);
        let limit = usize::try_from(limit).unwrap_or(usize::MAX);
        let mut out = Vec::new();
        let page = cap_per_publisher(timelines, MAX_PER_PUBLISHER)
            .skip(offset)
            .take(limit);
        for timeline in page {
            if let Some(package) = self.load_known_package(timeline.latest.repo_id).await? {
                out.push(NewPackage {
                    package,
                    first_indexed_at: timeline.first_indexed.to_rfc3339(),
                });
            }
        }
        Ok(out)
    }

    /// List the most recent update of each package, newest first.
    ///
    /// A release's time is the publisher's `org.opencontainers.image.created`
    /// manifest annotation when present and valid RFC 3339, then the config
    /// blob's `created` field, falling back to when this registry first
    /// indexed it. Only semver tags count. A
    /// package's first release is its debut (see
    /// [`Self::list_new_known_packages`]), so packages with a single release
    /// are left out. Each package appears at most once (with its newest
    /// release) and each publisher at most [`MAX_PER_PUBLISHER`] times, so
    /// bursts can't crowd out everything else.
    pub(crate) async fn list_recent_releases(
        &self,
        limit: u32,
    ) -> anyhow::Result<Vec<PackageRelease>> {
        let mut timelines = self.package_timelines().await?;
        timelines.retain(PackageTimeline::has_update);
        timelines.sort_by_key(|t| Reverse(t.latest.sort_key()));
        let limit = usize::try_from(limit).unwrap_or(usize::MAX);
        let mut out = Vec::new();
        let page = cap_per_publisher(timelines, MAX_PER_PUBLISHER).take(limit);
        for PackageTimeline { latest, .. } in page {
            if let Some(package) = self.load_known_package(latest.repo_id).await? {
                out.push(PackageRelease {
                    package,
                    version: latest.tag,
                    released_at: latest.released_at.to_rfc3339(),
                });
            }
        }
        Ok(out)
    }

    /// One release timeline per package, across every indexed tag.
    async fn package_timelines(&self) -> anyhow::Result<Vec<PackageTimeline>> {
        let backend = self.db.get_database_backend();
        let stmt = Statement::from_string(backend, RELEASE_ROWS_SQL);
        let rows = ReleaseRow::find_by_statement(stmt).all(&self.db).await?;
        Ok(package_timelines(rows))
    }

    /// Load a repository as a [`KnownPackage`], if it still exists.
    async fn load_known_package(&self, repo_id: i64) -> anyhow::Result<Option<KnownPackage>> {
        let repo = oci_repository::Entity::find_by_id(repo_id)
            .one(&self.db)
            .await?;
        match repo {
            Some(repo) => Ok(Some(known_package_from_repo(&self.db, repo).await?)),
            None => Ok(None),
        }
    }

    /// Load the newest repository for a WIT package that has at least one
    /// semver release, skipping mirrors that only carry e.g. `latest`.
    async fn load_released_wit_package(
        &self,
        namespace: &str,
        name: &str,
    ) -> anyhow::Result<Option<KnownPackage>> {
        let repos = oci_repository::Entity::find()
            .filter(oci_repository::Column::WitNamespace.eq(namespace))
            .filter(oci_repository::Column::WitName.eq(name))
            .order_by_desc(oci_repository::Column::Id)
            .all(&self.db)
            .await?;
        for repo in repos {
            let package = known_package_from_repo(&self.db, repo).await?;
            if !package.tags.is_empty() {
                return Ok(Some(package));
            }
        }
        Ok(None)
    }

    /// List packages ranked by how many distinct *other* indexed
    /// repositories declare them as a WIT dependency.
    ///
    /// Repositories sharing a WIT package name count as one package, shown
    /// via its newest repository with semver releases. Packages without
    /// semver tags are skipped before pagination, so pages
    /// stay full and offsets don't skip valid entries.
    pub(crate) async fn list_popular_known_packages(
        &self,
        offset: u32,
        limit: u32,
    ) -> anyhow::Result<Vec<PopularPackage>> {
        let sql = "\
            SELECT repo.wit_namespace AS wit_namespace, repo.wit_name AS wit_name, \
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
            GROUP BY repo.wit_namespace, repo.wit_name \
            ORDER BY dependents DESC, repo.wit_namespace ASC, repo.wit_name ASC";
        let backend = self.db.get_database_backend();
        let stmt = Statement::from_string(backend, sql);
        #[derive(FromQueryResult)]
        struct Row {
            wit_namespace: String,
            wit_name: String,
            dependents: i64,
        }
        let rows = Row::find_by_statement(stmt).all(&self.db).await?;
        let mut skip = usize::try_from(offset).unwrap_or(usize::MAX);
        let limit = usize::try_from(limit).unwrap_or(usize::MAX);
        let mut out = Vec::new();
        for row in rows {
            if out.len() >= limit {
                break;
            }
            let package = self
                .load_released_wit_package(&row.wit_namespace, &row.wit_name)
                .await?;
            let Some(package) = package else {
                continue;
            };
            if skip > 0 {
                skip -= 1;
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

    use chrono::{DateTime, Utc};
    use sea_orm::sea_query::Expr;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    use wasm_package_manager_migration::entities::{oci_manifest, oci_tag};

    use super::super::{
        Store, insert_wit_package_dependency, upsert_oci_manifest, upsert_oci_repository_full,
        upsert_oci_tag, upsert_wit_package,
    };
    use super::timeline::release_time;

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
        let names: Vec<_> = pkgs.iter().map(|p| p.package.repository.as_str()).collect();
        assert_eq!(names, ["c/second", "a/first"]);

        // A tag-less repo at the top must not eat into the page.
        seed_repo(&store, "d", "newest-untagged", &[]).await;
        let first = store.list_new_known_packages(0, 1).await.expect("query");
        assert_eq!(first[0].package.repository, "c/second");
        let second = store.list_new_known_packages(1, 1).await.expect("query");
        assert_eq!(second[0].package.repository, "a/first");
    }

    /// Add `tag` to `repo_id` on its own manifest, optionally annotated with
    /// a publisher creation time. Returns the manifest ID.
    async fn seed_release(store: &Store, repo_id: i64, tag: &str, created: Option<&str>) -> i64 {
        let digest = format!("sha256:{repo_id}-{tag}");
        let annotations: HashMap<String, String> = created
            .map(|c| ("org.opencontainers.image.created".to_owned(), c.to_owned()))
            .into_iter()
            .collect();
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
            &annotations,
        )
        .await
        .expect("upsert manifest");
        upsert_oci_tag(&store.db, repo_id, tag, &digest)
            .await
            .expect("upsert tag");
        manifest_id
    }

    #[tokio::test]
    async fn recent_releases_fall_back_to_config_created_time() {
        let store = Store::open_in_memory().await.expect("open store");
        let (config_repo, _) = seed_repo(&store, "a", "config", &[]).await;
        let (plain_repo, _) = seed_repo(&store, "b", "plain", &[]).await;
        seed_release(&store, config_repo, "0.1.0", Some("1999-01-01T00:00:00Z")).await;
        let manifest = seed_release(&store, config_repo, "0.2.0", None).await;
        store
            .set_manifest_config_created(manifest, "2001-02-03T04:05:06Z")
            .await
            .expect("set config created");
        // Without either timestamp, the release is dated at index time (now).
        seed_release(&store, plain_repo, "0.1.0", Some("1999-01-01T00:00:00Z")).await;
        let unchecked = seed_release(&store, plain_repo, "0.2.0", None).await;
        store
            .set_manifest_config_created(unchecked, "")
            .await
            .expect("set config created");

        let releases = store.list_recent_releases(10).await.expect("query");
        let got: Vec<_> = releases
            .iter()
            .map(|r| (r.package.repository.as_str(), r.released_at.as_str()))
            .collect();
        assert_eq!(got[0].0, "b/plain");
        assert_eq!(got[1], ("a/config", "2001-02-03T04:05:06+00:00"));
    }

    #[tokio::test]
    async fn recent_releases_skip_debuts_and_show_each_package_once() {
        let store = Store::open_in_memory().await.expect("open store");
        seed_repo(&store, "a", "one", &["1.0.0", "latest", "1.1.0"]).await;
        seed_repo(&store, "b", "two", &["0.1.0", "0.2.0"]).await;
        // A package's only release is its debut, covered by new packages.
        seed_repo(&store, "c", "debut", &["0.1.0"]).await;

        let releases = store.list_recent_releases(10).await.expect("query");
        let got: Vec<_> = releases
            .iter()
            .map(|r| (r.package.repository.as_str(), r.version.as_str()))
            .collect();
        assert_eq!(got, [("b/two", "0.2.0"), ("a/one", "1.1.0")]);

        let limited = store.list_recent_releases(1).await.expect("query");
        assert_eq!(limited.len(), 1);
        assert_eq!(limited[0].package.repository, "b/two");
    }

    #[tokio::test]
    async fn recent_releases_prefer_publish_time_over_index_time() {
        let store = Store::open_in_memory().await.expect("open store");
        let (old_repo, _) = seed_repo(&store, "a", "old", &[]).await;
        let (new_repo, _) = seed_repo(&store, "b", "new", &[]).await;
        let (plain_repo, _) = seed_repo(&store, "c", "plain", &[]).await;
        // `b:new` was published more recently but indexed first; `a:old` was
        // published earlier but indexed afterwards.
        seed_release(&store, new_repo, "1.0.0", Some("2001-01-01T00:00:00Z")).await;
        seed_release(&store, new_repo, "0.9.0", Some("1999-01-01T00:00:00Z")).await;
        seed_release(&store, old_repo, "0.9.0", Some("1998-01-01T00:00:00Z")).await;
        seed_release(&store, old_repo, "1.0.0", Some("2000-01-01T00:00:00Z")).await;
        // Unparseable annotations fall back to index time (now): newest.
        seed_release(&store, plain_repo, "0.1.0", Some("1990-01-01T00:00:00Z")).await;
        seed_release(&store, plain_repo, "0.2.0", Some("not a date")).await;

        let releases = store.list_recent_releases(10).await.expect("query");
        let got: Vec<_> = releases
            .iter()
            .map(|r| (r.package.repository.as_str(), r.version.as_str()))
            .collect();
        assert_eq!(
            got,
            [("c/plain", "0.2.0"), ("b/new", "1.0.0"), ("a/old", "1.0.0")]
        );
        assert_eq!(releases[1].released_at, "2001-01-01T00:00:00+00:00");
    }

    #[tokio::test]
    async fn highlights_cap_entries_per_publisher() {
        let store = Store::open_in_memory().await.expect("open store");
        seed_repo(&store, "solo", "x", &["0.1.0", "0.2.0"]).await;
        for name in ["a", "b", "c"] {
            seed_repo(&store, "bulk", name, &["0.1.0", "0.2.0"]).await;
        }

        let pkgs = store.list_new_known_packages(0, 10).await.expect("query");
        let names: Vec<_> = pkgs.iter().map(|p| p.package.repository.as_str()).collect();
        assert_eq!(names, ["bulk/c", "bulk/b", "solo/x"]);
        let second = store.list_new_known_packages(2, 1).await.expect("query");
        assert_eq!(
            second[0].package.repository, "solo/x",
            "offset counts capped list"
        );

        let releases = store.list_recent_releases(10).await.expect("query");
        let names: Vec<_> = releases
            .iter()
            .map(|r| r.package.repository.as_str())
            .collect();
        assert_eq!(names, ["bulk/c", "bulk/b", "solo/x"]);
    }

    /// Backdate when every manifest and tag of `repo_id` was indexed.
    async fn set_indexed_at(store: &Store, repo_id: i64, when: &str) {
        let when = DateTime::parse_from_rfc3339(when)
            .expect("valid timestamp")
            .with_timezone(&Utc);
        oci_manifest::Entity::update_many()
            .col_expr(oci_manifest::Column::CreatedAt, Expr::value(when))
            .filter(oci_manifest::Column::OciRepositoryId.eq(repo_id))
            .exec(&store.db)
            .await
            .expect("backdate manifests");
        oci_tag::Entity::update_many()
            .col_expr(oci_tag::Column::CreatedAt, Expr::value(when))
            .filter(oci_tag::Column::OciRepositoryId.eq(repo_id))
            .exec(&store.db)
            .await
            .expect("backdate tags");
    }

    #[tokio::test]
    async fn new_packages_rank_by_first_indexed_and_show_latest_tag() {
        let store = Store::open_in_memory().await.expect("open store");
        let (fresh, _) = seed_repo(&store, "a", "fresh", &[]).await;
        let (veteran, _) = seed_repo(&store, "b", "veteran", &[]).await;
        // `b:veteran` has a brand-new release, but the registry has known it
        // since 2020. `a:fresh` was published long ago but only just
        // discovered, so it is the fresh addition.
        seed_release(&store, veteran, "1.0.0", Some("2019-01-01T00:00:00Z")).await;
        set_indexed_at(&store, veteran, "2020-01-01T00:00:00Z").await;
        seed_release(&store, veteran, "2.0.0", Some("2025-01-01T00:00:00Z")).await;
        seed_release(&store, fresh, "0.1.0", Some("2010-01-01T00:00:00Z")).await;

        let pkgs = store.list_new_known_packages(0, 10).await.expect("query");
        let got: Vec<_> = pkgs
            .iter()
            .map(|p| {
                (
                    p.package.repository.as_str(),
                    p.package.tags.first().map(String::as_str),
                )
            })
            .collect();
        assert_eq!(
            got,
            [("a/fresh", Some("0.1.0")), ("b/veteran", Some("2.0.0"))]
        );
        assert_eq!(pkgs[1].first_indexed_at, "2020-01-01T00:00:00+00:00");
        assert!(pkgs[0].first_indexed_at > pkgs[1].first_indexed_at);
    }

    #[tokio::test]
    async fn new_packages_discovered_together_rank_by_first_publish() {
        let store = Store::open_in_memory().await.expect("open store");
        let (newer, _) = seed_repo(&store, "a", "newer", &[]).await;
        let (older, _) = seed_repo(&store, "b", "older", &[]).await;
        seed_release(&store, newer, "1.0.0", Some("2024-01-01T00:00:00Z")).await;
        seed_release(&store, older, "1.0.0", Some("2023-01-01T00:00:00Z")).await;
        for repo in [newer, older] {
            set_indexed_at(&store, repo, "2025-06-01T00:00:00Z").await;
        }

        let pkgs = store.list_new_known_packages(0, 10).await.expect("query");
        let names: Vec<_> = pkgs.iter().map(|p| p.package.repository.as_str()).collect();
        assert_eq!(names, ["a/newer", "b/older"]);
    }

    #[tokio::test]
    async fn highlights_dedupe_repositories_sharing_a_package_name() {
        let store = Store::open_in_memory().await.expect("open store");
        let (primary, _) = seed_repo(&store, "acme", "widget", &[]).await;
        let mirror = upsert_oci_repository_full(
            &store.db,
            "docker.io",
            "mirror/widget",
            Some("acme"),
            Some("widget"),
            None,
        )
        .await
        .expect("upsert mirror");
        seed_release(&store, primary, "1.0.0", Some("2020-01-01T00:00:00Z")).await;
        seed_release(&store, mirror, "1.1.0", Some("2021-01-01T00:00:00Z")).await;

        let pkgs = store.list_new_known_packages(0, 10).await.expect("query");
        assert_eq!(pkgs.len(), 1, "one entry per WIT package");
        assert_eq!(
            pkgs[0].package.repository, "mirror/widget",
            "shows latest release"
        );

        let releases = store.list_recent_releases(10).await.expect("query");
        assert_eq!(releases.len(), 1, "one entry per WIT package");
        assert_eq!(releases[0].version, "1.1.0");

        let (_, app_manifest) = seed_repo(&store, "ba", "app", &["1.0.0"]).await;
        seed_dependency(&store, app_manifest, "ba:app", "acme:widget").await;
        let popular = store
            .list_popular_known_packages(0, 10)
            .await
            .expect("query");
        assert_eq!(popular.len(), 1, "one entry per WIT package");
        assert_eq!(popular[0].dependents, 1);
    }

    #[test]
    fn release_time_parses_rfc3339_and_caps_future_dates() {
        let indexed = DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
            .expect("valid")
            .with_timezone(&Utc);
        let past = release_time([Some("2025-06-01T12:00:00+02:00"), None], indexed);
        assert_eq!(past.to_rfc3339(), "2025-06-01T10:00:00+00:00");
        let future = release_time([Some("2030-01-01T00:00:00Z"), None], indexed);
        assert_eq!(future, indexed);
        assert_eq!(release_time([Some("garbage"), None], indexed), indexed);
        assert_eq!(release_time([None, None], indexed), indexed);
        // The config blob's `created` is used when the annotation is missing
        // or invalid; an empty value means the config had none.
        let config = release_time([None, Some("2025-03-01T00:00:00.5Z")], indexed);
        assert_eq!(config.to_rfc3339(), "2025-03-01T00:00:00.500+00:00");
        let bad_annotation = release_time([Some("x"), Some("2025-03-01T00:00:00Z")], indexed);
        assert_eq!(bad_annotation.to_rfc3339(), "2025-03-01T00:00:00+00:00");
        assert_eq!(release_time([None, Some("")], indexed), indexed);
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

    /// Packages without semver tags are filtered before pagination, so they
    /// neither underfill a page nor shift later pages.
    #[tokio::test]
    async fn popular_packages_paginate_after_semver_filter() {
        let store = Store::open_in_memory().await.expect("open store");
        seed_repo(&store, "top", "latest-only", &["latest"]).await;
        seed_repo(&store, "a", "second", &["1.0.0"]).await;
        seed_repo(&store, "a", "third", &["1.0.0"]).await;
        let mut users = Vec::new();
        for i in 0..3 {
            let (_, manifest) = seed_repo(&store, "user", &format!("u{i}"), &["1.0.0"]).await;
            users.push(manifest);
        }
        // latest-only is most depended upon, but has no semver release.
        for (i, manifest) in users.iter().enumerate() {
            let own = format!("user:u{i}");
            seed_dependency(&store, *manifest, &own, "top:latest-only").await;
            if i < 2 {
                seed_dependency(&store, *manifest, &own, "a:second").await;
            }
        }
        seed_dependency(&store, users[0], "user:u0", "a:third").await;

        let names = |page: Vec<wasm_meta_registry_types::PopularPackage>| {
            page.into_iter()
                .map(|p| p.package.repository)
                .collect::<Vec<_>>()
        };
        let first = store
            .list_popular_known_packages(0, 1)
            .await
            .expect("query");
        assert_eq!(names(first), ["a/second"]);
        let second = store
            .list_popular_known_packages(1, 1)
            .await
            .expect("query");
        assert_eq!(names(second), ["a/third"]);
    }

    /// A newer mirror without semver tags doesn't hide a package whose
    /// other repository has releases.
    #[tokio::test]
    async fn popular_packages_skip_unreleased_mirrors() {
        let store = Store::open_in_memory().await.expect("open store");
        seed_repo(&store, "wasi", "io", &["0.2.0"]).await;
        let mirror = upsert_oci_repository_full(
            &store.db,
            "ghcr.io",
            "mirror/io",
            Some("wasi"),
            Some("io"),
            None,
        )
        .await
        .expect("upsert mirror");
        upsert_oci_manifest(
            &store.db,
            mirror,
            "sha256:mirror",
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
        upsert_oci_tag(&store.db, mirror, "latest", "sha256:mirror")
            .await
            .expect("upsert tag");
        let (_, user) = seed_repo(&store, "ba", "app", &["1.0.0"]).await;
        seed_dependency(&store, user, "ba:app", "wasi:io").await;

        let popular = store
            .list_popular_known_packages(0, 10)
            .await
            .expect("query");
        let got: Vec<_> = popular
            .iter()
            .map(|p| (p.package.repository.as_str(), p.dependents))
            .collect();
        assert_eq!(got, [("wasi/io", 1)]);
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
