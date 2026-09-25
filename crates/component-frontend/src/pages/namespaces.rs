//! Alphabetical namespace directory, styled like the all-packages listing.

use html::text_content::Division;
use wasm_meta_registry_client::{ApiError, KnownNamespace, RegistryClient, RegistryPage};

use crate::components::ds::{listing, pagination::PaginationState};
use crate::escape::{escape_html_attr, escape_html_text};
use crate::{layout, package_source::urls::encode_segment};

pub(crate) async fn render(
    client: &RegistryClient,
    offset: u32,
    limit: u32,
) -> Result<String, ApiError> {
    let page = client.fetch_namespaces(offset, limit).await?;
    Ok(render_namespaces(&page))
}

fn render_namespaces(page: &RegistryPage<KnownNamespace>) -> String {
    let mut body = Division::builder();
    body.push(listing::header(
        "All Namespaces",
        page.results.len(),
        Some(page.total),
    ));
    if page.results.is_empty() {
        let message = if page.offset == 0 {
            "No namespaces have been registered yet."
        } else {
            "No namespaces on this page. Return to the previous page to keep browsing."
        };
        body.division(|div| {
            div.class("py-16 text-center")
                .paragraph(|p| p.class("text-ink-500").text(message))
        });
    }
    body.push(namespace_list(&page.results));
    body.push(
        PaginationState::with_next(page.results.len(), page.offset, page.limit, page.has_next)
            .render(|offset, limit| format!("/namespaces?offset={offset}&limit={limit}")),
    );
    layout::document_with_nav("All Namespaces", &body.build().to_string())
}

fn namespace_list(namespaces: &[KnownNamespace]) -> Division {
    let mut list = Division::builder();
    list.class("divide-y divide-lineSoft");
    for namespace in namespaces {
        list.division(|row| {
            row.anchor(|a| {
                a.href(escape_html_attr(&format!("/{}", encode_segment(&namespace.name))))
                    .class("flex flex-col gap-1 py-3 -mx-2 px-2 hover:bg-surfaceMuted focus-visible:bg-surfaceMuted transition-colors motion-reduce:transition-none")
                    .span(|s| {
                        s.class("min-w-0 max-w-full mono text-[14px] font-medium text-ink-900 [overflow-wrap:anywhere]")
                            .text(escape_html_text(&namespace.name))
                    })
                    .span(|s| {
                        s.class("text-[12px] sm:text-[13px] text-ink-500")
                            .title("Indexed packages with at least one semver release; packages awaiting indexing are not counted.")
                            .text(format!(
                                "{} indexed package{}",
                                namespace.packages,
                                if namespace.packages == 1 { "" } else { "s" }
                            ))
                    })
            })
        });
    }
    list.build()
}

pub(crate) fn render_error(title: &str, message: &str) -> String {
    let body = Division::builder()
        .division(|div| {
            div.class("pt-8 pb-6 border-b-[1.5px] border-rule mb-6")
                .push(listing::heading(title))
        })
        .division(|div| {
            div.class("py-16 text-center")
                .paragraph(|p| {
                    p.class("text-ink-900 font-medium")
                        .text(escape_html_text(message))
                })
                .paragraph(|p| {
                    p.class(crate::components::ds::typography::SUBTITLE_CLASS)
                        .text("The registry could not complete this lookup. Please try again.")
                })
        })
        .build();
    layout::document_with_nav(title, &body.to_string())
}

#[cfg(test)]
mod tests;
