//! 13 — Form Fields.

use html::text_content::Division;

use super::input_group::{self, ButtonWithDropdown, DropdownOption};

const SVG_SEARCH_SM: &str = concat!(
    r#"<svg class="absolute left-3 top-1/2 -translate-y-1/2 text-ink-400" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">"#,
    include_str!("../../../../../vendor/lucide/search.svg"),
    "</svg>"
);
const SVG_SEARCH_LG: &str = concat!(
    r#"<svg class="absolute left-3.5 top-1/2 -translate-y-1/2 text-ink-400" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">"#,
    include_str!("../../../../../vendor/lucide/search.svg"),
    "</svg>"
);
const SVG_CHEV_SELECT: &str = concat!(
    r#"<svg class="absolute right-3 top-1/2 -translate-y-1/2 text-ink-500 pointer-events-none" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
    include_str!("../../../../../vendor/lucide/chevron-down.svg"),
    "</svg>"
);
const SVG_CHEV_SPLIT: &str = concat!(
    r#"<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
    include_str!("../../../../../vendor/lucide/chevron-down.svg"),
    "</svg>"
);
const SVG_COPY: &str = concat!(
    r#"<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">"#,
    include_str!("../../../../../vendor/lucide/copy.svg"),
    "</svg>"
);
const SVG_CHECK: &str = concat!(
    r#"<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">"#,
    include_str!("../../../../../vendor/lucide/check.svg"),
    "</svg>"
);
const SVG_COPY_LONG: &str = concat!(
    r#"<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">"#,
    include_str!("../../../../../vendor/lucide/copy.svg"),
    "</svg>"
);
const SVG_FAIL: &str = concat!(
    r#"<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">"#,
    include_str!("../../../../../vendor/lucide/x.svg"),
    "</svg>"
);
const SVG_COPY_DISABLED: &str = concat!(
    r#"<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">"#,
    include_str!("../../../../../vendor/lucide/copy.svg"),
    "</svg>"
);
const SVG_MINUS: &str = concat!(
    r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
    include_str!("../../../../../vendor/lucide/minus.svg"),
    "</svg>"
);
const SVG_PLUS: &str = concat!(
    r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
    include_str!("../../../../../vendor/lucide/plus.svg"),
    "</svg>"
);
const SVG_ATTACH: &str = concat!(
    r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">"#,
    include_str!("../../../../../vendor/lucide/paperclip.svg"),
    "</svg>"
);
const SVG_CALENDAR: &str = concat!(
    r#"<svg class="absolute right-3 top-1/2 -translate-y-1/2 text-ink-500 pointer-events-none" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">"#,
    include_str!("../../../../../vendor/lucide/calendar.svg"),
    "</svg>"
);
const SVG_CLOCK: &str = concat!(
    r#"<svg class="absolute right-3 top-1/2 -translate-y-1/2 text-ink-500 pointer-events-none" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">"#,
    include_str!("../../../../../vendor/lucide/clock.svg"),
    "</svg>"
);

/// Input size variant.
#[derive(Clone, Copy)]
pub(crate) enum Size {
    Md,
    Sm,
}

#[allow(dead_code)]
/// Build an `<input>` element with standard field classes.
pub(crate) fn field_input(
    type_: &str,
    size: Size,
    class_extra: &str,
    placeholder: Option<&str>,
    value: Option<&str>,
    disabled: bool,
    readonly: bool,
) -> String {
    let (h, px, text) = match size {
        Size::Md => ("h-9", "px-3", "text-[14px]"),
        Size::Sm => ("h-7", "px-2.5", "text-[12px]"),
    };
    let base = format!(
        "block w-full {h} {px} rounded-md border border-line bg-surface {text} placeholder:text-ink-400 focus:outline-none focus:border-ink-900 {class_extra}"
    );
    let base = base.trim().to_owned();
    let type_ = type_.to_owned();
    let mut input = html::forms::Input::builder();
    input.type_(type_);
    input.class(base);
    if let Some(p) = placeholder {
        input.placeholder(p.to_owned());
    }
    if let Some(v) = value {
        input.value(v.to_owned());
    }
    if disabled {
        input.disabled("");
    }
    if readonly {
        input.readonly("");
    }
    input.build().to_string()
}

#[allow(dead_code)]
/// Build a labeled field (label + span + input).
pub(crate) fn labeled_field(label_text: &str, label_class: &str, inner: &str) -> String {
    let label_text = label_text.to_owned();
    let label_class = label_class.to_owned();
    let inner = inner.to_owned();
    html::forms::Label::builder()
        .class("block")
        .span(|s| s.class(label_class).text(label_text))
        .text(inner)
        .build()
        .to_string()
}

/// Build the Sizes subsection.
fn sizes() -> String {
    let md_input = field_input(
        "text",
        Size::Md,
        "mt-1",
        Some("Lorem ipsum"),
        None,
        false,
        false,
    );
    let sm_input = field_input(
        "text",
        Size::Sm,
        "mt-1",
        Some("Lorem ipsum"),
        None,
        false,
        false,
    );

    Division::builder()
        .division(|d| d.class("text-[12px] text-ink-500 mb-2").text("Sizes"))
        .division(|d| {
            d.class("space-y-3")
                .division(|d| {
                    d.span(|s| s.class("text-[11px] text-ink-500 mono uppercase tracking-wider").text("md \u{00b7} default"))
                        .division(|d| {
                            d.class("mt-1 text-[11px] text-ink-500")
                                .code(|c| c.class("mono text-[10.5px]").text("h-9 \u{00b7} px-3 \u{00b7} text-[14px]"))
                                .text(" \u{2014} primary forms, settings pages, modal dialogs.")
                        })
                        .text(md_input)
                })
                .division(|d| {
                    d.span(|s| s.class("text-[11px] text-ink-500 mono uppercase tracking-wider").text("sm \u{00b7} compact"))
                        .division(|d| {
                            d.class("mt-1 text-[11px] text-ink-500")
                                .code(|c| c.class("mono text-[10.5px]").text("h-7 \u{00b7} px-2.5 \u{00b7} text-[12px]"))
                                .text(" \u{2014} sidebars, metadata strips, toolbars, inline filters. Mono content uses ")
                                .code(|c| c.class("mono text-[10.5px]").text("text-[12.5px]"))
                                .text(" to match optical weight.")
                        })
                        .text(sm_input)
                })
        })
        .paragraph(|p| {
            p.class("mt-3 text-[11px] text-ink-500")
                .text("Pick by context, not preference. ")
                .strong(|s| s.text("Don't mix sizes within one form."))
                .text(" Addons (prefix, suffix, button) match the field's size \u{2014} never combine an md input with an sm button.")
        })
        .build()
        .to_string()
}

/// A state demo entry.
pub(crate) struct StateEntry {
    pub(crate) label: &'static str,
    pub(crate) label_class: &'static str,
    pub(crate) input_class: &'static str,
    pub(crate) placeholder: Option<&'static str>,
    pub(crate) value: Option<&'static str>,
    pub(crate) disabled: bool,
    pub(crate) readonly: bool,
}

pub(crate) const STATES: &[StateEntry] = &[
    StateEntry {
        label: "Default",
        label_class: "text-[11px] text-ink-500 mono uppercase tracking-wider",
        input_class: "mt-1 block w-full h-9 px-3 rounded-md border border-line bg-surface text-[14px] placeholder:text-ink-400",
        placeholder: Some("Lorem ipsum"),
        value: None,
        disabled: false,
        readonly: false,
    },
    StateEntry {
        label: "Hover",
        label_class: "text-[11px] text-ink-500 mono uppercase tracking-wider",
        input_class: "mt-1 block w-full h-9 px-3 rounded-md border border-ink-400 bg-surface text-[14px] placeholder:text-ink-400",
        placeholder: Some("Lorem ipsum"),
        value: None,
        disabled: false,
        readonly: true,
    },
    StateEntry {
        label: "Focus",
        label_class: "text-[11px] text-ink-500 mono uppercase tracking-wider",
        input_class: "mt-1 block w-full h-9 px-3 rounded-md border border-ink-900 bg-surface text-[14px] focus:outline-none",
        placeholder: None,
        value: Some("Aenean lectus"),
        disabled: false,
        readonly: true,
    },
    StateEntry {
        label: "Filled",
        label_class: "text-[11px] text-ink-500 mono uppercase tracking-wider",
        input_class: "mt-1 block w-full h-9 px-3 rounded-md border border-line bg-surface text-[14px]",
        placeholder: None,
        value: Some("Vestibulum ante ipsum"),
        disabled: false,
        readonly: false,
    },
    StateEntry {
        label: "Error",
        label_class: "text-[11px] text-negative mono uppercase tracking-wider",
        input_class: "mt-1 block w-full h-9 px-3 rounded-md border border-negative bg-surface text-[14px]",
        placeholder: None,
        value: Some("Invalid value"),
        disabled: false,
        readonly: false,
    },
    StateEntry {
        label: "Disabled",
        label_class: "text-[11px] text-ink-400 mono uppercase tracking-wider",
        input_class: "mt-1 block w-full h-9 px-3 rounded-md border border-dashed border-line bg-surfaceMuted text-[14px] text-ink-400 cursor-not-allowed opacity-70",
        placeholder: None,
        value: Some("Read only"),
        disabled: true,
        readonly: false,
    },
];

/// Build the States subsection.
fn states(entries: &[StateEntry]) -> String {
    let mut states_div = Division::builder();
    states_div.class("space-y-3");
    for entry in entries {
        let label = entry.label.to_owned();
        let label_class = entry.label_class.to_owned();
        let input_class = entry.input_class.to_owned();
        let placeholder = entry.placeholder.map(ToOwned::to_owned);
        let value = entry.value.map(ToOwned::to_owned);
        let disabled = entry.disabled;
        let readonly = entry.readonly;
        states_div.division(|d| {
            d.span(|s| s.class(label_class).text(label));
            let mut input = html::forms::Input::builder();
            input.type_("text");
            input.class(input_class);
            if let Some(ref p) = placeholder {
                input.placeholder(p.clone());
            }
            if let Some(ref v) = value {
                input.value(v.clone());
            }
            if disabled {
                input.disabled("");
            }
            if readonly {
                input.readonly("");
            }
            d.text(input.build().to_string())
        });
    }

    Division::builder()
        .division(|d| d.class("text-[12px] text-ink-500 mb-2").text("States"))
        .text(states_div.build().to_string())
        .paragraph(|p| {
            p.class("mt-3 text-[11px] text-ink-500")
                .text("Focus darkens the border to ink \u{2014} no thicker, no glow.")
        })
        .build()
        .to_string()
}

/// Build the basic Input subsection.
fn input_basic() -> String {
    html::forms::Label::builder()
        .class("block")
        .span(|s| s.class("text-[12px] text-ink-500").text("Label"))
        .input(|i| {
            i.type_("text")
                .placeholder("Lorem ipsum")
                .class("mt-1 block w-full h-9 px-3 rounded-md border border-line bg-surface text-[14px] placeholder:text-ink-400 focus:outline-none focus:border-ink-900")
        })
        .build()
        .to_string()
}

/// Build the Input with help text subsection.
fn input_help() -> String {
    html::forms::Label::builder()
        .class("block")
        .span(|s| s.class("text-[12px] text-ink-500").text("With helper text"))
        .input(|i| {
            i.type_("text")
                .value("Aenean lectus")
                .class("mt-1 block w-full h-9 px-3 rounded-md border border-line bg-surface text-[14px] focus:outline-none focus:border-ink-900")
        })
        .span(|s| s.class("mt-1 block text-[11px] text-ink-500").text("Vestibulum ante ipsum primis."))
        .build()
        .to_string()
}

/// Build the Error state subsection.
fn input_error() -> String {
    html::forms::Label::builder()
        .class("block")
        .span(|s| s.class("text-[12px] text-negative").text("Error state"))
        .input(|i| {
            i.type_("text")
                .value("Invalid value")
                .class("mt-1 block w-full h-9 px-3 rounded-md border border-negative bg-surface text-[14px] focus:outline-none")
        })
        .span(|s| s.class("mt-1 block text-[11px] text-negative").text("Pellentesque habitant morbi."))
        .build()
        .to_string()
}

/// Build the Disabled subsection.
fn input_disabled() -> String {
    html::forms::Label::builder()
        .class("block")
        .span(|s| s.class("text-[12px] text-ink-400").text("Disabled"))
        .input(|i| {
            i.type_("text")
                .value("Read only")
                .disabled("")
                .class("mt-1 block w-full h-9 px-3 rounded-md border border-dashed border-line bg-surfaceMuted text-[14px] text-ink-400 cursor-not-allowed opacity-70")
        })
        .build()
        .to_string()
}

#[allow(dead_code)]
/// Build the Search subsection.
pub(crate) fn search() -> String {
    let input = html::forms::Input::builder()
        .type_("search")
        .placeholder("Search\u{2026}")
        .class("block w-full h-9 pl-9 pr-3 rounded-md border border-line bg-surface text-[14px] placeholder:text-ink-400 focus:outline-none focus:border-ink-900")
        .build()
        .to_string();

    html::forms::Label::builder()
        .class("block")
        .span(|s| s.class("text-[12px] text-ink-500").text("Search"))
        .text(
            Division::builder()
                .class("mt-1 relative")
                .text(SVG_SEARCH_SM)
                .text(input)
                .build()
                .to_string(),
        )
        .build()
        .to_string()
}

#[allow(dead_code)]
/// Build the prominent Search subsection.
pub(crate) fn search_prominent() -> String {
    let input = html::forms::Input::builder()
        .type_("search")
        .placeholder("Search 12\u{00a0}480 packages\u{2026}")
        .class("block w-full h-10 pl-10 pr-24 rounded-lg border border-line bg-canvas text-[14px] placeholder:text-ink-400 focus:outline-none focus:border-ink-900")
        .build()
        .to_string();

    let kbd = r#"<kbd class="absolute right-3 top-1/2 -translate-y-1/2 inline-flex items-center gap-1 h-6 px-2 rounded border border-lineSoft bg-surfaceMuted text-[11px] mono text-ink-500"><span>&#x2318;</span><span>K</span></kbd>"#;

    let help_text = Division::builder()
        .class("mt-1 relative")
        .text(SVG_SEARCH_LG)
        .text(input)
        .text(kbd)
        .build()
        .to_string();

    html::forms::Label::builder()
        .class("block")
        .span(|s| s.class("text-[12px] text-ink-500").text("Search \u{00b7} prominent with shortcut hint"))
        .text(help_text)
        .span(|s| {
            s.class("mt-1 block text-[11px] text-ink-500")
                .text("For browse / index landing surfaces where search is the primary action. Departures from the ")
                .code(|c| c.class("mono text-[10.5px]").text("md"))
                .text(" default: ")
                .code(|c| c.class("mono text-[10.5px]").text("h-10"))
                .text(" (one step taller, anchors the section without competing with the heading); ")
                .code(|c| c.class("mono text-[10.5px]").text("rounded-lg"))
                .text(" (softer than the ")
                .code(|c| c.class("mono text-[10.5px]").text("rounded-md"))
                .text(" form default \u{2014} search is a noun, not a transactional control); ")
                .code(|c| c.class("mono text-[10.5px]").text("bg-canvas"))
                .text(" instead of ")
                .code(|c| c.class("mono text-[10.5px]").text("bg-surface"))
                .text(" when the section sits on ")
                .code(|c| c.class("mono text-[10.5px]").text("surface"))
                .text("; 16px icon at ")
                .code(|c| c.class("mono text-[10.5px]").text("pl-10"))
                .text("; trailing ")
                .code(|c| c.class("mono text-[10.5px]").text("&lt;kbd&gt;"))
                .text(" hint (")
                .code(|c| c.class("mono text-[10.5px]").text("h-6 \u{00b7} 11px mono \u{00b7} border-lineSoft \u{00b7} bg-surfaceMuted"))
                .text(") with ")
                .code(|c| c.class("mono text-[10.5px]").text("pr-24"))
                .text(" on the input to clear it.")
        })
        .build()
        .to_string()
}

#[allow(dead_code)]
/// Build the Select subsection.
pub(crate) fn select() -> String {
    let select_el = html::forms::Select::builder()
        .class("appearance-none block w-full h-9 pl-3 pr-8 rounded-md border border-line bg-surface text-[14px] focus:outline-none focus:border-ink-900")
        .option(|o| o.text("Tellus"))
        .option(|o| o.text("Pellentesque"))
        .option(|o| o.text("Vestibulum"))
        .build()
        .to_string();

    let wrapper = Division::builder()
        .class("mt-1 relative")
        .text(select_el)
        .text(SVG_CHEV_SELECT)
        .build()
        .to_string();

    html::forms::Label::builder()
        .class("block")
        .span(|s| s.class("text-[12px] text-ink-500").text("Select"))
        .text(wrapper)
        .build()
        .to_string()
}

#[allow(dead_code)]
/// Build the Textarea subsection.
pub(crate) fn textarea() -> String {
    html::forms::Label::builder()
        .class("block")
        .span(|s| s.class("text-[12px] text-ink-500").text("Textarea"))
        .text_area(|ta| {
            ta.rows(3)
                .placeholder("Lorem ipsum dolor sit amet\u{2026}")
                .class("mt-1 block w-full px-3 py-2 rounded-md border border-line bg-surface text-[14px] placeholder:text-ink-400 resize-y focus:outline-none focus:border-ink-900")
        })
        .build()
        .to_string()
}

#[allow(dead_code)]
/// Build the prefix addon subsection.
pub(crate) fn input_prefix() -> String {
    let inner = Division::builder()
        .class("mt-1 flex")
        .span(|s| {
            s.class("inline-flex items-center px-3 h-9 rounded-l-md border border-r-0 border-line bg-surfaceMuted text-[13px] text-ink-500 mono")
                .text("https://")
        })
        .text(html::forms::Input::builder()
            .type_("text")
            .value("lorem.ipsum")
            .class("block w-full h-9 px-3 rounded-r-md border border-line bg-surface text-[14px] focus:outline-none focus:border-ink-900")
            .build()
            .to_string())
        .build()
        .to_string();

    labeled_field(
        "Input group \u{00b7} prefix",
        "text-[12px] text-ink-500",
        &inner,
    )
}

#[allow(dead_code)]
/// Build the suffix addon subsection.
pub(crate) fn input_suffix() -> String {
    let inner = Division::builder()
        .class("mt-1 flex")
        .text(html::forms::Input::builder()
            .type_("text")
            .value("42")
            .class("block w-full h-9 px-3 rounded-l-md border border-r-0 border-line bg-surface text-[14px] tabular-nums focus:outline-none focus:border-ink-900")
            .build()
            .to_string())
        .span(|s| {
            s.class("inline-flex items-center px-3 h-9 rounded-r-md border border-line bg-surfaceMuted text-[13px] text-ink-500 mono")
                .text("kg")
        })
        .build()
        .to_string();

    labeled_field(
        "Input group \u{00b7} suffix",
        "text-[12px] text-ink-500",
        &inner,
    )
}

#[allow(dead_code)]
/// Build the button group subsection.
pub(crate) fn input_button() -> String {
    let inner = Division::builder()
        .class("mt-1 flex")
        .text(html::forms::Input::builder()
            .type_("text")
            .placeholder("Search registry\u{2026}")
            .class("block w-full h-9 px-3 rounded-l-md border border-r-0 border-line bg-surface text-[14px] placeholder:text-ink-400 focus:outline-none focus:border-ink-900")
            .build()
            .to_string())
        .button(|b| {
            b.type_("button")
                .class("inline-flex items-center px-3 h-9 rounded-r-md border-[1.5px] border-ink-900 bg-surface text-ink-900 text-[13px] hover:bg-surfaceMuted")
                .text("Search")
        })
        .build()
        .to_string();

    labeled_field(
        "Input group \u{00b7} button",
        "text-[12px] text-ink-500",
        &inner,
    )
}

/// Build the button with dropdown group subsection.
fn input_btn_dropdown() -> String {
    let group = input_group::button_with_dropdown(&ButtonWithDropdown {
        options: &[
            DropdownOption {
                id: "tellus",
                label: "Tellus",
                value: "lorem ipsum dolor",
            },
            DropdownOption {
                id: "aenean",
                label: "Aenean",
                value: "consectetur adipiscing",
            },
            DropdownOption {
                id: "mauris",
                label: "Mauris",
                value: "sed do eiusmod",
            },
        ],
        selected: 0,
        trigger_aria_label: "Choose an option",
        field_aria_label: "Selected value",
        field_class: "",
        with_copy: false,
        prefix: "",
    });
    let inner = format!(r#"<div class="mt-1">{group}</div>"#);

    let inner_with_help = format!(
        "{inner}{}",
        html::inline_text::Span::builder()
            .class("mt-1 block text-[11px] text-ink-500")
            .text("Quiet dropdown addon paired with the field \u{2014} pick an option to swap the field value. Reusable via <code>ds::input_group::button_with_dropdown</code>.")
            .build()
    );

    labeled_field(
        "Input group \u{00b7} button with dropdown",
        "text-[12px] text-ink-500",
        &inner_with_help,
    )
}

/// Build the button with dropdown + prefix addon subsection: the same fused
/// dropdown, now with a leading `$` prompt box and a trailing copy button.
fn input_btn_dropdown_prefix() -> String {
    let group = input_group::button_with_dropdown(&ButtonWithDropdown {
        options: &[
            DropdownOption {
                id: "bash",
                label: "bash",
                value: "curl -LsSf https://lorem.ipsum/install.sh | sh",
            },
            DropdownOption {
                id: "pwsh",
                label: "PowerShell",
                value: "irm https://lorem.ipsum/install.ps1 | iex",
            },
        ],
        selected: 0,
        trigger_aria_label: "Choose a shell",
        field_aria_label: "Install command",
        field_class: "mono",
        with_copy: true,
        prefix: "$",
    });
    let inner = format!(r#"<div class="mt-1">{group}</div>"#);

    let inner_with_help = format!(
        "{inner}{}",
        html::inline_text::Span::builder()
            .class("mt-1 block text-[11px] text-ink-500")
            .text("Add a leading <code>prefix</code> for a decorative affix such as a <code>$</code> shell prompt. The addon is not part of the copied value, so the copy button still yields just the command.")
            .build()
    );

    labeled_field(
        "Input group \u{00b7} button with dropdown + prefix",
        "text-[12px] text-ink-500",
        &inner_with_help,
    )
}

/// Build the split dropdown group subsection.
fn input_split_dropdown() -> String {
    let inner = Division::builder()
        .class("mt-1 flex")
        .text(html::forms::Input::builder()
            .type_("text")
            .value("https://lorem.ipsum/dolor")
            .class("block w-full h-9 px-3 rounded-l-md border border-r-0 border-line bg-surface text-[14px] focus:outline-none focus:border-ink-900")
            .build()
            .to_string())
        .button(|b| {
            b.type_("button")
                .class("inline-flex items-center gap-2 px-3 h-9 border border-r-0 border-line bg-surfaceMuted text-ink-700 text-[13px] hover:bg-ink-300 hover:text-ink-900")
                .text("Copy")
        })
        .button(|b| {
            b.type_("button")
                .aria_label("More")
                .class("inline-flex items-center px-2 h-9 rounded-r-md border border-line bg-surfaceMuted text-ink-700 hover:bg-ink-300 hover:text-ink-900")
                .text(SVG_CHEV_SPLIT)
        })
        .build()
        .to_string();

    labeled_field(
        "Input group \u{00b7} split dropdown",
        "text-[12px] text-ink-500",
        &inner,
    )
}

/// Command demo entry.
pub(crate) struct CommandEntry {
    pub(crate) label: &'static str,
    pub(crate) cmd_text: &'static str,
    pub(crate) btn_class: &'static str,
    pub(crate) btn_label: &'static str,
    pub(crate) btn_svg: &'static str,
    pub(crate) help: &'static str,
    pub(crate) wrapper_class: &'static str,
    pub(crate) dollar_class: &'static str,
    pub(crate) code_class: &'static str,
    pub(crate) disabled: bool,
}

pub(crate) const COMMANDS: &[CommandEntry] = &[
    CommandEntry {
        label: "Command \u{00b7} default",
        cmd_text: "component install wasi:http-handler",
        btn_class: "inline-flex items-center justify-center w-9 h-9 rounded-r-md border border-l-0 border-line bg-surface text-ink-500 hover:text-ink-900 hover:bg-surfaceMuted focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-[-2px] focus-visible:outline-ink-900",
        btn_label: "Copy command",
        btn_svg: SVG_COPY,
        help: r#"Use for install snippets and shareable shell commands. Renders as <code class="mono text-[10.5px]">&lt;code&gt;</code> — keyboard select-all + copy works natively. Don't use for inline code in prose; use a <code class="mono text-[10.5px]">&lt;code&gt;</code> chip instead."#,
        wrapper_class: "mt-1 flex group",
        dollar_class: "inline-flex items-center px-3 h-9 rounded-l-md border border-r-0 border-line bg-surfaceMuted text-[13px] text-ink-500 mono select-none",
        code_class: "block w-full h-9 px-3 inline-flex items-center border border-line bg-surface mono text-[13px] text-ink-900 overflow-x-auto whitespace-nowrap",
        disabled: false,
    },
    CommandEntry {
        label: "Command \u{00b7} copied (1.6s after click)",
        cmd_text: "component install wasi:http-handler",
        btn_class: "inline-flex items-center justify-center w-9 h-9 rounded-r-md border border-l-0 border-line bg-surface text-positive",
        btn_label: "Copied",
        btn_svg: SVG_CHECK,
        help: r#"Icon swaps to a check in <code class="mono text-[10.5px]">text-positive</code>; reverts after 1.6s."#,
        wrapper_class: "mt-1 flex",
        dollar_class: "inline-flex items-center px-3 h-9 rounded-l-md border border-r-0 border-line bg-surfaceMuted text-[13px] text-ink-500 mono select-none",
        code_class: "block w-full h-9 px-3 inline-flex items-center border border-line bg-surface mono text-[13px] text-ink-900 overflow-x-auto whitespace-nowrap",
        disabled: false,
    },
    CommandEntry {
        label: "Command \u{00b7} long (horizontal overflow)",
        cmd_text: "component install wasi:http-handler@0.4.2 --registry https://registry.example.com --keychain",
        btn_class: "inline-flex items-center justify-center w-9 h-9 rounded-r-md border border-l-0 border-line bg-surface text-ink-500 hover:text-ink-900 hover:bg-surfaceMuted",
        btn_label: "Copy command",
        btn_svg: SVG_COPY_LONG,
        help: "Field scrolls horizontally; the prefix and copy button stay anchored.",
        wrapper_class: "mt-1 flex",
        dollar_class: "inline-flex items-center px-3 h-9 rounded-l-md border border-r-0 border-line bg-surfaceMuted text-[13px] text-ink-500 mono select-none",
        code_class: "block w-full h-9 px-3 inline-flex items-center border border-line bg-surface mono text-[13px] text-ink-900 overflow-x-auto whitespace-nowrap",
        disabled: false,
    },
    CommandEntry {
        label: "Command \u{00b7} copy failed",
        cmd_text: "component install wasi:http-handler",
        btn_class: "inline-flex items-center justify-center w-9 h-9 rounded-r-md border border-l-0 border-line bg-surface text-negative",
        btn_label: "Copy failed",
        btn_svg: SVG_FAIL,
        help: r#"Clipboard API blocked (insecure context, permission denied). Border + icon shift to <code class="mono text-[10.5px]">negative</code>; helper toast (not shown) explains &quot;select and copy manually.&quot;"#,
        wrapper_class: "mt-1 flex",
        dollar_class: "inline-flex items-center px-3 h-9 rounded-l-md border border-r-0 border-line bg-surfaceMuted text-[13px] text-ink-500 mono select-none",
        code_class: "block w-full h-9 px-3 inline-flex items-center border border-line bg-surface mono text-[13px] text-ink-900 overflow-x-auto whitespace-nowrap",
        disabled: false,
    },
    CommandEntry {
        label: "Command \u{00b7} disabled",
        cmd_text: "component install wasi:http-handler",
        btn_class: "inline-flex items-center justify-center w-9 h-9 rounded-r-md border border-l-0 border-dashed border-line bg-surfaceMuted text-ink-400 cursor-not-allowed",
        btn_label: "Copy command",
        btn_svg: SVG_COPY_DISABLED,
        help: "Use during pending/async states \u{2014} e.g. while a registry version is being resolved. Shares the dashed-border / opacity-70 vocabulary with disabled inputs.",
        wrapper_class: "mt-1 flex opacity-70",
        dollar_class: "inline-flex items-center px-3 h-9 rounded-l-md border border-r-0 border-dashed border-line bg-surfaceMuted text-[13px] text-ink-400 mono select-none",
        code_class: "block w-full h-9 px-3 inline-flex items-center border border-dashed border-line bg-surfaceMuted mono text-[13px] text-ink-400 overflow-x-auto whitespace-nowrap cursor-not-allowed",
        disabled: true,
    },
];

#[allow(dead_code)]
/// Build a command demo.
pub(crate) fn command(entry: &CommandEntry) -> String {
    let label = entry.label.to_owned();
    let wrapper_class = entry.wrapper_class.to_owned();
    let dollar_class = entry.dollar_class.to_owned();
    let code_class = entry.code_class.to_owned();
    let cmd_text = entry.cmd_text.to_owned();
    let btn_class = entry.btn_class.to_owned();
    let btn_label = entry.btn_label.to_owned();
    let btn_svg = entry.btn_svg.to_owned();
    let help = entry.help.to_owned();
    let disabled = entry.disabled;

    let mut btn = html::forms::Button::builder();
    btn.type_("button");
    btn.class(btn_class);
    btn.aria_label(btn_label);
    btn.text(btn_svg);
    if disabled {
        btn.disabled(true);
    }

    Division::builder()
        .span(|s| s.class("text-[12px] text-ink-500").text(label))
        .division(|d| {
            d.class(wrapper_class)
                .span(|s| {
                    let mut s = s.class(dollar_class);
                    s = s.aria_hidden(true);
                    s.text("$")
                })
                .code(|c| c.class(code_class).text(cmd_text))
                .text(btn.build().to_string())
        })
        .span(|s| s.class("mt-1 block text-[11px] text-ink-500").text(help))
        .build()
        .to_string()
}

#[allow(dead_code)]
/// Build the number stepper subsection.
pub(crate) fn stepper() -> String {
    let input = html::forms::Input::builder()
        .type_("text")
        .value("12")
        .class("block w-16 h-9 px-2 border-y-[1.5px] border-ink-900 bg-surface text-[14px] tabular-nums text-center focus:outline-none")
        .build()
        .to_string();

    let inner = Division::builder()
        .class("mt-1 inline-flex")
        .button(|b| {
            b.type_("button")
                .aria_label("Decrement")
                .class("h-9 w-9 grid place-items-center rounded-l-md border-[1.5px] border-r-0 border-ink-900 bg-surface text-ink-900 hover:bg-surfaceMuted")
                .text(SVG_MINUS)
        })
        .text(input)
        .button(|b| {
            b.type_("button")
                .aria_label("Increment")
                .class("h-9 w-9 grid place-items-center rounded-r-md border-[1.5px] border-l-0 border-ink-900 bg-surface text-ink-900 hover:bg-surfaceMuted")
                .text(SVG_PLUS)
        })
        .build()
        .to_string();

    labeled_field("Number stepper", "text-[12px] text-ink-500", &inner)
}

#[allow(dead_code)]
/// Build the file input subsection.
pub(crate) fn file_input() -> String {
    let inner = Division::builder()
        .class("mt-1 flex")
        .button(|b| {
            b.type_("button")
                .class("inline-flex items-center gap-2 px-3 h-9 rounded-l-md border border-r-0 border-line bg-surfaceMuted text-ink-700 text-[13px] hover:bg-ink-300 hover:text-ink-900 whitespace-nowrap")
                .text(SVG_ATTACH)
                .text("Choose file")
        })
        .span(|s| {
            s.class("block w-full h-9 px-3 inline-flex items-center rounded-r-md border border-line bg-surface text-[13px] text-ink-500")
                .text("No file selected")
        })
        .build()
        .to_string();

    labeled_field("File input", "text-[12px] text-ink-500", &inner)
}

#[allow(dead_code)]
/// Build the range slider subsection.
pub(crate) fn range_slider() -> String {
    html::forms::Label::builder()
        .class("block")
        .span(|s| {
            s.class("text-[12px] text-ink-500 flex items-center justify-between")
                .text("Range ")
                .span(|inner| inner.class("mono text-ink-700").text("64"))
        })
        .input(|i| {
            i.type_("range")
                .min("0")
                .max("100")
                .value("64")
                .class("mt-2 block w-full accent-ink-900")
        })
        .build()
        .to_string()
}

#[allow(dead_code)]
/// Build the color input subsection.
pub(crate) fn color_input() -> String {
    let inner = Division::builder()
        .class("mt-1 flex")
        .span(|s| {
            s.class("inline-flex items-center justify-center h-9 w-9 rounded-l-md border border-r-0 border-line bg-surface")
                // Raw HTML: color swatch uses inline style= for the background.
                // Span::style() creates a <style> child, not an inline style attribute.
                .text(r#"<span class="block h-5 w-5 rounded border border-line" style="background:#9B4F5E"></span>"#)
        })
        .text(html::forms::Input::builder()
            .type_("text")
            .value("#9B4F5E")
            .class("block w-full h-9 px-3 rounded-r-md border border-line bg-surface text-[14px] mono uppercase focus:outline-none focus:border-ink-900")
            .build()
            .to_string())
        .build()
        .to_string();

    labeled_field("Color", "text-[12px] text-ink-500", &inner)
}

#[allow(dead_code)]
/// Build the date/time subsection.
pub(crate) fn date_time() -> String {
    let date_label = html::forms::Label::builder()
        .class("block")
        .span(|s| s.class("text-[12px] text-ink-500").text("Date"))
        .text(Division::builder()
            .class("mt-1 relative")
            .text(html::forms::Input::builder()
                .type_("text")
                .value("2026-04-18")
                .class("block w-full h-9 pl-3 pr-9 rounded-md border border-line bg-surface text-[14px] mono focus:outline-none focus:border-ink-900")
                .build()
                .to_string())
            .text(SVG_CALENDAR)
            .build()
            .to_string())
        .build()
        .to_string();

    let time_label = html::forms::Label::builder()
        .class("block")
        .span(|s| s.class("text-[12px] text-ink-500").text("Time"))
        .text(Division::builder()
            .class("mt-1 relative")
            .text(html::forms::Input::builder()
                .type_("text")
                .value("14:30")
                .class("block w-full h-9 pl-3 pr-9 rounded-md border border-line bg-surface text-[14px] mono focus:outline-none focus:border-ink-900")
                .build()
                .to_string())
            .text(SVG_CLOCK)
            .build()
            .to_string())
        .build()
        .to_string();

    Division::builder()
        .class("grid grid-cols-2 gap-3")
        .text(date_label)
        .text(time_label)
        .build()
        .to_string()
}

#[allow(dead_code)]
/// Build the combobox subsection.
pub(crate) fn combobox() -> String {
    let dropdown = Division::builder()
        .class("absolute left-0 right-0 mt-1 rounded-md border border-line bg-surface shadow-tooltip overflow-hidden text-[14px] z-10")
        .division(|d| {
            d.class("px-3 h-9 flex items-center bg-surfaceMuted text-ink-900")
                .span(|s| s.class("font-medium").text("Pellen"))
                .span(|s| s.class("text-ink-500").text("tesque habitant"))
        })
        .division(|d| {
            d.class("px-3 h-9 flex items-center text-ink-700")
                .span(|s| s.class("font-medium").text("Pellen"))
                .span(|s| s.class("text-ink-500").text("tesque morbi"))
        })
        .division(|d| {
            d.class("px-3 h-9 flex items-center text-ink-700")
                .span(|s| s.class("font-medium").text("Pellen"))
                .span(|s| s.class("text-ink-500").text("tesque vivamus"))
        })
        .build()
        .to_string();

    let inner = Division::builder()
        .class("mt-1 relative")
        .text(html::forms::Input::builder()
            .type_("text")
            .value("Pellen")
            .class("block w-full h-9 px-3 rounded-md border border-line bg-surface text-[14px] focus:outline-none focus:border-ink-900")
            .build()
            .to_string())
        .text(dropdown)
        .build()
        .to_string();

    labeled_field("Combobox", "text-[12px] text-ink-500", &inner)
}

/// Render this section.
pub(crate) fn render(
    section_id: &str,
    num: &str,
    title: &str,
    desc: &str,
    state_entries: &[StateEntry],
    commands: &[CommandEntry],
) -> String {
    let mut content = Division::builder();
    content.class("space-y-8 max-w-md");

    content.text(sizes());
    content.text(states(state_entries));
    content.text(input_basic());
    content.text(input_help());
    content.text(input_error());
    content.text(input_disabled());
    content.text(search());
    content.text(search_prominent());
    content.text(select());
    content.text(textarea());
    content.text(input_prefix());
    content.text(input_suffix());
    content.text(input_button());
    content.text(input_btn_dropdown());
    content.text(input_btn_dropdown_prefix());
    content.text(input_split_dropdown());
    for entry in commands {
        content.text(command(entry));
    }
    content.text(stepper());
    content.text(file_input());
    content.text(range_slider());
    content.text(color_input());
    content.text(date_time());
    content.text(combobox());

    let content = content.build().to_string();

    super::section(section_id, num, title, desc, &content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot() {
        insta::assert_snapshot!(crate::components::ds::pretty_html(&render(
            "fields",
            "13",
            "Form Fields",
            "Inputs sit on a surface with a 1px line border. Focus darkens the border to ink \u{2014} no thickening, no glow. Two sizes: <strong>md</strong> (default) for primary forms, <strong>sm</strong> for dense contexts like sidebars, metadata strips, and toolbars.",
            STATES,
            COMMANDS,
        )));
    }
}
