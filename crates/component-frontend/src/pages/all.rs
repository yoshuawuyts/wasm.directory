//! All packages listing page.

// r[impl frontend.pages.all]

use html::text_content::Division;
use wasm_meta_registry_client::KnownPackage;

use crate::components::ds::results_page::{Notice, ResultsPage};
use crate::components::ds::{package_row, pagination::PaginationState};
use wasm_meta_registry_client::{ApiError, RegistryClient};

const TITLE: &str = "All Packages";

/// Fetch a package page and the index-wide total, then render the list.
pub(crate) async fn render(client: &RegistryClient, offset: u32, limit: u32) -> String {
    match client.fetch_all_packages(offset, limit).await {
        Ok(packages) => {
            let total = fetch_total(client).await;
            render_packages(&packages, total, offset, limit)
        }
        Err(err) => render_error(&err, offset, limit),
    }
}

/// A stats failure must not hide a successfully fetched package page.
async fn fetch_total(client: &RegistryClient) -> Option<u64> {
    match client.fetch_stats().await {
        Ok(stats) => Some(stats.packages),
        Err(err) => {
            eprintln!("component-frontend: all packages total unavailable: {err}");
            None
        }
    }
}

/// Render the package listing page with an optional index-wide total.
fn render_packages(
    packages: &[KnownPackage],
    total: Option<u64>,
    offset: u32,
    limit: u32,
) -> String {
    let page = ResultsPage::new(TITLE)
        .total(total)
        .rows(packages.iter().map(package_row::render))
        .empty(Notice::new(
            "No packages found. The registry may still be syncing.",
        ));
    if packages.is_empty() {
        page.render()
    } else {
        page.pagination(render_pagination(packages, offset, limit))
            .render()
    }
}

/// Render the page with an API error message.
fn render_error(err: &ApiError, offset: u32, limit: u32) -> String {
    ResultsPage::new(TITLE)
        .pagination(render_pagination(&[], offset, limit))
        .render_error(&Notice::new("Unable to load packages").detail(err.to_string()))
}

fn render_pagination(packages: &[KnownPackage], offset: u32, limit: u32) -> Division {
    PaginationState::new(packages.len(), offset, limit)
        .render(|offset, limit| format!("/all?offset={offset}&limit={limit}"))
}

#[cfg(test)]
mod tests;
