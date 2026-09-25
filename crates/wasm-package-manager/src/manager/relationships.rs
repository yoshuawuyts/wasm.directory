//! Version-independent discovery of indexed package relationships.

use wasm_meta_registry_types::{
    DependentPackage, MatchingWorld, RelationshipPage, RelationshipTarget,
};

use super::Manager;

impl Manager {
    /// List packages that directly or transitively depend on `package`.
    ///
    /// Matches exact, version-independent WIT names across indexed dependency
    /// declarations. Each package appears once, at its highest matching semver
    /// tag, excluding the target itself. Results are ordered by package identity;
    /// release filtering and deduplication precede pagination.
    ///
    /// The target need not be indexed. Invalid targets and database failures
    /// return errors. Embedded package tag lists remain unchanged; use the
    /// result's `version` to link to the release that actually matches.
    pub async fn list_dependents(
        &self,
        package: &str,
        offset: u32,
        limit: u32,
    ) -> anyhow::Result<RelationshipPage<DependentPackage>> {
        let target = RelationshipTarget::new(package, None)?;
        self.store.list_dependents(&target, offset, limit).await
    }

    /// List worlds with indexed imports from `package`.
    ///
    /// If `interface` is provided, match that exact member; otherwise match
    /// any interface in the package. Each owning-package/world pair appears
    /// once, at its highest matching semver tag, ordered by package identity
    /// and world name. This query does not traverse dependencies.
    ///
    /// The target need not be indexed. Invalid targets and database failures
    /// return errors. Release filtering and deduplication precede pagination.
    pub async fn list_importing_worlds(
        &self,
        package: &str,
        interface: Option<&str>,
        offset: u32,
        limit: u32,
    ) -> anyhow::Result<RelationshipPage<MatchingWorld>> {
        let target = RelationshipTarget::new(package, interface)?;
        self.store
            .list_importing_worlds(&target, offset, limit)
            .await
    }

    /// List worlds with indexed exports from `package`.
    ///
    /// If `interface` is provided, match that exact member; otherwise match
    /// any interface in the package. Each owning-package/world pair appears
    /// once, at its highest matching semver tag, ordered by package identity
    /// and world name. This query does not traverse dependencies.
    ///
    /// The target need not be indexed. Invalid targets and database failures
    /// return errors. Release filtering and deduplication precede pagination.
    pub async fn list_exporting_worlds(
        &self,
        package: &str,
        interface: Option<&str>,
        offset: u32,
        limit: u32,
    ) -> anyhow::Result<RelationshipPage<MatchingWorld>> {
        let target = RelationshipTarget::new(package, interface)?;
        self.store
            .list_exporting_worlds(&target, offset, limit)
            .await
    }
}
