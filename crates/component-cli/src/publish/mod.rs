//! `component publish` — publish a single component or WIT interface
//! described by `wasm.toml` to an OCI registry.

#![allow(clippy::print_stdout)]

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use wasm_manifest::{Manifest, PackageKind};
use wasm_package_manager::manager::Manager;

/// Options for the top-level `component publish` command.
#[derive(clap::Args)]
pub(crate) struct Opts {
    /// Override the path to the artifact (component .wasm file or WIT
    /// directory). Mirrors the `[package].file` / `[package].wit`
    /// fields in the manifest.
    #[arg(long)]
    file: Option<PathBuf>,

    /// Print the publish plan, including layers, annotations, and the
    /// target reference that would be pushed, without actually
    /// contacting the registry.
    #[arg(long)]
    dry_run: bool,

    /// Path to the project directory containing `wasm.toml`. Defaults
    /// to the current directory.
    #[arg(long, default_value = ".")]
    manifest_path: PathBuf,
}

impl Opts {
    pub(crate) async fn run(self, offline: bool) -> Result<()> {
        let manifest_dir = self.manifest_path.clone();
        let manifest_file = manifest_dir.join("wasm.toml");
        let manifest_text = tokio::fs::read_to_string(&manifest_file)
            .await
            .with_context(|| format!("failed to read `{}`", manifest_file.display()))?;
        let mut manifest: Manifest = toml::from_str(&manifest_text)
            .with_context(|| format!("failed to parse `{}`", manifest_file.display()))?;

        // Apply --file override before anything else looks at the
        // manifest's [package] section.
        if let Some(file) = self.file.as_ref() {
            let pkg = manifest
                .package
                .as_mut()
                .context("--file requires the manifest to have a `[package]` section")?;
            match pkg.kind {
                PackageKind::Component => pkg.file = Some(file.clone()),
                PackageKind::Interface => pkg.wit = Some(file.clone()),
            }
        }

        let manager = if offline {
            Manager::open_offline().await?
        } else {
            Manager::open().await?
        };

        if self.dry_run {
            let plan = manager.publish_dry_run(&manifest, &manifest_dir).await?;
            println!("{}", plan.render());
            return Ok(());
        }

        if offline {
            bail!("cannot publish in offline mode");
        }

        let plan = manager.publish(&manifest, &manifest_dir).await?;
        println!(
            "{:>12} {} ({} bytes)",
            console::style("Published").green().bold(),
            plan.reference,
            plan.size_bytes,
        );
        Ok(())
    }
}
