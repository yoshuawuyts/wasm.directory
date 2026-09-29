//! Resolve release links without confusing mirrors of the same WIT package.

pub(crate) mod urls;

use axum::http::{StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use serde::Deserialize;
use wasm_meta_registry_client::{KnownPackage, RegistryClient};

/// Optional OCI provenance attached to package detail navigation.
#[derive(Debug, Default, Deserialize)]
pub(crate) struct PackageSource {
    registry: Option<String>,
    repository: Option<String>,
}

impl PackageSource {
    /// Resolve the selected repository, still validating the URL's WIT identity and version.
    pub(crate) async fn fetch(
        &self,
        client: &RegistryClient,
        namespace: &str,
        name: &str,
        version: &str,
    ) -> Result<Option<KnownPackage>, Box<Response>> {
        let Some(pkg) = self.fetch_package(client, namespace, name).await? else {
            return Ok(None);
        };
        if matches_release(&pkg, namespace, name, version) {
            return Ok(Some(pkg));
        }
        eprintln!("component-frontend: version not found for {namespace}/{name}: {version}");
        Ok(None)
    }

    /// Resolve a package identity, including for a source-pinned latest-version redirect.
    pub(crate) async fn fetch_package(
        &self,
        client: &RegistryClient,
        namespace: &str,
        name: &str,
    ) -> Result<Option<KnownPackage>, Box<Response>> {
        let selected = self.selected_repository()?;
        if crate::reserved::is_reserved(namespace) {
            return Ok(None);
        }
        let result = match selected {
            Some((registry, repository)) => client.fetch_package(registry, repository).await,
            None => client.fetch_package_by_wit(namespace, name).await,
        };
        match result {
            Ok(Some(pkg)) if matches_identity(&pkg, namespace, name) => Ok(Some(pkg)),
            Ok(_) => {
                eprintln!("component-frontend: matching package not found: {namespace}:{name}");
                Ok(None)
            }
            Err(error) => {
                eprintln!("component-frontend: failed to resolve matching release: {error}");
                Err(Box::new(crate::error_response(&error.to_string())))
            }
        }
    }

    /// Preserve a complete source query when redirecting a legacy detail path.
    pub(crate) fn redirect_href(&self, path: &str) -> Result<String, Box<Response>> {
        Ok(match self.selected_repository()? {
            Some((registry, repository)) => urls::with_source(path, registry, repository),
            None => path.to_owned(),
        })
    }

    fn selected_repository(&self) -> Result<Option<(&str, &str)>, Box<Response>> {
        match (&self.registry, &self.repository) {
            (None, None) => Ok(None),
            (Some(registry), Some(repository))
                if !registry.is_empty() && !repository.is_empty() =>
            {
                Ok(Some((registry, repository)))
            }
            _ => {
                let message = "A package source requires a nonempty registry and repository.";
                eprintln!("component-frontend: {message}");
                Err(Box::new(
                    (
                        StatusCode::BAD_REQUEST,
                        [(header::CACHE_CONTROL, "no-cache")],
                        Html(crate::pages::error::render(message)),
                    )
                        .into_response(),
                ))
            }
        }
    }
}

fn matches_identity(pkg: &KnownPackage, namespace: &str, name: &str) -> bool {
    pkg.wit_namespace.as_deref() == Some(namespace) && pkg.wit_name.as_deref() == Some(name)
}

fn matches_release(pkg: &KnownPackage, namespace: &str, name: &str, version: &str) -> bool {
    matches_identity(pkg, namespace, name) && pkg.tags.iter().any(|tag| tag == version)
}

#[cfg(test)]
mod tests {
    mod fixtures;
    mod navigation;
    mod registry;
    mod routes;

    use super::*;
    use crate::relationship_routes::tests::registry_response;

    #[test]
    fn provenance_cannot_substitute_a_different_package_or_version() {
        let pkg = crate::components::ds::package_row::tests::packages().remove(0);
        assert!(matches_release(&pkg, "example", "http", "0.1.0"));
        assert!(!matches_release(&pkg, "other", "http", "0.1.0"));
        assert!(!matches_release(&pkg, "example", "other", "0.1.0"));
        assert!(!matches_release(&pkg, "example", "http", "9.0.0"));
    }

    fn mirror_source() -> PackageSource {
        PackageSource {
            registry: Some("mirror.test".to_owned()),
            repository: Some("mirrors/http".to_owned()),
        }
    }

    #[tokio::test]
    async fn matching_release_uses_the_selected_repository_not_name_search() {
        let mut pkg = crate::components::ds::package_row::tests::packages().remove(0);
        pkg.registry = "mirror.test".to_owned();
        pkg.repository = "mirrors/http".to_owned();
        pkg.tags = vec!["9.0.0".to_owned(), "0.1.0".to_owned()];
        let json = serde_json::to_string(&pkg).expect("serialize package");
        let (client, request) = registry_response("200 OK", &json).await;
        let selected = mirror_source()
            .fetch(&client, "example", "http", "0.1.0")
            .await
            .expect("resolve matching repository")
            .expect("matching release exists");
        assert_eq!(selected.registry, "mirror.test");
        assert_eq!(selected.repository, "mirrors/http");
        assert_eq!(
            request.await.expect("fixture request"),
            "GET /v1/packages/mirror.test/mirrors/http HTTP/1.1\r\n"
        );
    }

    #[tokio::test]
    async fn missing_matching_repository_is_not_replaced_by_another_mirror() {
        let (client, request) = registry_response("404 Not Found", "{}").await;
        assert!(
            mirror_source()
                .fetch(&client, "example", "http", "0.1.0")
                .await
                .expect("missing repository response")
                .is_none()
        );
        request.await.expect("fixture request");
    }

    #[tokio::test]
    async fn source_errors_remain_uncached_failures() {
        for (status, body) in [
            ("503 Service Unavailable", "{}"),
            ("200 OK", "invalid package JSON"),
        ] {
            let (client, request) = registry_response(status, body).await;
            let response = mirror_source()
                .fetch(&client, "example", "http", "0.1.0")
                .await
                .expect_err("unavailable source must fail");
            assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
            assert_eq!(response.headers()[header::CACHE_CONTROL], "no-cache");
            assert!(!response.headers().contains_key(header::ETAG));
            request.await.expect("fixture request");
        }
    }

    #[tokio::test]
    async fn incomplete_source_parameters_are_rejected_without_lookup() {
        let client = RegistryClient::new("http://127.0.0.1:1");
        for source in [
            PackageSource {
                registry: Some("mirror.test".to_owned()),
                repository: None,
            },
            PackageSource {
                registry: None,
                repository: Some("mirrors/http".to_owned()),
            },
            PackageSource {
                registry: Some(String::new()),
                repository: Some("mirrors/http".to_owned()),
            },
            PackageSource {
                registry: Some("mirror.test".to_owned()),
                repository: Some(String::new()),
            },
        ] {
            let response = source
                .fetch(&client, "example", "http", "0.1.0")
                .await
                .expect_err("partial source must be rejected");
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
            assert_eq!(response.headers()[header::CACHE_CONTROL], "no-cache");
        }
    }
}
