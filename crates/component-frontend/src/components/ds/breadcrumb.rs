//! 18 — Breadcrumb & Pagination.

use html::content::Navigation;
use html::text_content::Division;

const SVG_CHEV_RIGHT: &str = concat!(
    r#"<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="text-ink-300">"#,
    include_str!("../../../../../vendor/lucide/chevron-right.svg"),
    "</svg>"
);
const SVG_CHEV_LEFT: &str = concat!(
    r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
    include_str!("../../../../../vendor/lucide/chevron-left.svg"),
    "</svg>"
);
const SVG_CHEV_RIGHT_LG: &str = concat!(
    r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
    include_str!("../../../../../vendor/lucide/chevron-right.svg"),
    "</svg>"
);

const PAGE_BTN: &str = "h-8 w-8 grid place-items-center rounded-md border border-line bg-surface hover:bg-surfaceMuted";
const PAGE_BTN_NAV: &str = "h-8 w-8 grid place-items-center rounded-md border border-line bg-surface text-ink-500 hover:bg-surfaceMuted";

/// A breadcrumb segment.
pub(crate) struct Crumb {
    /// Display text.
    pub label: String,
    /// Link target, or `None` for the current (last) segment.
    pub href: Option<String>,
}

#[allow(dead_code)]
/// Render a breadcrumb `<nav>` from a list of segments.
///
/// Each segment is a link except the last, which renders as a bold span.
pub(crate) fn render_breadcrumb(crumbs: &[Crumb]) -> String {
    let mut nav = Navigation::builder();
    nav.aria_label("Breadcrumb");
    nav.class("flex items-center gap-1.5 text-[13px] text-ink-500 min-w-0 overflow-x-auto whitespace-nowrap");
    for (i, crumb) in crumbs.iter().enumerate() {
        if i > 0 {
            nav.text(SVG_CHEV_RIGHT);
        }
        if let Some(href) = &crumb.href {
            let href = crate::escape::escape_html_attr(href);
            let label = crumb.label.clone();
            nav.anchor(|a| {
                a.href(href)
                    .class("no-underline hover:text-ink-900 truncate")
                    .text(label)
            });
        } else {
            let label = crumb.label.clone();
            nav.span(|s| s.class("text-ink-900 font-medium truncate").text(label));
        }
    }
    nav.build().to_string()
}

/// Class string for an enabled pagination button.
pub(crate) const PAGINATION_BUTTON_CLASS: &str =
    "px-3 py-1.5 border border-line text-[13px] hover:bg-surfaceMuted transition-colors";

/// Class string for a disabled pagination button.
pub(crate) const PAGINATION_DISABLED_CLASS: &str =
    "px-3 py-1.5 border border-line-light text-[13px] text-ink-400";

/// Render this section.
pub(crate) fn render(section_id: &str, num: &str, title: &str, desc: &str) -> String {
    let breadcrumb = Navigation::builder()
        .class("flex items-center gap-1.5 text-[13px] text-ink-500")
        .anchor(|a| {
            a.href("#".to_owned())
                .class("hover:text-ink-900")
                .text("Tellus")
        })
        .text(SVG_CHEV_RIGHT)
        .anchor(|a| {
            a.href("#".to_owned())
                .class("hover:text-ink-900")
                .text("Pellentesque")
        })
        .text(SVG_CHEV_RIGHT)
        .span(|s| s.class("text-ink-900 font-medium").text("Vestibulum ante"))
        .build();

    let content = Division::builder()
        .class("space-y-8")
        .division(|d| {
            d.heading_3(|h| h.class("text-[13px] mono uppercase tracking-wider text-ink-500 mb-3").text("Breadcrumb"))
                .push(breadcrumb)
        })
        .division(|d| {
            d.heading_3(|h| h.class("text-[13px] mono uppercase tracking-wider text-ink-500 mb-3").text("Pagination"))
                .division(|row| {
                    row.class("inline-flex items-center gap-1 text-[13px]")
                        .button(|b| b.class(PAGE_BTN_NAV).text(SVG_CHEV_LEFT))
                        .button(|b| b.class(PAGE_BTN).text("1"))
                        .button(|b| b.class("h-8 w-8 grid place-items-center rounded-md bg-ink-900 text-canvas font-medium").text("2"))
                        .button(|b| b.class(PAGE_BTN).text("3"))
                        .span(|s| s.class("px-1 text-ink-400").text("\u{2026}"))
                        .button(|b| b.class(PAGE_BTN).text("12"))
                        .button(|b| b.class(PAGE_BTN_NAV).text(SVG_CHEV_RIGHT_LG))
                })
        })
        .build()
        .to_string();

    super::section(section_id, num, title, desc, &content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot() {
        insta::assert_snapshot!(crate::components::ds::pretty_html(&render(
            "breadcrumb",
            "19",
            "Breadcrumb &<br />Pagination",
            "Navigation context. Breadcrumb uses chevron separators and dims all but the current item. Pagination is square-buttoned for compact toolbars.",
        )));
    }
}
