//! Documentation pages.
//!
//! Renders the markdown documents from the repository's top-level `docs/`
//! directory. The files are embedded into the binary at compile time via
//! `include_str!`, so the rendered docs are always in sync with the
//! repository's documentation.

use html::text_content::Division;

use crate::layout;
use crate::markdown;

/// A documentation page exposed at `/docs/<slug>`.
struct DocPage {
    /// URL slug (e.g. `"architecture"`).
    slug: &'static str,
    /// Human-readable title shown in the index and sidebar.
    title: &'static str,
    /// Raw markdown contents, embedded at compile time.
    markdown: &'static str,
}

/// All sub-pages of the documentation, in display order.
const PAGES: &[DocPage] = &[
    DocPage {
        slug: "architecture",
        title: "Architecture",
        markdown: include_str!("../../../../docs/architecture.md"),
    },
    DocPage {
        slug: "authentication",
        title: "Authentication",
        markdown: include_str!("../../../../docs/authentication.md"),
    },
    DocPage {
        slug: "configuration",
        title: "Configuration",
        markdown: include_str!("../../../../docs/configuration.md"),
    },
    DocPage {
        slug: "github-action",
        title: "GitHub Action",
        markdown: include_str!("../../../../docs/github-action.md"),
    },
    DocPage {
        slug: "usage",
        title: "Usage",
        markdown: include_str!("../../../../docs/usage.md"),
    },
];

/// Raw markdown contents of the docs index (`docs/README.md`).
const INDEX_MARKDOWN: &str = include_str!("../../../../docs/README.md");

/// Render the documentation index page (`/docs`).
#[must_use]
pub(crate) fn render() -> String {
    render_markdown_page("Docs", INDEX_MARKDOWN)
}

/// Render an individual documentation page (`/docs/<slug>`).
///
/// Returns `None` if no page with the given slug exists.
#[must_use]
pub(crate) fn render_page(slug: &str) -> Option<String> {
    let page = PAGES.iter().find(|p| p.slug == slug)?;
    Some(render_markdown_page(page.title, page.markdown))
}

/// Render a markdown document as a full documentation page.
fn render_markdown_page(title: &str, source: &str) -> String {
    let rewritten = rewrite_doc_links(source);
    let rendered_block = markdown::render_block(&rewritten, markdown::DOC_CLASS);

    let body = Division::builder()
        .class("pt-8 max-w-[65ch]")
        .text(rendered_block)
        .division(|d| {
            d.class("mt-12 pt-8 border-t border-lineSoft")
                .text(nav_html())
        })
        .build();

    layout::document_with_nav(title, &body.to_string())
}

/// Render the bottom-of-page navigation linking to all docs.
fn nav_html() -> String {
    use std::fmt::Write;

    let mut out = String::from(
        r#"<p class="text-[11px] uppercase tracking-wider text-ink-500 mb-2">More docs</p><ul class="flex flex-wrap gap-x-4 gap-y-1 text-[14px]">"#,
    );
    out.push_str(r#"<li><a class="text-accent underline underline-offset-2 hover:opacity-80" href="/docs">Index</a></li>"#);
    for page in PAGES {
        write!(
            out,
            r#"<li><a class="text-accent underline underline-offset-2 hover:opacity-80" href="/docs/{slug}">{title}</a></li>"#,
            slug = page.slug,
            title = page.title,
        )
        .expect("writing to a String never fails");
    }
    out.push_str("</ul>");
    out
}

/// Rewrite `*.md` link destinations in markdown source to clean URLs
/// served by the `/docs/{slug}` route.
///
/// For example, `](architecture.md)` becomes `](/docs/architecture)`. Only
/// links to known doc pages (or the index `README.md`) are rewritten —
/// everything else is left untouched.
fn rewrite_doc_links(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut rest = source;
    while let Some(idx) = rest.find("](") {
        out.push_str(&rest[..idx + 2]);
        rest = &rest[idx + 2..];
        let Some(end) = rest.find(')') else {
            break;
        };
        let dest = &rest[..end];
        let replaced = rewrite_dest(dest);
        out.push_str(&replaced);
        out.push(')');
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    out
}

/// Rewrite a single link destination if it points to a known doc page.
///
/// Preserves any trailing `#fragment` or `?query` suffix so links such as
/// `configuration.md#credential-helpers` are rewritten to
/// `/docs/configuration#credential-helpers`.
fn rewrite_dest(dest: &str) -> String {
    // Don't rewrite absolute URLs or anchors.
    if dest.starts_with("http://")
        || dest.starts_with("https://")
        || dest.starts_with('/')
        || dest.starts_with('#')
    {
        return dest.to_owned();
    }
    // Split off any `#fragment` or `?query` suffix; rewrite only the path.
    let suffix_start = dest.find(['#', '?']).unwrap_or(dest.len());
    let (path, suffix) = dest.split_at(suffix_start);
    if path.eq_ignore_ascii_case("README.md") {
        return format!("/docs{suffix}");
    }
    if let Some(stem) = path.strip_suffix(".md")
        && PAGES.iter().any(|p| p.slug == stem)
    {
        return format!("/docs/{stem}{suffix}");
    }
    dest.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites_known_md_links() {
        let input = "See [Authentication](authentication.md) for details.";
        let out = rewrite_doc_links(input);
        assert_eq!(
            out,
            "See [Authentication](/docs/authentication) for details."
        );
    }

    #[test]
    fn rewrites_readme_link() {
        let input = "[Index](README.md)";
        let out = rewrite_doc_links(input);
        assert_eq!(out, "[Index](/docs)");
    }

    #[test]
    fn leaves_external_links_alone() {
        let input = "[GitHub](https://github.com/x/y) and [anchor](#foo)";
        let out = rewrite_doc_links(input);
        assert_eq!(out, input);
    }

    #[test]
    fn leaves_unknown_md_links_alone() {
        let input = "[Other](unknown.md)";
        let out = rewrite_doc_links(input);
        assert_eq!(out, input);
    }

    #[test]
    fn rewrites_md_link_with_fragment() {
        let input = "See [Helpers](configuration.md#credential-helpers).";
        let out = rewrite_doc_links(input);
        assert_eq!(
            out,
            "See [Helpers](/docs/configuration#credential-helpers)."
        );
    }

    #[test]
    fn rewrites_md_link_with_query() {
        let input = "[Cfg](configuration.md?x=1)";
        let out = rewrite_doc_links(input);
        assert_eq!(out, "[Cfg](/docs/configuration?x=1)");
    }

    #[test]
    fn rewrites_readme_link_with_fragment() {
        let input = "[Index](README.md#top)";
        let out = rewrite_doc_links(input);
        assert_eq!(out, "[Index](/docs#top)");
    }

    #[test]
    fn leaves_unknown_md_link_with_fragment_alone() {
        let input = "[Other](unknown.md#foo)";
        let out = rewrite_doc_links(input);
        assert_eq!(out, input);
    }

    #[test]
    fn render_index_contains_doc_links() {
        let html = render();
        assert!(html.contains("/docs/architecture"));
        assert!(html.contains("/docs/usage"));
    }

    #[test]
    fn render_page_returns_some_for_known_slug() {
        let html = render_page("architecture").expect("known slug");
        assert!(html.contains("Architecture"));
    }

    #[test]
    fn usage_guide_uses_platform_installer_urls() {
        let html = render_page("usage").expect("usage guide should be available");
        for command in [
            "curl -fsSL https://wasm.directory/install/linux | sh",
            "curl -fsSL https://wasm.directory/install/macos | sh",
            "irm https://wasm.directory/install/windows | iex",
        ] {
            assert!(html.contains(command), "missing install command {command}");
        }
    }

    #[test]
    fn render_page_returns_none_for_unknown_slug() {
        assert!(render_page("does-not-exist").is_none());
    }
}
