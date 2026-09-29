use wasm_meta_registry_types::{DependentPackage, MatchingWorld, RelationshipPage};

use super::{ApiError, RegistryClient, percent_encode_query_component};

impl RegistryClient {
    /// Fetch direct and transitive dependents across indexed versions.
    ///
    /// Each package appears once at its newest matching release.
    pub async fn fetch_dependents(
        &self,
        package: &str,
        offset: u32,
        limit: u32,
    ) -> Result<RelationshipPage<DependentPackage>, ApiError> {
        self.fetch_relationship_page("dependents", package, None, offset, limit)
            .await
    }

    /// Fetch worlds importing a package or one exact interface within it.
    pub async fn fetch_importing_worlds(
        &self,
        package: &str,
        interface: Option<&str>,
        offset: u32,
        limit: u32,
    ) -> Result<RelationshipPage<MatchingWorld>, ApiError> {
        self.fetch_relationship_page("imported-by", package, interface, offset, limit)
            .await
    }

    /// Fetch worlds exporting a package or one exact interface within it.
    pub async fn fetch_exporting_worlds(
        &self,
        package: &str,
        interface: Option<&str>,
        offset: u32,
        limit: u32,
    ) -> Result<RelationshipPage<MatchingWorld>, ApiError> {
        self.fetch_relationship_page("exported-by", package, interface, offset, limit)
            .await
    }

    async fn fetch_relationship_page<T: serde::de::DeserializeOwned>(
        &self,
        relation: &str,
        package: &str,
        interface: Option<&str>,
        offset: u32,
        limit: u32,
    ) -> Result<RelationshipPage<T>, ApiError> {
        let url = relationship_url(&self.base_url, relation, package, interface, offset, limit);
        self.fetch_optional(&url).await?.ok_or_else(|| {
            ApiError::new(format!(
                "registry API returned unexpected status 404 Not Found for {url}"
            ))
        })
    }
}

fn relationship_url(
    base: &str,
    relation: &str,
    package: &str,
    interface: Option<&str>,
    offset: u32,
    limit: u32,
) -> String {
    let package = percent_encode_query_component(package);
    let interface = interface.map_or_else(String::new, |name| {
        format!("&interface={}", percent_encode_query_component(name))
    });
    format!(
        "{base}/v1/relationships/{relation}?package={package}{interface}&offset={offset}&limit={limit}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_targets_and_preserves_pagination() {
        assert_eq!(
            relationship_url(
                "http://example.test",
                "imported-by",
                "wasi:io&other=true",
                Some("streams /?"),
                200,
                100,
            ),
            "http://example.test/v1/relationships/imported-by?package=wasi%3Aio%26other%3Dtrue&interface=streams%20%2F%3F&offset=200&limit=100",
        );
        assert_eq!(
            relationship_url("http://example.test", "dependents", "wasi:io", None, 0, 100),
            "http://example.test/v1/relationships/dependents?package=wasi%3Aio&offset=0&limit=100",
        );
    }

    #[cfg(not(all(target_os = "wasi", target_env = "p2")))]
    mod requests {
        use super::*;
        use crate::client::tests::spawn_single_response_server;

        const EMPTY_PAGE: &str =
            r#"{"results":[],"total":0,"offset":100,"limit":100,"has_next":false}"#;

        #[tokio::test]
        async fn all_relationship_endpoints_deserialize_pages() {
            let base = spawn_single_response_server("200 OK", EMPTY_PAGE, "application/json");
            let page = RegistryClient::new(base)
                .fetch_dependents("wasi:io", 100, 100)
                .await
                .expect("fetch dependents");
            assert_eq!(page.total, Some(0));
            assert_eq!(page.offset, 100);
            assert!(!page.has_next);

            for importing in [true, false] {
                let base = spawn_single_response_server("200 OK", EMPTY_PAGE, "application/json");
                let client = RegistryClient::new(base);
                let page = if importing {
                    client
                        .fetch_importing_worlds("wasi:io", Some("streams"), 100, 100)
                        .await
                } else {
                    client
                        .fetch_exporting_worlds("wasi:io", None, 100, 100)
                        .await
                }
                .expect("fetch worlds");
                assert_eq!(page.total, Some(0));
                assert_eq!(page.limit, 100);
                assert!(page.results.is_empty());
            }
        }

        #[tokio::test]
        async fn relationship_worlds_preserve_synthetic_status_with_unknown_kind() {
            for synthetic in [None, Some(false), Some(true)] {
                let mut world = serde_json::json!({
                    "package": {
                        "registry": "registry.test",
                        "repository": "test/source",
                        "description": null,
                        "tags": ["1.0.0"],
                        "last_seen_at": "2026-01-01T00:00:00Z",
                        "created_at": "2026-01-01T00:00:00Z"
                    },
                    "name": "root",
                    "version": "1.0.0"
                });
                if let Some(synthetic) = synthetic {
                    world["is_synthetic"] = synthetic.into();
                }
                let body = serde_json::json!({
                    "results": [world], "total": 1, "offset": 0, "limit": 1, "has_next": false
                })
                .to_string();
                let base = spawn_single_response_server("200 OK", &body, "application/json");
                let page = RegistryClient::new(base)
                    .fetch_importing_worlds("wasi:io", None, 0, 1)
                    .await
                    .expect("world response");
                let world = page.results.first().expect("matching world");
                assert!(world.package.kind.is_none());
                assert_eq!(world.is_synthetic, synthetic.unwrap_or(false));
            }
        }

        #[tokio::test]
        async fn relationship_failures_never_become_empty_successes() {
            for status in [
                "400 Bad Request",
                "404 Not Found",
                "503 Service Unavailable",
            ] {
                let base = spawn_single_response_server(status, EMPTY_PAGE, "application/json");
                let error = RegistryClient::new(base)
                    .fetch_dependents("wasi:io", 0, 100)
                    .await
                    .expect_err("non-success response must be an error");
                assert!(error.to_string().contains(status));
            }
            let base = spawn_single_response_server("200 OK", "not json", "application/json");
            assert!(
                RegistryClient::new(base)
                    .fetch_importing_worlds("wasi:io", None, 0, 100)
                    .await
                    .is_err()
            );
        }
    }
}
