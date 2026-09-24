//! Background indexing: discover a package's tags upstream and enqueue the
//! versions that still need to be pulled.

use oci_client::Reference;
use wasm_meta_registry_types::PackageKind;

use super::{Manager, ManagerError, parse_tag_as_semver, pick_latest_stable_tag};
use crate::storage::{IndexerLease, KnownPackage, KnownPackageParams, KnownTags, Store};

/// `_sync_meta` key recording when the indexer last finished discovery.
const LAST_DISCOVERY_KEY: &str = "indexer_last_discovery_at";

/// How long a fetch task may sit `in_progress` before it is considered
/// abandoned by a worker that died.
const STALE_TASK_SECS: u64 = 15 * 60;

/// Longest a worker may spend on a single fetch task before giving up.
///
/// Kept well below [`STALE_TASK_SECS`] so a live worker has always
/// abandoned a task (and stopped writing for it) before anyone else can
/// recover and claim it again.
pub(super) const TASK_TIMEOUT_SECS: u64 = 10 * 60;

const _: () = assert!(TASK_TIMEOUT_SECS < STALE_TASK_SECS);

impl Manager {
    /// Discover a package's tags and enqueue every version not seen before.
    ///
    /// Lists the tags in the upstream repository and upserts the package
    /// (with its WIT namespace mapping and kind) into the known packages
    /// table. Then it enqueues a pull for every semver tag that has no fetch
    /// queue entry yet. Semver tags are treated as immutable, so tags that
    /// were already queued or pulled are never re-fetched here. Use
    /// [`Manager::index_package_refetch`] to force that.
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

    /// Index a package and re-pull every version tag.
    ///
    /// Unlike [`Manager::index_package`], every semver tag is (re-)enqueued
    /// at high priority, regardless of whether it was pulled before.
    ///
    /// # Errors
    ///
    /// Returns an error if offline mode is enabled or if network operations fail.
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

    /// Open a handle used to elect the single replica that runs the
    /// background indexer. See [`IndexerLease`].
    ///
    /// # Errors
    ///
    /// Returns an error if the lease connection cannot be established.
    pub async fn indexer_lease(&self) -> anyhow::Result<IndexerLease> {
        self.store.indexer_lease().await
    }

    /// Put fetch tasks left `in_progress` by a worker that died back into
    /// the `pending` state. Returns the number of tasks recovered.
    ///
    /// A task counts as abandoned once it has been `in_progress` for longer
    /// than [`STALE_TASK_SECS`]. Workers time out after
    /// [`TASK_TIMEOUT_SECS`], so a task another live worker is running is
    /// left alone.
    ///
    /// # Errors
    ///
    /// Returns an error if the database update fails.
    pub async fn recover_in_progress_tasks(&self) -> anyhow::Result<u64> {
        self.store
            .reset_stale_in_progress_tasks(STALE_TASK_SECS)
            .await
    }

    /// When the indexer last completed a discovery pass, if ever.
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub async fn last_index_discovery_at(
        &self,
    ) -> anyhow::Result<Option<chrono::DateTime<chrono::Utc>>> {
        let value = self.store.get_sync_meta(LAST_DISCOVERY_KEY).await?;
        Ok(value
            .and_then(|v| chrono::DateTime::parse_from_rfc3339(&v).ok())
            .map(Into::into))
    }

    /// Record that the indexer just completed a discovery pass.
    ///
    /// # Errors
    ///
    /// Returns an error if the database write fails.
    pub async fn record_index_discovery(&self) -> anyhow::Result<()> {
        self.store
            .set_sync_meta(LAST_DISCOVERY_KEY, &chrono::Utc::now().to_rfc3339())
            .await
    }

    async fn index_package_inner(
        &self,
        reference: &Reference,
        wit_namespace: Option<&str>,
        wit_name: Option<&str>,
        kind: Option<PackageKind>,
        refetch: bool,
    ) -> anyhow::Result<KnownPackage> {
        if self.offline {
            return Err(ManagerError::OfflineIndex.into());
        }
        let registry = reference.registry();
        let repository = reference.repository();

        tracing::debug!(%registry, %repository, "Discovering package tags");
        let tags = self.client.list_tags(reference).await?;
        if tags.is_empty() {
            return Err(ManagerError::NoTagsFound {
                registry: registry.to_string(),
                repository: repository.to_string(),
            }
            .into());
        }

        // Tags like `latest`, `nightly`, or `sha256-...` cannot be resolved by
        // the version solver and render badly in the frontend, so packages
        // without any semver tag are not indexed at all.
        let semver_tags = sorted_semver_tags(reference, &tags);
        if semver_tags.is_empty() {
            tracing::debug!(
                %registry,
                %repository,
                discovered = tags.len(),
                "Skipping package — no tags parse as strict semver"
            );
            return Err(ManagerError::NoSemverTags {
                registry: registry.to_string(),
                repository: repository.to_string(),
            }
            .into());
        }

        let known = if refetch {
            KnownTags::default()
        } else {
            self.store.known_tags(registry, repository).await?
        };
        let has_new_tags = semver_tags
            .iter()
            .any(|t| !known.queued.contains(*t) && !known.cached.contains(*t));

        // The description comes from a manifest annotation. Pulls store the
        // annotations of every version they fetch, so only look it up here
        // when there is something new to fetch.
        let description = if has_new_tags {
            self.fetch_description(reference, &tags, &semver_tags).await
        } else {
            None
        };
        self.store
            .add_known_package_with_params(&KnownPackageParams {
                registry,
                repository,
                tag: semver_tags.last().map(|t| t.as_str()),
                description: description.as_deref(),
                wit_namespace,
                wit_name,
                kind,
            })
            .await?;

        let enqueued = enqueue_new_tags(
            &self.store,
            registry,
            repository,
            &semver_tags,
            &known,
            refetch,
        )
        .await?;
        if enqueued > 0 {
            tracing::info!(%registry, %repository, enqueued, "Enqueued versions for pulling");
        }

        let mut pkg = self
            .store
            .get_known_package(registry, repository)
            .await?
            .ok_or(ManagerError::IndexRetrievalFailed)?;
        pkg.dependencies = self
            .store
            .get_package_dependencies(registry, repository)
            .await?;
        Ok(pkg)
    }

    /// Best-effort lookup of the package description from the
    /// `org.opencontainers.image.description` annotation.
    ///
    /// Uses the reference's own tag when it exists upstream, and otherwise
    /// the latest stable (or highest) semver tag.
    async fn fetch_description(
        &self,
        reference: &Reference,
        tags: &[String],
        semver_tags: &[&String],
    ) -> Option<String> {
        let meta_tag = reference
            .tag()
            .filter(|t| tags.iter().any(|remote| remote == *t))
            .map(str::to_owned)
            .or_else(|| pick_latest_stable_tag(tags))
            .or_else(|| semver_tags.last().map(|t| (*t).clone()))?;
        let meta_ref: Reference = format!(
            "{}/{}:{}",
            reference.registry(),
            reference.repository(),
            meta_tag
        )
        .parse()
        .ok()?;
        match self.client.pull_manifest(&meta_ref).await {
            Ok((manifest, _digest)) => manifest
                .annotations
                .as_ref()
                .and_then(|a| a.get("org.opencontainers.image.description").cloned()),
            Err(e) => {
                tracing::warn!(
                    registry = %reference.registry(),
                    repository = %reference.repository(),
                    tag = %meta_tag,
                    error = %e,
                    "Failed to fetch manifest for package description"
                );
                None
            }
        }
    }
}

/// Enqueue pulls for the tags in `semver_tags` that are not yet known.
///
/// `semver_tags` must be sorted ascending: the highest stable version is
/// enqueued (and so processed) last, which keeps its dependencies at the
/// top of the `get_package_dependencies` query.
///
/// Returns the number of tags enqueued.
// r[impl server.index.dependencies]
async fn enqueue_new_tags(
    store: &Store,
    registry: &str,
    repository: &str,
    semver_tags: &[&String],
    known: &KnownTags,
    refetch: bool,
) -> anyhow::Result<u64> {
    let mut enqueued = 0u64;
    for tag in semver_tags {
        if refetch {
            // High priority for an explicit refetch.
            store.enqueue_refetch(registry, repository, tag, -1).await?;
        } else if known.queued.contains(*tag) {
            continue;
        } else if known.cached.contains(*tag) {
            // Pulled before the queue tracked it (e.g. by the CLI):
            // record it so it shows up in the queue history.
            store.record_completed(registry, repository, tag).await?;
            continue;
        } else {
            store.enqueue_pull(registry, repository, tag, 0).await?;
        }
        enqueued += 1;
    }
    Ok(enqueued)
}

/// Keep only tags that parse as semver, sorted ascending by version.
fn sorted_semver_tags<'a>(reference: &Reference, tags: &'a [String]) -> Vec<&'a String> {
    let mut semver_tags: Vec<(&String, semver::Version)> = Vec::with_capacity(tags.len());
    for tag in tags {
        if let Some(v) = parse_tag_as_semver(tag) {
            semver_tags.push((tag, v));
            continue;
        }
        tracing::debug!(
            registry = %reference.registry(),
            repository = %reference.repository(),
            tag = %tag,
            "Skipping enqueue — tag is not a valid semver version"
        );
    }
    semver_tags.sort_by(|(_, a), (_, b)| a.cmp(b));
    semver_tags.into_iter().map(|(t, _)| t).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(list: &[&str]) -> Vec<String> {
        list.iter().map(|t| (*t).to_owned()).collect()
    }

    #[test]
    fn sorted_semver_tags_filters_and_orders() {
        let reference: Reference = "ghcr.io/user/repo".parse().unwrap();
        let all = tags(&["2.0.0", "latest", "1.0.0", "1.0.0-rc.1", "sha256-abc.sig"]);
        let sorted: Vec<&str> = sorted_semver_tags(&reference, &all)
            .into_iter()
            .map(String::as_str)
            .collect();
        assert_eq!(sorted, ["1.0.0-rc.1", "1.0.0", "2.0.0"]);
    }

    #[tokio::test]
    async fn enqueue_new_tags_skips_known_tags() {
        let store = Store::open_in_memory().await.unwrap();
        let (registry, repository) = ("ghcr.io", "user/repo");
        store
            .enqueue_pull(registry, repository, "1.0.0", 0)
            .await
            .unwrap();
        let all = tags(&["1.0.0", "1.1.0", "2.0.0"]);
        let semver: Vec<&String> = all.iter().collect();
        let mut known = store.known_tags(registry, repository).await.unwrap();
        // Pretend 1.1.0 was pulled by the CLI before the queue existed.
        known.cached.insert("1.1.0".to_owned());

        let enqueued = enqueue_new_tags(&store, registry, repository, &semver, &known, false)
            .await
            .unwrap();
        assert_eq!(enqueued, 1, "only 2.0.0 is new");
        assert_eq!(store.pending_count().await.unwrap(), 2);
        let status = store.get_queue_status().await.unwrap();
        assert_eq!(status.completed, 1, "1.1.0 recorded as completed");

        // A second pass with fresh state enqueues nothing.
        let known = store.known_tags(registry, repository).await.unwrap();
        let enqueued = enqueue_new_tags(&store, registry, repository, &semver, &known, false)
            .await
            .unwrap();
        assert_eq!(enqueued, 0);
        assert_eq!(store.pending_count().await.unwrap(), 2);
    }

    #[tokio::test]
    async fn enqueue_new_tags_refetch_requeues_everything() {
        let store = Store::open_in_memory().await.unwrap();
        let (registry, repository) = ("ghcr.io", "user/repo");
        store
            .enqueue_pull(registry, repository, "1.0.0", 0)
            .await
            .unwrap();
        let done = store.dequeue_next().await.unwrap().unwrap();
        store.complete_task(&done).await.unwrap();

        let all = tags(&["1.0.0", "2.0.0"]);
        let semver: Vec<&String> = all.iter().collect();
        let enqueued = enqueue_new_tags(
            &store,
            registry,
            repository,
            &semver,
            &KnownTags::default(),
            true,
        )
        .await
        .unwrap();
        assert_eq!(enqueued, 2);
        assert_eq!(store.pending_count().await.unwrap(), 2);
    }
}
