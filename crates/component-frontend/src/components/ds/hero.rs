//! Landing page hero — two-column kicker + headline + CTAs with a slot
//! on the right (typically the install card).

use html::content::Section;
use html::text_content::Division;
use html::text_content::builders::DivisionBuilder;

/// A call-to-action button used in the hero.
pub(crate) struct HeroCta {
    pub label: &'static str,
    pub href: &'static str,
    pub style: HeroCtaStyle,
}

/// Visual style for a hero CTA.
pub(crate) enum HeroCtaStyle {
    /// Filled, primary action.
    Primary,
    /// Outlined, secondary action.
    Secondary,
    /// Plain text-link with a leading icon.
    #[allow(dead_code)]
    Ghost,
}

/// Configuration for [`render`].
pub(crate) struct Hero<'a> {
    /// Small mono kicker before the headline (e.g. `["v0.4.0", "Stable · WASI 0.2"]`).
    pub kicker: &'a [&'a str],
    /// Large headline text.
    pub title: &'a str,
    /// Lede paragraph below the headline.
    pub lede: &'a str,
    /// CTA buttons under the lede.
    pub ctas: &'a [HeroCta],
    /// HTML to slot into the right column (e.g. install card).
    pub right: &'a str,
}

const ARROW_RIGHT: &str = r#"<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M5 12h14"/><path d="m12 5 7 7-7 7"/></svg>"#;

const GITHUB_ICON: &str = r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><path d="M9 19c-5 1.5-5-2.5-7-3"/><path d="M15 22v-4a3.4 3.4 0 0 0-1-2.6c3 0 6-2 6-5.5a4.3 4.3 0 0 0-1.2-3 4 4 0 0 0-.1-3s-1-.3-3.3 1.2a11.4 11.4 0 0 0-6 0C7 3.6 6 3.9 6 3.9a4 4 0 0 0-.1 3 4.3 4.3 0 0 0-1.2 3c0 3.5 3 5.5 6 5.5a3.4 3.4 0 0 0-1 2.6V22"/></svg>"#;

/// Render the hero section.
#[must_use]
pub(crate) fn render(hero: &Hero<'_>) -> String {
    let title = hero.title.to_owned();
    let lede = hero.lede.to_owned();
    let right = hero.right.to_owned();
    let has_kicker = !hero.kicker.is_empty();
    let kicker = render_kicker(hero.kicker);

    let has_ctas = !hero.ctas.is_empty();
    let mut ctas_div = Division::builder();
    ctas_div.class("mt-7 flex flex-wrap items-center gap-3");
    for cta in hero.ctas {
        push_cta(&mut ctas_div, cta);
    }
    let ctas = ctas_div.build();

    Section::builder()
        .class("mx-auto mx-auto max-w-[1280px] w-full px-4 md:px-8 pt-12 md:pt-20 pb-10 md:pb-14")
        .division(|grid| {
            grid.class("grid md:grid-cols-[1fr_600px] gap-10 md:gap-16 items-start")
                .division(|left| {
                    if has_kicker {
                        left.push(kicker);
                    }
                    let heading_class = if has_kicker {
                        "mt-4 text-[40px] md:text-[52px] lg:text-[64px] leading-[1.05] font-semibold tracking-tight text-balance max-w-[14ch] md:max-w-[18ch] lg:max-w-[20ch]"
                    } else {
                        "text-[40px] md:text-[52px] lg:text-[64px] leading-[1.05] font-semibold tracking-tight text-balance max-w-[14ch] md:max-w-[18ch] lg:max-w-[20ch]"
                    };
                    let left = left
                        .heading_1(|h| h.class(heading_class).text(title))
                        .paragraph(|p| {
                            p.class("mt-5 max-w-2xl text-[16px] md:text-[17px] text-ink-700 leading-relaxed")
                                .text(lede)
                        });
                    if has_ctas {
                        left.push(ctas);
                    }
                    left
                })
                .text(right)
        })
        .build()
        .to_string()
}

fn render_kicker(parts: &[&str]) -> Division {
    let mut div = Division::builder();
    div.class("flex items-center gap-2 text-[12px] text-ink-500 mono uppercase tracking-wider");
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            div.span(|s| s.class("h-1 w-1 rounded-full bg-ink-300"));
        }
        let part = (*part).to_owned();
        div.span(|s| s.text(part));
    }
    div.build()
}

fn push_cta(parent: &mut DivisionBuilder, cta: &HeroCta) {
    let label = cta.label.to_owned();
    let href = cta.href.to_owned();
    match cta.style {
        HeroCtaStyle::Primary => {
            parent.anchor(|a| {
                a.href(href)
                    .class("h-9 px-4 inline-flex items-center gap-2 rounded-lg bg-ink-900 text-canvas text-[13px] hover:opacity-90 no-underline")
                    .text(label)
                    .text(format!(" {ARROW_RIGHT}"))
            });
        }
        HeroCtaStyle::Secondary => {
            parent.anchor(|a| {
                a.href(href)
                    .class("h-9 px-4 inline-flex items-center gap-2 rounded-lg border-[1.5px] border-ink-900 bg-surface text-ink-900 text-[13px] hover:bg-surfaceMuted no-underline")
                    .text(label)
            });
        }
        HeroCtaStyle::Ghost => {
            parent.anchor(|a| {
                a.href(href)
                    .class("text-[13px] text-ink-500 hover:text-ink-900 ml-1 inline-flex items-center gap-1 no-underline")
                    .text(format!("{GITHUB_ICON} {label}"))
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot() {
        let html = render(&Hero {
            kicker: &["v0.4.0", "Stable \u{00b7} WASI 0.2"],
            title: "The package manager for WebAssembly Components.",
            lede: "Discover, install, and compose WIT-defined components from any registry.",
            ctas: &[
                HeroCta {
                    label: "Get started",
                    href: "/docs",
                    style: HeroCtaStyle::Primary,
                },
                HeroCta {
                    label: "Browse packages",
                    href: "/packages",
                    style: HeroCtaStyle::Secondary,
                },
                HeroCta {
                    label: "View on GitHub",
                    href: "https://github.com/yoshuawuyts/component-cli",
                    style: HeroCtaStyle::Ghost,
                },
            ],
            right: r#"<div class="rounded-lg border border-line p-6">install card</div>"#,
        });
        insta::assert_snapshot!(crate::components::ds::pretty_html(&html));
    }
}
