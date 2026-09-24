//! Per-package release timelines used to rank landing-page highlights.
//!
//! Every semver tag is placed in time using its publish time, and tags are
//! grouped by package identity so each package is summarized exactly once.

use std::collections::HashMap;
use std::collections::hash_map::Entry;

use chrono::{DateTime, Utc};
use sea_orm::FromQueryResult;

/// Select every tag together with its repository identity and the
/// publisher's creation-time annotation.
pub(super) const RELEASE_ROWS_SQL: &str = "\
    SELECT t.id AS tag_id, t.oci_repository_id AS repo_id, t.tag AS tag, \
           t.created_at AS indexed_at, m.oci_created AS oci_created, \
           r.registry AS registry, r.repository AS repository, \
           r.wit_namespace AS wit_namespace, r.wit_name AS wit_name \
    FROM oci_tag t \
    JOIN oci_repository r ON r.id = t.oci_repository_id \
    LEFT JOIN oci_manifest m \
      ON m.oci_repository_id = t.oci_repository_id \
     AND m.digest = t.manifest_digest";

/// A tag joined with its repository and its manifest's publish-time
/// annotation.
#[derive(FromQueryResult)]
pub(super) struct ReleaseRow {
    pub(super) tag_id: i64,
    pub(super) repo_id: i64,
    pub(super) tag: String,
    pub(super) indexed_at: DateTime<Utc>,
    pub(super) oci_created: Option<String>,
    pub(super) registry: String,
    pub(super) repository: String,
    pub(super) wit_namespace: Option<String>,
    pub(super) wit_name: Option<String>,
}

/// A single semver release, placed in time.
#[derive(Clone)]
pub(super) struct Release {
    pub(super) repo_id: i64,
    pub(super) tag_id: i64,
    pub(super) tag: String,
    pub(super) released_at: DateTime<Utc>,
}

impl Release {
    /// Order by publish time; ties break on insertion order so rankings are
    /// stable across requests.
    pub(super) fn sort_key(&self) -> (DateTime<Utc>, i64) {
        (self.released_at, self.tag_id)
    }
}

/// When one package was first and most recently published.
pub(super) struct PackageTimeline {
    /// The package's earliest release: when it first appeared.
    pub(super) first: Release,
    /// The package's most recent release.
    pub(super) latest: Release,
}

impl PackageTimeline {
    fn new(release: Release) -> Self {
        Self {
            first: release.clone(),
            latest: release,
        }
    }

    fn absorb(&mut self, release: Release) {
        if release.sort_key() < self.first.sort_key() {
            self.first = release.clone();
        }
        if release.sort_key() > self.latest.sort_key() {
            self.latest = release;
        }
    }
}

/// What makes two repositories "the same package" on the landing page:
/// the WIT package name when known, otherwise the OCI reference.
#[derive(PartialEq, Eq, Hash)]
enum PackageKey {
    Wit(String, String),
    Oci(String, String),
}

impl PackageKey {
    fn of(row: &ReleaseRow) -> Self {
        match (&row.wit_namespace, &row.wit_name) {
            (Some(ns), Some(name)) => Self::Wit(ns.clone(), name.clone()),
            _ => Self::Oci(row.registry.clone(), row.repository.clone()),
        }
    }
}

/// Build one timeline per package from its semver tags. Non-semver tags
/// (e.g. `latest`) are ignored, so packages without any releases are absent.
pub(super) fn package_timelines(rows: Vec<ReleaseRow>) -> Vec<PackageTimeline> {
    let mut by_package: HashMap<PackageKey, PackageTimeline> = HashMap::new();
    for row in rows {
        if crate::manager::parse_tag_as_semver(&row.tag).is_none() {
            continue;
        }
        let key = PackageKey::of(&row);
        let release = Release {
            repo_id: row.repo_id,
            tag_id: row.tag_id,
            released_at: release_time(row.oci_created.as_deref(), row.indexed_at),
            tag: row.tag,
        };
        match by_package.entry(key) {
            Entry::Occupied(mut entry) => entry.get_mut().absorb(release),
            Entry::Vacant(entry) => {
                entry.insert(PackageTimeline::new(release));
            }
        }
    }
    by_package.into_values().collect()
}

/// Prefer the publisher-supplied creation time; fall back to index time.
///
/// A release can't have been published after we indexed it, so a
/// future-dated annotation is capped at the index time rather than pinning
/// the release to the top of the list.
pub(super) fn release_time(oci_created: Option<&str>, indexed_at: DateTime<Utc>) -> DateTime<Utc> {
    oci_created
        .and_then(|s| DateTime::parse_from_rfc3339(s.trim()).ok())
        .map_or(indexed_at, |t| t.with_timezone(&Utc).min(indexed_at))
}
