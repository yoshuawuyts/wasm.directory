//! C03 — Page Header.

use html::text_content::Division;

const SVG_COPY: &str = concat!(
    r#"<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">"#,
    include_str!("../../../../../vendor/lucide/copy.svg"),
    "</svg>"
);

pub(crate) const ANATOMY_ITEMS: &[&str] = &[
    "<strong>Kicker</strong> \u{2014} 12px ink-500 mono uppercase, dot-separated tokens (version, category, format). Sets context before the title.",
    "<strong>Title</strong> \u{2014} 36/44px semibold, tight tracking, leading 1.05. The page\u{2019}s anchor.",
    "<strong>Tagline</strong> \u{2014} 15px ink-700, max-w-2xl, single paragraph. One sentence describing what the page <em>is</em>, not what it does.",
    "<strong>Metadata strip</strong> (optional) \u{2014} labelled key/value pairs separated by horizontal gap, wrapping to a new row at narrow widths. Each label is 11px ink-500 mono uppercase; each value uses the appropriate token (mono pill, link, or status badge).",
    r#"Vertical rhythm: <code class="mono text-[12px]">pt-8 md:pt-12 pb-8 md:pb-12</code>, with a strong <code class="mono text-[12px]">.rule</code> divider beneath separating the header from page content."#,
];

#[allow(dead_code)]
/// Render a page header with kicker, title, tagline, and optional metadata strip.
pub(crate) fn page_header_block(
    kicker_html: &str,
    title: &str,
    tagline: &str,
    metadata_html: Option<&str>,
) -> Division {
    let kicker_html = kicker_html.to_owned();
    let title = title.to_owned();
    let tagline = tagline.to_owned();
    let mut div = Division::builder();
    div.class("pb-10 border-b border-line");
    div.division(|d| {
        d.class("flex items-center gap-2 text-[12px] text-ink-500 mono uppercase tracking-wider")
            .text(kicker_html)
    });
    div.heading_1(|h| {
        h.class("mt-3 text-[36px] md:text-[44px] leading-[1.05] font-semibold tracking-tight")
            .text(title)
    });
    div.paragraph(|p| {
        p.class("mt-3 max-w-2xl text-[15px] text-ink-700 leading-relaxed")
            .text(tagline)
    });
    if let Some(meta) = metadata_html {
        let meta = meta.to_owned();
        div.division(|d| d.class("mt-6").text(meta));
    }
    div.build()
}

/// Render this section.
pub(crate) fn render(
    section_id: &str,
    num: &str,
    title: &str,
    desc: &str,
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
        .class("space-y-10")
        // Package demo
        .division(|d| {
            d.division(|l| l.class("text-[12px] text-ink-500 mb-3").text("Package \u{00b7} with install command"))
                .division(|card| {
                    card.class("border border-line rounded-lg bg-canvas px-6 py-8")
                        // Kicker
                        .division(|kicker| {
                            kicker.class("flex items-center gap-2 text-[12px] text-ink-500 mono uppercase tracking-wider")
                                .span(|s| s.text("v0.4.2"))
                                .span(|s| s.class("h-1 w-1 rounded-full bg-ink-300"))
                                .span(|s| s.text("wasi:http"))
                                .span(|s| s.class("h-1 w-1 rounded-full bg-ink-300"))
                                .span(|s| s.text("Apache-2.0"))
                        })
                        // Title
                        .heading_1(|h| {
                            h.class("mt-3 text-[36px] md:text-[44px] leading-[1.05] font-semibold tracking-tight")
                                .text("wasi-http-handler")
                        })
                        // Tagline
                        .paragraph(|p| {
                            p.class("mt-3 max-w-2xl text-[15px] text-ink-700 leading-relaxed")
                                .text("A composable HTTP request handler component implementing the ")
                                .span(|s| s.class("mono text-[13px]").text("wasi:http/incoming-handler"))
                                .text(" interface. Drop-in compatible with any wasi:http host.")
                        })
                        // Metadata strip
                        .division(|strip| {
                            strip.class("mt-6 flex flex-wrap items-center gap-x-6 gap-y-3 text-[13px]")
                                .division(|install| {
                                    install.class("inline-flex items-center gap-2")
                                        .span(|s| s.class("text-[11px] mono uppercase tracking-wider text-ink-500").text("Install"))
                                        .division(|cmd| {
                                            cmd.class("flex")
                                                .span(|s| {
                                                    s.class("inline-flex items-center px-2.5 h-7 rounded-l-md border border-r-0 border-line bg-surfaceMuted text-[12.5px] text-ink-500 mono select-none")
                                                        .aria_hidden(true)
                                                        .text("$")
                                                })
                                                .code(|c| {
                                                    c.class("inline-flex items-center px-2.5 h-7 border border-line bg-surface mono text-[12.5px] text-ink-900 whitespace-nowrap")
                                                        .text("component install wasi:http-handler")
                                                })
                                                .button(|b| {
                                                    b.type_("button")
                                                        .class("inline-flex items-center justify-center w-7 h-7 rounded-r-md border border-l-0 border-line bg-surface text-ink-500 hover:text-ink-900 hover:bg-surfaceMuted focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-[-2px] focus-visible:outline-ink-900")
                                                        .aria_label("Copy install command".to_owned())
                                                        .text(SVG_COPY)
                                                })
                                        })
                                })
                        })
                })
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
            "c-page-header",
            "C03",
            "Page Header",
            "Top-of-page identification block: a kicker, a large title, an optional tagline, and an optional metadata strip. Used to anchor reference and documentation pages.",
            ANATOMY_ITEMS,
        )));
    }
}
