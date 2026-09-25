//! Consistent, fixed-query relationship result pages.

use html::text_content::Division;
use wasm_meta_registry_client::{
    ApiError, DependentPackage, KnownPackage, MatchingWorld, RegistryClient, RelationshipPage,
    RelationshipTarget,
};

use crate::components::ds::{listing, package_row, pagination::PaginationState, search_bar};
use crate::escape::{escape_html_attr, escape_html_text};
use crate::layout;
use crate::relationships::Relationship;

/// Load only the selected relationship page; propagate unavailable data.
pub(crate) async fn render(
    client: &RegistryClient,
    relation: Relationship,
    target: &RelationshipTarget,
    offset: u32,
    limit: u32,
) -> Result<String, ApiError> {
    match relation {
        Relationship::Dependents => {
            let page = client
                .fetch_dependents(target.package(), offset, limit)
                .await?;
            Ok(render_page(relation, target, &page, render_dependent))
        }
        Relationship::ImportedBy | Relationship::ExportedBy => {
            let page = match relation {
                Relationship::ImportedBy => {
                    client
                        .fetch_importing_worlds(target.package(), target.interface(), offset, limit)
                        .await?
                }
                _ => {
                    client
                        .fetch_exporting_worlds(target.package(), target.interface(), offset, limit)
                        .await?
                }
            };
            Ok(render_page(relation, target, &page, render_world))
        }
    }
}

fn render_dependent(result: &DependentPackage) -> Division {
    package_row::render_matching_package(
        &result.package,
        &result.version,
        matching_href(&result.package, &result.version, None),
    )
}

fn render_world(world: &MatchingWorld) -> Division {
    let world_name = (!world.is_synthetic).then_some(world.name.as_str());
    package_row::render_matching_world(
        world,
        matching_href(&world.package, &world.version, world_name),
    )
}

fn render_page<T>(
    relation: Relationship,
    target: &RelationshipTarget,
    page: &RelationshipPage<T>,
    render_row: impl Fn(&T) -> Division,
) -> String {
    let title = relation.heading(target);
    let mut body = Division::builder();
    body.push(listing::header(&title, page.results.len(), page.total));
    body.paragraph(|p| {
        p.class("mb-6 text-[13px] text-ink-500")
            .text(relation.description())
    });
    body.push(target_link(target));
    if page.results.is_empty() {
        body.division(|div| {
            div.class("py-16 text-center").paragraph(|p| {
                p.class("text-ink-500").text(match page.offset {
                    0 => relation.empty_message(),
                    _ => "No results on this page. Return to an earlier page to see matches.",
                })
            })
        });
    } else {
        body.push(result_rows(&page.results, render_row));
    }
    let base = relation.href(target);
    body.push(
        PaginationState::with_next(page.results.len(), page.offset, page.limit, page.has_next)
            .render(|offset, limit| {
                escape_html_attr(&format!("{base}&offset={offset}&limit={limit}"))
            }),
    );
    if page.results.is_empty() && page.offset > 0 {
        body.paragraph(|p| {
            p.class("mt-4 text-[13px]").anchor(|a| {
                a.href(escape_html_attr(&base))
                    .class("text-accent hover:underline")
                    .text("Back to first page")
            })
        });
    }
    layout::document_with_nav(&title, &body.build().to_string())
}

fn result_rows<T>(results: &[T], render_row: impl Fn(&T) -> Division) -> Division {
    let mut list = Division::builder();
    list.class("divide-y divide-lineSoft");
    for result in results {
        list.push(render_row(result));
    }
    list.build()
}

/// Render a visible failure with retry/navigation, never a success-shaped empty list.
pub(crate) fn render_error(
    relation: Relationship,
    target: &RelationshipTarget,
    message: &str,
    offset: u32,
    limit: u32,
) -> String {
    let title = relation.heading(target);
    let mut body = Division::builder();
    body.division(|div| {
        div.class("pt-8 pb-6 border-b-[1.5px] border-rule mb-6")
            .push(listing::heading(&title))
    });
    body.push(target_link(target));
    body.division(|div| {
        div.class("py-16 text-center")
            .paragraph(|p| {
                p.class("text-ink-900 font-medium")
                    .text("Unable to load relationship results")
            })
            .paragraph(|p| {
                p.class(crate::components::ds::typography::SUBTITLE_CLASS)
                    .text(escape_html_text(message))
            })
            .paragraph(|p| {
                p.class("mt-4").anchor(|a| {
                    a.href(escape_html_attr(&format!(
                        "{}&offset={offset}&limit={limit}",
                        relation.href(target)
                    )))
                    .class("text-[13px] text-accent hover:underline")
                    .text("Try again")
                })
            })
    });
    layout::document_with_nav(&title, &body.build().to_string())
}

fn target_link(target: &RelationshipTarget) -> Division {
    Division::builder()
        .class("mb-6 text-[13px]")
        .anchor(|a| {
            a.href(format!("/{}", target.package().replace(':', "/")))
                .class("text-accent hover:underline [overflow-wrap:anywhere]")
                .text(format!("Back to {}", target.package()))
        })
        .build()
}

/// Keep the matching repository as well as the matching tag when mirrors differ.
fn matching_href(pkg: &KnownPackage, version: &str, world: Option<&str>) -> Option<String> {
    let namespace = pkg.wit_namespace.as_deref()?;
    let name = pkg.wit_name.as_deref()?;
    let mut href = format!(
        "/{}/{}/{}",
        encode_segment(namespace),
        encode_segment(name),
        encode_segment(version)
    );
    if let Some(world) = world {
        href.push_str("/world/");
        href.push_str(&encode_segment(world));
    }
    Some(format!(
        "{href}?registry={}&repository={}",
        search_bar::encode_query(&pkg.registry),
        search_bar::encode_query(&pkg.repository)
    ))
}

fn encode_segment(segment: &str) -> String {
    search_bar::encode_query(segment).replace('+', "%20")
}

#[cfg(test)]
mod tests;
