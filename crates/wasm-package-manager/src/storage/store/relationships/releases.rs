//! Select a deterministic matching release per identity before pagination.

use std::cmp::Ordering;

use sea_orm::FromQueryResult;
use wasm_meta_registry_types::RelationshipPage;

#[cfg(test)]
mod tests;

/// Lightweight match data; full packages and world descriptions are loaded
/// only after release filtering, deduplication, and pagination.
#[derive(Debug, FromQueryResult)]
pub(super) struct MatchingReleaseRow {
    pub(super) repo_id: i64,
    pub(super) registry: String,
    pub(super) repository: String,
    pub(super) source_name: Option<String>,
    pub(super) tag: String,
    pub(super) world_id: Option<i64>,
    pub(super) world_name: Option<String>,
    pub(super) is_synthetic: bool,
}

impl MatchingReleaseRow {
    fn key(&self) -> (PackageIdentity<'_>, Option<&str>) {
        let identity = match self.source_name.as_deref() {
            Some(name) => PackageIdentity::Wit(name),
            None => PackageIdentity::Oci(&self.registry, &self.repository),
        };
        (identity, self.world_name.as_deref())
    }

    fn tie_breaker(&self) -> (&str, &str, &str, Option<i64>) {
        (&self.registry, &self.repository, &self.tag, self.world_id)
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum PackageIdentity<'a> {
    Wit(&'a str),
    Oci(&'a str, &'a str),
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

/// Retain the requested page and one identity's best release, not every match.
pub(super) struct MatchingReleasePage {
    page: RelationshipPage<MatchingReleaseRow>,
    current: Option<Release>,
    total: u64,
}

impl MatchingReleasePage {
    pub(super) fn new(offset: u32, limit: u32) -> Self {
        Self {
            page: RelationshipPage {
                results: Vec::new(),
                total: None,
                offset,
                limit,
                has_next: false,
            },
            current: None,
            total: 0,
        }
    }

    pub(super) fn push(&mut self, row: MatchingReleaseRow) -> anyhow::Result<()> {
        let Some(version) = crate::manager::parse_tag_as_semver(&row.tag) else {
            return Ok(());
        };
        let release = Release { row, version };
        let Some(current) = &self.current else {
            self.current = Some(release);
            return Ok(());
        };
        match release.row.key().cmp(&current.row.key()) {
            Ordering::Less => anyhow::bail!("Relationship candidate identities are not ordered"),
            Ordering::Equal if release.preferred_to(current) => self.current = Some(release),
            Ordering::Equal => {}
            Ordering::Greater => {
                self.finish_identity();
                self.current = Some(release);
            }
        }
        Ok(())
    }

    pub(super) fn finish(mut self) -> RelationshipPage<MatchingReleaseRow> {
        self.finish_identity();
        self.page.total = Some(self.total);
        self.page
    }

    fn finish_identity(&mut self) {
        let Some(release) = self.current.take() else {
            return;
        };
        let in_page = self.total >= u64::from(self.page.offset);
        self.total += 1;
        if !in_page {
            return;
        }
        let limit = usize::try_from(self.page.limit).expect("u32 limits fit supported targets");
        if self.page.results.len() < limit {
            self.page.results.push(release.row);
        } else {
            self.page.has_next = true;
        }
    }
}
