//! 05 — Buttons.

use html::text_content::Division;

const SVG_CALENDAR: &str = concat!(
    r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">"#,
    include_str!("../../../../../vendor/lucide/calendar.svg"),
    "</svg>"
);
const SVG_CHEV: &str = concat!(
    r#"<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
    include_str!("../../../../../vendor/lucide/chevron-down.svg"),
    "</svg>"
);
const SVG_FILTER: &str = concat!(
    r#"<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">"#,
    include_str!("../../../../../vendor/lucide/sliders-horizontal.svg"),
    "</svg>"
);
const SVG_SAVE: &str = concat!(
    r#"<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">"#,
    include_str!("../../../../../vendor/lucide/save.svg"),
    "</svg>"
);
const SVG_UPLOAD: &str = concat!(
    r#"<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">"#,
    include_str!("../../../../../vendor/lucide/upload.svg"),
    "</svg>"
);

/// Shared compact icon-button styling.
pub(super) const ICON_BUTTON_CLASS: &str =
    "h-8 w-8 grid place-items-center rounded-md hover:bg-surfaceMuted text-ink-700";

#[allow(dead_code)]
/// Render a filled button.
pub(crate) fn filled_button(label: &str, compact: bool) -> Division {
    let h = if compact { "h-8" } else { "h-9" };
    let label = label.to_owned();
    Division::builder()
        .class("inline-block")
        .button(|b| {
            b.type_("button")
                .class(format!("{h} px-3 inline-flex items-center gap-2 rounded-lg bg-surfaceMuted text-ink-900 text-[13px] hover:bg-ink-300"))
                .text(label)
        })
        .build()
}

#[allow(dead_code)]
/// Render an outline button.
pub(crate) fn outline_button(label: &str, compact: bool) -> Division {
    let h = if compact { "h-8" } else { "h-9" };
    let label = label.to_owned();
    Division::builder()
        .class("inline-block")
        .button(|b| {
            b.type_("button")
                .class(format!("{h} px-3 inline-flex items-center gap-2 rounded-lg border-[1.5px] border-ink-900 bg-surface text-ink-900 text-[13px] hover:bg-surfaceMuted"))
                .text(label)
        })
        .build()
}

/// Render this section.
pub(crate) fn render(section_id: &str, num: &str, title: &str, desc: &str) -> String {
    let content = Division::builder()
        .class("space-y-8")
        // Filled
        .division(|d| {
            d.heading_3(|h| h.class("text-[13px] mono uppercase tracking-wider text-ink-500 mb-3").text("Filled"))
                .division(|g| {
                    g.class("flex flex-wrap items-center gap-3")
                        .button(|b| {
                            b.class("h-8 px-3 inline-flex items-center gap-2 rounded-lg bg-surfaceMuted text-ink-900 text-[13px] hover:bg-ink-300")
                                .text(SVG_CALENDAR)
                                .text(" Lorem \u{2013} Ipsum ")
                                .text(SVG_CHEV)
                        })
                        .button(|b| {
                            b.class("h-9 px-3 inline-flex items-center gap-2 rounded-lg bg-surfaceMuted text-ink-900 text-[13px] hover:bg-ink-300")
                                .text("Sodales")
                        })
                })
        })
        // Outline
        .division(|d| {
            d.heading_3(|h| h.class("text-[13px] mono uppercase tracking-wider text-ink-500 mb-3").text("Outline"))
                .division(|g| {
                    g.class("flex flex-wrap items-center gap-3")
                        .button(|b| {
                            b.class("h-8 px-3 inline-flex items-center gap-2 rounded-lg border-[1.5px] border-ink-900 bg-surface text-ink-900 text-[13px] hover:bg-surfaceMuted")
                                .text("Omnis Vehicula ")
                                .text(SVG_CHEV)
                        })
                        .button(|b| {
                            b.class("h-9 px-3 rounded-lg border-[1.5px] border-ink-900 bg-surface text-ink-900 text-[13px] hover:bg-surfaceMuted")
                                .text("Dismiss")
                        })
                })
        })
        // Icon
        .division(|d| {
            d.heading_3(|h| h.class("text-[13px] mono uppercase tracking-wider text-ink-500 mb-3").text("Icon"))
                .division(|g| {
                    g.class("flex items-center gap-1")
                        .button(|b| b.class(ICON_BUTTON_CLASS).text(SVG_FILTER))
                        .button(|b| b.class(ICON_BUTTON_CLASS).text(SVG_SAVE))
                        .button(|b| b.class(ICON_BUTTON_CLASS).text(SVG_UPLOAD))
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
            "buttons",
            "05",
            "Buttons",
            "Two variants: a soft gray fill or a 1.5px ink outline. The system reserves solid ink for typography only \u{2014} buttons are never pure black. Two heights: 32px (compact toolbars) and 36px (mobile / primary CTAs).",
        )));
    }
}
