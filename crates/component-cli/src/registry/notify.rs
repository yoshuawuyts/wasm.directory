//! `component registry notify` subcommand.

#![allow(clippy::print_stdout, clippy::print_stderr)]

use anyhow::{Result, bail};
use wasm_meta_registry_types::NotifyOutcome;
use wasm_package_manager::Reference;
use wasm_package_manager::manager::{Manager, SyncPolicy, install};

/// Notify a meta-registry that a new version of a package is available.
///
/// Sends a hint to the meta-registry asking it to fetch the given tag as
/// soon as possible, instead of waiting for the next periodic sync.
#[derive(clap::Args)]
pub(crate) struct NotifyOpts {
    /// The newly-published package, given as a WIT-style name
    /// (e.g., `wasi:http@0.2.11`).
    package: String,

    /// URL of the meta-registry to notify.
    ///
    /// Defaults to <https://api.wasm.directory>, or the `COMPONENT_REGISTRY_URL`
    /// environment variable when it is set.
    #[arg(long)]
    registry_url: Option<String>,
}

impl NotifyOpts {
    pub(crate) async fn run(self, offline: bool) -> Result<()> {
        // The notify endpoint requires an HTTP request to the meta-registry,
        // so refuse early in offline mode before opening the store.
        if offline {
            bail!("cannot notify meta-registry in offline mode");
        }

        let registry_url = self
            .registry_url
            .clone()
            .unwrap_or_else(Manager::default_registry_url);

        let manager = Manager::open().await?;
        let reference = resolve_reference(&self.package, &registry_url, &manager).await?;

        let registry = reference.registry();
        let repository = reference.repository();
        let Some(tag) = reference.tag() else {
            bail!(
                "'{}' has no version; specify one (e.g., `wasi:http@0.2.11`) so the registry knows which version to fetch",
                self.package
            );
        };

        let outcome = manager
            .notify_meta_registry(&registry_url, registry, repository, tag)
            .await?;

        match outcome {
            NotifyOutcome::Enqueued => {
                println!(
                    "Notified {}: '{}' enqueued for fetch",
                    registry_url,
                    reference.whole()
                );
            }
            NotifyOutcome::Skipped { reason } => {
                println!(
                    "Notified {}: '{}' skipped ({reason})",
                    registry_url,
                    reference.whole()
                );
            }
        }
        Ok(())
    }
}

/// Resolve a WIT-style name to an OCI [`Reference`].
///
/// Only WIT-style names (`namespace:package@version`) are accepted; full OCI
/// references are rejected. The known-package index is opportunistically
/// refreshed from `registry_url` before lookup.
async fn resolve_reference(
    input: &str,
    registry_url: &str,
    manager: &Manager,
) -> Result<Reference> {
    if !install::looks_like_wit_name(input) {
        bail!(
            "'{input}' is not a WIT-style package name; expected `namespace:package@version` (e.g., `wasi:http@0.2.11`)"
        );
    }

    // Refresh the known-package index so resolution can find packages
    // that haven't been touched locally yet. Failures here are
    // non-fatal — fall through to the local lookup.
    let _ = manager
        .sync_from_meta_registry(
            registry_url,
            Manager::DEFAULT_SYNC_INTERVAL,
            SyncPolicy::IfStale,
        )
        .await;
    install::resolve_wit_name(input, manager).await
}
