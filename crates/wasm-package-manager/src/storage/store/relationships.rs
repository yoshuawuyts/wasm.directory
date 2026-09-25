//! Relationship discovery over indexed WIT declarations and matching releases.

mod queries;
mod releases;

#[cfg(test)]
mod tests;

use anyhow::Context;
use sea_orm::{EntityTrait, FromQueryResult, Statement, Value};
use wasm_meta_registry_types::{
    DependentPackage, KnownPackage, MatchingWorld, RelationshipPage, RelationshipTarget,
};
use wasm_package_manager_migration::entities::{oci_repository, wit_world};

use super::{Store, bind_placeholders, known_package_from_repo};
use queries::WorldDirection;
use releases::{MatchingReleaseRow, matching_release_page};

impl Store {
    /// Find reverse dependency matches, retaining each origin's matching tag.
    pub(crate) async fn list_dependents(
        &self,
        target: &RelationshipTarget,
        offset: u32,
        limit: u32,
    ) -> anyhow::Result<RelationshipPage<DependentPackage>> {
        let page = self
            .relationship_candidates(
                &queries::dependents(),
                vec![target.package().into(), target.package().into()],
                offset,
                limit,
            )
            .await?;
        let mut results = Vec::with_capacity(page.results.len());
        for row in page.results {
            results.push(DependentPackage {
                package: self.relationship_package(row.repo_id).await?,
                version: row.tag,
            });
        }
        Ok(RelationshipPage {
            results,
            total: page.total,
            offset: page.offset,
            limit: page.limit,
            has_next: page.has_next,
        })
    }

    /// Find worlds importing the exact package and optional interface.
    pub(crate) async fn list_importing_worlds(
        &self,
        target: &RelationshipTarget,
        offset: u32,
        limit: u32,
    ) -> anyhow::Result<RelationshipPage<MatchingWorld>> {
        self.list_relationship_worlds(target, offset, limit, WorldDirection::Import)
            .await
    }

    /// Find worlds exporting the exact package and optional interface.
    pub(crate) async fn list_exporting_worlds(
        &self,
        target: &RelationshipTarget,
        offset: u32,
        limit: u32,
    ) -> anyhow::Result<RelationshipPage<MatchingWorld>> {
        self.list_relationship_worlds(target, offset, limit, WorldDirection::Export)
            .await
    }

    async fn list_relationship_worlds(
        &self,
        target: &RelationshipTarget,
        offset: u32,
        limit: u32,
        direction: WorldDirection,
    ) -> anyhow::Result<RelationshipPage<MatchingWorld>> {
        let mut values = vec![target.package().into()];
        if let Some(interface) = target.interface() {
            values.push(interface.into());
        }
        let page = self
            .relationship_candidates(
                &queries::worlds(direction, target.interface().is_some()),
                values,
                offset,
                limit,
            )
            .await?;
        let mut results = Vec::with_capacity(page.results.len());
        for row in page.results {
            let world_id = row.world_id.context("relationship world ID is missing")?;
            let world = wit_world::Entity::find_by_id(world_id)
                .one(&self.db)
                .await?
                .context("relationship world no longer exists")?;
            results.push(MatchingWorld {
                package: self.relationship_package(row.repo_id).await?,
                name: world.name,
                description: world.description,
                version: row.tag,
            });
        }
        Ok(RelationshipPage {
            results,
            total: page.total,
            offset: page.offset,
            limit: page.limit,
            has_next: page.has_next,
        })
    }

    async fn relationship_candidates(
        &self,
        sql: &str,
        values: Vec<Value>,
        offset: u32,
        limit: u32,
    ) -> anyhow::Result<RelationshipPage<MatchingReleaseRow>> {
        let backend = self.db.get_database_backend();
        let statement =
            Statement::from_sql_and_values(backend, bind_placeholders(backend, sql), values);
        let rows = MatchingReleaseRow::find_by_statement(statement)
            .all(&self.db)
            .await?;
        Ok(matching_release_page(rows, offset, limit))
    }

    async fn relationship_package(&self, repo_id: i64) -> anyhow::Result<KnownPackage> {
        let repo = oci_repository::Entity::find_by_id(repo_id)
            .one(&self.db)
            .await?
            .context("relationship repository no longer exists")?;
        known_package_from_repo(&self.db, repo).await
    }
}
