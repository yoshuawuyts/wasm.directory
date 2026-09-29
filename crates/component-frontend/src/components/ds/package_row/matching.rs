//! Relationship rows use their matching release, not latest-release metadata.

use html::inline_text::Span;
use html::text_content::Division;
use wasm_meta_registry_client::{KnownPackage, MatchingWorld, PackageKind};

use super::identity;
use crate::components::ds::badges;
use crate::escape::{escape_html_attr, escape_html_text};

const ROW_CLASS: &str = "flex flex-wrap items-center gap-x-3 gap-y-1 py-3 -mx-2 px-2";

/// Render a dependent package at the release that actually matched.
pub(crate) fn render_matching_package(
    pkg: &KnownPackage,
    version: &str,
    href: Option<String>,
) -> Division {
    render_row(
        &identity(pkg).0,
        href,
        version,
        crate::markdown::render_summary(pkg.description.as_deref().unwrap_or("")),
        kind_badge(pkg.kind),
    )
}

/// Render an individual world using the same flowing fields as packages.
pub(crate) fn render_matching_world(world: &MatchingWorld, href: Option<String>) -> Division {
    let kind = badges::status_badge("bg-cat-cream text-cat-creamInk", "bg-cat-creamInk", "World");
    render_row(
        &format!("{}/{}", identity(&world.package).0, world.name),
        href,
        &world.version,
        crate::markdown::render_summary(
            world
                .description
                .as_deref()
                .or(world.package.description.as_deref())
                .unwrap_or(""),
        ),
        Span::builder().class("shrink-0").push(kind).build(),
    )
}

fn render_row(
    display_name: &str,
    href: Option<String>,
    version: &str,
    description_html: String,
    kind_span: Span,
) -> Division {
    let [name_span, version_span, description_span] =
        spans(display_name, version, description_html);

    let mut row = Division::builder();
    match href {
        Some(href) => {
            let class = format!(
                "{ROW_CLASS} hover:bg-surfaceMuted transition-colors motion-reduce:transition-none"
            );
            row.anchor(|a| {
                a.href(escape_html_attr(&href))
                    .class(class)
                    .push(name_span)
                    .push(kind_span)
                    .push(version_span)
                    .push(description_span)
            });
        }
        None => {
            row.class(ROW_CLASS)
                .push(name_span)
                .push(kind_span)
                .push(version_span)
                .push(description_span);
        }
    }
    row.build()
}

fn spans(display_name: &str, version: &str, description_html: String) -> [Span; 3] {
    [
        Span::builder()
            .class("min-w-0 max-w-full font-medium text-ink-900 truncate")
            .text(escape_html_text(display_name))
            .build(),
        Span::builder()
            .class("min-w-0 max-w-full text-[12px] sm:text-[13px] text-ink-400 [overflow-wrap:anywhere]")
            .text(escape_html_text(version))
            .build(),
        Span::builder()
            .class("min-w-0 max-w-full text-[13px] text-ink-500 truncate")
            .text(description_html)
            .build(),
    ]
}

fn kind_badge(kind: Option<PackageKind>) -> Span {
    let (badge_class, dot_class, label) = match kind {
        Some(PackageKind::Component) => (
            "bg-cat-green text-cat-greenInk",
            "bg-cat-greenInk",
            "Component",
        ),
        Some(PackageKind::Interface) => (
            "bg-cat-blue text-cat-blueInk",
            "bg-cat-blueInk",
            "Interface",
        ),
        None => (
            "bg-cat-slate text-cat-slateInk",
            "bg-cat-slateInk",
            "Package",
        ),
    };
    let badge = badges::status_badge(badge_class, dot_class, label);
    Span::builder().class("shrink-0").push(badge).build()
}
