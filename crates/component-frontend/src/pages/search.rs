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
        // Table-style header
        body.division(|div| {
            div.class(package_row::HEADER_CLASS)
                .span(|s| s.class("w-48 shrink-0").text("Name"))
                .span(|s| s.class("w-20 shrink-0").text("Version"))
                .span(|s| s.text("Description"))
        });

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

    fn package_without_wit() -> KnownPackage {
        KnownPackage {
            registry: "ghcr.io".to_string(),
            repository: "example/no-wit".to_string(),
            kind: None,
            description: Some("demo".to_string()),
            tags: vec!["1.0.0".to_string()],
            signature_tags: vec![],
            attestation_tags: vec![],
            last_seen_at: "2026-01-01T00:00:00Z".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            wit_namespace: None,
            wit_name: None,
            dependencies: vec![],
        }
    }

    #[test]
    fn non_wit_rows_render_as_non_links() {
        let html = package_row::render(&package_without_wit()).to_string();
        assert!(!html.contains("href=\"#\""));
        assert!(!html.contains("<a "));
    }
}
