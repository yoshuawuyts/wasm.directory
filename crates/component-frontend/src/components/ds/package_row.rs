//! Package row component.
//!
//! List-style row for search results, namespaces, and all-packages pages.
//! Fields flow inline and wrap at every viewport width; versions remain
//! fully visible even when a tag is wider than the row.

use html::inline_text::Span;
use html::text_content::Division;
use wasm_meta_registry_client::{KnownPackage, PackageKind};

use super::badges;

const ROW_CLASS: &str = "flex flex-wrap items-center gap-x-3 gap-y-1 py-3 -mx-2 px-2";

/// Render a package as a list row (name · version · description).
pub(crate) fn render(pkg: &KnownPackage) -> Division {
    let (display_name, href) = identity(pkg);
    let description = pkg.description.as_deref().unwrap_or("");
    let version = pkg.tags.first().map_or("\u{2014}", String::as_str);
    let name_color = "text-ink-900";

    let [name_span, version_span, description_span] =
        spans(&display_name, version, description, name_color);
    let kind_span = kind_badge(pkg.kind);

    let mut row = Division::builder();
    match href {
        Some(href) => {
            let class = format!(
                "{ROW_CLASS} hover:bg-surfaceMuted transition-colors motion-reduce:transition-none"
            );
            row.anchor(|a| {
                a.href(href)
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

/// Extract display name and optional href from a package.
fn identity(pkg: &KnownPackage) -> (String, Option<String>) {
    match (&pkg.wit_namespace, &pkg.wit_name) {
        (Some(ns), Some(name)) => (format!("{ns}:{name}"), Some(format!("/{ns}/{name}"))),
        _ => (pkg.repository.clone(), None),
    }
}

/// Build the text spans for a package row.
fn spans(
    display_name: &str,
    version: &str,
    description: &str,
    name_color_class: &str,
) -> [Span; 3] {
    [
        Span::builder()
            .class(format!(
                "min-w-0 max-w-full font-medium {name_color_class} truncate"
            ))
            .text(display_name.to_owned())
            .build(),
        Span::builder()
            .class("min-w-0 max-w-full text-[12px] sm:text-[13px] text-ink-400 [overflow-wrap:anywhere]")
            .text(version.to_owned())
            .build(),
        Span::builder()
            .class("min-w-0 max-w-full text-[13px] text-ink-500 truncate")
            .text(crate::markdown::render_inline(description))
            .build(),
    ]
}

/// Build a color-coded kind badge for a package.
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

#[cfg(test)]
pub(crate) mod tests;
