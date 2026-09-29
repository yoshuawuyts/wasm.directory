//! Relationship discovery over indexed WIT declarations and matching releases.

mod hydration;
mod queries;
mod releases;

#[cfg(test)]
mod tests;

use anyhow::Context;
use sea_orm::{FromQueryResult, Statement, Value};
use tokio_stream::StreamExt;
use wasm_meta_registry_types::{
    DependentPackage, MatchingWorld, RelationshipPage, RelationshipTarget,
};

use super::{Store, bind_placeholders};
use queries::WorldDirection;
use releases::{MatchingReleasePage, MatchingReleaseRow};

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
        let packages = hydration::packages(&self.db, &page.results).await?;
        let mut results = Vec::with_capacity(page.results.len());
        for row in page.results {
            results.push(DependentPackage {
                package: packages
                    .get(&row.repo_id)
                    .context("relationship repository no longer exists")?
                    .clone(),
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
        let packages = hydration::packages(&self.db, &page.results).await?;
        let worlds = hydration::worlds(&self.db, &page.results).await?;
        let mut results = Vec::with_capacity(page.results.len());
        for row in page.results {
            let world_id = row.world_id.context("relationship world ID is missing")?;
            let world = worlds
                .get(&world_id)
                .context("relationship world no longer exists")?;
            results.push(MatchingWorld {
                package: packages
                    .get(&row.repo_id)
                    .context("relationship repository no longer exists")?
                    .clone(),
                name: world.name.clone(),
                description: world.description.clone(),
                version: row.tag,
                is_synthetic: row.is_synthetic,
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
        let sql = queries::ordered_candidates(sql, backend)?;
        let statement =
            Statement::from_sql_and_values(backend, bind_placeholders(backend, &sql), values);
        let mut rows = MatchingReleaseRow::find_by_statement(statement)
            .stream(&self.db)
            .await?;
        let mut page = MatchingReleasePage::new(offset, limit);
        while let Some(row) = rows.next().await {
            page.push(row?)?;
        }
        Ok(page.finish())
    }
}
