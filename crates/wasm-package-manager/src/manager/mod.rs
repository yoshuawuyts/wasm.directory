use oci_client::Reference;
use oci_client::errors::{OciDistributionError, OciErrorCode};
use std::path::Path;
use tokio_stream::StreamExt;

mod errors;
/// Install helpers — core logic for resolving inputs, managing lockfiles,
/// and unpacking WIT files.
pub mod install;
mod logic;
mod models;

use crate::config::Config;
use crate::oci::{Client, ImageEntry, InsertResult};
use crate::progress::ProgressEvent;
use crate::publish::oci_tag;
use crate::storage::{FetchTaskKind, KnownPackage, KnownPackageParams, StateInfo, Store};
use crate::types::WitPackage;
use wasm_meta_registry_types::PackageKind;

pub use errors::ManagerError;
pub(crate) use logic::parse_tag_as_semver;
pub use logic::{
    derive_component_name, filter_tag_suggestions, pick_latest_stable_tag,
    sanitize_to_wit_identifier, should_sync, vendor_filename,
};
pub use models::{InstallResult, PullResult, SyncPolicy, SyncResult};

/// Outcome of [`Manager::process_next_task`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskOutcome {
    /// A task ran to completion.
    Succeeded,
    /// A task was dequeued but its execution failed; the failure has
    /// been recorded in the queue.
    Failed,
    /// The queue was empty; no work was performed.
    Empty,
}

/// How long (in seconds) to skip re-pulling a tag during background indexing
/// when its layers are already present in the local store.  Set to one hour
/// so server restarts don't trigger a full re-fetch of every known version.
const PULL_COOLDOWN_SECS: u64 = 3600;

/// A cache on disk
///
/// # Example
///
/// ```no_run
/// use wasm_package_manager::manager::Manager;
///
/// # #[tokio::main]
/// # async fn main() -> anyhow::Result<()> {
/// let manager = Manager::open().await?;
/// let images = manager.list_all().await?;
/// for image in &images {
///     println!("{}", image.reference());
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct Manager {
    client: Client,
    store: Store,
    config: Config,
    offline: bool,
}

impl Manager {
    /// Default meta-registry URL used for syncing the known-package index
    /// and for notifying the registry about newly-published versions.
    ///
    /// Points at the public production meta-registry API, served on its own
    /// `api.` subdomain so the backend can be reached (and diagnosed)
    /// independently of the frontend website at `https://wasm.directory`.
    /// Local development can redirect the CLI at a different instance by
    /// setting the `COMPONENT_REGISTRY_URL` environment variable; see
    /// [`default_registry_url`](Self::default_registry_url).
    pub const DEFAULT_REGISTRY_URL: &str = "https://api.wasm.directory";

    /// Environment variable that overrides [`DEFAULT_REGISTRY_URL`].
    ///
    /// Set it to point commands that talk to the meta-registry at a locally
    /// running instance, e.g. `COMPONENT_REGISTRY_URL=http://localhost:8081`.
    ///
    /// [`DEFAULT_REGISTRY_URL`]: Self::DEFAULT_REGISTRY_URL
    pub const ENV_REGISTRY_URL: &str = "COMPONENT_REGISTRY_URL";

    /// Default sync interval in seconds (1 hour).
    ///
    /// Controls how often the local package index is refreshed from the
    /// meta-registry.
    pub const DEFAULT_SYNC_INTERVAL: u64 = 3600;

    /// Resolve the meta-registry URL commands should target by default.
    ///
    /// Honors the [`ENV_REGISTRY_URL`](Self::ENV_REGISTRY_URL) environment
    /// variable when it is set to a non-empty value, and otherwise falls back
    /// to [`DEFAULT_REGISTRY_URL`](Self::DEFAULT_REGISTRY_URL). Routing every
    /// command through this keeps a single environment variable able to point
    /// the whole CLI at a local meta-registry during development.
    #[must_use]
    pub fn default_registry_url() -> String {
        std::env::var(Self::ENV_REGISTRY_URL)
            .ok()
            .filter(|url| !url.is_empty())
            .unwrap_or_else(|| Self::DEFAULT_REGISTRY_URL.to_string())
    }
}

impl Manager {
    /// Create a new store at a location on disk.
    ///
    /// This may return an error if it fails to create the cache location on disk.
    /// Loads configuration from the default config location.
    pub async fn open() -> anyhow::Result<Self> {
        Self::open_with_offline(false).await
    }

    /// Create a new Manager at a location on disk with offline mode.
    ///
    /// When offline is true, network operations will fail with an error.
    /// This may return an error if it fails to create the cache location on disk.
    pub async fn open_offline() -> anyhow::Result<Self> {
        Self::open_with_offline(true).await
    }

    /// Create a new Manager with the specified offline mode.
    async fn open_with_offline(offline: bool) -> anyhow::Result<Self> {
        let config = Config::load()?;
        let client = Client::new(config.clone());
        let store = Store::open().await?;

        Ok(Self {
            client,
            store,
            config,
            offline,
        })
    }

    /// Create a new store at a custom data directory on disk.
    ///
    /// This opens a separate cache at the specified path, isolated from the
    /// default location. Useful for running multiple instances (e.g. a
    /// registry server) without sharing state.
    ///
    /// This may return an error if it fails to create the cache location on disk.
    pub async fn open_at(data_dir: impl Into<std::path::PathBuf>) -> anyhow::Result<Self> {
        let config = Config::load()?;
        let client = Client::new(config.clone());
        let store = Store::open_at(data_dir).await?;

        Ok(Self {
            client,
            store,
            config,
            offline: false,
        })
    }

    /// Returns whether the manager is in offline mode.
    #[must_use]
    pub fn is_offline(&self) -> bool {
        self.offline
    }

    /// Create a new store with a specific configuration.
    ///
    /// This may return an error if it fails to create the cache location on disk.
    pub async fn with_config(config: Config) -> anyhow::Result<Self> {
        let client = Client::new(config.clone());
        let store = Store::open().await?;

        Ok(Self {
            client,
            store,
            config,
            offline: false,
        })
    }

    /// Pull a package from the registry.
    /// Returns the insert result indicating whether the package was newly inserted
    /// or already existed in the database.
    ///
    /// This method also fetches all related tags for the package and stores them
    /// as known packages for discovery purposes.
    ///
    /// # Errors
    ///
    /// Returns an error if offline mode is enabled.
    pub async fn pull(&self, reference: Reference) -> anyhow::Result<PullResult> {
        if self.offline {
            return Err(ManagerError::OfflinePull.into());
        }

        let image = match self.client.pull(&reference).await {
            Ok(image) => image,
            Err(err) => return Err(self.enrich_manifest_error(err, &reference).await),
        };

        // Validate the OCI bundle has exactly one WASM layer.
        if let Some(ref manifest) = image.manifest {
            crate::oci::validate_single_wasm_layer(&manifest.layers)?;
        }

        let (result, digest, manifest, manifest_id) = self.store.insert(&reference, image).await?;

        // Add to known packages when pulling (with tag if present)
        self.store
            .add_known_package(
                reference.registry(),
                reference.repository(),
                reference.tag(),
                None,
            )
            .await?;

        // Enrichment (tag listing + referrer discovery) hits the network and
        // only matters when we stored a new manifest. Skip it on cache hits so
        // re-pulling an already-present version stays local.
        if result == InsertResult::Inserted {
            self.store_related_tags(&reference).await?;

            // Best-effort: discover and store referrers (signatures, SBOMs, etc.)
            if let (Some(manifest_id), Some(digest)) = (manifest_id, &digest) {
                self.try_store_referrers(&reference, digest, manifest_id)
                    .await;
            }
        }

        Ok(PullResult {
            insert_result: result,
            digest,
            manifest,
        })
    }

    /// Pull a package from the registry with per-layer progress reporting.
    ///
    /// This method streams layers individually and sends `ProgressEvent`s
    /// via the provided channel to enable progress bar rendering.
    ///
    /// # Errors
    ///
    /// Returns an error if offline mode is enabled or if any network/storage
    /// operation fails.
    pub async fn pull_with_progress(
        &self,
        reference: Reference,
        progress_tx: &tokio::sync::mpsc::Sender<ProgressEvent>,
    ) -> anyhow::Result<PullResult> {
        if self.offline {
            return Err(ManagerError::OfflinePull.into());
        }

        // Fetch manifest and config
        let (manifest, digest) = match self.client.pull_manifest(&reference).await {
            Ok(result) => result,
            Err(err) => return Err(self.enrich_manifest_error(err, &reference).await),
        };

        // Validate the OCI bundle has exactly one WASM layer.
        crate::oci::validate_single_wasm_layer(&manifest.layers)?;

        let layer_count = manifest.layers.len();
        let _ = progress_tx
            .send(ProgressEvent::ManifestFetched {
                layer_count,
                image_digest: digest.clone(),
            })
            .await;

        // Calculate total size from manifest layer descriptors
        let size_on_disk: u64 = manifest
            .layers
            .iter()
            // `max(0)` clamps any negative descriptor size to 0, so the
            // remaining non-negative `i64` always fits in a `u64`.
            .map(|l| u64::try_from(l.size.max(0)).unwrap_or(0))
            .sum();

        // Insert metadata into the database
        let (result, image_id) = self
            .store
            .insert_metadata(&reference, Some(&digest), &manifest, size_on_disk)
            .await?;

        if result == InsertResult::Inserted {
            // Stream and store each layer individually with progress
            for (index, layer_descriptor) in manifest.layers.iter().enumerate() {
                // Guarded by `size > 0`, so the positive `i64` always fits `u64`.
                let total_bytes = if layer_descriptor.size > 0 {
                    Some(u64::try_from(layer_descriptor.size).unwrap_or(0))
                } else {
                    None
                };

                let _ = progress_tx
                    .send(ProgressEvent::LayerStarted {
                        index,
                        digest: layer_descriptor.digest.clone(),
                        total_bytes,
                        title: layer_descriptor
                            .annotations
                            .as_ref()
                            .and_then(|a| a.get("org.opencontainers.image.title").cloned()),
                        media_type: layer_descriptor.media_type.clone(),
                    })
                    .await;

                // Stream the layer data
                let mut stream = self
                    .client
                    .pull_layer_stream(&reference, layer_descriptor)
                    .await?;

                let mut layer_data = Vec::new();
                let mut bytes_downloaded: u64 = 0;

                while let Some(chunk) = stream.next().await {
                    let chunk = chunk?;
                    // `usize` always fits in `u64` on supported platforms.
                    bytes_downloaded += u64::try_from(chunk.len()).unwrap_or(0);
                    layer_data.extend_from_slice(&chunk);

                    let _ = progress_tx
                        .send(ProgressEvent::LayerProgress {
                            index,
                            bytes_downloaded,
                        })
                        .await;
                }

                let _ = progress_tx
                    .send(ProgressEvent::LayerDownloaded { index })
                    .await;

                // Store the layer (with annotations from the descriptor)
                self.store
                    .insert_layer(
                        &layer_descriptor.digest,
                        &layer_data,
                        image_id,
                        Some(layer_descriptor.media_type.as_str()),
                        crate::convert::index_to_i32(index)?,
                        layer_descriptor.annotations.as_ref(),
                    )
                    .await?;

                let _ = progress_tx.send(ProgressEvent::LayerStored { index }).await;
            }
        } else {
            // Package already cached — show layers as completed
            for (index, layer_descriptor) in manifest.layers.iter().enumerate() {
                // Guarded by `size > 0`, so the positive `i64` always fits `u64`.
                let total_bytes = if layer_descriptor.size > 0 {
                    Some(u64::try_from(layer_descriptor.size).unwrap_or(0))
                } else {
                    None
                };

                let _ = progress_tx
                    .send(ProgressEvent::LayerStarted {
                        index,
                        digest: layer_descriptor.digest.clone(),
                        total_bytes,
                        title: layer_descriptor
                            .annotations
                            .as_ref()
                            .and_then(|a| a.get("org.opencontainers.image.title").cloned()),
                        media_type: layer_descriptor.media_type.clone(),
                    })
                    .await;

                let _ = progress_tx.send(ProgressEvent::LayerStored { index }).await;
            }
        }

        // Add to known packages when pulling (with tag if present)
        self.store
            .add_known_package(
                reference.registry(),
                reference.repository(),
                reference.tag(),
                None,
            )
            .await?;

        // Enrichment (tag listing + referrer discovery) hits the network and
        // only matters when we stored a new manifest. Skip it on cache hits so
        // re-pulling an already-present version stays local.
        if result == InsertResult::Inserted {
            self.store_related_tags(&reference).await?;

            // Best-effort: discover and store referrers (signatures, SBOMs, etc.)
            if let Some(manifest_id) = image_id {
                self.try_store_referrers(&reference, &digest, manifest_id)
                    .await;
            }
        }

        Ok(PullResult {
            insert_result: result,
            digest: Some(digest),
            manifest: Some(manifest),
        })
    }

    /// Reflink a cached layer to a destination path.
    ///
    /// Looks up the cached layer by `layer_digest` and creates a reflink
    /// (copy-on-write clone) at `dest` from the content-addressed file inside
    /// the global store.
    ///
    /// # Errors
    ///
    /// Returns an error if the layer is not present in the cache, if the
    /// destination and the cache are on different filesystems, if the
    /// filesystem does not support reflinks (known-working: APFS, XFS, btrfs,
    /// ReFS/Windows DevDrive), or if the destination path is invalid.
    pub async fn vendor(&self, layer_digest: &str, dest: &Path) -> anyhow::Result<()> {
        use anyhow::Context as _;
        let cache = self.store.state_info.store_dir();
        cacache::reflink(cache, layer_digest, dest)
            .await
            .with_context(|| {
                format!(
                    "failed to reflink layer {layer_digest} to {}",
                    dest.display()
                )
            })?;
        Ok(())
    }

    /// Install a package from the registry.
    ///
    /// This high-level method:
    /// 1. Pulls the package from the registry (or uses the cache)
    /// 2. Filters the manifest's layers for `application/wasm` media type
    /// 3. Reflinks each wasm layer to the vendor directory
    /// 4. Returns an `InstallResult` with metadata for updating manifest/lockfile
    ///
    /// # Errors
    ///
    /// Returns an error if pulling, vendoring, or filesystem operations fail.
    pub async fn install(
        &self,
        reference: Reference,
        vendor_dir: &Path,
    ) -> anyhow::Result<InstallResult> {
        self.install_inner(reference, vendor_dir, None).await
    }

    /// Install a package from the registry with per-layer progress reporting.
    ///
    /// Like [`install`](Self::install), but sends `ProgressEvent`s via the provided
    /// channel to enable progress bar rendering in the CLI.
    ///
    /// # Errors
    ///
    /// Returns an error if pulling, vendoring, or filesystem operations fail.
    pub async fn install_with_progress(
        &self,
        reference: Reference,
        vendor_dir: &Path,
        progress_tx: &tokio::sync::mpsc::Sender<ProgressEvent>,
    ) -> anyhow::Result<InstallResult> {
        self.install_inner(reference, vendor_dir, Some(progress_tx))
            .await
    }

    /// Shared implementation for [`install`](Self::install) and
    /// [`install_with_progress`](Self::install_with_progress).
    ///
    /// When `progress_tx` is `Some`, layers are pulled with per-layer progress
    /// reporting and a [`ProgressEvent::InstallComplete`] event is emitted at
    /// the end; when `None`, the package is pulled without progress reporting.
    async fn install_inner(
        &self,
        reference: Reference,
        vendor_dir: &Path,
        progress_tx: Option<&tokio::sync::mpsc::Sender<ProgressEvent>>,
    ) -> anyhow::Result<InstallResult> {
        // Fast path: a fully-cached concrete version is served by reflinking
        // from the local store with zero network round-trips.
        if let Some(result) = self
            .try_install_from_cache(&reference, vendor_dir, progress_tx)
            .await?
        {
            return Ok(result);
        }

        // Offline mode can only serve packages already in the cache; if the
        // fast path missed there is nothing more we can do without network.
        if self.offline {
            return Err(ManagerError::OfflineNotCached {
                reference: reference.to_string(),
            }
            .into());
        }

        // Network path: pull the manifest (and any missing layers) from the
        // registry, then vendor exactly as the cache path does.
        //
        // `Box::pin` keeps the (large) pull futures on the heap so this shared
        // method's own future stays small enough to satisfy `clippy::large_futures`.
        let pull_result = match progress_tx {
            Some(tx) => Box::pin(self.pull_with_progress(reference.clone(), tx)).await?,
            None => Box::pin(self.pull(reference.clone())).await?,
        };

        let result = match pull_result.manifest {
            Some(ref manifest) => {
                self.vendor_manifest(&reference, vendor_dir, manifest, pull_result.digest)
                    .await?
            }
            // A wasm artifact always carries a manifest; the `None` case is
            // purely defensive and yields an otherwise-empty result.
            None => InstallResult {
                registry: reference.registry().to_string(),
                repository: reference.repository().to_string(),
                tag: reference.tag().map(str::to_string),
                digest: pull_result.digest,
                package_name: None,
                oci_title: None,
                vendored_files: Vec::new(),
                is_component: true,
                dependencies: Vec::new(),
            },
        };

        if let Some(tx) = progress_tx {
            let _ = tx.send(ProgressEvent::InstallComplete).await;
        }

        Ok(result)
    }

    /// Attempt to install `reference` entirely from the local cache, without
    /// any network I/O.
    ///
    /// Returns `Ok(Some(result))` on a full cache hit — a concrete version tag
    /// whose manifest and every layer are already stored locally. Returns
    /// `Ok(None)` when the caller should fall back to the network: the
    /// reference has no tag or a floating `latest` tag, the manifest is not
    /// cached, or some layer blob is missing.
    async fn try_install_from_cache(
        &self,
        reference: &Reference,
        vendor_dir: &Path,
        progress_tx: Option<&tokio::sync::mpsc::Sender<ProgressEvent>>,
    ) -> anyhow::Result<Option<InstallResult>> {
        // Only trust the cache for concrete version tags. `latest` (and the
        // no-tag case, which OCI treats as `latest`) is mutable, so we must
        // re-check the registry for freshness.
        let tag = match reference.tag() {
            Some(tag) if tag != "latest" => tag,
            _ => return Ok(None),
        };

        let Some((digest, manifest)) = self
            .store
            .cached_manifest_for_reference(reference.registry(), reference.repository(), tag)
            .await?
        else {
            return Ok(None);
        };

        // Every referenced layer blob must be present locally, otherwise we
        // cannot vendor without the network — fall back to the pull path.
        if !self.store.all_layers_cached(&manifest).await {
            return Ok(None);
        }

        // Drive the progress display from cached metadata so the CLI renders
        // the same phases as a network install, just instantly.
        Self::emit_cached_progress(progress_tx, &manifest, &digest).await;

        let result = self
            .vendor_manifest(reference, vendor_dir, &manifest, Some(digest))
            .await?;

        if let Some(tx) = progress_tx {
            let _ = tx.send(ProgressEvent::InstallComplete).await;
        }

        Ok(Some(result))
    }

    /// Build an [`InstallResult`] by vendoring the wasm layer(s) of an
    /// already-available manifest — whether just pulled from the registry or
    /// loaded from the local cache.
    ///
    /// Inspects the (cached) wasm layers for their WIT package name, kind, and
    /// dependencies, then reflinks them into `vendor_dir`. Performs no network
    /// I/O; both the fast path and the network path share it so their results
    /// are constructed identically.
    async fn vendor_manifest(
        &self,
        reference: &Reference,
        vendor_dir: &Path,
        manifest: &oci_client::manifest::OciImageManifest,
        digest: Option<String>,
    ) -> anyhow::Result<InstallResult> {
        use crate::oci::filter_wasm_layers;

        let oci_title = manifest
            .annotations
            .as_ref()
            .and_then(|a| a.get("org.opencontainers.image.title").cloned());

        let wasm_layers = filter_wasm_layers(&manifest.layers);
        let (package_name, is_component, dependencies) =
            self.inspect_wasm_layers(&wasm_layers).await;

        let mut vendored_files = Vec::new();
        if !wasm_layers.is_empty() {
            let name = package_name.as_deref().ok_or_else(|| {
                anyhow::anyhow!("could not determine WIT package name from `{reference}`")
            })?;
            let filename = vendor_filename(name, reference.tag());
            vendored_files = self
                .vendor_wasm_layers(&wasm_layers, vendor_dir, &filename)
                .await?;
        }

        Ok(InstallResult {
            registry: reference.registry().to_string(),
            repository: reference.repository().to_string(),
            tag: reference.tag().map(str::to_string),
            digest,
            package_name,
            oci_title,
            vendored_files,
            is_component,
            dependencies,
        })
    }

    /// Emit synthetic progress events for a cache hit so the CLI progress bars
    /// advance through the manifest and per-layer phases before completing.
    async fn emit_cached_progress(
        progress_tx: Option<&tokio::sync::mpsc::Sender<ProgressEvent>>,
        manifest: &oci_client::manifest::OciImageManifest,
        digest: &str,
    ) {
        let Some(tx) = progress_tx else { return };
        let _ = tx
            .send(ProgressEvent::ManifestFetched {
                layer_count: manifest.layers.len(),
                image_digest: digest.to_string(),
            })
            .await;
        for (index, layer) in manifest.layers.iter().enumerate() {
            let total_bytes = if layer.size > 0 {
                u64::try_from(layer.size).ok()
            } else {
                None
            };
            let _ = tx
                .send(ProgressEvent::LayerStarted {
                    index,
                    digest: layer.digest.clone(),
                    total_bytes,
                    title: layer
                        .annotations
                        .as_ref()
                        .and_then(|a| a.get("org.opencontainers.image.title").cloned()),
                    media_type: layer.media_type.clone(),
                })
                .await;
            let _ = tx.send(ProgressEvent::LayerStored { index }).await;
        }
    }

    /// Inspect cached wasm layers up-front to learn the WIT package name; this
    /// lets us name the vendored artifact after `namespace:package@version`
    /// rather than the OCI reference.
    ///
    /// Returns the discovered package name, whether the artifact is a component
    /// (vs. a WIT package), and its dependencies.
    async fn inspect_wasm_layers(
        &self,
        wasm_layers: &[&oci_client::manifest::OciDescriptor],
    ) -> (Option<String>, bool, Vec<crate::types::DependencyItem>) {
        let mut package_name = None;
        let mut is_component = true; // Default to component
        let mut dependencies = Vec::new();

        for layer in wasm_layers {
            if package_name.is_none() {
                self.try_extract_layer_metadata(
                    &layer.digest,
                    &mut package_name,
                    &mut is_component,
                    &mut dependencies,
                )
                .await;
            }
        }

        (package_name, is_component, dependencies)
    }

    /// Reflink each wasm layer into the vendor directory under `filename`,
    /// returning the vendored file paths.
    async fn vendor_wasm_layers(
        &self,
        wasm_layers: &[&oci_client::manifest::OciDescriptor],
        vendor_dir: &Path,
        filename: &str,
    ) -> anyhow::Result<Vec<std::path::PathBuf>> {
        let mut vendored_files = Vec::new();

        for layer in wasm_layers {
            let dest = vendor_dir.join(filename);

            // Ensure vendor directory exists
            tokio::fs::create_dir_all(vendor_dir).await?;

            // Remove existing file if present before reflinking
            let _ = tokio::fs::remove_file(&dest).await;

            self.vendor(&layer.digest, &dest).await?;
            vendored_files.push(dest);
        }

        Ok(vendored_files)
    }

    /// List all stored images and their metadata.
    pub async fn list_all(&self) -> anyhow::Result<Vec<ImageEntry>> {
        Ok(self
            .store
            .list_all()
            .await?
            .into_iter()
            .map(ImageEntry::from)
            .collect())
    }

    /// Resolve a WIT dependency to an OCI [`Reference`].
    ///
    /// Resolution order:
    /// 1. Exact match via `RawWitPackage::find_oci_reference()` (DB JOIN lookup).
    /// 2. Fuzzy match via `RawKnownPackage::search_by_wit_name()` (repository pattern).
    /// 3. Error with an actionable message.
    ///
    /// When no version is specified, the latest stable semver tag is
    /// selected instead of `"latest"`. Pre-release, hash-based, and
    /// non-semver tags are skipped.
    pub async fn resolve_wit_dependency(
        &self,
        dep: &crate::types::DependencyItem,
    ) -> anyhow::Result<Option<Reference>> {
        // 1. Exact DB lookup: WIT package → OCI reference
        if let Some((registry, repository)) = self
            .store
            .find_oci_reference_by_wit_name(&dep.package, dep.version.as_deref())
            .await?
        {
            let tag = self.resolve_tag_for_dep(dep, &registry, &repository).await;
            // Map SemVer build metadata (`0.1.0+meta`) onto a valid OCI tag
            // (`0.1.0_meta`) — the inverse of what `publish` does — so a `+`
            // dependency resolves to the tag the registry actually stores.
            let ref_str = format!("{registry}/{repository}:{}", oci_tag(&tag));
            return Ok(Some(ref_str.parse()?));
        }

        // 2. Fallback: search known packages by WIT name
        if let Some(known) = self
            .store
            .search_known_package_by_wit_name(&dep.package)
            .await?
        {
            let tag = if let Some(v) = dep.version.as_deref() {
                v.to_string()
            } else {
                // Try tags from the OCI store first, then fall back to
                // versions stored in the `wit_package` table (populated by
                // sync stubs even when no OCI manifest has been pulled yet).
                if let Some(t) = pick_latest_stable_tag(&known.tags) {
                    t
                } else if let Some(t) = self.pick_latest_wit_package_version(&dep.package).await {
                    t
                } else {
                    "latest".to_string()
                }
            };
            // Same `+`→`_` tag mapping as the exact-lookup path above.
            let ref_str = format!("{}/{}:{}", known.registry, known.repository, oci_tag(&tag));
            return Ok(Some(ref_str.parse()?));
        }

        // 3. Not resolvable
        Ok(None)
    }

    /// Pick the tag to use for an exact-DB-lookup dependency.
    ///
    /// When the dependency carries an explicit version, use it directly.
    /// Otherwise, try to find the latest stable semver tag from the
    /// known-package cache for the same registry/repository.
    async fn resolve_tag_for_dep(
        &self,
        dep: &crate::types::DependencyItem,
        registry: &str,
        repository: &str,
    ) -> String {
        if let Some(v) = dep.version.as_deref() {
            return v.to_string();
        }
        if let Ok(Some(known)) = self.store.get_known_package(registry, repository).await
            && let Some(tag) = pick_latest_stable_tag(&known.tags)
        {
            return tag;
        }
        // Fall back to versions from the `wit_package` table (sync stubs).
        if let Some(v) = self.pick_latest_wit_package_version(&dep.package).await {
            return v;
        }
        "latest".to_string()
    }

    /// Pick the latest stable semver version from the `wit_package` table.
    ///
    /// This is used as a fallback when OCI tags are not yet available (e.g.
    /// on a fresh DB where sync has stored `wit_package` stubs but no OCI
    /// manifests have been pulled).  The synthetic `0.0.0` shim used for
    /// unversioned packages is excluded.
    async fn pick_latest_wit_package_version(&self, package_name: &str) -> Option<String> {
        let versions = self
            .store
            .list_wit_package_versions(package_name)
            .await
            .ok()?;
        let tags: Vec<String> = versions.into_iter().filter(|v| v != "0.0.0").collect();
        pick_latest_stable_tag(&tags)
    }

    /// Get data from the store
    pub async fn get(&self, key: &str) -> cacache::Result<Vec<u8>> {
        cacache::read(self.store.state_info.store_dir(), key).await
    }

    /// Get information about the current state of the package manager.
    pub fn state_info(&self) -> StateInfo {
        self.store.state_info.clone()
    }

    /// Get the current configuration.
    #[must_use]
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Delete an image from the store by its reference.
    pub async fn delete(&self, reference: Reference) -> anyhow::Result<bool> {
        self.store.delete(&reference).await
    }

    /// Search for known packages by query string.
    /// Searches in both registry and repository fields.
    /// Uses pagination with `offset` and `limit` parameters.
    pub async fn search_packages(
        &self,
        query: &str,
        offset: u32,
        limit: u32,
    ) -> anyhow::Result<Vec<KnownPackage>> {
        {
            let raws = self
                .store
                .search_known_packages(query, offset, limit)
                .await?;
            let mut out = Vec::with_capacity(raws.len());
            for mut pkg in raws {
                pkg.dependencies = self
                    .store
                    .get_package_dependencies(&pkg.registry, &pkg.repository)
                    .await?;
                out.push(pkg);
            }
            Ok(out)
        }
    }

    /// Search for known packages that import a given interface.
    /// Uses pagination with `offset` and `limit` parameters.
    pub async fn search_packages_by_import(
        &self,
        interface: &str,
        offset: u32,
        limit: u32,
    ) -> anyhow::Result<Vec<KnownPackage>> {
        {
            let raws = self
                .store
                .search_known_packages_by_import(interface, offset, limit)
                .await?;
            let mut out = Vec::with_capacity(raws.len());
            for mut pkg in raws {
                pkg.dependencies = self
                    .store
                    .get_package_dependencies(&pkg.registry, &pkg.repository)
                    .await?;
                out.push(pkg);
            }
            Ok(out)
        }
    }

    /// Search for known packages that export a given interface.
    /// Uses pagination with `offset` and `limit` parameters.
    pub async fn search_packages_by_export(
        &self,
        interface: &str,
        offset: u32,
        limit: u32,
    ) -> anyhow::Result<Vec<KnownPackage>> {
        {
            let raws = self
                .store
                .search_known_packages_by_export(interface, offset, limit)
                .await?;
            let mut out = Vec::with_capacity(raws.len());
            for mut pkg in raws {
                pkg.dependencies = self
                    .store
                    .get_package_dependencies(&pkg.registry, &pkg.repository)
                    .await?;
                out.push(pkg);
            }
            Ok(out)
        }
    }

    /// Get all known packages.
    /// Uses pagination with `offset` and `limit` parameters.
    ///
    /// Each returned [`KnownPackage`] has its `dependencies` field populated
    /// from the local `wit_package_dependency` table.
    ///
    /// **Note:** the current implementation performs one dependency query per
    /// package (N+1). This is acceptable for the typical page sizes used by
    /// search (~50 items) and keeps the code simple. A future
    /// optimisation could batch-load all dependencies in a single query keyed
    /// by `(registry, repository)` pairs.
    pub async fn list_known_packages(
        &self,
        offset: u32,
        limit: u32,
    ) -> anyhow::Result<Vec<KnownPackage>> {
        {
            let raws = self.store.list_known_packages(offset, limit).await?;
            let mut out = Vec::with_capacity(raws.len());
            for mut pkg in raws {
                pkg.dependencies = self
                    .store
                    .get_package_dependencies(&pkg.registry, &pkg.repository)
                    .await?;
                out.push(pkg);
            }
            Ok(out)
        }
    }

    /// Get recently updated known packages.
    ///
    /// Uses pagination with `offset` and `limit` parameters.
    pub async fn list_recent_known_packages(
        &self,
        offset: u32,
        limit: u32,
    ) -> anyhow::Result<Vec<KnownPackage>> {
        {
            let raws = self.store.list_recent_known_packages(offset, limit).await?;
            let mut out = Vec::with_capacity(raws.len());
            for mut pkg in raws {
                pkg.dependencies = self
                    .store
                    .get_package_dependencies(&pkg.registry, &pkg.repository)
                    .await?;
                out.push(pkg);
            }
            Ok(out)
        }
    }

    /// Add or update a known package entry.
    pub async fn add_known_package(
        &self,
        registry: &str,
        repository: &str,
        tag: Option<&str>,
        description: Option<&str>,
    ) -> anyhow::Result<()> {
        self.store
            .add_known_package(registry, repository, tag, description)
            .await
    }

    /// Add or update a known package entry with WIT namespace mapping.
    pub async fn add_known_package_with_params(
        &self,
        params: &KnownPackageParams<'_>,
    ) -> anyhow::Result<()> {
        self.store.add_known_package_with_params(params).await
    }

    /// List all tags for a given reference from the registry.
    ///
    /// In offline mode, returns cached tags from the local database instead of
    /// fetching from the registry.
    pub async fn list_tags(&self, reference: &Reference) -> anyhow::Result<Vec<String>> {
        if self.offline {
            // Return cached tags from known packages
            return self.list_cached_tags(reference).await;
        }
        self.client.list_tags(reference).await
    }

    /// List tags from the local cache for a given reference.
    ///
    /// This is a private helper method used by `list_tags` when in offline mode.
    /// Returns all cached tags (release, signature, and attestation) for the given
    /// reference from the local known packages database.
    async fn list_cached_tags(&self, reference: &Reference) -> anyhow::Result<Vec<String>> {
        // Use efficient lookup by registry and repository
        match self
            .store
            .get_known_package(reference.registry(), reference.repository())
            .await?
        {
            Some(pkg) => {
                // Combine all tag types: release, signature, and attestation
                let tags: Vec<String> = pkg
                    .tags
                    .into_iter()
                    .chain(pkg.signature_tags)
                    .chain(pkg.attestation_tags)
                    .collect();
                Ok(tags)
            }
            None => Ok(Vec::new()),
        }
    }

    /// Get a known package by registry and repository.
    ///
    /// The returned [`KnownPackage`] has its `dependencies` field populated
    /// from the local `wit_package_dependency` table.
    pub async fn get_known_package(
        &self,
        registry: &str,
        repository: &str,
    ) -> anyhow::Result<Option<KnownPackage>> {
        match self.store.get_known_package(registry, repository).await? {
            None => Ok(None),
            Some(mut pkg) => {
                pkg.dependencies = self
                    .store
                    .get_package_dependencies(registry, repository)
                    .await?;
                Ok(Some(pkg))
            }
        }
    }

    /// Look up a known package by its WIT name (`namespace:name`) in the
    /// local index, returning `None` when no matching package is indexed.
    ///
    /// The lookup is an exact `(wit_namespace, wit_name)` match with a fuzzy
    /// repository fallback, so callers that need a strict identity match
    /// should verify the returned package's `wit_namespace`/`wit_name`.
    pub async fn find_known_package_by_wit_name(
        &self,
        wit_name: &str,
    ) -> anyhow::Result<Option<KnownPackage>> {
        self.store.search_known_package_by_wit_name(wit_name).await
    }

    /// Index a package from the registry, also extracting WIT dependency
    /// metadata from the package's wasm layer.
    ///
    /// Re-extract WIT metadata for all cached packages.
    ///
    /// Reads the original wasm bytes from the content-addressable cache and
    /// re-derives `wit_text` and related metadata using the current
    /// extraction logic.  OCI data (manifests, layers, blobs) is untouched.
    ///
    /// Returns the number of packages that were re-indexed.
    pub async fn reindex_wit(&self) -> anyhow::Result<u64> {
        self.store.reindex_wit_packages().await
    }

    /// Enqueue reindex tasks for all known tags that have cached layers.
    ///
    /// Returns the number of tasks enqueued.
    pub async fn enqueue_reindex_all(&self) -> anyhow::Result<u64> {
        self.store.enqueue_reindex_all().await
    }

    /// Seed the fetch queue with completed entries for tags that were
    /// pulled before the queue existed.
    pub async fn seed_completed_from_tags(&self) -> anyhow::Result<u64> {
        self.store.seed_completed_from_tags().await
    }

    /// Return the current fetch queue status.
    pub async fn get_queue_status(&self) -> anyhow::Result<wasm_meta_registry_types::QueueStatus> {
        self.store.get_queue_status().await
    }

    /// Notify the registry that a specific version of a package was just
    /// published, requesting it be pulled as soon as possible.
    ///
    /// This is the entry point for external publishers (e.g. CI pipelines)
    /// that have just pushed a new image and want the registry to index it
    /// without waiting for the next periodic sync cycle.
    ///
    /// To prevent abuse and avoid hammering upstream registries, the request
    /// is rejected when the tag was already pulled within
    /// `PULL_COOLDOWN_SECS` (the same freshness window used by the periodic
    /// sync). The caller MUST treat this as a hint, not a guarantee.
    ///
    /// Enqueued tasks are given high priority (priority `-1`) so they jump
    /// ahead of the routine sync backlog.
    pub async fn notify_new_version(
        &self,
        registry: &str,
        repository: &str,
        tag: &str,
    ) -> anyhow::Result<wasm_meta_registry_types::NotifyOutcome> {
        use wasm_meta_registry_types::NotifyOutcome;

        if self
            .store
            .is_tag_fresh(registry, repository, tag, PULL_COOLDOWN_SECS)
            .await
        {
            return Ok(NotifyOutcome::Skipped {
                reason: "fresh".to_string(),
            });
        }

        // High priority so external notifications jump ahead of the routine
        // sync backlog. Use `enqueue_refetch` rather than `enqueue_pull` so a
        // previous "completed" or "failed" queue entry for this tag is reset
        // to "pending" instead of silently no-oping (which would happen with
        // `enqueue_pull`'s `ON CONFLICT DO NOTHING`).
        self.store
            .enqueue_refetch(registry, repository, tag, -1)
            .await?;
        Ok(NotifyOutcome::Enqueued)
    }

    /// Fetches the manifest and config to extract metadata (description from
    /// OCI annotations), lists all tags, and upserts into the known packages
    /// table. Also pulls the wasm layer for the most recent tag to extract
    /// WIT dependency information and store it in the local database.
    ///
    /// When `wit_namespace` / `wit_name` are provided, the WIT namespace
    /// mapping is stored alongside the OCI coordinates so that WIT-style
    /// lookups (e.g. `ba:sample-wasi-http-rust`) can resolve to the correct
    /// OCI repository.
    ///
    /// # Errors
    ///
    /// Returns an error if offline mode is enabled or if network operations fail.
    pub async fn index_package(
        &self,
        reference: &Reference,
        wit_namespace: Option<&str>,
        wit_name: Option<&str>,
        kind: Option<PackageKind>,
    ) -> anyhow::Result<KnownPackage> {
        self.index_package_inner(reference, wit_namespace, wit_name, kind, false)
            .await
    }

    /// Index a package, optionally bypassing the pull cooldown.
    ///
    /// When `skip_cooldown` is `true`, every version tag is re-pulled
    /// from the registry regardless of when it was last fetched.
    pub async fn index_package_refetch(
        &self,
        reference: &Reference,
        wit_namespace: Option<&str>,
        wit_name: Option<&str>,
        kind: Option<PackageKind>,
    ) -> anyhow::Result<KnownPackage> {
        self.index_package_inner(reference, wit_namespace, wit_name, kind, true)
            .await
    }

    async fn index_package_inner(
        &self,
        reference: &Reference,
        wit_namespace: Option<&str>,
        wit_name: Option<&str>,
        kind: Option<PackageKind>,
        skip_cooldown: bool,
    ) -> anyhow::Result<KnownPackage> {
        if self.offline {
            return Err(ManagerError::OfflineIndex.into());
        }

        tracing::debug!(
            registry = %reference.registry(),
            repository = %reference.repository(),
            "Discovering package tags"
        );

        // Discover available tags first — the reference may not carry a valid
        // tag (e.g. the default "latest" might not exist).
        let tags = self.client.list_tags(reference).await?;
        if tags.is_empty() {
            return Err(ManagerError::NoTagsFound {
                registry: reference.registry().to_string(),
                repository: reference.repository().to_string(),
            }
            .into());
        }

        // Pick the tag to use for pulling metadata: prefer the tag on the
        // reference if it exists in the remote, otherwise fall back to the
        // first available tag.
        let meta_tag = reference
            .tag()
            .filter(|t| tags.iter().any(|remote| remote == *t))
            .unwrap_or_else(|| tags.first().expect("tags verified non-empty"));

        // Build a reference with the chosen tag so we can pull its manifest.
        let meta_ref: Reference = format!(
            "{}/{}:{}",
            reference.registry(),
            reference.repository(),
            meta_tag
        )
        .parse()?;

        // Fetch manifest to extract metadata (e.g. description).
        let (manifest, _digest) = self.client.pull_manifest(&meta_ref).await?;
        let description = manifest
            .annotations
            .as_ref()
            .and_then(|a| a.get("org.opencontainers.image.description").cloned());

        // Filter to tags that parse as semver (e.g. `1.2.3`). Tags like
        // `latest`, `nightly`, or `sha256-...` are excluded here: they cannot be
        // resolved by the version solver and cause garbled rendering in the
        // frontend. If no tags are valid, skip indexing this package entirely
        // so it does not pollute search results.
        let valid_tags: Vec<&String> = tags
            .iter()
            .filter(|t| parse_tag_as_semver(t).is_some())
            .collect();
        if valid_tags.is_empty() {
            tracing::debug!(
                registry = %reference.registry(),
                repository = %reference.repository(),
                discovered = tags.len(),
                "Skipping package — no tags parse as strict semver"
            );
            return Err(ManagerError::NoSemverTags {
                registry: reference.registry().to_string(),
                repository: reference.repository().to_string(),
            }
            .into());
        }

        // Store every valid tag.
        for tag in &valid_tags {
            self.store
                .add_known_package_with_params(&KnownPackageParams {
                    registry: reference.registry(),
                    repository: reference.repository(),
                    tag: Some(tag),
                    description: description.as_deref(),
                    wit_namespace,
                    wit_name,
                    kind,
                })
                .await?;
        }

        // Enqueue every semver-tagged version for pulling.  Tags that are
        // not valid semver (e.g. `latest`, hash-based signatures like
        // `sha256-...`, or arbitrary strings such as `dev`/`nightly`) are
        // skipped — they typically duplicate a semver tag and cannot be
        // reasoned about by the resolver.
        //
        // The semver tags are sorted in ascending order so that the highest
        // stable version is enqueued last (and thus processed last), keeping
        // the most recent stable version's dependencies at the top of the
        // `get_package_dependencies` query.
        //
        // Tags that were already pulled recently (within `PULL_COOLDOWN_SECS`
        // seconds) are skipped unless `skip_cooldown` is set (--refetch).
        // r[impl server.index.dependencies]
        let mut semver_tags: Vec<(&String, semver::Version)> = Vec::with_capacity(tags.len());
        for tag in &tags {
            match parse_tag_as_semver(tag) {
                Some(v) => semver_tags.push((tag, v)),
                None => {
                    tracing::debug!(
                        registry = %reference.registry(),
                        repository = %reference.repository(),
                        tag = %tag,
                        "Skipping enqueue — tag is not a valid semver version"
                    );
                }
            }
        }
        // Sort ascending: pre-releases first, then stable versions in order.
        semver_tags.sort_by(|(_, a), (_, b)| a.cmp(b));

        for (tag, _version) in &semver_tags {
            if skip_cooldown {
                self.store
                    .enqueue_refetch(
                        reference.registry(),
                        reference.repository(),
                        tag,
                        -1, // high priority for explicit refetch
                    )
                    .await?;
            } else if !self
                .store
                .is_tag_fresh(
                    reference.registry(),
                    reference.repository(),
                    tag,
                    PULL_COOLDOWN_SECS,
                )
                .await
            {
                self.store
                    .enqueue_pull(
                        reference.registry(),
                        reference.repository(),
                        tag,
                        0, // normal priority
                    )
                    .await?;
            } else {
                // Tag is fresh — record it as completed so it appears
                // in the queue history for visibility.
                self.store
                    .record_completed(reference.registry(), reference.repository(), tag)
                    .await?;
            }
        }

        if let Ok(pending) = self.store.pending_count().await
            && pending > 0
        {
            tracing::info!(
                registry = %reference.registry(),
                repository = %reference.repository(),
                pending,
                "Enqueued versions for pulling"
            );
        }

        // Return the indexed package with its now-populated dependencies.
        let mut pkg = self
            .store
            .get_known_package(reference.registry(), reference.repository())
            .await?
            .ok_or(ManagerError::IndexRetrievalFailed)?;
        pkg.dependencies = self
            .store
            .get_package_dependencies(reference.registry(), reference.repository())
            .await?;
        Ok(pkg)
    }

    /// Process the next pending task from the fetch queue.
    ///
    /// Returns [`TaskOutcome::Empty`] when there are no pending tasks,
    /// [`TaskOutcome::Succeeded`] when a task ran to completion, and
    /// [`TaskOutcome::Failed`] when the task itself failed (the failure
    /// is recorded in the queue).  Errors are reserved for failures that
    /// prevent us from interacting with the queue at all.
    pub async fn process_next_task(&self) -> anyhow::Result<TaskOutcome> {
        let Some(task) = self.store.dequeue_next().await? else {
            return Ok(TaskOutcome::Empty);
        };

        tracing::info!(
            registry = %task.registry,
            repository = %task.repository,
            tag = %task.tag,
            kind = ?task.kind,
            attempt = task.attempts + 1,
            "Processing fetch task"
        );

        let result = match task.kind {
            FetchTaskKind::Pull => self.execute_pull_task(&task).await,
            FetchTaskKind::Reindex => self.execute_reindex_task(&task).await,
        };

        match result {
            Ok(()) => {
                self.store.complete_task(task.id).await?;
                Ok(TaskOutcome::Succeeded)
            }
            Err(e) => {
                tracing::warn!(
                    registry = %task.registry,
                    repository = %task.repository,
                    tag = %task.tag,
                    error = %e,
                    "Fetch task failed"
                );
                self.store.fail_task(task.id, &e.to_string()).await?;
                Ok(TaskOutcome::Failed)
            }
        }
    }

    /// Execute a pull task: download the OCI image for a specific tag.
    async fn execute_pull_task(&self, task: &crate::storage::FetchTask) -> anyhow::Result<()> {
        let tag_ref: Reference = format!("{}/{}:{}", task.registry, task.repository, task.tag)
            .parse()
            .map_err(|e| anyhow::anyhow!("invalid reference: {e}"))?;
        self.pull(tag_ref).await?;
        Ok(())
    }

    /// Execute a reindex task: re-derive WIT from cached layers for a tag.
    async fn execute_reindex_task(&self, task: &crate::storage::FetchTask) -> anyhow::Result<()> {
        self.store
            .reindex_tag(&task.registry, &task.repository, &task.tag)
            .await
    }

    /// Get all WIT interfaces with their associated component references.
    pub async fn list_wit_packages_with_components(
        &self,
    ) -> anyhow::Result<Vec<(WitPackage, String)>> {
        Ok(self
            .store
            .list_wit_packages_with_components()
            .await?
            .into_iter()
            .map(|(wt, s)| (WitPackage::from(wt), s))
            .collect())
    }

    /// Get declared dependencies for a package identified by its WIT name and
    /// optional version.
    ///
    /// Queries `wit_package_dependency` directly by package name, bypassing
    /// the OCI registry/repository path. This is the primary entry point for
    /// the dependency resolver.
    ///
    /// Returns an empty list when the package has no recorded dependencies.
    pub async fn get_dependencies_by_name(
        &self,
        package_name: &str,
        version: Option<&str>,
    ) -> anyhow::Result<Vec<crate::storage::PackageDependencyRef>> {
        self.store
            .get_package_dependencies_by_name(package_name, version)
            .await
    }

    /// Resolve the complete transitive dependency graph for a root package and
    /// version using the PubGrub algorithm over locally-cached metadata.
    ///
    /// Returns a map from WIT package name to the single selected version for
    /// every package in the resolved set (including the root).
    ///
    /// # Errors
    ///
    /// Returns [`crate::resolver::ResolveError::NoSolution`] when no
    /// conflict-free version assignment exists.
    /// Returns [`crate::resolver::ResolveError::Db`] when a database query
    /// fails.
    pub fn resolve_dependencies(
        &self,
        package: &str,
        version: crate::resolver::WitVersion,
    ) -> Result<
        std::collections::HashMap<String, crate::resolver::WitVersion>,
        crate::resolver::ResolveError,
    > {
        crate::resolver::resolve_from_db(&self.store, package, version)
    }

    /// Resolve the transitive dependency graph for multiple root packages at
    /// once, using a single PubGrub solver pass.
    ///
    /// All `roots` are fed into one resolution.  This ensures shared
    /// transitive dependencies are resolved consistently across all roots
    /// instead of running separate per-root passes that could select
    /// different versions.
    ///
    /// Returns a map from WIT package name to the selected version for every
    /// package in the resolved set (including the roots themselves).
    ///
    /// # Errors
    ///
    /// Returns [`crate::resolver::ResolveError::NoSolution`] when no
    /// conflict-free version assignment exists.
    /// Returns [`crate::resolver::ResolveError::Db`] when a database query
    /// fails.
    pub fn resolve_all_dependencies(
        &self,
        roots: &[(String, crate::resolver::WitVersion)],
    ) -> Result<
        std::collections::HashMap<String, crate::resolver::WitVersion>,
        crate::resolver::ResolveError,
    > {
        crate::resolver::resolve_all_from_db(&self.store, roots)
    }

    // ================================================================
    // Rich query methods for the meta-registry API
    // ================================================================

    /// Return all versions of a package with full per-version metadata.
    ///
    /// Each version includes OCI annotations, WIT worlds (with imports and
    /// exports), Wasm components (with targets), dependencies, referrers,
    /// and WIT source text.
    pub async fn get_package_versions(
        &self,
        registry: &str,
        repository: &str,
    ) -> anyhow::Result<Vec<wasm_meta_registry_types::PackageVersion>> {
        self.store.get_package_versions(registry, repository).await
    }

    /// Return a single version of a package by its tag.
    pub async fn get_package_version(
        &self,
        registry: &str,
        repository: &str,
        version_tag: &str,
    ) -> anyhow::Result<Option<wasm_meta_registry_types::PackageVersion>> {
        self.store
            .get_package_version(registry, repository, version_tag)
            .await
    }

    /// Return full package detail including all versions and metadata.
    pub async fn get_package_detail(
        &self,
        registry: &str,
        repository: &str,
    ) -> anyhow::Result<Option<wasm_meta_registry_types::PackageDetail>> {
        self.store.get_package_detail(registry, repository).await
    }

    /// Sync the local package index from a meta-registry over HTTP.
    ///
    /// Checks the `_sync_meta` table for `last_synced_at` and skips the sync
    /// if less than `sync_interval` seconds have elapsed. Passes the cached
    /// ETag to the registry for conditional fetches.
    ///
    /// When `policy` is [`SyncPolicy::Force`], the minimum-interval check is
    /// skipped.
    ///
    /// # Errors
    ///
    /// Returns an error only when the sync fails **and** no cached data exists.
    /// When cached data exists but the sync fails, returns `SyncResult::Degraded`.
    #[cfg(feature = "http-sync")]
    pub async fn sync_from_meta_registry(
        &self,
        url: &str,
        sync_interval: u64,
        policy: SyncPolicy,
    ) -> anyhow::Result<SyncResult> {
        use wasm_meta_registry_client::{FetchResult, RegistryClient};

        // Check the minimum interval unless forced.
        if policy == SyncPolicy::IfStale {
            let last_synced_epoch = self
                .store
                .get_sync_meta("last_synced_at")
                .await?
                .and_then(|s| s.parse::<i64>().ok());
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
                .try_into()
                .unwrap_or(i64::MAX);
            if !should_sync(last_synced_epoch, sync_interval, now) {
                return Ok(SyncResult::Skipped);
            }
        }

        let etag = self.store.get_sync_meta("packages_etag").await?;
        let client = RegistryClient::new(url);

        let has_cached_data = {
            let existing = self.store.list_known_packages(0, 1).await?;
            !existing.is_empty()
        };

        match client.fetch_packages(etag.as_deref(), 1000).await {
            Ok(FetchResult::NotModified) => {
                self.update_last_synced_at().await?;
                Ok(SyncResult::NotModified)
            }
            Ok(FetchResult::Updated { packages, etag }) => {
                self.handle_update(&packages, etag).await
            }
            Err(e) if has_cached_data => Ok(SyncResult::Degraded {
                error: e.to_string(),
            }),
            Err(e) => Err(ManagerError::SyncNoLocalData {
                reason: e.to_string(),
            }
            .into()),
        }
    }

    /// Notify a meta-registry that a new version of a package was just
    /// published, requesting it be pulled as soon as possible.
    ///
    /// This sends an HTTP `POST` to the meta-registry's
    /// `/v1/packages/notify/{registry}/{repository}?tag={tag}` endpoint. The
    /// meta-registry treats the request as a hint and may dedupe or skip it
    /// based on its own freshness/cooldown policy. The returned
    /// [`NotifyOutcome`] describes what the server actually did.
    ///
    /// [`NotifyOutcome`]: wasm_meta_registry_types::NotifyOutcome
    ///
    /// # Errors
    ///
    /// Returns an error when offline mode is enabled or when the HTTP
    /// request fails (e.g. the meta-registry is unreachable, or returns an
    /// unknown package).
    #[cfg(feature = "http-sync")]
    pub async fn notify_meta_registry(
        &self,
        url: &str,
        registry: &str,
        repository: &str,
        tag: &str,
    ) -> anyhow::Result<wasm_meta_registry_types::NotifyOutcome> {
        use anyhow::Context as _;
        use wasm_meta_registry_client::RegistryClient;

        if self.offline {
            anyhow::bail!("cannot notify meta-registry in offline mode");
        }

        let client = RegistryClient::new(url);
        client
            .notify_new_version(registry, repository, tag)
            .await
            .map_err(|e| anyhow::Error::msg(e.to_string()))
            .with_context(|| format!("failed to notify meta-registry at {url}"))
    }

    #[cfg(feature = "http-sync")]
    async fn handle_update(
        &self,
        packages: &[KnownPackage],
        etag: Option<String>,
    ) -> anyhow::Result<SyncResult> {
        let count = packages.len();
        // Bulk upsert all packages.
        for pkg in packages {
            let first_tag = pkg.tags.first().map(String::as_str);
            self.store
                .add_known_package_with_params(&KnownPackageParams {
                    registry: &pkg.registry,
                    repository: &pkg.repository,
                    tag: first_tag,
                    description: pkg.description.as_deref(),
                    wit_namespace: pkg.wit_namespace.as_deref(),
                    wit_name: pkg.wit_name.as_deref(),
                    kind: pkg.kind,
                })
                .await?;
            // Also add remaining tags.
            for tag in pkg.tags.iter().skip(1) {
                self.store
                    .add_known_package_with_params(&KnownPackageParams {
                        registry: &pkg.registry,
                        repository: &pkg.repository,
                        tag: Some(tag),
                        description: pkg.description.as_deref(),
                        wit_namespace: pkg.wit_namespace.as_deref(),
                        wit_name: pkg.wit_name.as_deref(),
                        kind: pkg.kind,
                    })
                    .await?;
            }

            // r[impl db.wit-package-dependency.populate-on-sync]
            // Store package and dependency information from the sync response
            // so the local database can answer dependency and version queries
            // without network access.  A `wit_package` stub row is created
            // even for packages with no dependencies — the resolver needs the
            // row to exist so that `choose_version` can enumerate available
            // versions.
            if let (Some(ns), Some(name)) = (&pkg.wit_namespace, &pkg.wit_name) {
                let package_name = format!("{ns}:{name}");
                // Use the latest stable semver tag as the canonical version;
                // strip any leading "v" so it matches the WIT version string.
                // When no stable semver tag is available, fall back to "0.0.0"
                // so the resolver can still find the package (the installer
                // shims unversioned roots to 0.0.0 for PubGrub resolution).
                let version = pick_latest_stable_tag(&pkg.tags).map_or_else(
                    || "0.0.0".to_string(),
                    |t| t.trim_start_matches('v').to_string(),
                );
                if let Err(e) = self
                    .store
                    .upsert_package_dependencies_from_sync(
                        &package_name,
                        Some(&version),
                        &pkg.dependencies,
                    )
                    .await
                {
                    tracing::warn!(
                        package = %package_name,
                        error = %e,
                        "Failed to store synced package"
                    );
                }
            }
        }
        if let Some(etag_val) = etag {
            self.store.set_sync_meta("packages_etag", &etag_val).await?;
        }
        self.update_last_synced_at().await?;
        Ok(SyncResult::Updated { count })
    }

    /// Update the `last_synced_at` timestamp in `_sync_meta`.
    #[cfg(feature = "http-sync")]
    async fn update_last_synced_at(&self) -> anyhow::Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.store
            .set_sync_meta("last_synced_at", &now.to_string())
            .await
    }

    /// Fetch all related tags for a reference and store them as known packages.
    ///
    /// Errors from the registry are silently ignored (best-effort).
    async fn store_related_tags(&self, reference: &Reference) -> anyhow::Result<()> {
        let Ok(tags) = self.client.list_tags(reference).await else {
            return Ok(());
        };
        for tag in tags {
            self.store
                .add_known_package(
                    reference.registry(),
                    reference.repository(),
                    Some(&tag),
                    None,
                )
                .await?;
        }
        Ok(())
    }

    /// Try to extract WIT metadata from a cached layer.
    ///
    /// On success, updates `package_name`, `is_component`, and `dependencies`
    /// in place. Silently skips if the layer data cannot be read or parsed.
    async fn try_extract_layer_metadata(
        &self,
        layer_digest: &str,
        package_name: &mut Option<String>,
        is_component: &mut bool,
        dependencies: &mut Vec<crate::types::DependencyItem>,
    ) {
        use crate::types::{extract_wit_metadata, is_wit_package};

        let Ok(data) = self.get(layer_digest).await else {
            return;
        };
        *is_component = !is_wit_package(&data);
        if let Some(metadata) = extract_wit_metadata(&data) {
            *package_name = metadata.package_name;
            *dependencies = metadata.dependencies;
        }
    }

    /// Best-effort: fetch and store referrers (signatures, SBOMs, attestations)
    /// for a manifest. Silently skips if the registry doesn't support the
    /// Referrers API or if any error occurs, but logs unexpected errors.
    async fn try_store_referrers(&self, reference: &Reference, digest: &str, manifest_id: i64) {
        let index = match self.client.pull_referrers(reference, digest).await {
            Ok(Some(index)) => index,
            Ok(None) => return,
            Err(e) => {
                tracing::debug!(
                    "Failed to pull referrers for {}/{}: {}",
                    reference.registry(),
                    reference.repository(),
                    e
                );
                return;
            }
        };

        for entry in &index.manifests {
            // The per-entry ImageIndexEntry only exposes the referrer
            // manifest's media_type (e.g. the generic OCI manifest media
            // type), not its artifact type. Fetch the referrer manifest to
            // read its top-level `artifactType` (falling back to the config
            // mediaType) so referrers are classified correctly.
            let artifact_type = match self
                .client
                .pull_referrer_manifest(reference, &entry.digest)
                .await
            {
                Ok(artifact_type) => artifact_type,
                Err(e) => {
                    tracing::warn!("Failed to fetch referrer manifest {}: {}", entry.digest, e);
                    continue;
                }
            };

            if let Err(e) = self
                .store
                .store_referrer(
                    manifest_id,
                    reference.registry(),
                    reference.repository(),
                    &entry.digest,
                    &artifact_type,
                )
                .await
            {
                tracing::warn!("Failed to store referrer {}: {}", entry.digest, e);
            }
        }
    }

    /// Enrich a pull error with available tag information when the registry
    /// reports "manifest unknown" (i.e. the requested tag does not exist).
    ///
    /// If the error is not a manifest-unknown error, it is returned as-is.
    async fn enrich_manifest_error(
        &self,
        err: anyhow::Error,
        reference: &Reference,
    ) -> anyhow::Error {
        if !is_manifest_unknown(&err) {
            return err;
        }

        let tag = reference.tag().unwrap_or("latest").to_string();
        let registry = reference.registry().to_string();
        let repository = reference.repository().to_string();

        // Best-effort: fetch available tags to include in the hint.
        let hint = match self.client.list_tags(reference).await {
            Ok(tags) if tags.is_empty() => {
                format!("no tags exist for {registry}/{repository}")
            }
            Ok(tags) => format_available_tags_hint(&tags, Some(&tag)),
            Err(_) => "could not fetch available tags from the registry".to_string(),
        };

        ManagerError::ManifestNotFound {
            tag,
            registry,
            repository,
            hint,
        }
        .into()
    }

    /// Detect local WebAssembly files under a directory.
    ///
    /// Wraps [`component_detector::WasmDetector`] so callers do not need a direct
    /// dependency on the detector crate.
    #[must_use]
    pub fn detect_local_wasm(
        root: &Path,
        include_hidden: bool,
        follow_symlinks: bool,
    ) -> Vec<component_detector::WasmEntry> {
        let detector = component_detector::WasmDetector::new(root)
            .include_hidden(include_hidden)
            .follow_symlinks(follow_symlinks);
        detector.into_iter().filter_map(Result::ok).collect()
    }

    /// Build a [`crate::publish::PublishPlan`] for the given manifest
    /// without performing any network I/O.
    ///
    /// This is what `component publish --dry-run` calls; it loads the
    /// component file from disk (or builds the WIT package), constructs
    /// the OCI annotations, and computes the target reference, but does
    /// not contact the registry.
    pub async fn publish_dry_run(
        &self,
        manifest: &wasm_manifest::Manifest,
        manifest_dir: &Path,
    ) -> anyhow::Result<crate::publish::PublishPlan> {
        crate::publish::plan(manifest, manifest_dir).await
    }

    /// Publish the artifact described by `manifest` to an OCI registry.
    ///
    /// Components are pushed as-is; WIT interfaces are first packaged
    /// via [`crate::publish::build_wit_package`] (which stamps the
    /// manifest version onto the WIT package decl).
    ///
    /// The target registry comes from the manifest's `[package].registry`
    /// field (the full OCI location) — there is no implicit default.
    pub async fn publish(
        &self,
        manifest: &wasm_manifest::Manifest,
        manifest_dir: &Path,
    ) -> anyhow::Result<crate::publish::PublishPlan> {
        if self.offline {
            anyhow::bail!("cannot publish in offline mode");
        }
        let mut plan = crate::publish::plan(manifest, manifest_dir).await?;
        let bytes = std::mem::take(&mut plan.bytes);
        let annotations = std::mem::take(&mut plan.annotations);
        let _response = self
            .client
            .push(&plan.reference, bytes, annotations)
            .await?;

        // NOTE: locally recording the freshly-published tag (so
        // `component registry tags` reflects it without a registry
        // round-trip) is a future enhancement. For now the registry is
        // the canonical source — the next `tags` call will refetch it.
        tracing::debug!(reference = %plan.reference, "published artifact");
        Ok(plan)
    }
}

/// Check whether an `anyhow::Error` wraps an OCI "manifest unknown" error.
///
/// The OCI distribution spec returns this error code when a requested tag
/// (or digest) does not exist in the repository.
fn is_manifest_unknown(err: &anyhow::Error) -> bool {
    err.chain().any(|cause| {
        matches!(
            cause.downcast_ref::<OciDistributionError>(),
            Some(OciDistributionError::RegistryError { envelope, .. })
                if envelope.errors.iter().any(|e| e.code == OciErrorCode::ManifestUnknown)
        )
    })
}

/// Format a human-readable hint listing available tags.
///
/// Uses [`filter_tag_suggestions`] for context-aware pre-release filtering:
/// pre-release tags are only shown when the requested tag shares the same
/// major.minor prefix. Non-semver tags, `latest`, and hash tags are always
/// excluded.
fn format_available_tags_hint(tags: &[String], requested_tag: Option<&str>) -> String {
    const MAX_SHOWN: usize = 10;

    let filtered = filter_tag_suggestions(tags, requested_tag);

    // Fallback: if the semver filter removed everything, show raw
    // human-meaningful tags (skip `latest` and sha256-digest tags).
    let tags_to_show: Vec<&str> = if filtered.is_empty() {
        tags.iter()
            .map(String::as_str)
            .filter(|t| *t != "latest" && !t.starts_with("sha256-"))
            .collect()
    } else {
        filtered.iter().map(String::as_str).collect()
    };

    if tags_to_show.is_empty() {
        return "no installable tags found".to_string();
    }

    if tags_to_show.len() <= MAX_SHOWN {
        format!("available tags: {}", tags_to_show.join(", "))
    } else {
        let shown: Vec<&str> = tags_to_show.iter().take(MAX_SHOWN).copied().collect();
        format!(
            "available tags (showing {MAX_SHOWN} of {}): {}",
            tags_to_show.len(),
            shown.join(", ")
        )
    }
}
