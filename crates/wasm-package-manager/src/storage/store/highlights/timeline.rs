//! Per-package release timelines used to rank landing-page highlights.
//!
//! Every semver tag is placed in time using its publish time and the time
//! this registry first indexed it, and tags are grouped by package identity
//! so each package is summarized exactly once.

use std::collections::HashMap;
use std::collections::hash_map::Entry;

use super::super::releases::ReleaseRow;
use chrono::{DateTime, Utc};

impl ReleaseRow {
    /// Who published the release: the registry plus the repository's
    /// first path segment (e.g. `ghcr.io/bytecodealliance`).
    fn publisher(&self) -> String {
        let owner = self
            .repository
            .split_once('/')
            .map_or(self.repository.as_str(), |(owner, _)| owner);
        format!("{}/{owner}", self.registry)
    }
}

/// A single semver release, placed in time.
#[derive(Clone)]
pub(super) struct Release {
    pub(super) repo_id: i64,
    pub(super) publisher: String,
    pub(super) tag_id: i64,
    pub(super) tag: String,
    pub(super) version: semver::Version,
    pub(super) released_at: DateTime<Utc>,
    pub(super) indexed_at: DateTime<Utc>,
}

impl Release {
    /// Order by publish time; ties break on insertion order so rankings are
    /// stable across requests.
    pub(super) fn sort_key(&self) -> (DateTime<Utc>, i64) {
        (self.released_at, self.tag_id)
    }
}

/// When one package was first and most recently published, and when this
/// registry first learned about it.
pub(super) struct PackageTimeline {
    /// The package's earliest release: when it first appeared.
    pub(super) first: Release,
    /// The package's most recent release.
    pub(super) latest: Release,
    /// When this registry first indexed any of the package's releases.
    pub(super) first_indexed: DateTime<Utc>,
}

impl PackageTimeline {
    /// Whether the latest release is an update rather than the package's
    /// debut. Compares versions, not tag rows, so one version mirrored to
    /// several repositories doesn't count as an update.
    pub(super) fn has_update(&self) -> bool {
        self.latest.version != self.first.version
    }

    /// Order by when the registry first learned about the package; ties
    /// (e.g. packages discovered in the same sync) break on first publish.
    pub(super) fn discovery_key(&self) -> (DateTime<Utc>, (DateTime<Utc>, i64)) {
        (self.first_indexed, self.first.sort_key())
    }

    fn new(release: Release) -> Self {
        Self {
            first_indexed: release.indexed_at,
            first: release.clone(),
            latest: release,
        }
    }

    fn absorb(&mut self, release: Release) {
        self.first_indexed = self.first_indexed.min(release.indexed_at);
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
        let Some(version) = crate::manager::parse_tag_as_semver(&row.tag) else {
            continue;
        };
        let key = PackageKey::of(&row);
        let indexed_at = row.indexed_at();
        let release = Release {
            repo_id: row.repo_id,
            publisher: row.publisher(),
            tag_id: row.tag_id,
            released_at: row.released_at(),
            indexed_at,
            tag: row.tag,
            version,
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

/// Keep at most `max` timelines per publisher (of their latest release),
/// preserving order, so one bulk publisher can't fill a whole column.
pub(super) fn cap_per_publisher(
    timelines: impl IntoIterator<Item = PackageTimeline>,
    max: usize,
) -> impl Iterator<Item = PackageTimeline> {
    let mut seen: HashMap<String, usize> = HashMap::new();
    timelines.into_iter().filter(move |t| {
        let count = seen.entry(t.latest.publisher.clone()).or_default();
        *count += 1;
        *count <= max
    })
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};

    use super::{ReleaseRow, package_timelines};

    fn at(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s)
            .expect("valid timestamp")
            .with_timezone(&Utc)
    }

    fn row(repository: &str, tag_at: &str, manifest_at: Option<&str>) -> ReleaseRow {
        ReleaseRow {
            tag_id: 1,
            repo_id: 1,
            tag: "1.0.0".to_owned(),
            tag_indexed_at: at(tag_at),
            manifest_indexed_at: manifest_at.map(at),
            oci_created: None,
            config_created: None,
            registry: "ghcr.io".to_owned(),
            repository: repository.to_owned(),
            wit_namespace: None,
            wit_name: None,
        }
    }

    #[test]
    fn indexed_at_prefers_earliest_sighting() {
        let recreated_tag = row("a/b", "2026-09-24T00:00:00Z", Some("2026-05-12T00:00:00Z"));
        assert_eq!(recreated_tag.indexed_at(), at("2026-05-12T00:00:00Z"));
        let no_manifest = row("a/b", "2026-09-24T00:00:00Z", None);
        assert_eq!(no_manifest.indexed_at(), at("2026-09-24T00:00:00Z"));
    }

    #[test]
    fn first_indexed_is_earliest_sighting_across_releases() {
        let mut debut = row("a/b", "2026-09-24T00:00:00Z", Some("2026-05-12T00:00:00Z"));
        debut.tag = "1.0.0".to_owned();
        let mut update = row("a/b", "2026-09-24T00:00:00Z", Some("2026-09-24T00:00:00Z"));
        update.tag_id = 2;
        update.tag = "2.0.0".to_owned();

        let timelines = package_timelines(vec![update, debut]);
        assert_eq!(timelines.len(), 1);
        assert_eq!(timelines[0].first_indexed, at("2026-05-12T00:00:00Z"));
        assert_eq!(timelines[0].latest.tag, "2.0.0");
    }

    #[test]
    fn mirrored_version_is_not_an_update() {
        let mut primary = row("a/b", "2026-09-01T00:00:00Z", None);
        primary.wit_namespace = Some("a".to_owned());
        primary.wit_name = Some("b".to_owned());
        let mut mirror = row("mirror/b", "2026-09-02T00:00:00Z", None);
        mirror.tag_id = 2;
        mirror.repo_id = 2;
        mirror.wit_namespace = Some("a".to_owned());
        mirror.wit_name = Some("b".to_owned());

        let timelines = package_timelines(vec![primary, mirror]);
        assert_eq!(timelines.len(), 1);
        assert!(!timelines[0].has_update());
    }

    #[test]
    fn publisher_is_registry_and_owner() {
        let t = "2026-01-01T00:00:00Z";
        assert_eq!(
            row("componentized/valkey/cli", t, None).publisher(),
            "ghcr.io/componentized"
        );
        assert_eq!(row("standalone", t, None).publisher(), "ghcr.io/standalone");
    }
}
