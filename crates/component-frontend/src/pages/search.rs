//! Search results page.

// r[impl frontend.pages.search]

use html::text_content::Division;
use wasm_meta_registry_client::KnownPackage;

use crate::components::ds::{package_row, search_bar};
use crate::escape::escape_html_text;
use crate::layout;
use wasm_meta_registry_client::{ApiError, RegistryClient};

/// Fetch matching packages and render the search results page.
pub(crate) async fn render(client: &RegistryClient, query: &str) -> String {
    match client.search_packages(query).await {
        Ok(packages) => render_results(query, &packages),
        Err(err) => render_error(query, &err),
    }
}

/// Render the search results.
fn render_results(query: &str, packages: &[KnownPackage]) -> String {
    let mut body = Division::builder();

    // Page header
    body.division(|div| {
        div.class("pt-8 pb-6 border-b-[1.5px] border-rule mb-6")
            .heading_1(|h1| {
                h1.class(crate::components::ds::typography::H1_CLASS)
                    .text(format!(
                        "Results for \u{201c}{}\u{201d}",
                        escape_html_text(query)
                    ))
            })
            .paragraph(|p| {
                p.class(format!(
                    "{} mt-2",
                    crate::components::ds::typography::SUBTITLE_CLASS
                ))
                .text(format!(
                    "{} result{} found",
                    packages.len(),
                    if packages.len() == 1 { "" } else { "s" }
                ))
            })
    });

    // Search box so users can refine
    body.push(render_search_form(query));

    if packages.is_empty() {
        body.division(|div| {
            div.class("py-16 text-center")
                .paragraph(|p| {
                    p.class("text-ink-500")
                        .text("No results matched your query.")
                })
                .paragraph(|p| {
                    p.class("mt-4").anchor(|a| {
                        a.href("/all")
                            .class("text-[13px] text-accent hover:underline")
                            .text("Browse all →")
                    })
                })
        });
    } else {
        let mut list = Division::builder();
        list.class("divide-y divide-lineSoft");
        for pkg in packages {
            list.push(package_row::render(pkg));
        }
        body.push(list.build());
    }

    layout::document_with_nav("Search", &body.build().to_string())
}

/// Render the page with an API error message.
fn render_error(query: &str, err: &ApiError) -> String {
    let mut body = Division::builder();

    body.division(|div| {
        div.class("pt-8 pb-6 border-b-[1.5px] border-rule mb-6")
            .heading_1(|h1| {
                h1.class(crate::components::ds::typography::H1_CLASS)
                    .text(format!(
                        "Results for \u{201c}{}\u{201d}",
                        escape_html_text(query)
                    ))
            })
    });

    body.push(render_search_form(query));

    body.division(|div| {
        div.class("py-16 text-center")
            .paragraph(|p| p.class("text-ink-900 font-medium").text("Unable to search"))
            .paragraph(|p| {
                p.class(crate::components::ds::typography::SUBTITLE_CLASS)
                    .text(err.to_string())
            })
    });

    layout::document_with_nav("Search", &body.build().to_string())
}

/// Inline search form for refining queries.
fn render_search_form(query: &str) -> Division {
    search_bar::inline(query)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_flowing_rows_without_column_headings() {
        let packages = package_row::tests::packages();
        let html = render_results("example", &packages);
        package_row::tests::assert_listing(&html, &packages);
        assert!(html.contains("4 results found"));
        assert!(html.contains("value=\"example\""));
    }
}
