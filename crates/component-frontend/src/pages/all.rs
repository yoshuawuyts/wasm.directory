//! All packages listing page.

// r[impl frontend.pages.all]

use html::text_content::Division;
use wasm_meta_registry_client::KnownPackage;

use crate::components::ds::{listing, package_row, pagination::PaginationState};
use crate::layout;
use wasm_meta_registry_client::{ApiError, RegistryClient};

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

fn render_header(page_count: usize, total: Option<u64>) -> Division {
    listing::header("All Packages", page_count, total)
}

/// Render the package listing page with an optional index-wide total.
fn render_packages(
    packages: &[KnownPackage],
    total: Option<u64>,
    offset: u32,
    limit: u32,
) -> String {
    let mut body = Division::builder();
    body.push(render_header(packages.len(), total));
    if packages.is_empty() {
        body.division(|div| {
            div.class("py-16 text-center").paragraph(|p| {
                p.class("text-ink-500")
                    .text("No packages found. The registry may still be syncing.")
            })
        });
    } else {
        let mut list = Division::builder();
        list.class("divide-y divide-lineSoft");
        for pkg in packages {
            list.push(package_row::render(pkg));
        }
        body.push(list.build());

        body.push(render_pagination(packages, offset, limit));
    }

    layout::document_with_nav("All Packages", &body.build().to_string())
}

/// Render the page with an API error message.
fn render_error(err: &ApiError, offset: u32, limit: u32) -> String {
    let mut body = Division::builder();

    body.division(|div| {
        div.class("pt-8 pb-6 border-b-[1.5px] border-rule mb-6")
            .heading_1(|h1| {
                h1.class(crate::components::ds::typography::H1_CLASS)
                    .text("All Packages")
            })
    });

    body.division(|div| {
        div.class("py-16 text-center")
            .paragraph(|p| {
                p.class("text-ink-900 font-medium")
                    .text("Unable to load packages")
            })
            .paragraph(|p| {
                p.class(crate::components::ds::typography::SUBTITLE_CLASS)
                    .text(err.to_string())
            })
    });

    body.push(render_pagination(&[], offset, limit));

    layout::document_with_nav("All Packages", &body.build().to_string())
}

fn render_pagination(packages: &[KnownPackage], offset: u32, limit: u32) -> Division {
    PaginationState::new(packages.len(), offset, limit)
        .render(|offset, limit| format!("/all?offset={offset}&limit={limit}"))
}

#[cfg(test)]
mod tests;
