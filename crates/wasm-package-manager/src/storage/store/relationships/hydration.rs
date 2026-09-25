//! Hydrate only selected, deduplicated repository and world IDs.

use std::collections::{BTreeSet, HashMap};

use anyhow::Context;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use wasm_meta_registry_types::KnownPackage;
use wasm_package_manager_migration::entities::{oci_repository, wit_world};

use super::super::{known_packages_from_repos, package_metadata::BATCH_SIZE};
use super::releases::MatchingReleaseRow;

pub(super) async fn packages(
    db: &DatabaseConnection,
    rows: &[MatchingReleaseRow],
) -> anyhow::Result<HashMap<i64, KnownPackage>> {
    let ids: Vec<_> = rows
        .iter()
        .map(|row| row.repo_id)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut repos = Vec::with_capacity(ids.len());
    for batch in ids.chunks(BATCH_SIZE) {
        repos.extend(
            oci_repository::Entity::find()
                .filter(oci_repository::Column::Id.is_in(batch.iter().copied()))
                .all(db)
                .await?,
        );
    }
    let loaded_ids: Vec<_> = repos.iter().map(|repo| repo.id).collect();
    let packages = known_packages_from_repos(db, repos).await?;
    Ok(loaded_ids.into_iter().zip(packages).collect())
}

pub(super) async fn worlds(
    db: &DatabaseConnection,
    rows: &[MatchingReleaseRow],
) -> anyhow::Result<HashMap<i64, wit_world::Model>> {
    let ids: Vec<_> = rows
        .iter()
        .map(|row| row.world_id.context("relationship world ID is missing"))
        .collect::<anyhow::Result<BTreeSet<_>>>()?
        .into_iter()
        .collect();
    let mut worlds = HashMap::with_capacity(ids.len());
    for batch in ids.chunks(BATCH_SIZE) {
        let loaded = wit_world::Entity::find()
            .filter(wit_world::Column::Id.is_in(batch.iter().copied()))
            .all(db)
            .await?;
        worlds.extend(loaded.into_iter().map(|world| (world.id, world)));
    }
    Ok(worlds)
}
