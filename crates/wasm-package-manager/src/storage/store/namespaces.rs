//! Registered namespace discovery, with indexed-release counts separate from membership.

use std::collections::{BTreeSet, HashMap, HashSet};

use sea_orm::sea_query::LikeExpr;
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
        let names: Vec<&str> = registered
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let names = page(names, offset, limit);
        let counts = namespace_counts(&self.db, &names.results).await?;
        Ok(RegistryPage {
            results: names
                .results
                .iter()
                .map(|&name| KnownNamespace {
                    name: name.to_owned(),
                    packages: counts.get(name).copied().unwrap_or_default(),
                })
                .collect(),
            total: names.total,
            offset: names.offset,
            limit: names.limit,
            has_next: names.has_next,
        })
    }

    pub(crate) async fn list_namespace_packages(
        &self,
        namespace: &str,
        offset: u32,
        limit: u32,
    ) -> anyhow::Result<RegistryPage<KnownPackage>> {
        let condition = namespace_condition(&[namespace]);
        let released = released_repositories(&self.db, condition.clone()).await?;
        let repos = oci_repository::Entity::find()
            .filter(condition)
            .order_by_asc(oci_repository::Column::Repository)
            .order_by_asc(oci_repository::Column::Registry)
            .all(&self.db)
            .await?;
        let repos = repos
            .into_iter()
            .filter(|repo| {
                released.contains(&repo.id)
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

/// Count released repositories for only the requested namespaces.
async fn namespace_counts(
    db: &DatabaseConnection,
    names: &[&str],
) -> anyhow::Result<HashMap<String, u64>> {
    if names.is_empty() {
        return Ok(HashMap::new());
    }
    let condition = namespace_condition(names);
    let released = released_repositories(db, condition.clone()).await?;
    let repos: Vec<(i64, String, Option<String>)> = oci_repository::Entity::find()
        .select_only()
        .column(oci_repository::Column::Id)
        .column(oci_repository::Column::Repository)
        .column(oci_repository::Column::WitNamespace)
        .filter(condition)
        .into_tuple()
        .all(db)
        .await?;
    let requested: HashSet<&str> = names.iter().copied().collect();
    let mut counts = HashMap::new();
    for (id, repository, wit_namespace) in repos {
        let name = namespace_name(&repository, wit_namespace.as_deref());
        if released.contains(&id) && requested.contains(name) {
            *counts.entry(name.to_owned()).or_default() += 1;
        }
    }
    Ok(counts)
}

/// Match repositories whose WIT namespace, or owner fallback, may be one of `names`.
///
/// The owner-prefix `LIKE` can over-match (for example, case-insensitively on
/// SQLite), so callers must still confirm each row with [`namespace_name`].
fn namespace_condition(names: &[&str]) -> Condition {
    let mut condition = Condition::any();
    for &name in names {
        let owner = Condition::any()
            .add(oci_repository::Column::Repository.eq(name))
            .add(
                oci_repository::Column::Repository
                    .like(LikeExpr::new(format!("{}/%", escape_like(name))).escape('\\')),
            );
        condition = condition
            .add(oci_repository::Column::WitNamespace.eq(name))
            .add(
                Condition::all()
                    .add(oci_repository::Column::WitNamespace.is_null())
                    .add(owner),
            );
    }
    condition
}

fn escape_like(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for c in value.chars() {
        if matches!(c, '\\' | '%' | '_') {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    escaped
}

/// Repositories matching `condition` that have at least one semver release tag.
async fn released_repositories(
    db: &DatabaseConnection,
    condition: Condition,
) -> anyhow::Result<HashSet<i64>> {
    let tags: Vec<(i64, String)> = oci_tag::Entity::find()
        .select_only()
        .column(oci_tag::Column::OciRepositoryId)
        .column(oci_tag::Column::Tag)
        .inner_join(oci_repository::Entity)
        .filter(condition)
        .filter(oci_tag::Column::Tag.ne("latest"))
        .filter(oci_tag::Column::Tag.not_like("sha256-%"))
        .into_tuple()
        .all(db)
        .await?;
    Ok(tags
        .into_iter()
        .filter(|(_, tag)| crate::manager::parse_tag_as_semver(tag).is_some())
        .map(|(repo_id, _)| repo_id)
        .collect())
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
