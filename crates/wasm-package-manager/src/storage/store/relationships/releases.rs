//! Select a deterministic matching release per identity before pagination.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

use sea_orm::FromQueryResult;
use wasm_meta_registry_types::RelationshipPage;

/// Lightweight match data; full packages and world descriptions are loaded
/// only after release filtering, deduplication, and pagination.
#[derive(FromQueryResult)]
pub(super) struct MatchingReleaseRow {
    pub(super) repo_id: i64,
    pub(super) registry: String,
    pub(super) repository: String,
    pub(super) source_name: Option<String>,
    pub(super) tag: String,
    pub(super) world_id: Option<i64>,
    pub(super) world_name: Option<String>,
}

impl MatchingReleaseRow {
    fn key(&self) -> (PackageIdentity, Option<String>) {
        let identity = match &self.source_name {
            Some(name) => PackageIdentity::Wit(name.clone()),
            None => PackageIdentity::Oci(self.registry.clone(), self.repository.clone()),
        };
        (identity, self.world_name.clone())
    }

    fn tie_breaker(&self) -> (&str, &str, &str, Option<i64>) {
        (&self.registry, &self.repository, &self.tag, self.world_id)
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum PackageIdentity {
    Wit(String),
    Oci(String, String),
}

struct Release {
    row: MatchingReleaseRow,
    version: semver::Version,
}

impl Release {
    fn preferred_to(&self, other: &Self) -> bool {
        match self.version.cmp(&other.version) {
            Ordering::Greater => true,
            Ordering::Less => false,
            Ordering::Equal => self.row.tie_breaker() < other.row.tie_breaker(),
        }
    }
}

/// Totals and next-page detection both use the eligible deduplicated matches;
/// has_next is detected from an extra result, not from the display total.
pub(super) fn matching_release_page(
    rows: Vec<MatchingReleaseRow>,
    offset: u32,
    limit: u32,
) -> RelationshipPage<MatchingReleaseRow> {
    let mut matches: BTreeMap<_, Release> = BTreeMap::new();
    for row in rows {
        let Some(version) = crate::manager::parse_tag_as_semver(&row.tag) else {
            continue;
        };
        let key = row.key();
        let release = Release { row, version };
        match matches.entry(key) {
            Entry::Occupied(mut entry) => {
                if release.preferred_to(entry.get()) {
                    entry.insert(release);
                }
            }
            Entry::Vacant(entry) => {
                entry.insert(release);
            }
        }
    }
    let total = u64::try_from(matches.len()).ok();
    let mut remaining = matches
        .into_values()
        .skip(usize::try_from(offset).unwrap_or(usize::MAX));
    let results = remaining
        .by_ref()
        .take(usize::try_from(limit).unwrap_or(usize::MAX))
        .map(|release| release.row)
        .collect();
    RelationshipPage {
        results,
        total,
        offset,
        limit,
        has_next: remaining.next().is_some(),
    }
}
