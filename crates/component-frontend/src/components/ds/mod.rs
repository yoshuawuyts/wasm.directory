//! Design system section components.
//!
//! Each submodule renders one section of the design system reference page.
//! Shared helpers live at the module level; individual sections are submodules.

use html::content::Section;

/// Render a standard design-system section with the two-column
/// `[600px | 1fr]` grid layout used by every numbered section.
///
/// `id` is the anchor, `num` the section number label (e.g. `"01"`),
/// `title` the heading text, `desc` the synopsis paragraph, and `content`
/// is the pre-rendered HTML string for the right-hand column.
pub(crate) fn section(id: &str, num: &str, title: &str, desc: &str, content: &str) -> String {
    let id = id.to_owned();
    let num = num.to_owned();
    let title = title.to_owned();
    let desc = desc.to_owned();
    let content = content.to_owned();
    let sec = Section::builder()
        .id(id)
        .class("pt-12 md:pt-16")
        .division(|grid| {
            grid.class("grid md:grid-cols-[600px_1fr] gap-6 md:gap-12")
                .division(|left| {
                    left.division(|n| {
                        n.class("text-[12px] mono uppercase tracking-wider text-ink-500")
                            .text(num.clone())
                    })
                    .heading_2(|h| {
                        h.class("mt-2 text-[24px] font-semibold tracking-tight")
                            .text(title.clone())
                    })
                    .paragraph(|p| {
                        p.class("mt-2 text-[13px] text-ink-500 leading-relaxed")
                            .text(desc.clone())
                    })
                })
                .text(content.clone())
        })
        .build();
    sec.to_string()
}

/// Render a subsection heading (h3).
#[allow(dead_code)]
pub(crate) fn sub(text: &str) -> String {
    let text = text.to_owned();
    html::content::Heading3::builder()
        .class("text-[13px] mono uppercase tracking-wider text-ink-500 mb-3")
        .text(text)
        .build()
        .to_string()
}

pub(crate) mod badges;
pub(crate) mod breadcrumb;
pub(crate) mod buttons;
pub(crate) mod code;
pub(crate) mod color;
pub(crate) mod cta_strip;
pub(crate) mod details;
pub(crate) mod dropdown;
pub(crate) mod elevation;
pub(crate) mod empty;
pub(crate) mod fields;
pub(crate) mod footer;
pub(crate) mod grid;
pub(crate) mod header;
pub(crate) mod hero;
pub(crate) mod icons;
pub(crate) mod input_group;
pub(crate) mod install_card;
pub(crate) mod install_widget;
pub(crate) mod item_details;
pub(crate) mod item_list;
pub(crate) mod labels;
pub(crate) mod link_list;
pub(crate) mod metrics_strip;
pub(crate) mod modal;
pub(crate) mod motion;
pub(crate) mod navbar;
pub(crate) mod navigation;
pub(crate) mod on_this_page;
pub(crate) mod page_header;
pub(crate) mod part_two;
pub(crate) mod principles_grid;
pub(crate) mod progress;
pub(crate) mod quick_start;
pub(crate) mod regions;
pub(crate) mod sidebar;
pub(crate) mod sigil;
pub(crate) mod spacing;
pub(crate) mod table;
pub(crate) mod tabs;
pub(crate) mod toc;
pub(crate) mod toggles;
pub(crate) mod tooltip;
pub(crate) mod typography;
pub(crate) mod wit_item;

/// Crude HTML pretty-printer for snapshot tests.
///
/// Inserts newlines before opening/closing tags and indents by nesting depth.
/// Not spec-compliant — just enough to make snapshot diffs readable.
#[cfg(test)]
pub(super) fn pretty_html(html: &str) -> String {
    let mut out = String::with_capacity(html.len() * 2);
    let mut depth: usize = 0;
    let mut i = 0;
    let bytes = html.as_bytes();

    while i < bytes.len() {
        if bytes[i] == b'<' {
            // Find the end of this tag.
            let tag_end = match memchr(b'>', bytes, i) {
                Some(pos) => pos,
                None => break,
            };
            let tag = &html[i..=tag_end];

            let is_close = tag.starts_with("</");
            let is_void = tag.ends_with("/>") || is_void_element(tag);

            if is_close {
                depth = depth.saturating_sub(1);
            }

            // Newline + indent before the tag.
            if !out.is_empty() {
                out.push('\n');
            }
            indent(&mut out, depth);
            out.push_str(tag);

            if !is_close && !is_void {
                depth += 1;
            }

            i = tag_end + 1;
        } else {
            // Text node — collect until next '<'.
            let text_end = memchr(b'<', bytes, i).unwrap_or(bytes.len());
            let text = &html[i..text_end];
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                out.push('\n');
                indent(&mut out, depth);
                out.push_str(trimmed);
            }
            i = text_end;
        }
    }
    out
}

#[cfg(test)]
fn memchr(needle: u8, haystack: &[u8], start: usize) -> Option<usize> {
    haystack[start..]
        .iter()
        .position(|&b| b == needle)
        .map(|p| p + start)
}

#[cfg(test)]
fn indent(out: &mut String, depth: usize) {
    for _ in 0..depth {
        out.push_str("  ");
    }
}

#[cfg(test)]
fn is_void_element(tag: &str) -> bool {
    // Extract just the tag name.
    let name = tag
        .trim_start_matches('<')
        .split(|c: char| c.is_whitespace() || c == '/' || c == '>')
        .next()
        .unwrap_or("");
    matches!(
        name,
        "br" | "hr" | "img" | "input" | "meta" | "link" | "source" | "area" | "base" | "col"
    )
}
pub(crate) mod button;
pub(crate) mod copy_button;
pub(crate) mod detail_row;
pub(crate) mod link_button;
pub(crate) mod metadata_table;
pub(crate) mod nav_list;
pub(crate) mod package_card;
pub(crate) mod package_row;
pub(crate) mod search_bar;
pub(crate) mod section_group;
