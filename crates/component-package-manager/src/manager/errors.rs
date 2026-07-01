//! Error types for the package manager.

use miette::Diagnostic;

/// Error type for package manager operation failures.
///
/// Each variant carries a stable [diagnostic error code][miette::Diagnostic::code]
/// that uniquely identifies the failure.
///
/// # Example
///
/// ```rust
/// use miette::Diagnostic;
/// use component_package_manager::manager::ManagerError;
///
/// let err = ManagerError::OfflinePull;
/// assert_eq!(
///     err.code().expect("should have a code").to_string(),
///     "component::manager::offline_pull",
/// );
///
/// let err = ManagerError::OfflineIndex;
/// assert_eq!(
///     err.code().expect("should have a code").to_string(),
///     "component::manager::offline_index",
/// );
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Diagnostic)]
#[must_use]
pub enum ManagerError {
    /// An attempt was made to pull a package while in offline mode.
    #[diagnostic(
        code(component::manager::offline_pull),
        help("run without `--offline` to pull packages from the registry")
    )]
    OfflinePull,

    /// An install was requested in offline mode for a package that is not
    /// present in the local cache.
    #[diagnostic(
        code(component::manager::offline_not_cached),
        help("run without `--offline` to fetch '{reference}' from the registry")
    )]
    OfflineNotCached {
        /// The reference that could not be served from the local cache.
        reference: String,
    },

    /// An attempt was made to index a package while in offline mode.
    #[diagnostic(
        code(component::manager::offline_index),
        help("run without `--offline` to index packages from the registry")
    )]
    OfflineIndex,

    /// A previously indexed package could not be retrieved from the database.
    #[diagnostic(
        code(component::manager::index_retrieval_failed),
        help("try re-indexing the package with `component registry sync`")
    )]
    IndexRetrievalFailed,

    /// Syncing the package index failed and no local data is available.
    #[diagnostic(
        code(component::manager::sync_no_local_data),
        help(
            "{reason}; check your network connection and run \
             `component registry sync` to fetch the package index"
        )
    )]
    SyncNoLocalData {
        /// The underlying error message from the failed sync.
        reason: String,
    },

    /// No tags were found for the given package reference.
    #[diagnostic(
        code(component::manager::no_tags_found),
        help("verify the package name '{registry}/{repository}' is correct")
    )]
    NoTagsFound {
        /// The registry host (e.g. `ghcr.io`).
        registry: String,
        /// The repository path (e.g. `webassembly/wasi-logging`).
        repository: String,
    },

    /// The package has tags in the registry, but none of them parse as
    /// strict semver (e.g. all tags are `latest`, `vX.Y.Z`, or hashes).
    #[diagnostic(
        code(component::manager::no_semver_tags),
        help("none of the tags for '{registry}/{repository}' parse as strict semver")
    )]
    NoSemverTags {
        /// The registry host.
        registry: String,
        /// The repository path.
        repository: String,
    },

    /// The requested tag does not exist in the registry.
    #[diagnostic(
        code(component::manager::manifest_not_found),
        help("tag '{tag}' not found for {registry}/{repository}; {hint}")
    )]
    ManifestNotFound {
        /// The tag that was requested (e.g. `latest`, `1.0.0`).
        tag: String,
        /// The registry host (e.g. `ghcr.io`).
        registry: String,
        /// The repository path (e.g. `webassembly/wasi-logging`).
        repository: String,
        /// Human-readable hint about available tags.
        hint: String,
    },
}

impl std::fmt::Display for ManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ManagerError::OfflinePull => {
                write!(f, "cannot pull packages in offline mode")
            }
            ManagerError::OfflineNotCached { reference } => {
                write!(f, "'{reference}' is not in the local cache")
            }
            ManagerError::OfflineIndex => {
                write!(f, "cannot index packages in offline mode")
            }
            ManagerError::IndexRetrievalFailed => {
                write!(f, "failed to retrieve indexed package")
            }
            ManagerError::SyncNoLocalData { reason } => {
                write!(f, "{reason}. No local data available")
            }
            ManagerError::NoTagsFound {
                registry,
                repository,
            } => {
                write!(f, "no tags found for {registry}/{repository}")
            }
            ManagerError::NoSemverTags {
                registry,
                repository,
            } => {
                write!(
                    f,
                    "no semver-tagged versions found for {registry}/{repository}"
                )
            }
            ManagerError::ManifestNotFound {
                tag,
                registry,
                repository,
                hint,
            } => {
                write!(
                    f,
                    "tag '{tag}' not found for {registry}/{repository}\n  help: {hint}"
                )
            }
        }
    }
}

impl std::error::Error for ManagerError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_variants_have_error_codes() {
        use miette::Diagnostic;

        let offline_pull = ManagerError::OfflinePull;
        assert_eq!(
            offline_pull
                .code()
                .expect("OfflinePull must have a diagnostic code")
                .to_string(),
            "component::manager::offline_pull",
        );
        assert!(
            offline_pull.help().is_some(),
            "OfflinePull must have a help message"
        );

        let offline_not_cached = ManagerError::OfflineNotCached {
            reference: "ghcr.io/example/comp:1.2.3".to_string(),
        };
        assert_eq!(
            offline_not_cached
                .code()
                .expect("OfflineNotCached must have a diagnostic code")
                .to_string(),
            "component::manager::offline_not_cached",
        );
        assert!(
            offline_not_cached.help().is_some(),
            "OfflineNotCached must have a help message"
        );

        let offline_index = ManagerError::OfflineIndex;
        assert_eq!(
            offline_index
                .code()
                .expect("OfflineIndex must have a diagnostic code")
                .to_string(),
            "component::manager::offline_index",
        );
        assert!(
            offline_index.help().is_some(),
            "OfflineIndex must have a help message"
        );

        let index_failed = ManagerError::IndexRetrievalFailed;
        assert_eq!(
            index_failed
                .code()
                .expect("IndexRetrievalFailed must have a diagnostic code")
                .to_string(),
            "component::manager::index_retrieval_failed",
        );
        assert!(
            index_failed.help().is_some(),
            "IndexRetrievalFailed must have a help message"
        );

        let sync_failed = ManagerError::SyncNoLocalData {
            reason: "connection refused".to_string(),
        };
        assert_eq!(
            sync_failed
                .code()
                .expect("SyncNoLocalData must have a diagnostic code")
                .to_string(),
            "component::manager::sync_no_local_data",
        );
        assert!(
            sync_failed.help().is_some(),
            "SyncNoLocalData must have a help message"
        );

        let no_tags = ManagerError::NoTagsFound {
            registry: "ghcr.io".to_string(),
            repository: "example/component".to_string(),
        };
        assert_eq!(
            no_tags
                .code()
                .expect("NoTagsFound must have a diagnostic code")
                .to_string(),
            "component::manager::no_tags_found",
        );
        assert!(
            no_tags.help().is_some(),
            "NoTagsFound must have a help message"
        );

        let manifest_not_found = ManagerError::ManifestNotFound {
            tag: "latest".to_string(),
            registry: "ghcr.io".to_string(),
            repository: "example/component".to_string(),
            hint: "available tags: 1.0.0, 2.0.0".to_string(),
        };
        assert_eq!(
            manifest_not_found
                .code()
                .expect("ManifestNotFound must have a diagnostic code")
                .to_string(),
            "component::manager::manifest_not_found",
        );
        assert!(
            manifest_not_found.help().is_some(),
            "ManifestNotFound must have a help message"
        );
        assert_eq!(
            manifest_not_found.to_string(),
            "tag 'latest' not found for ghcr.io/example/component\n  help: available tags: 1.0.0, 2.0.0",
        );
    }
}
