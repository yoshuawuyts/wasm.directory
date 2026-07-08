//! CLI entry point for the component-meta-registry server.

use std::sync::Arc;

use anyhow::Context;
use clap::Parser;
use tracing::{error, info, warn};
use wasm_package_manager::manager::Manager;

use component_meta_registry::{Config, Indexer, router};

/// An HTTP server that indexes OCI registries for WebAssembly package
/// metadata and exposes a search API.
#[derive(Parser)]
#[command(author, version, about)]
struct Cli {
    /// Path to the registry directory containing per-namespace TOML files.
    registry_dir: std::path::PathBuf,

    /// Data directory for the registry's own cache (separate from the CLI cache).
    /// Defaults to `<OS data dir>/wasm-registry`.
    #[arg(long)]
    data_dir: Option<std::path::PathBuf>,

    /// Sync interval in seconds.
    #[arg(long, default_value_t = 3600)]
    sync_interval: u64,

    /// HTTP server bind address.
    #[arg(long, default_value = "0.0.0.0:8081")]
    bind: String,

    /// Re-index cached WIT packages during startup.
    ///
    /// This can significantly delay readiness on large caches.
    #[arg(long, default_value_t = false)]
    reindex_wit_on_startup: bool,

    /// Force re-fetch all package versions from the registry,
    /// bypassing the pull cooldown.
    #[arg(long, default_value_t = false)]
    refetch: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing — default to `info` level when RUST_LOG is not set.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    // Read and parse configuration from registry directory
    let config = Config::from_registry_dir(&cli.registry_dir, cli.sync_interval, cli.bind)?;

    // Determine the registry data directory (separate from the CLI cache)
    let data_dir = match cli.data_dir {
        Some(dir) => dir,
        None => dirs::data_local_dir()
            .context("No local data dir known for the current OS")?
            .join("wasm-registry"),
    };

    info!(
        bind = %config.bind,
        packages = config.packages.len(),
        sync_interval = config.sync_interval,
        data_dir = %data_dir.display(),
        "Starting component-meta-registry"
    );

    // Open the Manager for the HTTP server with its own data directory
    let server_manager = Manager::open_at(&data_dir).await?;

    // Back-fill the queue history with tags that were pulled before the
    // queue was introduced, so the status page shows them immediately.
    match server_manager.seed_completed_from_tags().await {
        Ok(n) if n > 0 => info!(count = n, "Seeded queue history from existing tags"),
        Ok(_) => {}
        Err(e) => warn!(error = %e, "Failed to seed queue history (non-fatal)"),
    }

    if cli.reindex_wit_on_startup {
        // Enqueue reindex tasks for all cached packages.  The background
        // indexer will process them during its first cycle.
        match server_manager.enqueue_reindex_all().await {
            Ok(n) if n > 0 => info!(count = n, "Enqueued WIT reindex tasks"),
            Ok(_) => {}
            Err(e) => warn!(error = %e, "Failed to enqueue WIT reindex tasks (non-fatal)"),
        }
    } else {
        info!("Skipping WIT re-index at startup (use --reindex-wit-on-startup to enable)");
    }

    let state = Arc::new(tokio::sync::RwLock::new(server_manager));

    // Run the background indexer on its own dedicated OS thread with a
    // single-threaded runtime and `LocalSet`, isolating its long-running
    // indexing loop from the Axum server's worker runtime. (The server
    // shares the `Manager` via `Arc<RwLock<Manager>>`, so `Manager` is
    // `Send + Sync`; the indexer simply runs on a separate runtime.)
    let indexer_config = config.clone();
    let cli_refetch = cli.refetch;
    let indexer_handle = std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to build tokio runtime for indexer");
        let local = tokio::task::LocalSet::new();
        local.block_on(&rt, async move {
            let manager = match Manager::open_at(&data_dir).await {
                Ok(m) => m,
                Err(e) => {
                    error!(error = %e, "Failed to open manager for indexer");
                    return;
                }
            };
            let indexer = Indexer::new(indexer_config, manager).with_refetch(cli_refetch);
            indexer.run().await;
        });
    });

    // Monitor indexer thread health
    tokio::spawn(async move {
        loop {
            if indexer_handle.is_finished() {
                error!("Indexer thread has stopped unexpectedly");
                break;
            }
            tokio::time::sleep(std::time::Duration::from_mins(1)).await;
        }
    });

    // Build and start HTTP server
    let app = router(state);
    let bind_addr = config.bind.clone();
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    info!("Listening on {}", bind_addr);
    axum::serve(listener, app).await?;

    Ok(())
}
