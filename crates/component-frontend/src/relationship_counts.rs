//! Result totals shown beside the sidebar relationship links.

use futures_concurrency::prelude::*;
use wasm_meta_registry_client::{
    ApiError, KnownPackage, PackageKind, RegistryClient, RelationshipPage, RelationshipTarget,
};

use crate::components::page_shell;
use crate::relationships::Relationship;

/// Totals for the fixed relationship queries; `None` means unavailable.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct RelationshipCounts {
    /// Total direct and transitive dependents of the package.
    pub(crate) dependents: Option<u64>,
    /// Total worlds importing the package or interface.
    pub(crate) imported_by: Option<u64>,
    /// Total worlds exporting the package or interface.
    pub(crate) exported_by: Option<u64>,
}

impl RelationshipCounts {
    /// The total for one relationship, if it was loaded.
    #[must_use]
    pub(crate) fn get(self, relation: Relationship) -> Option<u64> {
        match relation {
            Relationship::Dependents => self.dependents,
            Relationship::ImportedBy => self.imported_by,
            Relationship::ExportedBy => self.exported_by,
        }
    }

    /// Load the totals matching the sidebar's relationship links.
    ///
    /// Import and export totals are scoped to `interface` when given, to the
    /// whole package for interface-type packages, and skipped otherwise.
    /// Failed lookups degrade to an unavailable count.
    pub(crate) async fn fetch(
        client: &RegistryClient,
        pkg: &KnownPackage,
        interface: Option<&str>,
    ) -> Self {
        let package = page_shell::display_name_for(pkg);
        let Ok(package_target) = RelationshipTarget::new(&package, None) else {
            return Self::default();
        };
        let world_target = match interface {
            Some(interface) => RelationshipTarget::new(&package, Some(interface)).ok(),
            None if pkg.kind == Some(PackageKind::Interface) => Some(package_target.clone()),
            None => None,
        };
        let world_target = world_target.as_ref();
        let (dependents, imported_by, exported_by) = (
            async {
                let page = client
                    .fetch_dependents(package_target.package(), 0, 1)
                    .await;
                total(Relationship::Dependents, &package_target, page)
            },
            async {
                let target = world_target?;
                let page = client
                    .fetch_importing_worlds(target.package(), target.interface(), 0, 1)
                    .await;
                total(Relationship::ImportedBy, target, page)
            },
            async {
                let target = world_target?;
                let page = client
                    .fetch_exporting_worlds(target.package(), target.interface(), 0, 1)
                    .await;
                total(Relationship::ExportedBy, target, page)
            },
        )
            .join()
            .await;
        Self {
            dependents,
            imported_by,
            exported_by,
        }
    }
}

fn total<T>(
    relation: Relationship,
    target: &RelationshipTarget,
    page: Result<RelationshipPage<T>, ApiError>,
) -> Option<u64> {
    match page {
        Ok(page) => page.total,
        Err(error) => {
            eprintln!(
                "component-frontend: {} count for {} failed: {error}",
                relation.slug(),
                target.package()
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_are_looked_up_per_relationship() {
        let counts = RelationshipCounts {
            dependents: Some(3),
            imported_by: Some(0),
            exported_by: None,
        };
        assert_eq!(counts.get(Relationship::Dependents), Some(3));
        assert_eq!(counts.get(Relationship::ImportedBy), Some(0));
        assert_eq!(counts.get(Relationship::ExportedBy), None);
        assert_eq!(
            RelationshipCounts::default().get(Relationship::Dependents),
            None
        );
    }
}
