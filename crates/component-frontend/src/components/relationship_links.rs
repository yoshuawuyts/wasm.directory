//! A dedicated sidebar section for fixed relationship queries at every viewport.

use html::content::Navigation;
use html::inline_text::Anchor;
use html::text_content::Division;
use wasm_meta_registry_client::RelationshipTarget;

use super::ds::icons;
use super::page_sidebar::{SidebarActive, SidebarContext};
use crate::escape::escape_html_attr;
use crate::relationships::Relationship;

fn package_target(ctx: &SidebarContext<'_>) -> Option<RelationshipTarget> {
    match RelationshipTarget::new(ctx.display_name, None) {
        Ok(target) => Some(target),
        Err(error) => {
            eprintln!(
                "component-frontend: relationship navigation unavailable for {}: {error}",
                ctx.display_name
            );
            None
        }
    }
}

/// Group search-page links without a visible heading, separate from the item tree.
pub(crate) fn render(ctx: &SidebarContext<'_>) -> String {
    let Some(package) = package_target(ctx) else {
        return String::new();
    };
    let mut links = Division::builder();
    links
        .class("space-y-0.5 text-[13px]")
        .push(link(Relationship::Dependents, &package));
    if let Some(target) = interface_target(ctx, package) {
        links.push(link(Relationship::ImportedBy, &target));
        links.push(link(Relationship::ExportedBy, &target));
    }
    Navigation::builder()
        .aria_label("Relationships")
        .class("pt-4 border-t-[1.5px] border-rule")
        .push(links.build())
        .build()
        .to_string()
}

fn link(relation: Relationship, target: &RelationshipTarget) -> Anchor {
    // The builder emits a valueless aria-hidden attribute, not the required token.
    let arrow = format!(
        r#"<span aria-hidden="true" class="shrink-0 inline-flex items-center h-[18px]">{}</span>"#,
        icons::ARROW_UP_RIGHT
    );
    Anchor::builder()
        .href(escape_html_attr(&relation.href(target)))
        .class("tree-link")
        .span(|content| {
            content
                .class("inline-flex items-center gap-1")
                .text(relation.label())
                .text(arrow)
        })
        .build()
}

fn interface_target(
    ctx: &SidebarContext<'_>,
    package: RelationshipTarget,
) -> Option<RelationshipTarget> {
    let interface = match ctx.active {
        SidebarActive::Interface(name) | SidebarActive::Item(name, _) => ctx
            .doc
            .and_then(|doc| doc.interfaces.iter().find(|iface| iface.name == name))
            .map(|iface| iface.name.as_str()),
        _ => None,
    };
    if let Some(interface) = interface {
        return match RelationshipTarget::new(ctx.display_name, Some(interface)) {
            Ok(target) => Some(target),
            Err(error) => {
                eprintln!("component-frontend: interface relationship link unavailable: {error}");
                None
            }
        };
    }
    (ctx.kind_label == "Interface Types").then_some(package)
}

#[cfg(test)]
mod tests;
