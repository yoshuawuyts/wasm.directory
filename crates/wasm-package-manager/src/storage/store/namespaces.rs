//! Registered namespace discovery, with indexed-release counts separate from membership.

use std::collections::{BTreeMap, HashMap};

use sea_orm::{
    ColumnTrait, Condition, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
};
use wasm_meta_registry_types::{KnownNamespace, KnownPackage, RegistryPage};
use wasm_package_manager_migration::entities::{oci_repository, oci_tag};

use super::{Store, known_packages_from_repos};

impl Store {
    pub(crate) async fn list_namespaces(
        &self,
        registered: &[String],
        offset: u32,
        limit: u32,
    ) -> anyhow::Result<RegistryPage<KnownNamespace>> {
        let releases = release_counts(&self.db).await?;
        let repos: Vec<(i64, String, Option<String>)> = oci_repository::Entity::find()
            .select_only()
            .column(oci_repository::Column::Id)
            .column(oci_repository::Column::Repository)
            .column(oci_repository::Column::WitNamespace)
            .into_tuple()
            .all(&self.db)
            .await?;
        let mut namespaces: BTreeMap<String, u64> =
            registered.iter().map(|name| (name.clone(), 0)).collect();
        for (id, repository, wit_namespace) in repos {
            let name = namespace_name(&repository, wit_namespace.as_deref());
            if releases.contains_key(&id)
                && let Some(count) = namespaces.get_mut(name)
            {
                *count += 1;
            }
        }
        let results = namespaces
            .into_iter()
            .map(|(name, packages)| KnownNamespace { name, packages })
            .collect();
        Ok(page(results, offset, limit))
    }

    pub(crate) async fn list_namespace_packages(
        &self,
        namespace: &str,
        offset: u32,
        limit: u32,
    ) -> anyhow::Result<RegistryPage<KnownPackage>> {
        let releases = release_counts(&self.db).await?;
        let repos = oci_repository::Entity::find()
            .filter(
                Condition::any()
                    .add(oci_repository::Column::WitNamespace.eq(namespace))
                    .add(oci_repository::Column::WitNamespace.is_null()),
            )
            .order_by_asc(oci_repository::Column::Repository)
            .order_by_asc(oci_repository::Column::Registry)
            .all(&self.db)
            .await?;
        let repos = repos
            .into_iter()
            .filter(|repo| {
                releases.contains_key(&repo.id)
                    && namespace_name(&repo.repository, repo.wit_namespace.as_deref()) == namespace
            })
            .collect();
        let page = page(repos, offset, limit);
        Ok(RegistryPage {
            results: known_packages_from_repos(&self.db, page.results).await?,
            total: page.total,
            offset: page.offset,
            limit: page.limit,
            has_next: page.has_next,
        })
    }
}

/// Keep namespace discovery and statistics consistent for unmapped repositories.
pub(super) fn namespace_name<'a>(repository: &'a str, wit_namespace: Option<&'a str>) -> &'a str {
    wit_namespace.unwrap_or_else(|| repository.split('/').next().unwrap_or_default())
}

/// Count only tags accepted by the canonical release parser.
pub(super) async fn release_counts(db: &DatabaseConnection) -> anyhow::Result<HashMap<i64, u64>> {
    let tags: Vec<(i64, String)> = oci_tag::Entity::find()
        .select_only()
        .column(oci_tag::Column::OciRepositoryId)
        .column(oci_tag::Column::Tag)
        .filter(oci_tag::Column::Tag.ne("latest"))
        .filter(oci_tag::Column::Tag.not_like("sha256-%"))
        .into_tuple()
        .all(db)
        .await?;
    let mut counts = HashMap::new();
    for (repo_id, tag) in tags {
        if crate::manager::parse_tag_as_semver(&tag).is_some() {
            *counts.entry(repo_id).or_default() += 1;
        }
    }
    Ok(counts)
}

fn page<T>(results: Vec<T>, offset: u32, limit: u32) -> RegistryPage<T> {
    let limit = limit.max(1);
    let total = u64::try_from(results.len()).expect("registry count fits in u64");
    RegistryPage {
        results: results
            .into_iter()
            .skip(usize::try_from(offset).expect("offset fits in usize"))
            .take(usize::try_from(limit).expect("limit fits in usize"))
            .collect(),
        total,
        offset,
        limit,
        has_next: u64::from(offset) + u64::from(limit) < total,
    }
}

#[cfg(test)]
mod tests;
