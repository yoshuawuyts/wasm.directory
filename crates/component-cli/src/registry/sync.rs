//! `component registry sync` subcommand.

use anyhow::Result;
use component_package_manager::manager::{Manager, SyncPolicy, SyncResult};

/// Default sync interval in seconds (1 hour).
const SYNC_INTERVAL: u64 = Manager::DEFAULT_SYNC_INTERVAL;

/// Force-sync the package index from the configured meta-registry.
#[derive(clap::Args)]
pub(crate) struct SyncOpts {}

impl SyncOpts {
    pub(crate) async fn run(self) -> Result<()> {
        let manager = Manager::open().await?;
        let registry_url = Manager::default_registry_url();

        match manager
            .sync_from_meta_registry(&registry_url, SYNC_INTERVAL, SyncPolicy::Force)
            .await?
        {
            SyncResult::Updated { count } => {
                println!("Synced {count} packages from {registry_url}");
            }
            SyncResult::NotModified => {
                println!("Already up to date (verified with registry)");
            }
            SyncResult::Skipped => {
                println!("Already up to date (synced recently)");
            }
            SyncResult::Degraded { error } => {
                return Err(super::errors::SyncError::Degraded { reason: error }.into());
            }
        }

        Ok(())
    }
}
