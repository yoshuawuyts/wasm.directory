//! Error types for the `component run` CLI command.
//!
//! Validation errors (`CoreModule`, `InvalidBinary`, `NoVersionHeader`) live
//! in [`component_cli_internal_run::RunError`].

use miette::Diagnostic;

/// CLI-specific error type for `component run` command failures.
///
/// Each variant carries a stable [diagnostic error code][miette::Diagnostic::code]
/// that uniquely identifies the failure.
#[derive(Debug, Clone, PartialEq, Eq, Diagnostic)]
#[must_use]
pub(crate) enum RunError {
    /// The pulled OCI image has no manifest.
    #[diagnostic(
        code(component::run::no_manifest),
        help("ensure the OCI reference points to a valid Wasm package")
    )]
    NoManifest,

    /// The OCI manifest contains no `application/wasm` layer.
    #[diagnostic(
        code(component::run::no_wasm_layer),
        help("ensure the image contains an `application/wasm` layer")
    )]
    NoWasmLayer,

    /// A manifest component key is not present in the lockfile.
    #[diagnostic(
        code(component::run::not_in_lockfile),
        help("run `component install {name}` to populate the lockfile")
    )]
    NotInLockfile {
        /// The component key that was looked up.
        name: String,
    },

    /// The vendored file for a manifest component does not exist on disk.
    #[diagnostic(
        code(component::run::vendored_file_missing),
        help("'{path}' not found; run `component install {name}` to vendor the component")
    )]
    VendoredFileMissing {
        /// The expected file path.
        path: String,
        /// The component key.
        name: String,
    },

    /// The HTTP server could not bind to the requested address.
    #[diagnostic(
        code(component::run::http_bind_failed),
        help(
            "{reason}; ensure the address '{addr}' is available and not in use by another process"
        )
    )]
    HttpBindFailed {
        /// The requested bind address.
        addr: String,
        /// The underlying OS error message.
        reason: String,
    },

    /// The HTTP server failed to accept an incoming connection.
    #[diagnostic(
        code(component::run::http_accept_failed),
        help("the HTTP listener encountered an error: {reason}")
    )]
    HttpAcceptFailed {
        /// The underlying OS error message.
        reason: String,
    },

    /// The component requested with `--global` is not present in the local cache.
    #[diagnostic(
        code(component::run::not_in_global_cache),
        help("run `component install {name}` to fetch '{name}' into the cache first")
    )]
    NotInGlobalCache {
        /// The manifest key that was looked up.
        name: String,
    },
}

impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunError::NoManifest => {
                write!(f, "pulled image has no manifest")
            }
            RunError::NoWasmLayer => {
                write!(f, "manifest contains no application/wasm layer")
            }
            RunError::NotInLockfile { name } => {
                write!(
                    f,
                    "component '{name}' is in the manifest but not in the lockfile",
                )
            }
            RunError::VendoredFileMissing { path, name } => {
                write!(f, "vendored file '{path}' not found for component '{name}'")
            }
            RunError::HttpBindFailed { addr, reason } => {
                write!(f, "failed to bind HTTP server to {addr}: {reason}")
            }
            RunError::HttpAcceptFailed { reason } => {
                write!(f, "failed to accept incoming HTTP connection: {reason}")
            }
            RunError::NotInGlobalCache { name } => {
                write!(f, "component '{name}' is not present in the global cache")
            }
        }
    }
}

impl std::error::Error for RunError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_variants_have_error_codes() {
        use miette::Diagnostic;

        let variants: Vec<Box<dyn Diagnostic>> = vec![
            Box::new(RunError::NoManifest),
            Box::new(RunError::NoWasmLayer),
            Box::new(RunError::NotInLockfile {
                name: "test".to_string(),
            }),
            Box::new(RunError::VendoredFileMissing {
                path: "test".to_string(),
                name: "test".to_string(),
            }),
            Box::new(RunError::HttpBindFailed {
                addr: "127.0.0.1:8080".to_string(),
                reason: "test".to_string(),
            }),
            Box::new(RunError::HttpAcceptFailed {
                reason: "test".to_string(),
            }),
        ];

        let expected_codes = [
            "component::run::no_manifest",
            "component::run::no_wasm_layer",
            "component::run::not_in_lockfile",
            "component::run::vendored_file_missing",
            "component::run::http_bind_failed",
            "component::run::http_accept_failed",
        ];

        for (variant, expected_code) in variants.iter().zip(expected_codes.iter()) {
            assert_eq!(
                variant
                    .code()
                    .unwrap_or_else(|| panic!("{expected_code} must have a diagnostic code"))
                    .to_string(),
                *expected_code,
            );
            assert!(
                variant.help().is_some(),
                "{expected_code} must have a help message"
            );
        }
    }
}
