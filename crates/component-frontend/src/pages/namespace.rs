//! Namespace (publisher) page — lists all packages under a given namespace.

use html::text_content::Division;
use wasm_meta_registry_client::{ApiError, KnownPackage, RegistryClient, RegistryPage};

use crate::components::ds::{listing, package_row, pagination::PaginationState};
use crate::layout;
use crate::package_source::urls::encode_segment;

/// Render the namespace page listing all packages for a publisher.
pub(crate) async fn render(
    client: &RegistryClient,
    namespace: &str,
    offset: u32,
    limit: u32,
) -> Result<String, ApiError> {
    let page = client
        .fetch_namespace_packages(namespace, offset, limit)
        .await?;
    Ok(render_packages(namespace, &page))
}

/// Render the package listing for a namespace.
fn render_packages(namespace: &str, page: &RegistryPage<KnownPackage>) -> String {
    let mut body = Division::builder();
    body.push(listing::header(
        namespace,
        page.results.len(),
        Some(page.total),
    ));
    if page.results.is_empty() {
        let message = if page.offset == 0 {
            "No indexed packages found under this namespace yet."
        } else {
            "No packages on this page. Return to the previous page to keep browsing."
        };
        body.division(|div| {
            div.class("py-16 text-center")
                .paragraph(|p| p.class("text-ink-500").text(message))
        });
    }
    let mut list = Division::builder();
    list.class("divide-y divide-lineSoft");
    for pkg in &page.results {
        list.push(package_row::render(pkg));
    }
    body.push(list.build());
    body.push(
        PaginationState::with_next(page.results.len(), page.offset, page.limit, page.has_next)
            .render(|offset, limit| {
                format!(
                    "/{}?offset={offset}&limit={limit}",
                    encode_segment(namespace)
                )
            }),
    );
    layout::document_with_nav(namespace, &body.build().to_string())
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
        let page = RegistryPage {
            results: packages.clone(),
            total: 3,
            offset: 0,
            limit: 100,
            has_next: false,
        };
        let html = render_packages("example", &page);
        package_row::tests::assert_listing(&html, &packages);
        assert!(html.contains("showing 3 of 3 results"));
    }
}
