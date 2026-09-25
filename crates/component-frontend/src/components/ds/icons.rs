//! 12 — Icons.

use html::text_content::Division;

/// The existing 14px destination arrow from the inline icon set.
pub(crate) const ARROW_UP_RIGHT: &str = r#"<svg class="h-3.5 w-3.5 text-ink-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><path d="M7 7h10v10" /><path d="M7 17 17 7" /></svg>"#;

/// Inline icon entries: (svg, label).
pub(crate) const INLINE_ICONS: &[(&str, &str)] = &[
    (
        r#"<svg class="h-3.5 w-3.5 text-ink-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8" /><path d="m21 21-4.3-4.3" /></svg>"#,
        "search",
    ),
    (
        r#"<svg class="h-3.5 w-3.5 text-ink-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><path d="m6 9 6 6 6-6" /></svg>"#,
        "chevron-down",
    ),
    (
        r#"<svg class="h-3.5 w-3.5 text-ink-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><path d="m9 18 6-6-6-6" /></svg>"#,
        "chevron-right",
    ),
    (ARROW_UP_RIGHT, "arrow-up-right"),
    (
        r#"<svg class="h-3.5 w-3.5 text-ink-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="11" height="11" rx="2" /><path d="M5 15V6a2 2 0 0 1 2-2h9" /></svg>"#,
        "copy",
    ),
    (
        r#"<svg class="h-3.5 w-3.5 text-ink-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><path d="M15 3h6v6" /><path d="M10 14 21 3" /><path d="M21 14v5a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5" /></svg>"#,
        "external-link",
    ),
    (
        r#"<svg class="h-3.5 w-3.5 text-ink-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><path d="M20 6 9 17l-5-5" /></svg>"#,
        "check",
    ),
    (
        r#"<svg class="h-3.5 w-3.5 text-ink-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10" /><path d="M12 8v4" /><path d="M12 16h.01" /></svg>"#,
        "info",
    ),
    (
        r#"<svg class="h-3.5 w-3.5 text-ink-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><path d="M10.3 3.86 1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0Z" /><path d="M12 9v4" /><path d="M12 17h.01" /></svg>"#,
        "triangle-alert",
    ),
    (
        r#"<svg class="h-3.5 w-3.5 text-ink-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" /><path d="M14 2v6h6" /></svg>"#,
        "file",
    ),
    (
        r#"<svg class="h-3.5 w-3.5 text-ink-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Z" /></svg>"#,
        "folder",
    ),
    (
        r#"<svg class="h-3.5 w-3.5 text-ink-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><polyline points="4 17 10 11 4 5" /><line x1="12" x2="20" y1="19" y2="19" /></svg>"#,
        "terminal",
    ),
    (
        r#"<svg class="h-3.5 w-3.5 text-ink-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="4" /><path d="M12 2v2" /><path d="M12 20v2" /><path d="m4.93 4.93 1.41 1.41" /><path d="m17.66 17.66 1.41 1.41" /><path d="M2 12h2" /><path d="M20 12h2" /><path d="m6.34 17.66-1.41 1.41" /><path d="m19.07 4.93-1.41 1.41" /></svg>"#,
        "sun",
    ),
    (
        r#"<svg class="h-3.5 w-3.5 text-ink-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" /></svg>"#,
        "moon",
    ),
];

/// Grid icon entries: (svg, title).
pub(crate) const GRID_ICONS: &[(&str, &str)] = &[
    (
        r#"<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><path d="M3 6h18" /><path d="M7 12h10" /><path d="M10 18h4" /></svg>"#,
        "list-filter",
    ),
    (
        r#"<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><path d="M15.2 3a2 2 0 0 1 1.4.6l3.8 3.8a2 2 0 0 1 .6 1.4V19a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z" /><path d="M17 21v-7a1 1 0 0 0-1-1H8a1 1 0 0 0-1 1v7" /><path d="M7 3v4a1 1 0 0 0 1 1h7" /></svg>"#,
        "save",
    ),
    (
        r#"<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><rect width="18" height="18" x="3" y="4" rx="2" /><path d="M16 2v4" /><path d="M8 2v4" /><path d="M3 10h18" /></svg>"#,
        "calendar",
    ),
    (
        r#"<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><path d="M16 7h6v6" /><path d="m22 7-8.5 8.5-5-5L2 17" /></svg>"#,
        "trending-up",
    ),
    (
        r#"<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><path d="M3 3v16a2 2 0 0 0 2 2h16" /><path d="M18 17V9" /><path d="M13 17V5" /><path d="M8 17v-3" /></svg>"#,
        "chart-column",
    ),
    (
        r#"<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><line x1="18" x2="18" y1="20" y2="10" /><line x1="12" x2="12" y1="20" y2="4" /><line x1="6" x2="6" y1="20" y2="14" /></svg>"#,
        "chart-no-axes-column",
    ),
    (
        r#"<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><path d="M21.174 6.812a1 1 0 0 0-3.986-3.987L3.842 16.174a2 2 0 0 0-.5.83l-1.321 4.352a.5.5 0 0 0 .623.622l4.353-1.32a2 2 0 0 0 .83-.497z" /><path d="m15 5 4 4" /></svg>"#,
        "pencil",
    ),
    (
        r#"<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10" /></svg>"#,
        "circle",
    ),
    (
        r#"<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8" /><path d="M21 3v5h-5" /><path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16" /><path d="M8 16H3v5" /></svg>"#,
        "refresh-cw",
    ),
    (
        r#"<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><line x1="19" x2="5" y1="5" y2="19" /><circle cx="6.5" cy="6.5" r="2.5" /><circle cx="17.5" cy="17.5" r="2.5" /></svg>"#,
        "percent",
    ),
    (
        r#"<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><path d="M12 3v12" /><path d="m17 8-5-5-5 5" /><path d="M21 21H3" /></svg>"#,
        "upload",
    ),
    (
        r#"<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m6 9 6 6 6-6" /></svg>"#,
        "chevron-down",
    ),
    (
        r#"<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10" /><path d="M12 16v-4" /><path d="M12 8h.01" /></svg>"#,
        "info",
    ),
];

const INLINE_DESC: &str = r#"These are the icons you'll see most across the site — in the top bar, in tree links, beside copyable code, and in callouts. They sit at <code class="mono text-[12px]">h-3.5 w-3.5</code> (14px), coloured <code class="mono text-[12px]">text-ink-500</code>, paired with body text or a mono label. Larger swatches below are reference at 20px so the stroke geometry is visible."#;

#[allow(dead_code)]
/// Render a Lucide icon SVG at the given size.
///
/// `inner` is the SVG fragment from a vendored file (via `include_str!`).
/// Returns a full `<svg>` string. Uses `format!` because the `html` crate
/// has no SVG element support.
pub(crate) fn icon(size: u8, inner: &str) -> String {
    let inner = inner.trim();
    format!(
        r#"<svg width="{size}" height="{size}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">{inner}</svg>"#,
    )
}

/// Render this section.
pub(crate) fn render(
    section_id: &str,
    num: &str,
    title: &str,
    desc: &str,
    inline_icons: &[(&str, &str)],
    grid_icons: &[(&str, &str)],
) -> String {
    // Inline icons
    let mut inline_grid = Division::builder();
    inline_grid.class("flex flex-wrap items-center gap-x-6 gap-y-3 rounded-lg border border-line bg-canvas px-4 py-3.5 text-[13px] text-ink-700");
    for (svg, label) in inline_icons {
        let svg = (*svg).to_owned();
        let label = (*label).to_owned();
        let icon = html::inline_text::Span::builder()
            .class("inline-flex items-center gap-1.5")
            .text(svg)
            .span(|s| s.class("mono text-[12.5px]").text(label))
            .build();
        inline_grid.push(icon);
    }

    // Grid icons
    let mut ref_grid = Division::builder();
    ref_grid.class("grid grid-cols-3 md:grid-cols-6 gap-3");
    for (svg, title) in grid_icons {
        let svg = (*svg).to_owned();
        let title = (*title).to_owned();
        let cell = Division::builder()
            .class("aspect-square grid place-items-center border border-lineSoft rounded-md text-ink-700")
            .title(title)
            .text(svg)
            .build();
        ref_grid.push(cell);
    }

    let content = Division::builder()
        .class("space-y-10")
        .division(|d| {
            d.division(|l| {
                l.class("text-[12px] text-ink-500 mb-3")
                    .text("In context \u{00b7} 14px / ink-500 paired with text")
            })
            .push(inline_grid.build())
            .paragraph(|p| p.class("mt-3 text-[12px] text-ink-500").text(INLINE_DESC))
        })
        .division(|d| {
            d.division(|l| {
                l.class("text-[12px] text-ink-500 mb-3")
                    .text("Reference grid \u{00b7} 20px swatches")
            })
            .push(ref_grid.build())
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
            "icons",
            "12",
            "Icons",
            r#"<a href="https://lucide.dev" class="text-ink-700 underline decoration-line decoration-1 underline-offset-[3px] hover:text-ink-900">Lucide</a> outline icons, drawn at <code class="mono text-[12px]">stroke-width="1.75"</code> with <code class="mono text-[12px]">stroke-linecap="round"</code> and <code class="mono text-[12px]">stroke-linejoin="round"</code>. Sizes: <strong>14px</strong> inside dense controls (tree links, kbd hints, tabs), <strong>16px</strong> in toolbars and buttons, <strong>18px</strong> on mobile and in empty states. Always <code class="mono text-[12px]">currentColor</code> so they pick up the surrounding ink scale; never coloured directly."#,
            INLINE_ICONS,
            GRID_ICONS,
        )));
    }
}
