use std::collections::BTreeMap;

use docker_credential::DockerCredential;
use oci_client::Reference;
use oci_client::client::{ClientConfig, ClientProtocol, ImageData, PushResponse, SizedStream};
use oci_client::manifest::{OciDescriptor, OciImageIndex, OciImageManifest};
use oci_client::secrets::RegistryAuth;
use oci_wasm::{WasmClient, WasmConfig};

use crate::config::Config;

pub(crate) struct Client {
    inner: WasmClient,
    config: Config,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client").finish_non_exhaustive()
    }
}

impl Client {
    pub(crate) fn new(config: Config) -> Self {
        let client_config = ClientConfig {
            protocol: ClientProtocol::Https,
            ..Default::default()
        };
        let client = WasmClient::new(oci_client::Client::new(client_config));
        Self {
            inner: client,
            config,
        }
    }

    pub(crate) async fn pull(&self, reference: &Reference) -> anyhow::Result<ImageData> {
        let auth = resolve_auth(reference, &self.config)?;
        let image = self.inner.pull(reference, &auth).await?;
        Ok(image)
    }

    /// Push a single-layer wasm artifact (component or WIT package) to the
    /// registry, mirroring [`Client::pull`].
    ///
    /// Builds a [`WasmConfig`] from the supplied bytes (using
    /// [`WasmConfig::from_raw_component`], which works for both compiled
    /// components and WIT-only packages) and uploads the layer + config +
    /// manifest. The supplied `annotations` are attached to the OCI
    /// manifest using the `org.opencontainers.image.*` keys (callers are
    /// responsible for picking the right keys).
    pub(crate) async fn push(
        &self,
        reference: &Reference,
        bytes: Vec<u8>,
        annotations: BTreeMap<String, String>,
    ) -> anyhow::Result<PushResponse> {
        let auth = resolve_auth(reference, &self.config)?;
        let (config, layer) = WasmConfig::from_raw_component(bytes, None)?;
        let annotations_opt = if annotations.is_empty() {
            None
        } else {
            Some(annotations)
        };
        self.inner
            .push(reference, &auth, layer, config, annotations_opt)
            .await
    }

    /// Fetches the manifest and config digest for a given reference.
    ///
    /// Returns the OCI image manifest and the content digest.
    pub(crate) async fn pull_manifest(
        &self,
        reference: &Reference,
    ) -> anyhow::Result<(OciImageManifest, String)> {
        let auth = resolve_auth(reference, &self.config)?;
        let (manifest, _config, digest) = self
            .inner
            .pull_manifest_and_config(reference, &auth)
            .await?;
        Ok((manifest, digest))
    }

    /// Streams a single layer from the registry.
    ///
    /// Returns a `SizedStream` that yields chunks of bytes and optionally
    /// provides the content length.
    pub(crate) async fn pull_layer_stream(
        &self,
        reference: &Reference,
        layer: &OciDescriptor,
    ) -> anyhow::Result<SizedStream> {
        let auth = resolve_auth(reference, &self.config)?;
        // Ensure auth is stored before calling pull_blob_stream
        self.inner
            .store_auth_if_needed(reference.resolve_registry(), &auth)
            .await;
        let stream = self.inner.pull_blob_stream(reference, layer).await?;
        Ok(stream)
    }

    /// Fetches all tags for a given reference from the registry.
    ///
    /// This method handles pagination automatically, fetching all available tags
    /// by making multiple requests if necessary.
    pub(crate) async fn list_tags(&self, reference: &Reference) -> anyhow::Result<Vec<String>> {
        let auth = resolve_auth(reference, &self.config)?;
        let mut all_tags = Vec::new();
        let mut last: Option<String> = None;

        loop {
            // Some registries return null for tags instead of an empty array,
            // which causes deserialization to fail. We handle this gracefully.
            let response = match self
                .inner
                .list_tags(reference, &auth, None, last.as_deref())
                .await
            {
                Ok(resp) => resp,
                Err(_) if all_tags.is_empty() => {
                    // First request failed, likely due to null tags - return empty
                    return Ok(Vec::new());
                }
                Err(_) => {
                    // Subsequent request failed, return what we have
                    break;
                }
            };

            if response.tags.is_empty() {
                break;
            }

            last = response.tags.last().cloned();
            all_tags.extend(response.tags);

            // If we got fewer tags than a typical page size, we're done
            // The API doesn't provide a "next" link, so we detect the end
            // by checking if the last tag changed
            if last.is_none() {
                break;
            }

            // Make another request to check if there are more tags
            let Ok(next_response) = self
                .inner
                .list_tags(reference, &auth, Some(1), last.as_deref())
                .await
            else {
                break;
            };

            if next_response.tags.is_empty() {
                break;
            }
        }

        Ok(all_tags)
    }

    /// Fetches referrers (signatures, SBOMs, attestations) for a given reference.
    ///
    /// The OCI Referrers API requires a digest-based reference, so this method
    /// builds a digest-pinned [`Reference`] internally while using the original
    /// reference for authentication.
    ///
    /// Returns the OCI image index listing all referrer manifests. If the
    /// registry does not support the Referrers API, returns `Ok(None)`.
    pub(crate) async fn pull_referrers(
        &self,
        reference: &Reference,
        digest: &str,
    ) -> anyhow::Result<Option<OciImageIndex>> {
        let auth = resolve_auth(reference, &self.config)?;
        self.inner
            .store_auth_if_needed(reference.resolve_registry(), &auth)
            .await;

        // The Referrers API requires a digest-based reference — build one
        // from the original reference with the digest instead of a tag.
        let digest_ref = Reference::with_digest(
            reference.registry().to_owned(),
            reference.repository().to_owned(),
            digest.to_owned(),
        );

        match self.inner.pull_referrers(&digest_ref, None).await {
            Ok(index) => Ok(Some(index)),
            // Registry may not support the Referrers API — log and skip.
            Err(e) => {
                tracing::debug!(
                    "Failed to pull referrers for {} (resolved from {}, treating as no referrers): {}",
                    digest_ref,
                    reference,
                    e
                );
                Ok(None)
            }
        }
    }

    /// Resolve the OCI artifact type of a referrer by fetching its manifest.
    ///
    /// The per-entry [`ImageIndexEntry`](oci_client::manifest::ImageIndexEntry)
    /// returned by the Referrers API only exposes `media_type` (the manifest's
    /// own media type, e.g. `application/vnd.oci.image.manifest.v1+json`), not
    /// the artifact type. To classify a referrer (signature, SBOM,
    /// attestation, …) we must fetch the referrer manifest and read its
    /// top-level `artifactType`, falling back to the config descriptor's
    /// `mediaType` as the OCI spec prescribes when `artifactType` is absent.
    pub(crate) async fn pull_referrer_manifest(
        &self,
        reference: &Reference,
        referrer_digest: &str,
    ) -> anyhow::Result<String> {
        let auth = resolve_auth(reference, &self.config)?;
        let digest_ref = Reference::with_digest(
            reference.registry().to_owned(),
            reference.repository().to_owned(),
            referrer_digest.to_owned(),
        );
        let (manifest, _digest) = self.inner.pull_image_manifest(&digest_ref, &auth).await?;
        Ok(resolve_artifact_type(&manifest))
    }
}

/// Determine the OCI artifact type of a manifest.
///
/// Uses the manifest's top-level `artifactType` when present and non-empty,
/// otherwise falls back to the config descriptor's `mediaType`, as prescribed
/// by the OCI image spec (the config `mediaType` carries the artifact type
/// when `artifactType` is unset).
fn resolve_artifact_type(manifest: &OciImageManifest) -> String {
    manifest
        .artifact_type
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or(&manifest.config.media_type)
        .to_owned()
}

/// Resolve authentication for a registry reference.
///
/// The authentication is resolved in the following order:
/// 1. Check if a credential helper is configured in the config file for this registry
/// 2. Fall back to Docker credential store
/// 3. Use anonymous access if no credentials are found
fn resolve_auth(reference: &Reference, config: &Config) -> anyhow::Result<RegistryAuth> {
    let registry = reference.resolve_registry();

    // First, check if a credential helper is configured in the config file.
    // If a helper is configured but fails, propagate the error rather than
    // silently falling back to Docker credentials.
    match config.get_credentials(registry)? {
        Some((username, password)) => {
            tracing::debug!(registry, "using credential helper for authentication");
            return Ok(RegistryAuth::Basic(username, password));
        }
        None => {
            tracing::debug!(registry, "no credential helper configured");
        }
    }

    // Fall back to Docker credential store
    // NOTE: copied approach from https://github.com/bytecodealliance/wasm-pkg-tools/blob/48c28825a7dfb585b3fe1d42be65fe73a17d84fe/crates/wkg/src/oci.rs#L59-L66
    let server_url = match registry {
        "index.docker.io" => "https://index.docker.io/v1/",
        other => other,
    };

    match docker_credential::get_credential(server_url) {
        Ok(DockerCredential::UsernamePassword(username, password)) => {
            tracing::debug!(registry, "using Docker credential store for authentication");
            Ok(RegistryAuth::Basic(username, password))
        }
        Ok(DockerCredential::IdentityToken(_)) => {
            Err(crate::oci::OciLayerError::IdentityTokenNotSupported.into())
        }
        Err(_) => {
            tracing::debug!(registry, "no credentials found, using anonymous access");
            Ok(RegistryAuth::Anonymous)
        }
    }
}

#[cfg(test)]
mod tests {
    use oci_client::Reference;
    use oci_client::manifest::{OciDescriptor, OciImageManifest};

    use super::resolve_artifact_type;

    /// The top-level `artifactType` is used to classify the referrer when it
    /// is present, regardless of the config descriptor's media type.
    #[test]
    fn artifact_type_prefers_top_level() {
        let manifest = OciImageManifest {
            artifact_type: Some("application/vnd.dev.sigstore.bundle.v0.3+json".to_owned()),
            config: OciDescriptor {
                media_type: "application/vnd.oci.empty.v1+json".to_owned(),
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(
            resolve_artifact_type(&manifest),
            "application/vnd.dev.sigstore.bundle.v0.3+json"
        );
    }

    /// When `artifactType` is absent or empty, the config descriptor's
    /// `mediaType` is used as the artifact type per the OCI image spec.
    #[test]
    fn artifact_type_falls_back_to_config_media_type() {
        let config_media_type = "application/vnd.dev.cosign.simplesigning.v1+json";
        for artifact_type in [None, Some(String::new())] {
            let manifest = OciImageManifest {
                artifact_type,
                config: OciDescriptor {
                    media_type: config_media_type.to_owned(),
                    ..Default::default()
                },
                ..Default::default()
            };
            assert_eq!(resolve_artifact_type(&manifest), config_media_type);
        }
    }

    /// Verify that a digest-pinned reference built from a tag-based reference
    /// has `digest().is_some()` and `tag().is_none()`, matching the
    /// requirements of the OCI Referrers API.
    #[test]
    fn digest_reference_has_digest_and_no_tag() {
        let tag_ref: Reference = "ghcr.io/microsoft/fetch-rs:v0.1.0"
            .parse()
            .expect("valid tag reference");
        assert!(tag_ref.tag().is_some());
        assert!(tag_ref.digest().is_none());

        let digest = "sha256:abc123def456";
        let digest_ref = Reference::with_digest(
            tag_ref.registry().to_owned(),
            tag_ref.repository().to_owned(),
            digest.to_owned(),
        );

        assert!(digest_ref.digest().is_some());
        assert!(digest_ref.tag().is_none());
        assert_eq!(digest_ref.digest().unwrap(), digest);
        assert_eq!(digest_ref.registry(), tag_ref.registry());
        assert_eq!(digest_ref.repository(), tag_ref.repository());
    }
}
