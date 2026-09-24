//! Page header.

use html::content::Header;

use super::navbar;

/// Render the page header.
pub(crate) fn render(version: &str, subtitle: &str, title: &str, description: &str) -> String {
    let version = version.to_owned();
    let subtitle = subtitle.to_owned();
    let title = title.to_owned();
    let description = description.to_owned();
    Header::builder()
        .class("pt-8 md:pt-12 pb-8 md:pb-12")
        .division(|div| {
            div.class(
                "flex items-center gap-2 text-[12px] text-ink-500 mono uppercase tracking-wider",
            )
            .span(|s| s.text(version.clone()))
            .span(|s| s.class("h-1 w-1 rounded-full bg-ink-300"))
            .span(|s| s.text(subtitle.clone()))
            .span(|s| s.class("ml-auto"))
            .text(navbar::theme_toggle())
        })
        .heading_1(|h1| {
            h1.class("mt-3 text-[36px] md:text-[44px] leading-[1.05] font-semibold tracking-tight")
                .text(title.clone())
        })
        .paragraph(|p| {
            p.class("mt-3 max-w-2xl text-[15px] text-ink-700 leading-relaxed")
                .text(description.clone())
        })
        .build()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot() {
        insta::assert_snapshot!(crate::components::ds::pretty_html(&render(
            "v1.0",
            "Foundations \u{00b7} Components \u{00b7} Patterns",
            "Design System",
            "A quiet, data-forward visual language built around soft rules, neutral ink, and a categorical pastel palette. Optimized for dense dashboards and analytical interfaces.",
        )));
    }
}
