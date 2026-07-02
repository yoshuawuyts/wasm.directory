//! Call-to-action strip used as the pre-footer band on the landing page.
//!
//! Top hairline, two-column layout: kicker + headline + paragraph on the
//! left, primary + secondary CTA buttons on the right.

use html::content::Section;
use html::text_content::builders::DivisionBuilder;

/// Configuration for [`render`].
pub(crate) struct CtaStrip<'a> {
    pub kicker: &'a str,
    pub title: &'a str,
    /// HTML body — callers may include `<code>` spans inline.
    pub body_html: &'a str,
    pub primary_label: &'a str,
    /// Primary CTA destination. `None` renders a disabled, muted button that
    /// stays visible but is not navigable.
    pub primary_href: Option<&'a str>,
    pub secondary_label: &'a str,
    /// Secondary CTA destination. `None` renders a disabled, muted button that
    /// stays visible but is not navigable.
    pub secondary_href: Option<&'a str>,
}

const ARROW_RIGHT: &str = r#"<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M5 12h14"/><path d="m12 5 7 7-7 7"/></svg>"#;

/// Render the CTA strip.
#[must_use]
pub(crate) fn render(cfg: &CtaStrip<'_>) -> String {
    let kicker = cfg.kicker.to_owned();
    let title = cfg.title.to_owned();
    let body = cfg.body_html.to_owned();
    let plabel = cfg.primary_label.to_owned();
    let phref = cfg.primary_href.map(ToOwned::to_owned);
    let slabel = cfg.secondary_label.to_owned();
    let shref = cfg.secondary_href.map(ToOwned::to_owned);

    Section::builder()
        .class("mx-auto mx-auto max-w-[1280px] w-full px-4 md:px-8 mt-12 md:mt-16")
        .division(|grid| {
            grid.class("grid md:grid-cols-[1fr_auto] gap-6 items-center border-t border-lineSoft pt-10 md:pt-12")
                .division(|left| {
                    left.division(|d| {
                        d.class("text-[12px] mono uppercase tracking-wider text-ink-500")
                            .text(kicker)
                    })
                    .heading_3(|h| {
                        h.class("mt-2 text-[24px] font-semibold tracking-tight").text(title)
                    })
                    .paragraph(|p| {
                        p.class("mt-2 max-w-xl text-[13px] text-ink-700 leading-relaxed")
                            .text(body)
                    })
                })
                .division(|right| {
                    let right = right.class("flex flex-wrap items-center gap-3 md:justify-end");
                    push_primary(right, plabel, phref);
                    push_secondary(right, slabel, shref);
                    right
                })
        })
        .build()
        .to_string()
}

/// Append the primary (filled) CTA. A `None` href renders a disabled, muted,
/// non-navigable button so it stays visible while signalling work in progress.
fn push_primary(parent: &mut DivisionBuilder, label: String, href: Option<String>) {
    match href {
        Some(href) => {
            parent.anchor(|a| {
                a.href(href)
                    .class("h-9 px-4 inline-flex items-center gap-2 rounded-lg bg-ink-900 text-canvas text-[13px] hover:opacity-90 no-underline")
                    .text(label)
                    .text(format!(" {ARROW_RIGHT}"))
            });
        }
        None => {
            parent.span(|s| {
                s.class("h-9 px-4 inline-flex items-center gap-2 rounded-lg bg-surfaceMuted text-ink-400 text-[13px] cursor-not-allowed")
                    .text(label)
                    .text(format!(" {ARROW_RIGHT}"))
            });
        }
    }
}

/// Append the secondary (outline) CTA. A `None` href renders a disabled, muted,
/// non-navigable button so it stays visible while signalling work in progress.
fn push_secondary(parent: &mut DivisionBuilder, label: String, href: Option<String>) {
    match href {
        Some(href) => {
            parent.anchor(|a| {
                a.href(href)
                    .class("h-9 px-4 inline-flex items-center gap-2 rounded-lg border-[1.5px] border-ink-900 bg-canvas text-ink-900 text-[13px] hover:bg-surfaceMuted no-underline")
                    .text(label)
            });
        }
        None => {
            parent.span(|s| {
                s.class("h-9 px-4 inline-flex items-center gap-2 rounded-lg border-[1.5px] border-lineSoft bg-canvas text-ink-400 text-[13px] cursor-not-allowed")
                    .text(label)
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot() {
        let html = render(&CtaStrip {
            kicker: "Ship",
            title: "Build with components today.",
            body_html: r#"Install the CLI with <code class="mono text-[12px]">brew install component</code> and start composing."#,
            primary_label: "Get started",
            primary_href: Some("/docs"),
            secondary_label: "View on GitHub",
            secondary_href: Some("https://github.com/yoshuawuyts/component-cli"),
        });
        insta::assert_snapshot!(crate::components::ds::pretty_html(&html));
    }

    #[test]
    fn snapshot_disabled() {
        // Both CTAs render as muted, non-navigable spans when their href is
        // `None`, matching the design system's disabled-control convention.
        let html = render(&CtaStrip {
            kicker: "Ship",
            title: "Build with components today.",
            body_html: r#"Install the CLI with <code class="mono text-[12px]">brew install component</code> and start composing."#,
            primary_label: "Get started",
            primary_href: None,
            secondary_label: "View on GitHub",
            secondary_href: None,
        });
        insta::assert_snapshot!(crate::components::ds::pretty_html(&html));
    }
}
