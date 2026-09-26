//! Consistent, fixed-query relationship result pages.

use html::text_content::Division;
use wasm_meta_registry_client::{
    ApiError, DependentPackage, KnownPackage, MatchingWorld, RegistryClient, RelationshipPage,
    RelationshipTarget,
};

use crate::components::ds::results_page::{Notice, ResultsPage};
use crate::components::ds::{package_row, pagination::PaginationState};
use crate::escape::escape_html_attr;
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
    let base = relation.href(target);
    let empty = match page.offset {
        0 => Notice::new(relation.empty_message()),
        _ => Notice::new("No results on this page. Return to an earlier page to see matches.")
            .action("Back to first page", base.clone()),
    };
    page_shell(relation, target, &title)
        .total(page.total)
        .rows(page.results.iter().map(render_row))
        .empty(empty)
        .pagination(
            PaginationState::with_next(page.results.len(), page.offset, page.limit, page.has_next)
                .render(|offset, limit| {
                    escape_html_attr(&format!("{base}&offset={offset}&limit={limit}"))
                }),
        )
        .render()
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
    let retry = format!("{}&offset={offset}&limit={limit}", relation.href(target));
    page_shell(relation, target, &title).render_error(
        &Notice::new("Unable to load relationship results")
            .detail(message)
            .action("Try again", retry),
    )
}

/// The heading, description, and target link shared by results and errors.
fn page_shell<'a>(
    relation: Relationship,
    target: &RelationshipTarget,
    title: &'a str,
) -> ResultsPage<'a> {
    ResultsPage::new(title)
        .intro(
            Division::builder()
                .paragraph(|p| {
                    p.class("mb-6 text-[13px] text-ink-500")
                        .text(relation.description())
                })
                .build(),
        )
        .intro(target_link(target))
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
    use crate::package_source::urls::{append_path, encode_segment};

    pkg.wit_namespace.as_ref()?;
    pkg.wit_name.as_ref()?;
    let href = crate::components::page_shell::url_base_for(pkg, version);
    Some(match world {
        Some(world) => append_path(&href, &format!("/world/{}", encode_segment(world))),
        None => href,
    })
}

#[cfg(test)]
mod tests;
