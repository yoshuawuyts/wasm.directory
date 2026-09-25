//! Page-batched enrichment without index-wide release reads or per-row aggregates.

#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use sea_orm::DatabaseConnection;
use wasm_package_manager_migration::entities::oci_repository;

use super::{dependents::dependent_counts, releases::release_rows};

// Stay below SQLite's bind limit even for callers requesting very large pages.
const BATCH_SIZE: usize = 400;

#[derive(Default)]
pub(super) struct PackageMetadata {
    dependents: HashMap<String, u64>,
    latest_releases: HashMap<i64, DateTime<Utc>>,
}

impl PackageMetadata {
    pub(super) async fn load(
        db: &DatabaseConnection,
        repos: &[oci_repository::Model],
    ) -> anyhow::Result<Self> {
        let mut metadata = Self::default();
        let identities: HashSet<_> = repos.iter().filter_map(identity).collect();
        let identities: Vec<_> = identities.into_iter().collect();
        for batch in identities.chunks(BATCH_SIZE) {
            for row in dependent_counts(db, Some(batch)).await? {
                let name = format!("{}:{}", row.wit_namespace, row.wit_name);
                metadata
                    .dependents
                    .insert(name, u64::try_from(row.dependents)?);
            }
        }
        let ids: Vec<_> = repos.iter().map(|repo| repo.id).collect();
        for batch in ids.chunks(BATCH_SIZE) {
            metadata.load_releases(db, batch).await?;
        }
        Ok(metadata)
    }

    async fn load_releases(&mut self, db: &DatabaseConnection, ids: &[i64]) -> anyhow::Result<()> {
        for row in release_rows(db, Some(ids)).await? {
            if crate::manager::parse_tag_as_semver(&row.tag).is_none() {
                continue;
            }
            let released_at = row.released_at();
            self.latest_releases
                .entry(row.repo_id)
                .and_modify(|time| *time = (*time).max(released_at))
                .or_insert(released_at);
        }
        Ok(())
    }

    pub(super) fn dependents(&self, repo: &oci_repository::Model) -> Option<u64> {
        identity(repo).map(|name| self.dependents.get(&name).copied().unwrap_or(0))
    }

    pub(super) fn latest_release_at(&self, repo_id: i64) -> Option<String> {
        self.latest_releases.get(&repo_id).map(DateTime::to_rfc3339)
    }
}

fn identity(repo: &oci_repository::Model) -> Option<String> {
    Some(format!(
        "{}:{}",
        repo.wit_namespace.as_deref()?,
        repo.wit_name.as_deref()?
    ))
}
