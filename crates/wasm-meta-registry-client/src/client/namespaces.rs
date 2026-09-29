use wasm_meta_registry_types::{KnownNamespace, KnownPackage, RegistryPage};

use super::{ApiError, RegistryClient, percent_encode_query_component};

impl RegistryClient {
    /// Fetch all registered namespaces alphabetically with indexed-release package counts.
    pub async fn fetch_namespaces(
        &self,
        offset: u32,
        limit: u32,
    ) -> Result<RegistryPage<KnownNamespace>, ApiError> {
        let url = format!(
            "{}/v1/namespaces?offset={offset}&limit={limit}",
            self.base_url
        );
        self.fetch_registry_page(&url).await
    }

    /// Fetch packages belonging to one exact WIT namespace or repository owner.
    pub async fn fetch_namespace_packages(
        &self,
        namespace: &str,
        offset: u32,
        limit: u32,
    ) -> Result<RegistryPage<KnownPackage>, ApiError> {
        let namespace = percent_encode_query_component(namespace);
        let url = format!(
            "{}/v1/namespaces/{namespace}/packages?offset={offset}&limit={limit}",
            self.base_url
        );
        self.fetch_registry_page(&url).await
    }

    async fn fetch_registry_page<T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
    ) -> Result<RegistryPage<T>, ApiError> {
        self.fetch_optional(url).await?.ok_or_else(|| {
            ApiError::new(format!(
                "registry API returned unexpected status 404 Not Found for {url}"
            ))
        })
    }
}
