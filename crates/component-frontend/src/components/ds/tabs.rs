//! 06 — Tabs & Pills.

use html::text_content::Division;

#[allow(dead_code)]
/// Render the tabs & pills section.
/// Render a segmented tab control (binary toggle).
pub(crate) fn segmented_tabs(items: &[(&str, bool)]) -> Division {
    let mut div = Division::builder();
    div.class("inline-flex rounded-md border border-line overflow-hidden text-[13px]");
    for (label, active) in items {
        let class = if *active {
            "px-4 h-8 bg-surface text-ink-900 font-medium border-r border-line last:border-r-0"
        } else {
            "px-4 h-8 bg-canvas text-ink-500 hover:text-ink-900 border-r border-line last:border-r-0"
        };
        let label = (*label).to_owned();
        let class = class.to_owned();
        div.button(|b| b.type_("button").class(class).text(label));
    }
    div.build()
}

#[allow(dead_code)]
/// Render underline-style tabs.
pub(crate) fn underline_tabs(items: &[(&str, bool)]) -> Division {
    let mut div = Division::builder();
    div.class("flex gap-4 border-b-[1.5px] border-rule text-[13px]");
    for (label, active) in items {
        let class = if *active {
            "pb-2 border-b-[1.5px] border-ink-900 text-ink-900 font-medium -mb-px"
        } else {
            "pb-2 text-ink-500 hover:text-ink-900"
        };
        let label = (*label).to_owned();
        let class = class.to_owned();
        div.button(|b| b.type_("button").class(class).text(label));
    }
    div.build()
}

/// Render a panel with a tab strip fused to its top edge.
///
/// The active tab visually merges with the panel below it (top-left corner
/// zeroed, shared surface). `body_html` is rendered inside the padded panel
/// so callers can drop in whatever content they want (a copy command box,
/// prose, additional controls, etc.).
#[allow(dead_code)]
pub(crate) fn panel_tabs(items: &[(&str, bool)], body_html: &str) -> Division {
    panel_tabs_with_label(None, items, body_html)
}

/// Render multiple switchable tabs with their own panel bodies. Includes
/// inline JS to switch the active panel on tab click. The first item is
/// active by default.
#[allow(dead_code)]
pub(crate) fn panel_tabs_switchable(items: &[(&str, &str)]) -> String {
    use std::fmt::Write as _;
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let group = format!("ptabs-{id}");

    let mut tabs_html = String::from(r#"<div class="id-lang-tabs">"#);
    let mut panels_html = String::new();
    for (i, (label, body)) in items.iter().enumerate() {
        let active = i == 0;
        let tab_class = if active {
            "id-lang-tab is-clickable is-active"
        } else {
            "id-lang-tab is-clickable"
        };
        let style = if active {
            " style=\"background: var(--c-canvas);\""
        } else {
            ""
        };
        let _ = write!(
            tabs_html,
            r#"<span class="{tab_class}"{style} tabindex="0" data-ptabs-group="{group}" data-ptabs-target="{i}"><span class="dot"></span>{label}</span>"#,
        );
        let hidden = if active { "" } else { " hidden" };
        let _ = write!(
            panels_html,
            r#"<div data-ptabs-group="{group}" data-ptabs-panel="{i}"{hidden}>{body}</div>"#,
        );
    }
    tabs_html.push_str("</div>");

    let script = format!(
        r#"<script>(function(){{var g='{group}';var tabs=document.querySelectorAll('[data-ptabs-group="'+g+'"][data-ptabs-target]');var panels=document.querySelectorAll('[data-ptabs-group="'+g+'"][data-ptabs-panel]');function activate(t){{var target=t.getAttribute('data-ptabs-target');tabs.forEach(function(o){{var on=o===t;o.classList.toggle('is-active',on);o.style.background=on?'var(--c-canvas)':'';}});panels.forEach(function(p){{p.hidden=p.getAttribute('data-ptabs-panel')!==target;}});}}tabs.forEach(function(t){{t.addEventListener('click',function(){{activate(t);}});t.addEventListener('keydown',function(e){{if(e.key==='Enter'||e.key===' '){{e.preventDefault();activate(t);}}}});}});}})();</script>"#,
    );

    format!(
        r#"<div class="flex flex-col">{tabs_html}<div class="border border-lineSoft rounded-md rounded-tl-none bg-canvas p-4">{panels_html}</div></div>{script}"#,
    )
}

/// Like [`panel_tabs`] but with a leading uppercase label (e.g. "Install")
/// in the tab row, before the tabs themselves.
#[allow(dead_code)]
pub(crate) fn panel_tabs_with_label(
    label: Option<&str>,
    items: &[(&str, bool)],
    body_html: &str,
) -> Division {
    let labels: Vec<(String, bool)> = items.iter().map(|(l, a)| ((*l).to_owned(), *a)).collect();
    let row_label = label.map(str::to_owned);
    let body_html = body_html.to_owned();

    Division::builder()
        .class("flex flex-col")
        .division(|tabs| {
            let mut tabs = tabs.class("id-lang-tabs");
            if let Some(label) = row_label {
                tabs = tabs.span(|s| {
                    s.class(
                        "text-[11px] mono uppercase tracking-wider text-ink-500 \
                         inline-flex items-center h-[30px] pr-3 mr-1 \
                         border-b border-lineSoft",
                    )
                    .text(label)
                });
            }
            for (label, active) in labels {
                let class = if active {
                    "id-lang-tab is-active"
                } else {
                    "id-lang-tab"
                };
                tabs = tabs.span(|s| {
                    let mut s = s.class(class);
                    if active {
                        s = s.style("background: var(--c-canvas);");
                    }
                    s.span(|dot| dot.class("dot")).text(label)
                });
            }
            tabs
        })
        .division(|panel| {
            panel
                .class(
                    "border border-lineSoft rounded-md rounded-tl-none \
                     bg-canvas p-4",
                )
                .text(body_html)
        })
        .build()
}

pub(crate) fn render(section_id: &str, num: &str, title: &str, desc: &str) -> String {
    // Placeholder body for the panel-tabs demo.
    let panel_body = Division::builder()
        .class("flex")
        .span(|s| {
            s.class("inline-flex items-center px-2.5 h-7 rounded-l-md border border-r-0 border-line bg-surfaceMuted text-[12.5px] text-ink-500 mono select-none")
                .aria_hidden(true)
                .text("$")
        })
        .code(|c| {
            c.class("inline-flex items-center px-2.5 h-7 rounded-r-md border border-line bg-surface mono text-[12.5px] text-ink-900 whitespace-nowrap")
                .text("component install wasi:http@0.2.4")
        })
        .build()
        .to_string();

    let content = Division::builder()
        .class("space-y-8")
        .division(|seg_group| {
            seg_group
                .heading_3(|h| {
                    h.class("text-[13px] mono uppercase tracking-wider text-ink-500 mb-3")
                        .text("Segmented")
                })
                .division(|seg| {
                    seg.class("flex p-1 rounded-lg bg-surfaceMuted w-[200px]")
                        .button(|b| {
                            b.class("flex-1 h-7 rounded-md bg-ink-900 text-canvas text-[13px] font-medium")
                                .text("Lorem")
                        })
                        .button(|b| {
                            b.class("flex-1 h-7 rounded-md text-[13px] text-ink-500")
                                .text("Ipsum")
                        })
                })
        })
        .division(|tab_group| {
            tab_group
                .heading_3(|h| {
                    h.class("text-[13px] mono uppercase tracking-wider text-ink-500 mb-3")
                        .text("Underline tabs")
                })
                .division(|tabs| {
                    tabs.class("flex items-center gap-6 border-b-[1.5px] border-rule")
                        .button(|b| {
                            b.class("relative pb-3 text-[15px] font-medium")
                                .text("Aenean")
                                .span(|s| {
                                    s.class("absolute left-0 right-0 -bottom-[1.5px] h-[1.5px] bg-ink-900")
                                })
                        })
                        .button(|b| b.class("pb-3 text-[15px] text-ink-500").text("Mauris"))
                        .button(|b| b.class("pb-3 text-[15px] text-ink-500").text("Vivamus"))
                })
        })
        .division(|panel_group| {
            panel_group
                .heading_3(|h| {
                    h.class("text-[13px] mono uppercase tracking-wider text-ink-500 mb-3")
                        .text("Panel tabs")
                })
                .push(panel_tabs(&[("CLI", true)], &panel_body))
                .paragraph(|p| {
                    p.class("mt-3 text-[12px] text-ink-500")
                        .text("Tabbed surface for grouping a primary action with related content. Active tab fuses with the padded panel below; the panel can host a copy box, prose, or additional controls. Used for the install section on package pages.")
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
            "tabs",
            "06",
            "Tabs & Pills",
            "Segmented controls for binary scoping; underline tabs for sub-views; pills for filterable chips.",
        )));
    }
}
