//! C04 — Item List.

use crate::escape::{escape_html_attr, escape_html_text};
use html::inline_text::Anchor;
use html::text_content::Division;

pub(crate) struct ItemRow {
    pub(crate) sigil_bg: &'static str,
    pub(crate) sigil_color: &'static str,
    pub(crate) sigil_text: &'static str,
    pub(crate) name: &'static str,
    pub(crate) desc: &'static str,
    pub(crate) meta: &'static str,
    pub(crate) deprecated: bool,
}

/// Dynamic item row with owned strings, for use with runtime data.
pub(crate) struct DynItemRow {
    pub(crate) sigil_bg: String,
    pub(crate) sigil_color: String,
    pub(crate) sigil_text: String,
    pub(crate) name: String,
    pub(crate) href: String,
    /// Raw Markdown excerpt, rendered once as a non-interactive summary.
    pub(crate) desc: String,
    pub(crate) version: String,
    pub(crate) meta: String,
    pub(crate) meta_title: String,
    pub(crate) deprecated: bool,
    /// Optional HTML id for anchor targeting.
    pub(crate) id: Option<String>,
}

#[allow(dead_code)]
pub(crate) fn render_item_row(item: &ItemRow) -> Anchor {
    let row_class = if item.deprecated {
        "item-row deprecated"
    } else {
        "item-row"
    };
    let sigil_style = format!("background:{};color:{};", item.sigil_bg, item.sigil_color);
    Anchor::builder()
        .href("#c-item-list".to_owned())
        .class(row_class)
        .span(|s| s.class("sigil").style(sigil_style).text(item.sigil_text))
        .division(|d| {
            d.span(|s| s.class("name").text(item.name))
                .division(|desc| desc.class("desc").text(item.desc))
        })
        .span(|s| s.class("meta").text(item.meta))
        .build()
}

#[allow(dead_code)]
pub(crate) fn render_item_list(items: &[ItemRow]) -> Division {
    let mut list = Division::builder();
    list.class("item-list");
    for item in items {
        list.push(render_item_row(item));
    }
    list.build()
}

/// Render a dynamic item row (owned strings, real href).
pub(crate) fn render_dyn_item_row(item: &DynItemRow) -> Anchor {
    let row_class = if item.deprecated {
        "item-row deprecated"
    } else {
        "item-row"
    };
    let _sigil_style = format!("background:{};color:{};", item.sigil_bg, item.sigil_color);
    let sigil_text = escape_html_text(&item.sigil_text);
    let name = escape_html_text(&item.name);
    let desc = crate::markdown::render_summary(&item.desc);
    let meta = escape_html_text(&item.meta);
    let href = escape_html_attr(&item.href);
    // Raw HTML: Span::style() creates a <style> child, not an inline style attribute.
    // `sigil_bg`/`sigil_color` are design-system palette constants (trusted).
    let sigil_html = format!(
        r#"<span class="sigil" style="background:{};color:{};">{sigil_text}</span>"#,
        item.sigil_bg, item.sigil_color
    );
    let mut a = Anchor::builder();
    a.href(href).class(row_class);
    if let Some(id) = &item.id {
        a.id(escape_html_attr(id));
    }
    let version_tag = if item.version.is_empty() {
        String::new()
    } else {
        let v = escape_html_text(&item.version);
        format!(
            r#" <span class="inline-flex items-center px-1.5 h-5 rounded border border-line text-[10px] mono text-ink-500 ml-2 align-middle" title="The version of the item we're targeting">{v}</span>"#
        )
    };
    let name_html = format!(r#"<span class="name">{name}</span>{version_tag}"#);
    a.text(sigil_html).division(|d| {
        d.text(name_html)
            .division(|dd| dd.class("desc prose-doc").text(desc))
    });
    if !meta.is_empty() {
        let meta_title = escape_html_attr(&item.meta_title);
        let meta_tag = format!(
            r#"<span class="meta inline-flex items-center px-1.5 h-5 rounded border border-line text-[10px] mono text-ink-500" title="{meta_title}">{meta}</span>"#
        );
        a.text(meta_tag);
    }
    a.build()
}

/// Render a list of dynamic item rows.
pub(crate) fn render_dyn_item_list(title: &str, items: &[DynItemRow]) -> Division {
    let title = escape_html_text(title);
    let mut list_html = String::new();
    list_html.push_str(r#"<div class="item-list">"#);
    for item in items {
        list_html.push_str(&render_dyn_item_row(item).to_string());
    }
    list_html.push_str("</div>");

    let chevron = r#"<svg class="h-4 w-4 text-ink-400 transition-transform duration-200 rotate-90 group-open:rotate-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="m6 9 6 6 6-6"/></svg>"#;

    let mut wrapper = Division::builder();
    wrapper.text(format!(
        r#"<details open class="group space-y-3"><summary class="flex items-center justify-between cursor-pointer list-none [&::-webkit-details-marker]:hidden"><h2 class="text-[22px] font-semibold tracking-tight text-ink-700">{title}</h2>{chevron}</summary>{list_html}</details>"#
    ));
    wrapper.build()
}

pub(crate) const CMD_ROWS: &[ItemRow] = &[
    ItemRow {
        sigil_bg: "var(--c-cat-green)",
        sigil_color: "var(--c-cat-green-ink)",
        sigil_text: "c",
        name: "add",
        desc: "Register a new namespace and point it at an OCI registry URL.",
        meta: "1.2.0",
        deprecated: false,
    },
    ItemRow {
        sigil_bg: "var(--c-cat-green)",
        sigil_color: "var(--c-cat-green-ink)",
        sigil_text: "c",
        name: "remove",
        desc: "Forget a registered namespace. Locally cached artifacts are kept.",
        meta: "1.2.0",
        deprecated: false,
    },
    ItemRow {
        sigil_bg: "var(--c-cat-green)",
        sigil_color: "var(--c-cat-green-ink)",
        sigil_text: "c",
        name: "list",
        desc: "Print the effective set of registries, with the source of each entry.",
        meta: "1.2.0",
        deprecated: false,
    },
    ItemRow {
        sigil_bg: "var(--c-cat-green)",
        sigil_color: "var(--c-cat-green-ink)",
        sigil_text: "c",
        name: "login",
        desc: "Store credentials for a registry in the OS keychain.",
        meta: "1.4.0",
        deprecated: false,
    },
    ItemRow {
        sigil_bg: "var(--c-cat-green)",
        sigil_color: "var(--c-cat-green-ink)",
        sigil_text: "c",
        name: "publish",
        desc: "Build and upload the current package to a registry.",
        meta: "1.3.0",
        deprecated: false,
    },
    ItemRow {
        sigil_bg: "var(--c-cat-green)",
        sigil_color: "var(--c-cat-green-ink)",
        sigil_text: "c",
        name: "login-token",
        desc: "Legacy token-only login. Removed in 2.0 \u{2014} use <span class=\"mono text-[12px]\">login --password-stdin</span> instead.",
        meta: "deprecated",
        deprecated: true,
    },
];

pub(crate) const ENDPOINT_ROWS: &[ItemRow] = &[
    ItemRow {
        sigil_bg: "var(--c-cat-blue)",
        sigil_color: "var(--c-cat-blue-ink)",
        sigil_text: "G",
        name: "/v1/packages/{name}",
        desc: "Resolve a package by canonical name. Returns the latest version metadata.",
        meta: "v1",
        deprecated: false,
    },
    ItemRow {
        sigil_bg: "var(--c-cat-blue)",
        sigil_color: "var(--c-cat-blue-ink)",
        sigil_text: "G",
        name: "/v1/packages/{name}/versions",
        desc: "List every published version of a package, newest first.",
        meta: "v1",
        deprecated: false,
    },
    ItemRow {
        sigil_bg: "var(--c-cat-green)",
        sigil_color: "var(--c-cat-green-ink)",
        sigil_text: "P",
        name: "/v1/packages/{name}",
        desc: "Publish a new version. Body is a streaming OCI image manifest.",
        meta: "v1",
        deprecated: false,
    },
    ItemRow {
        sigil_bg: "var(--c-cat-pink)",
        sigil_color: "var(--c-cat-pink-ink)",
        sigil_text: "D",
        name: "/v1/packages/{name}/versions/{ver}",
        desc: "Yank a version. The artifact remains, but it stops resolving by default.",
        meta: "v1",
        deprecated: false,
    },
];

pub(crate) const ANATOMY_ITEMS: &[&str] = &[
    r#"The whole row is the link — <code class="mono text-[12px]">&lt;a class="item-row"&gt;</code> — so the entire chip is the click target. The inner <code class="mono text-[12px]">.name</code> stays a <code class="mono text-[12px]">&lt;span&gt;</code>."#,
    r#"Three-column grid: <code class="mono text-[12px]">sigil · name+desc · meta</code>; the middle column is the only flexible one."#,
    r#"The whole list sits in a <code class="mono text-[12px]">rounded-lg border border-line bg-canvas</code> card — same surface treatment as the search trigger and Item Details. Rows separate with a 1px <code class="mono text-[12px]">--c-line-soft</code> hairline inside the card; the first row drops it."#,
    "Name is 13.5px mono ink-900 medium; underlines on hover only. Description is 13px ink-700, one line, no wrapping pressure.",
    r#"Trailing <code class="mono text-[12px]">.meta</code> stays mono ink-500 — version, status, or count. Drop it entirely if there's nothing to show."#,
    "Sigil colour follows the C01 convention so the same kind reads consistently across sidebar and item list.",
    r#"<strong>Hover</strong> tints the entire row to <code class="mono text-[12px]">--c-surface-muted</code> — the only affordance that the row is interactive. No border or accent change."#,
    r#"<strong>Deprecated</strong> rows fade name + description to ink-400, strike through the name with a 1px line, and drop the sigil opacity to 50%. Set the trailing <code class="mono text-[12px]">.meta</code> to a one-word state ("deprecated", "removed") instead of a version."#,
];

/// Render this section.
pub(crate) fn render(
    section_id: &str,
    num: &str,
    title: &str,
    desc: &str,
    cmd_rows: &[ItemRow],
    endpoint_rows: &[ItemRow],
    anatomy_items: &[&str],
) -> String {
    let mut anatomy_ul = html::text_content::UnorderedList::builder();
    anatomy_ul.class(
        "text-[13px] text-ink-700 leading-relaxed space-y-1.5 pl-5 list-disc marker:text-ink-400",
    );
    for item in anatomy_items {
        let item = (*item).to_owned();
        anatomy_ul.list_item(|li| li.paragraph(|p| p.text(item)));
    }

    let content = Division::builder()
        .class("space-y-8")
        // Subcommands demo
        .division(|d| {
            d.division(|l| {
                l.class("text-[12px] text-ink-500 mb-3")
                    .text("Subcommands of ")
                    .span(|s| s.class("mono").text("component registry"))
            })
            .push(render_item_list(cmd_rows))
        })
        // Endpoints demo
        .division(|d| {
            d.division(|l| {
                l.class("text-[12px] text-ink-500 mb-3")
                    .text("Endpoints under ")
                    .span(|s| s.class("mono").text("/v1/packages"))
            })
            .push(render_item_list(endpoint_rows))
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
            "c-item-list",
            "C04",
            "Item List",
            "Compact index of a group\u{2019}s children \u{2014} subcommands, endpoints, schemas. Each row is a sigil, a name + one-line description, and trailing meta. Rows separate with hairline rules, no card chrome.",
            CMD_ROWS,
            ENDPOINT_ROWS,
            ANATOMY_ITEMS,
        )));
    }

    fn malicious_row() -> DynItemRow {
        DynItemRow {
            sigil_bg: "var(--c-cat-green)".to_owned(),
            sigil_color: "var(--c-cat-green-ink)".to_owned(),
            sigil_text: "c".to_owned(),
            name: "</span><script>alert(1)</script>".to_owned(),
            href: r#"/x"><img src=x onerror=alert(1)>"#.to_owned(),
            desc: "<script>alert(2)</script>".to_owned(),
            version: r#"<img src=x onerror=alert(3)>"#.to_owned(),
            meta: "<b>m</b>".to_owned(),
            meta_title: r#""><script>alert(4)</script>"#.to_owned(),
            deprecated: false,
            id: Some(r#"x"><script>alert(5)</script>"#.to_owned()),
        }
    }

    // r[verify frontend.security.html-escaping]
    #[test]
    fn dyn_item_row_neutralizes_injection() {
        let html = render_dyn_item_row(&malicious_row()).to_string();
        assert!(!html.contains("<script>"));
        assert!(!html.contains("<img"));
        assert!(html.contains("&lt;script&gt;"));
    }

    #[test]
    fn dyn_item_list_escapes_title() {
        let html = render_dyn_item_list("</h2><script>alert(1)</script>", &[]).to_string();
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;"));
    }

    #[test]
    fn dyn_item_row_renders_raw_markdown_once_without_changing_navigation() {
        let mut item = malicious_row();
        item.href = "/example/item?one=1&two=2".to_owned();
        item.desc = "A `value` and [**link**](https://example.com).\n\nLater.".to_owned();
        let html = render_dyn_item_row(&item).to_string();
        assert!(html.contains("<code>value</code>"));
        assert!(html.contains("<strong>link</strong>"));
        assert!(!html.contains("&lt;code&gt;"));
        assert!(!html.contains("Later."));
        assert_eq!(html.matches("<a ").count(), 1);
        assert!(html.contains(r#"href="/example/item?one=1&amp;two=2""#));
        assert!(html.contains(r#"class="desc prose-doc""#));
        assert!(!html.contains("<script>"));
    }
}
