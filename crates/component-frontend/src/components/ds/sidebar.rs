//! C01 — Nested Sidebar.

use crate::escape::{escape_html_attr, escape_html_text};
use html::content::Navigation;
use html::interactive::Details;
use html::text_content::Division;

/// C01 section label shared by sidebar navigation groups.
pub(crate) const SECTION_LABEL_CLASS: &str =
    "mono uppercase tracking-wider text-[10px] text-ink-500 mb-2";

/// Info bubble: a small info icon with a tooltip.
pub(crate) const INFO_BUBBLE: &str = concat!(
    r#"<span class="inline-flex items-center justify-center w-3 h-3 text-ink-400 cursor-help" title="The OCI tag for this release">"#,
    r#"<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
    include_str!("../../../../../vendor/lucide/info.svg"),
    "</svg></span>",
);

/// Info bubble for the digest field.
pub(crate) const INFO_BUBBLE_DIGEST: &str = concat!(
    r#"<span class="inline-flex items-center justify-center w-3 h-3 text-ink-400 cursor-help" title="Content-addressable SHA-256 digest of the OCI manifest. We use this to pin an exact image for reproducible builds.">"#,
    r#"<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
    include_str!("../../../../../vendor/lucide/info.svg"),
    "</svg></span>",
);

/// Info bubble for the revision field.
pub(crate) const INFO_BUBBLE_REVISION: &str = concat!(
    r#"<span class="inline-flex items-center justify-center w-3 h-3 text-ink-400 cursor-help" title="Source control revision (e.g. the git commit) this image was built from.">"#,
    r#"<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
    include_str!("../../../../../vendor/lucide/info.svg"),
    "</svg></span>",
);

#[allow(dead_code)]
const SVG_CHEV_DOWN: &str = concat!(
    r#"<svg class="h-3 w-3 text-ink-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">"#,
    include_str!("../../../../../vendor/lucide/chevron-down.svg"),
    "</svg>"
);
const SVG_CHEV_RIGHT: &str = concat!(
    r#"<svg class="chev" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">"#,
    include_str!("../../../../../vendor/lucide/chevron-right.svg"),
    "</svg>"
);
#[allow(dead_code)]
const SVG_GITHUB: &str = r#"<svg class="h-3.5 w-3.5 text-ink-500 flex-shrink-0" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true"><path d="M8 .2a8 8 0 0 0-2.5 15.6c.4 0 .55-.17.55-.38v-1.4c-2.22.48-2.69-1.07-2.69-1.07-.36-.92-.89-1.17-.89-1.17-.73-.5.05-.49.05-.49.8.06 1.23.83 1.23.83.71 1.23 1.87.87 2.33.66.07-.52.28-.87.5-1.07-1.77-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.83-2.15-.08-.2-.36-1.02.08-2.13 0 0 .67-.22 2.2.82A7.6 7.6 0 0 1 8 4.04c.68 0 1.37.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.11.16 1.93.08 2.13.52.56.83 1.28.83 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.74.54 1.49v2.21c0 .21.15.46.55.38A8 8 0 0 0 8 .2Z" /></svg>"#;
#[allow(dead_code)]
const SVG_CRATE: &str = r#"<svg class="h-3.5 w-3.5 text-ink-500 flex-shrink-0" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" aria-hidden="true"><rect x="2.5" y="3" width="11" height="10" rx="1" /><path d="M2.5 6.5h11M6 3v10" /></svg>"#;

// Raw HTML: Span::style() creates a <style> child, not an inline style attribute.
#[allow(dead_code)]
pub(crate) fn sigil(bg: &str, color: &str, text: &str) -> String {
    format!(r#"<span class="sigil" style="background:{bg};color:{color};">{text}</span>"#)
}

#[allow(dead_code)]
pub(crate) fn tree_link(sigil_html: &str, name: &str, meta: &str) -> html::inline_text::Anchor {
    let mut a = html::inline_text::Anchor::builder();
    a.href("#".to_owned()).class("tree-link");
    a.text(sigil_html.to_owned());
    a.span(|s| s.class("mono").text(name.to_owned()));
    if !meta.is_empty() {
        let meta = meta.to_owned();
        a.span(|s| {
            s.class("ml-auto mono text-[10.5px] text-ink-400")
                .text(meta)
        });
    }
    a.build()
}

const SIGIL_CMD: &str = "var(--c-cat-green)";
const SIGIL_CMD_INK: &str = "var(--c-cat-green-ink)";
const SIGIL_GRP: &str = "var(--c-cat-lilac)";
const SIGIL_GRP_INK: &str = "var(--c-cat-lilac-ink)";

/// A sidebar nav entry.
pub(crate) struct SidebarEntry {
    /// Sigil background color CSS value.
    pub sigil_bg: &'static str,
    /// Sigil text color CSS value.
    pub sigil_color: &'static str,
    /// Sigil character.
    pub sigil_text: &'static str,
    /// Display name.
    pub name: String,
    /// Link href.
    pub href: String,
    /// Trailing meta text (e.g. "root").
    pub meta: String,
    /// Whether this entry is currently active.
    pub active: bool,
}

/// A collapsible group in the sidebar.
pub(crate) struct SidebarGroup {
    /// Group label.
    pub label: String,
    /// Link href for the group label (navigates on click).
    pub href: Option<String>,
    /// Sigil background color. Defaults to group lilac if `None`.
    pub sigil_bg: Option<&'static str>,
    /// Sigil text color. Defaults to group lilac ink if `None`.
    pub sigil_color: Option<&'static str>,
    /// Sigil character. Defaults to "G" if `None`.
    pub sigil_text: Option<&'static str>,
    /// Whether the group is open.
    pub open: bool,
    /// Override the displayed count. If `None`, uses `children.len()`.
    pub count: Option<usize>,
    /// Child entries.
    pub children: Vec<SidebarEntry>,
}

/// A sidebar item — either a flat entry or a collapsible group.
pub(crate) enum SidebarItem {
    /// A flat nav entry (not inside a group).
    Entry(SidebarEntry),
    /// A collapsible group of entries.
    Group(SidebarGroup),
}

/// Render the version selector as a native `<select>` dropdown.
///
/// `base_url` should end with `/` (e.g. `"/wasi/http/"`). Each option
/// navigates to `{base_url}{version}` on change.
///
/// Returns `None` if `versions` is empty.
pub(crate) fn render_version_selector(
    current_version: &str,
    versions: &[&str],
    base_url: &str,
) -> Option<String> {
    use std::fmt::Write;

    if versions.is_empty() {
        return None;
    }
    let mut options = String::new();
    for (i, v) in versions.iter().enumerate() {
        let selected = if *v == current_version {
            " selected"
        } else {
            ""
        };
        let label = if i == 0 {
            format!("v{v} (latest)")
        } else {
            format!("v{v}")
        };
        let value = escape_html_attr(&format!("{base_url}{v}"));
        let label = escape_html_text(&label);
        let _ = write!(
            options,
            r#"<option value="{value}"{selected}>{label}</option>"#
        );
    }
    let html = Division::builder()
        .division(|l| {
            l.class("mono uppercase tracking-wider text-[10px] text-ink-500 mb-1 flex items-center gap-1")
                .text("Version")
                .text(INFO_BUBBLE)
        })
        .text(format!(
            r#"<select class="w-full h-7 px-2.5 rounded-md border border-line bg-surface text-ink-900 hover:bg-surfaceMuted text-[12px] mono cursor-pointer" onchange="window.location.href=this.value">{options}</select>"#
        ))
        .build()
        .to_string();
    Some(html)
}

/// Render the items navigation tree with an optional section label.
pub(crate) fn render_items_nav(section_label: Option<&str>, items: &[SidebarItem]) -> String {
    let mut nav = Navigation::builder();
    nav.class("space-y-0.5 text-[13px]");

    for item in items {
        match item {
            SidebarItem::Entry(entry) => {
                nav.push(render_entry(entry));
            }
            SidebarItem::Group(group) => {
                nav.push(render_group(group));
            }
        }
    }

    let mut wrapper = Division::builder();

    if let Some(label) = section_label {
        wrapper.division(|l| l.class(SECTION_LABEL_CLASS).text(label.to_owned()));
    }

    wrapper.push(nav.build());
    wrapper.build().to_string()
}

/// Build a nested sidebar matching the DS C01 pattern.
///
/// Renders a card with an optional version button, optional section
/// label, a nav tree of flat entries and collapsible `<details>` groups with
/// colored sigils, and an optional footer section.
pub(crate) fn render_nested_sidebar(
    current_version: &str,
    versions: &[&str],
    section_label: Option<&str>,
    items: &[SidebarItem],
    footer_html: Option<&str>,
) -> String {
    let mut card = Division::builder();
    card.class("max-w-[300px]");

    if let Some(version) = render_version_selector(current_version, versions, "#") {
        card.text(version);
    }

    card.text(render_items_nav(section_label, items));

    // Footer section
    if let Some(footer) = footer_html {
        card.text(footer.to_owned());
    }

    card.build().to_string()
}

/// Render a single flat entry as a tree-link anchor.
fn render_entry(entry: &SidebarEntry) -> html::inline_text::Anchor {
    let entry_sigil = sigil(entry.sigil_bg, entry.sigil_color, entry.sigil_text);
    let cls = if entry.active {
        "tree-link active"
    } else {
        "tree-link"
    };
    let mut a = html::inline_text::Anchor::builder();
    a.href(escape_html_attr(&entry.href)).class(cls);
    a.text(entry_sigil);
    a.span(|s| s.class("mono").text(escape_html_text(&entry.name)));
    if !entry.meta.is_empty() {
        // `meta` is a trusted pre-rendered HTML fragment built by callers
        // (already escaped); embed it verbatim.
        let meta = entry.meta.clone();
        a.span(|s| {
            s.class("ml-auto mono text-[10.5px] text-ink-400")
                .text(meta)
        });
    }
    a.build()
}

/// Render a collapsible group as a `<details>` element.
///
/// The chevron toggles open/close, while the label navigates to `href`
/// (if provided) without toggling.
fn render_group(group: &SidebarGroup) -> Details {
    let bg = group.sigil_bg.unwrap_or(SIGIL_GRP);
    let color = group.sigil_color.unwrap_or(SIGIL_GRP_INK);
    let text = group.sigil_text.unwrap_or("G");
    let grp_sigil = sigil(bg, color, text);
    let count = group.count.unwrap_or(group.children.len());
    let label = &group.label;

    let mut children_html = String::new();
    let has_active = group.open || group.children.iter().any(|e| e.active);

    for entry in &group.children {
        children_html.push_str(&render_entry(entry).to_string());
    }

    let summary_cls = if has_active {
        "tree-link active"
    } else {
        "tree-link"
    };

    let mut details = Details::builder();
    if group.open || has_active {
        details.open(true);
    }

    let children_div = if group.children.is_empty() {
        String::new()
    } else {
        format!(r#"<div class="tree-children space-y-0.5">{children_html}</div>"#)
    };

    // Build the label part — either a navigating <a> or plain spans
    let label = escape_html_text(label);
    let label_html = match &group.href {
        Some(href) => {
            let href = escape_html_attr(href);
            format!(
                r#"<a href="{href}" class="flex items-center gap-1.5 flex-1 min-w-0" onclick="event.stopPropagation()">{grp_sigil}<span class="mono">{label}</span><span class="ml-auto mono text-[10.5px] text-ink-400">{count}</span></a>"#
            )
        }
        None => {
            format!(
                r#"{grp_sigil}<span class="mono">{label}</span><span class="ml-auto mono text-[10.5px] text-ink-400">{count}</span>"#
            )
        }
    };

    details.text(format!(
        r#"<summary class="{summary_cls}">{SVG_CHEV_RIGHT}{label_html}</summary>{children_div}"#,
    ));
    details.build()
}

pub(crate) const SIGIL_LEGEND: &[(&str, &str, &str, &str)] = &[
    (
        "var(--c-cat-green)",
        "var(--c-cat-green-ink)",
        "c",
        "Command",
    ),
    ("var(--c-cat-lilac)", "var(--c-cat-lilac-ink)", "G", "Group"),
    ("var(--c-cat-blue)", "var(--c-cat-blue-ink)", "F", "Flag"),
    ("var(--c-cat-peach)", "var(--c-cat-peach-ink)", "E", "Env"),
    (
        "var(--c-cat-pink)",
        "var(--c-cat-pink-ink)",
        "X",
        "Exit code",
    ),
    (
        "var(--c-cat-plum)",
        "var(--c-cat-plum-ink)",
        "\u{00b7}",
        "Root / misc",
    ),
];

pub(crate) const ANATOMY_ITEMS: &[&str] = &[
    r#"One <code class="mono text-[12px]">&lt;details&gt;</code> per group; rotates the chevron via <code class="mono text-[12px]">details[open]</code>."#,
    r#"Children indent 14px with a 1px <code class="mono text-[12px]">--c-line-soft</code> guide on the left."#,
    r#"Active state uses <code class="mono text-[12px]">--c-surface-muted</code> + medium weight — no border, no accent."#,
    r#"Trailing meta (counts, tags) sits in a <code class="mono text-[12px]">ml-auto</code> slot in 10.5px ink-400 mono."#,
    "Sigil colour signals <em>kind</em>, never status. Use the categorical palette and pair consistently across the surface.",
    "Bottom <strong>Project</strong> section uses lucide-style 14px outline icons (ink-500) instead of sigils \u{2014} reserved for external links (repo, package registry, issues).",
];

/// Render this section.
pub(crate) fn render(
    section_id: &str,
    num: &str,
    title: &str,
    desc: &str,
    sigil_legend: &[(&str, &str, &str, &str)],
    anatomy_items: &[&str],
) -> String {
    // Build demo sidebar using the production function
    let demo_versions: &[&str] = &["2.4.0", "2.3.1", "2.3.0", "2.2.0", "1.0.0"];

    #[allow(clippy::items_after_statements)]
    fn cmd(name: &str) -> SidebarEntry {
        SidebarEntry {
            sigil_bg: SIGIL_CMD,
            sigil_color: SIGIL_CMD_INK,
            sigil_text: "c",
            name: name.to_owned(),
            href: "#".to_owned(),
            meta: String::new(),
            active: false,
        }
    }

    let demo_items = vec![
        SidebarItem::Entry(SidebarEntry {
            sigil_bg: "var(--c-cat-plum)",
            sigil_color: "var(--c-cat-plum-ink)",
            sigil_text: "\u{00b7}",
            name: "component".to_owned(),
            href: "#".to_owned(),
            meta: "root".to_owned(),
            active: false,
        }),
        SidebarItem::Entry(cmd("init")),
        SidebarItem::Entry(cmd("build")),
        SidebarItem::Entry(cmd("run")),
        SidebarItem::Group(SidebarGroup {
            label: "registry".to_owned(),
            href: None,
            sigil_bg: None,
            sigil_color: None,
            sigil_text: None,
            open: true,
            count: Some(7),
            children: vec![cmd("add"), cmd("remove"), cmd("list"), cmd("publish")],
        }),
        SidebarItem::Group(SidebarGroup {
            label: "component".to_owned(),
            href: None,
            sigil_bg: None,
            sigil_color: None,
            sigil_text: None,
            open: false,
            count: Some(5),
            children: vec![],
        }),
        SidebarItem::Group(SidebarGroup {
            label: "wit".to_owned(),
            href: None,
            sigil_bg: None,
            sigil_color: None,
            sigil_text: None,
            open: false,
            count: Some(4),
            children: vec![],
        }),
        SidebarItem::Entry(cmd("help")),
    ];

    // Project links footer
    let footer = Division::builder()
        .class("mt-5 pt-4 border-t-[1.5px] border-rule")
        .division(|l| l.class(SECTION_LABEL_CLASS).text("Project"))
        .push(
            Navigation::builder()
                .class("space-y-px")
                .anchor(|a| {
                    a.href("#".to_owned())
                        .class("tree-link")
                        .text(format!("{SVG_GITHUB} Repository"))
                })
                .anchor(|a| {
                    a.href("#".to_owned())
                        .class("tree-link")
                        .text(format!("{SVG_CRATE} Crates.io"))
                })
                .build(),
        )
        .build()
        .to_string();

    let sidebar_card = render_nested_sidebar(
        "2.4.0",
        demo_versions,
        Some("Commands"),
        &demo_items,
        Some(&footer),
    );

    // Sigil legend
    let mut legend_grid = Division::builder();
    legend_grid.class("flex flex-wrap gap-x-5 gap-y-2 text-[12px]");
    for (bg, color, text, label) in sigil_legend {
        let sigil_html = sigil(bg, color, text);
        let label = (*label).to_owned();
        let entry = Division::builder()
            .class("flex items-center gap-2")
            .text(sigil_html)
            .span(|s| s.class("text-ink-700").text(label))
            .build();
        legend_grid.push(entry);
    }

    // Anatomy UL
    let mut anatomy_ul = html::text_content::UnorderedList::builder();
    anatomy_ul.class(
        "text-[13px] text-ink-700 leading-relaxed space-y-1.5 pl-5 list-disc marker:text-ink-400",
    );
    for item in anatomy_items {
        let item = (*item).to_owned();
        anatomy_ul.list_item(|li| li.paragraph(|p| p.text(item)));
    }

    let content = Division::builder()
        .class("space-y-6")
        // Live demo — rendered by the same production function
        .text(sidebar_card)
        // Sigil legend
        .division(|d| {
            d.division(|l| l.class("text-[12px] text-ink-500 mb-3").text("Sigil kinds"))
                .push(legend_grid.build())
        })
        // Anatomy
        .division(|d| {
            d.division(|l| l.class("text-[12px] text-ink-500 mb-3").text("Anatomy"))
                .push(anatomy_ul.build())
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
            "c-sidebar",
            "C01",
            "Nested Sidebar",
            r#"Hierarchical navigation for reference docs. Top-level entries collapse with native <code class="mono text-[12px]">&lt;details&gt;</code>; sigils classify each row by kind (command, group, flag, env, etc.)."#,
            SIGIL_LEGEND,
            ANATOMY_ITEMS,
        )));
    }

    // r[verify frontend.security.html-escaping]
    #[test]
    fn version_selector_neutralizes_injection() {
        let html = render_version_selector(
            r#"1"><script>alert(1)</script>"#,
            &[r#"1"><script>alert(1)</script>"#],
            "/v/",
        )
        .expect("selector should render");
        assert!(!html.contains("<script>"));
        assert!(!html.contains(r#""><script>"#));
        assert!(html.contains("&lt;script&gt;"));
    }
}
