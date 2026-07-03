//! Search bar component.
//!
//! Renders a search form with an input, optional carousel placeholder,
//! keyboard shortcut badge, and submit button.

use html::text_content::Division;

const SEARCH_ICON: &str = concat!(
    r#"<svg class="absolute left-3.5 top-1/2 -translate-y-1/2 text-ink-400" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">"#,
    include_str!("../../../../../vendor/lucide/search.svg"),
    "</svg>"
);

const ARROW_RIGHT: &str = r#"<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M5 12h14"/><path d="m12 5 7 7-7 7"/></svg>"#;

/// Pre-formatted registry counts shown on the landing search card.
pub(crate) struct LandingStats<'a> {
    /// Total indexed packages (already thousands-formatted).
    pub packages: &'a str,
    /// Distinct namespaces (already thousands-formatted).
    pub namespaces: &'a str,
    /// Total release versions (already thousands-formatted).
    pub versions: &'a str,
}

/// A single example row shown beneath the landing search card. Build one with
/// [`Example::search`] to run a query, or [`Example::link`] to point straight
/// at a page (for example, a namespace). Chain [`Example::struck`] to cross the
/// row out when the destination is not working yet.
pub(crate) struct Example<'a> {
    /// Search query used to build the default `/search?q=…` link.
    query: &'a str,
    /// Human-readable description shown after the "Example:" label.
    description: &'a str,
    /// Explicit link target; overrides the derived search URL when set.
    href: Option<&'a str>,
    /// Cross the row out to signal the example is not working yet.
    struck: bool,
}

impl<'a> Example<'a> {
    /// An example row that runs a search for `query`.
    #[allow(dead_code)]
    pub(crate) fn search(query: &'a str, description: &'a str) -> Self {
        Self {
            query,
            description,
            href: None,
            struck: false,
        }
    }

    /// An example row that links directly to `href` (e.g. a namespace page).
    pub(crate) fn link(href: &'a str, description: &'a str) -> Self {
        Self {
            query: "",
            description,
            href: Some(href),
            struck: false,
        }
    }

    /// Cross the row out and dim it, signalling the example is not working yet.
    /// Typically paired with a dead (`#`) link.
    pub(crate) fn struck(mut self) -> Self {
        self.struck = true;
        self
    }
}

/// Render the landing-page hero search card — a bordered "find a component"
/// panel that anchors the right column of the hero (callout left, search
/// right). The card carries a headline, a federated meta-registry subline, a
/// search input joined to a dark submit button, a stat-bearing "browse all"
/// link, and a short list of examples. Each [`Example`] renders as a clickable
/// row that either runs a search or links straight to a page.
///
/// Submitting the form navigates to `/search?q=...`. The placeholder and the
/// browse link are formatted from the supplied [`LandingStats`].
pub(crate) fn landing_card(stats: &LandingStats<'_>, examples: &[Example<'_>]) -> Division {
    let placeholder = format!("Search {} packages\u{2026}", stats.packages);
    let browse_label = format!(
        "Browse {} packages \u{00b7} {} namespaces \u{00b7} {} versions",
        stats.packages, stats.namespaces, stats.versions
    );
    let rows: Vec<(String, String, bool)> = examples
        .iter()
        .map(|ex| (example_href(ex), ex.description.to_owned(), ex.struck))
        .collect();

    let mut wrapper = Division::builder();
    wrapper.division(|card| {
        card.class("rounded-lg border border-line bg-surface shadow-card p-5 md:p-6")
            .heading_2(|h| {
                h.class("text-[22px] md:text-[24px] font-semibold tracking-tight")
                    .text("Search the meta-registry.".to_owned())
            })
            .form(|form| {
                let placeholder = placeholder.clone();
                form.action("/search")
                    .method("get")
                    .class("mt-5 flex")
                    .division(|field| {
                        field
                            .class("relative flex-1")
                            .text(SEARCH_ICON)
                            .input(|input| {
                                input
                                    .type_("search")
                                    .name("q")
                                    .placeholder(placeholder)
                                    .aria_label("Search packages")
                                    .class("block w-full h-11 pl-10 pr-3 rounded-l-lg border border-r-0 border-line bg-canvas text-[14px] text-ink-900 placeholder:text-ink-400 focus:outline-none focus:border-ink-900")
                            })
                    })
                    .button(|btn| {
                        btn.type_("submit")
                            .class("h-11 px-5 rounded-r-lg bg-ink-900 text-canvas text-[13px] font-medium inline-flex items-center gap-2 hover:opacity-90")
                            .text("Search".to_owned())
                            .text(format!(" {ARROW_RIGHT}"))
                    })
            })
            .division(|secondary| {
                secondary
                    .class("mt-5 pt-4 border-t border-dashed border-lineSoft text-[13px]")
                    .anchor(|a| {
                        a.href("/all")
                            .class("text-ink-700 inline-flex items-center gap-1.5 no-underline hover:text-ink-900 hover:underline")
                            .text(browse_label.clone())
                            .text(format!(" {ARROW_RIGHT}"))
                    })
            })
    });

    if !rows.is_empty() {
        push_example_queries(&mut wrapper, &rows);
    }

    wrapper.build()
}

/// Resolve the link target for an example row: an explicit `href` when set,
/// otherwise a `/search?q=…` link derived from the query.
fn example_href(example: &Example<'_>) -> String {
    match example.href {
        Some(href) => href.to_owned(),
        None => format!("/search?q={}", encode_query(example.query)),
    }
}

fn push_example_queries(
    wrapper: &mut html::text_content::builders::DivisionBuilder,
    rows: &[(String, String, bool)],
) {
    wrapper.division(|list| {
        let mut list = list.class("mt-5 space-y-2.5");
        for (i, (href, description, struck)) in rows.iter().enumerate() {
            let num = format!("{:02}", i + 1);
            let href = href.clone();
            let description = description.clone();
            // A crossed-out row dims the text and drops the hover emphasis,
            // marking the example as not working yet (see `Example::struck`).
            let text_class = if *struck {
                "text-ink-500 line-through decoration-1"
            } else {
                "text-ink-700 group-hover:text-ink-900"
            };
            let desc_class = if *struck { "" } else { "group-hover:underline" };
            list = list.anchor(|row| {
                row.href(href)
                    .class("flex gap-3 text-[13px] no-underline group")
                    .span(|n| n.class("mono tabular-nums text-ink-400").text(num))
                    .span(|t| {
                        t.class(text_class)
                            .span(|e| e.class("text-ink-400").text("Example: ".to_owned()))
                            .span(|d| d.class(desc_class).text(description))
                    })
            });
        }
        list
    });
}

/// Minimal percent-encoding for an example query placed in a `q=` parameter.
fn encode_query(q: &str) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(q.len());
    for b in q.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            b' ' => out.push('+'),
            _ => {
                let _ = write!(out, "%{b:02X}");
            }
        }
    }
    out
}

/// Configuration for the search bar.
#[allow(dead_code)]
pub(crate) struct SearchBar {
    /// Current query value (empty for no pre-fill).
    pub query: String,
    /// HTML id for the input element (for focus-on-/ shortcut).
    pub input_id: &'static str,
    /// Whether to show the animated carousel placeholder.
    pub carousel: bool,
}

impl Default for SearchBar {
    fn default() -> Self {
        Self {
            query: String::new(),
            input_id: "search-input",
            carousel: false,
        }
    }
}

/// Render a compact search bar for nav / inline use.
///
/// 36px tall, border + surface background, `/` kbd badge.
#[allow(dead_code)]
pub(crate) fn compact(input_id: &str) -> Division {
    Division::builder()
        .form(|form| {
            form.action("/search")
                .method("get")
                .class("relative flex search-form")
                .input(|input| {
                    input
                        .type_("search")
                        .name("q")
                        .placeholder("Search\u{2026}")
                        .aria_label("Search")
                        .id(input_id.to_owned())
                        .class("w-full sm:w-48 h-9 px-3 pr-10 rounded-md border border-line bg-surface text-[14px] text-ink-900 placeholder:text-ink-400 focus:outline-none focus:border-ink-900")
                })
                .span(|kbd| {
                    kbd.class("search-kbd")
                        .aria_hidden(true)
                        .text("/".to_owned())
                })
        })
        .build()
}

/// Render the hero search bar with carousel placeholder and submit button.
#[allow(dead_code)]
pub(crate) fn hero(cfg: &SearchBar) -> Division {
    let mut wrapper = Division::builder();
    wrapper.form(|form| {
        form.action("/search")
            .method("get")
            .class("flex flex-1 max-w-lg search-form")
            .division(|inner| {
                inner
                    .class("flex-1 relative")
                    .input(|input| {
                        let mut i = input
                            .type_("search")
                            .name("q")
                            .id(cfg.input_id.to_owned())
                            .aria_label("Search components and interfaces")
                            .autofocus(true)
                            .class("w-full h-10 pl-10 pr-8 rounded-l-lg border border-line bg-canvas text-[14px] text-ink-900 placeholder:text-ink-400 focus:outline-none focus:border-ink-900");
                        if !cfg.query.is_empty() {
                            i = i.value(cfg.query.clone());
                        }
                        i
                    });
                if cfg.carousel {
                    inner
                        .span(|overlay| {
                            overlay
                                .id("search-carousel")
                                .class("search-carousel")
                                .aria_hidden(true)
                                .span(|prefix| prefix.text("Search ".to_owned()))
                                .span(|word| {
                                    word.id("carousel-word")
                                        .class("carousel-word")
                                        .text("components\u{2026}")
                                })
                        });
                }
                inner
            })
            .button(|btn| {
                btn.type_("submit")
                    .class("h-10 px-4 rounded-r-md border-[1.5px] border-l-0 border-ink-900 bg-surface text-ink-900 text-[13px] font-medium hover:bg-surfaceMuted")
                    .text("Search")
            })
    });
    wrapper.build()
}

/// Render a simple inline search form (for search results page refinement).
pub(crate) fn inline(query: &str) -> Division {
    Division::builder()
        .class("mb-8")
        .form(|form| {
            form.class("flex gap-2")
                .method("get")
                .action("/search")
                .input(|input| {
                    input
                        .type_("search")
                        .name("q")
                        .value(crate::escape::escape_html_attr(query))
                        .placeholder("Search\u{2026}")
                        .class("flex-1 h-9 px-3 rounded-md border border-line bg-surface text-[14px] text-ink-900 placeholder:text-ink-400 focus:outline-none focus:border-ink-900")
                })
                .button(|btn| {
                    btn.type_("submit")
                        .class("h-9 px-4 rounded-md bg-ink-900 text-canvas text-[13px] font-medium hover:bg-ink-700 transition-colors")
                        .text("Search")
                })
        })
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn landing_card_renders_search_form_and_notes() {
        let html = landing_card(
            &LandingStats {
                packages: "1 248",
                namespaces: "73",
                versions: "4 902",
            },
            &[
                Example::search("wasi:http", "Find components that handle HTTP requests"),
                Example::link("/wasi", "Browse the standard WASI interfaces"),
            ],
        )
        .to_string();
        // Submits a `q` query to the search endpoint.
        assert!(html.contains(r#"action="/search""#));
        assert!(html.contains(r#"method="get""#));
        assert!(html.contains(r#"name="q""#));
        // Carries the formatted placeholder and the stat-bearing browse link.
        assert!(html.contains("Search 1 248 packages"));
        assert!(html.contains(r#"href="/all""#));
        assert!(
            html.contains("Browse 1 248 packages \u{00b7} 73 namespaces \u{00b7} 4 902 versions")
        );
        // Example queries are numbered, link to their target, and show copy.
        assert!(html.contains(">01<"));
        assert!(html.contains(">02<"));
        // A `search` example links to the encoded search URL.
        assert!(html.contains(r#"href="/search?q=wasi%3Ahttp""#));
        // A `link` example links straight to the given path.
        assert!(html.contains(r#"href="/wasi""#));
        assert!(html.contains("Find components that handle HTTP requests"));
        assert!(html.contains("Browse the standard WASI interfaces"));
    }

    #[test]
    fn struck_example_is_crossed_out() {
        let html = landing_card(
            &LandingStats {
                packages: "1 248",
                namespaces: "73",
                versions: "4 902",
            },
            &[Example::link("#", "Find components that handle HTTP requests").struck()],
        )
        .to_string();
        // Points at a dead link and crosses the description out. Assert the
        // exact struck class attribute rather than a loose global substring.
        assert!(html.contains(r##"href="#""##));
        assert!(html.contains(r#"class="text-ink-500 line-through decoration-1""#));
    }
}
