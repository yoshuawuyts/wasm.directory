//! Publish times taken from manifest config blobs.
//!
//! OCI image configs (including wasm configs) carry a `created` timestamp
//! that most publishers set even when they skip the
//! `org.opencontainers.image.created` manifest annotation. We record it in
//! `oci_manifest.config_created` so release lists can show when something
//! was actually published rather than when we first indexed it.

use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};
use wasm_package_manager_migration::entities::{oci_manifest, oci_repository};

use super::Store;

/// A manifest whose config blob hasn't been checked for a `created`
/// timestamp yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingConfig {
    /// The manifest's row ID.
    pub manifest_id: i64,
    /// Registry host (e.g. `ghcr.io`).
    pub registry: String,
    /// Repository path within the registry.
    pub repository: String,
    /// Digest of the config blob to fetch.
    pub config_digest: String,
}

/// Read the `created` timestamp from a config blob.
///
/// Returns an empty string when the blob isn't JSON or has no `created`
/// string, which records that the config was checked and had nothing.
#[must_use]
pub(crate) fn created_from_config(data: &[u8]) -> String {
    serde_json::from_slice::<serde_json::Value>(data)
        .ok()
        .and_then(|v| v.get("created")?.as_str().map(str::to_owned))
        .unwrap_or_default()
}

impl Store {
    /// Record the config blob's `created` timestamp for a manifest.
    pub(crate) async fn set_manifest_config_created(
        &self,
        manifest_id: i64,
        created: &str,
    ) -> anyhow::Result<()> {
        oci_manifest::ActiveModel {
            id: Set(manifest_id),
            config_created: Set(Some(created.to_owned())),
            ..Default::default()
        }
        .update(&self.db)
        .await?;
        Ok(())
    }

    /// List manifests whose config blob hasn't been checked yet, in ID
    /// order, starting after `after_id`.
    pub(crate) async fn manifests_missing_config_created(
        &self,
        after_id: i64,
        limit: u64,
    ) -> anyhow::Result<Vec<PendingConfig>> {
        let rows = oci_manifest::Entity::find()
            .find_also_related(oci_repository::Entity)
            .filter(oci_manifest::Column::Id.gt(after_id))
            .filter(oci_manifest::Column::ConfigCreated.is_null())
            .filter(oci_manifest::Column::ConfigDigest.is_not_null())
            .order_by_asc(oci_manifest::Column::Id)
            .limit(limit)
            .all(&self.db)
            .await?;
        Ok(rows
            .into_iter()
            .filter_map(|(manifest, repo)| {
                Some(PendingConfig {
                    manifest_id: manifest.id,
                    registry: repo.as_ref()?.registry.clone(),
                    repository: repo?.repository,
                    config_digest: manifest.config_digest?,
                })
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::super::{Store, upsert_oci_manifest, upsert_oci_repository_full};
    use super::*;

    #[test]
    fn reads_created_from_config_json() {
        let wasm = br#"{"created":"2026-07-09T15:55:43.6Z","architecture":"wasm"}"#;
        assert_eq!(created_from_config(wasm), "2026-07-09T15:55:43.6Z");
        assert_eq!(created_from_config(br#"{"architecture":"wasm"}"#), "");
        assert_eq!(created_from_config(br#"{"created":null}"#), "");
        assert_eq!(created_from_config(b"not json"), "");
    }

    #[tokio::test]
    async fn pending_configs_are_listed_until_recorded() {
        let store = Store::open_in_memory().await.expect("open store");
        let repo_id = upsert_oci_repository_full(&store.db, "ghcr.io", "a/b", None, None, None)
            .await
            .expect("upsert repo");
        let mut ids = Vec::new();
        for (digest, config) in [("sha256:m1", Some("sha256:c1")), ("sha256:m2", None)] {
            let (id, _) = upsert_oci_manifest(
                &store.db,
                repo_id,
                digest,
                None,
                None,
                None,
                None,
                None,
                config,
                &HashMap::new(),
            )
            .await
            .expect("upsert manifest");
            ids.push(id);
        }

        let pending = store
            .manifests_missing_config_created(0, 10)
            .await
            .expect("query");
        assert_eq!(
            pending,
            [PendingConfig {
                manifest_id: ids[0],
                registry: "ghcr.io".into(),
                repository: "a/b".into(),
                config_digest: "sha256:c1".into(),
            }],
            "manifests without a config digest are skipped"
        );
        let after = store
            .manifests_missing_config_created(ids[0], 10)
            .await
            .expect("query");
        assert!(after.is_empty(), "cursor skips earlier rows");

        store
            .set_manifest_config_created(ids[0], "")
            .await
            .expect("record");
        let pending = store
            .manifests_missing_config_created(0, 10)
            .await
            .expect("query");
        assert!(pending.is_empty(), "checked configs are not re-fetched");
    }
}
