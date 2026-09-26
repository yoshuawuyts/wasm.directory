//! Namespace (publisher) page — lists all packages under a given namespace.

use wasm_meta_registry_client::{ApiError, KnownPackage, RegistryClient};

use crate::components::ds::package_row;
use crate::components::ds::results_page::{Notice, ResultsPage};

/// Render the namespace page listing all packages for a publisher.
pub(crate) async fn render(client: &RegistryClient, namespace: &str) -> String {
    match client.search_packages(namespace).await {
        Ok(packages) => {
            let filtered: Vec<_> = packages
                .iter()
                .filter(|p| p.wit_namespace.as_deref().is_some_and(|ns| ns == namespace))
                .collect();
            render_packages(namespace, &filtered)
        }
        Err(err) => {
            eprintln!("component-frontend: namespace page error for {namespace}: {err}");
            render_error(namespace, &err)
        }
    }
}

/// Render the package listing for a namespace.
fn render_packages(namespace: &str, packages: &[&KnownPackage]) -> String {
    ResultsPage::new(namespace)
        .rows(packages.iter().map(|pkg| package_row::render(pkg)))
        .empty(Notice::new("No packages found under this namespace."))
        .render()
}

/// Render the page with an API error message.
fn render_error(namespace: &str, err: &ApiError) -> String {
    ResultsPage::new(namespace)
        .render_error(&Notice::new("Unable to load packages").detail(err.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_two_row_items_without_column_headings() {
        let packages: Vec<_> = package_row::tests::packages()
            .into_iter()
            .filter(|pkg| pkg.wit_namespace.as_deref() == Some("example"))
            .collect();
        let refs: Vec<_> = packages.iter().collect();
        let html = render_packages("example", &refs);
        package_row::tests::assert_listing(&html, &packages);
        assert!(html.contains("showing 3 results (total unavailable)"));
    }
}
