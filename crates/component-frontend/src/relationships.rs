//! Fixed relationship views and their URL identities.

use wasm_meta_registry_client::RelationshipTarget;

use crate::components::ds::search_bar::encode_query;

/// The three relationship queries available from package navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Relationship {
    /// Direct and transitive package dependents.
    Dependents,
    /// Worlds importing a package or interface.
    ImportedBy,
    /// Worlds exporting a package or interface.
    ExportedBy,
}

impl Relationship {
    /// The path segment shared by frontend and API relationship routes.
    pub(crate) fn slug(self) -> &'static str {
        match self {
            Self::Dependents => "dependents",
            Self::ImportedBy => "imported-by",
            Self::ExportedBy => "exported-by",
        }
    }

    /// The compact navigation label.
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Dependents => "Dependents",
            Self::ImportedBy => "Imported by",
            Self::ExportedBy => "Exported by",
        }
    }

    /// Build the version-independent URL for a fixed query.
    pub(crate) fn href(self, target: &RelationshipTarget) -> String {
        let mut href = format!(
            "/search/{}?package={}",
            self.slug(),
            encode_query(target.package())
        );
        if let Some(interface) = target.interface() {
            href.push_str("&interface=");
            href.push_str(&encode_query(interface));
        }
        href
    }

    /// Describe the actual relationship rather than an ordinary text search.
    pub(crate) fn heading(self, target: &RelationshipTarget) -> String {
        let identity = match target.interface() {
            Some(interface) => format!("{}/{interface}", target.package()),
            None => target.package().to_owned(),
        };
        match self {
            Self::Dependents => format!("Dependents of {identity}"),
            Self::ImportedBy => format!("Worlds importing {identity}"),
            Self::ExportedBy => format!("Worlds exporting {identity}"),
        }
    }

    /// Explain the scope and indexed-data boundary.
    pub(crate) fn description(self) -> &'static str {
        match self {
            Self::Dependents => {
                "Direct and transitive dependents across indexed versions. Each package appears at its newest matching release."
            }
            Self::ImportedBy => {
                "Worlds with matching imports across indexed versions. Each world appears at its newest matching release."
            }
            Self::ExportedBy => {
                "Worlds with matching exports across indexed versions. Each world appears at its newest matching release."
            }
        }
    }

    /// Explain an empty result without implying an indexing failure.
    pub(crate) fn empty_message(self) -> &'static str {
        match self {
            Self::Dependents => "No dependents found in the indexed package dependencies.",
            Self::ImportedBy => "No worlds with matching imports found in the index.",
            Self::ExportedBy => "No worlds with matching exports found in the index.",
        }
    }
}
