//! All packages listing page.

// r[impl frontend.pages.all]

use html::text_content::Division;
use wasm_meta_registry_client::KnownPackage;

use crate::components::ds::package_row;
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

/// Render a page count without inventing a total when stats are unavailable.
fn result_summary(page_count: usize, total: Option<u64>) -> String {
    let page_count = u64::try_from(page_count).expect("package page count should fit in u64");
    let total = match total {
        Some(total) if total < page_count => {
            eprintln!(
                "component-frontend: all packages total {total} is smaller than page count {page_count}"
            );
            None
        }
        total => total,
    };
    match total {
        Some(1) => format!("showing {page_count} of 1 result"),
        Some(total) => format!("showing {page_count} of {total} results"),
        None if page_count == 1 => "showing 1 result (total unavailable)".to_owned(),
        None => format!("showing {page_count} results (total unavailable)"),
    }
}

fn render_header(page_count: usize, total: Option<u64>) -> Division {
    Division::builder()
        .class("pt-8 flex flex-wrap items-baseline justify-between gap-x-4 gap-y-2 pb-6 border-b-[1.5px] border-rule mb-6")
        .heading_1(|h1| {
            h1.class(crate::components::ds::typography::H1_CLASS)
                .text("All Packages")
        })
        .span(|s| {
            s.class(crate::components::ds::typography::SUBTITLE_CLASS)
                .text(result_summary(page_count, total))
        })
        .build()
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
    let state = PaginationState::new(packages.len(), offset, limit);
    let mut container = Division::builder();
    container
        .class("flex items-center justify-between gap-4 mt-8 pt-6 border-t-[1.5px] border-rule");
    container.span(|s| {
        s.class("text-[13px] text-ink-400")
            .text(format!("Showing {}–{}", state.start, state.end))
    });
    container.push(render_pagination_controls(&state));
    container.build()
}

#[derive(Debug)]
struct PaginationState {
    effective_limit: u32,
    prev_offset: u32,
    next_offset: u32,
    has_prev: bool,
    has_next: bool,
    start: u32,
    end: u32,
}

impl PaginationState {
    #[must_use]
    fn new(package_count: usize, offset: u32, limit: u32) -> Self {
        let effective_limit = limit.max(1);
        let has_prev = offset > 0;
        let has_next = u32::try_from(package_count) == Ok(effective_limit);
        let prev_offset = offset.saturating_sub(effective_limit);
        let next_offset = offset.saturating_add(effective_limit);
        let count = u32::try_from(package_count).unwrap_or(0);
        let (start, end) = if count == 0 {
            (0, 0)
        } else {
            (offset.saturating_add(1), offset.saturating_add(count))
        };

        Self {
            effective_limit,
            prev_offset,
            next_offset,
            has_prev,
            has_next,
            start,
            end,
        }
    }
}

fn render_pagination_controls(state: &PaginationState) -> Division {
    let mut controls = Division::builder();
    controls.class("flex items-center gap-2");
    if state.has_prev {
        controls.anchor(|a| {
            a.href(format!(
                "/all?offset={}&limit={}",
                state.prev_offset, state.effective_limit
            ))
            .class(crate::components::ds::breadcrumb::PAGINATION_BUTTON_CLASS)
            .text("Previous")
        });
    } else {
        controls.span(|s| {
            s.class(crate::components::ds::breadcrumb::PAGINATION_DISABLED_CLASS)
                .text("Previous")
        });
    }
    if state.has_next {
        controls.anchor(|a| {
            a.href(format!(
                "/all?offset={}&limit={}",
                state.next_offset, state.effective_limit
            ))
            .class(crate::components::ds::breadcrumb::PAGINATION_BUTTON_CLASS)
            .text("Next")
        });
    } else {
        controls.span(|s| {
            s.class(crate::components::ds::breadcrumb::PAGINATION_DISABLED_CLASS)
                .text("Next")
        });
    }
    controls.build()
}

#[cfg(test)]
mod tests;
