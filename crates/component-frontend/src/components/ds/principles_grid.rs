//! Principles grid — 2×2 card grid inside a hairline shell. Each tile has
//! a coloured square sigil with an inline SVG icon, a title, and a body.

use html::content::Section;

/// A single principle card.
pub(crate) struct Principle<'a> {
    /// Tailwind background class for the icon tile (e.g. `"bg-cat-blue"`).
    pub bg_class: &'a str,
    /// Tailwind text colour class for the icon tile (e.g. `"text-cat-blueInk"`).
    pub fg_class: &'a str,
    /// Inline SVG markup for the icon (16×16, currentColor).
    pub icon_svg: &'a str,
    pub title: &'a str,
    pub body: &'a str,
}

/// Render the principles grid section.
#[must_use]
pub(crate) fn render(kicker: &str, title: &str, lede: &str, items: &[Principle<'_>]) -> String {
    let has_kicker = !kicker.is_empty();
    let kicker = kicker.to_owned();
    let title = title.to_owned();
    let lede = lede.to_owned();

    let mut tiles = html::text_content::Division::builder();
    tiles.class(
        "grid sm:grid-cols-2 gap-px bg-lineSoft border border-lineSoft rounded-lg overflow-hidden",
    );
    for item in items {
        push_tile(&mut tiles, item);
    }
    let tiles = tiles.build();

    Section::builder()
        .class("mx-auto mx-auto max-w-[1280px] w-full px-4 md:px-8 pt-12 md:pt-16")
        .division(|grid| {
            grid.class("grid md:grid-cols-[200px_1fr] gap-6 md:gap-12")
                .division(|left| {
                    if has_kicker {
                        left.division(|d| {
                            d.class("text-[12px] mono uppercase tracking-wider text-ink-500")
                                .text(kicker)
                        });
                    }
                    let heading_class = if has_kicker {
                        "mt-2 text-[24px] font-semibold tracking-tight"
                    } else {
                        "text-[24px] font-semibold tracking-tight"
                    };
                    left.heading_2(|h| h.class(heading_class).text(title))
                        .paragraph(|p| {
                            p.class("mt-2 text-[13px] text-ink-500 leading-relaxed")
                                .text(lede)
                        })
                })
                .push(tiles)
        })
        .build()
        .to_string()
}

fn push_tile(parent: &mut html::text_content::builders::DivisionBuilder, p: &Principle<'_>) {
    let icon = p.icon_svg.to_owned();
    let title = p.title.to_owned();
    let body = p.body.to_owned();
    let sigil_class = format!(
        "h-8 w-8 grid place-items-center rounded-md {} {}",
        p.bg_class, p.fg_class
    );
    parent.division(|tile| {
        tile.class("bg-surface p-6")
            .division(|d| d.class(sigil_class.clone()).text(icon))
            .division(|d| {
                d.class("mt-3 text-[15px] font-semibold tracking-tight")
                    .text(title)
            })
            .paragraph(|p| {
                p.class("mt-1.5 text-[13px] text-ink-700 leading-relaxed")
                    .text(body)
            })
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot() {
        let icon = r#"<svg width="16" height="16"></svg>"#;
        let html = render(
            "Principles",
            "Built for components",
            "Three commitments that shape every choice in the toolchain.",
            &[
                Principle {
                    bg_class: "bg-cat-blue",
                    fg_class: "text-cat-blueInk",
                    icon_svg: icon,
                    title: "WIT-first",
                    body: "Interfaces are the contract; implementations follow.",
                },
                Principle {
                    bg_class: "bg-cat-green",
                    fg_class: "text-cat-greenInk",
                    icon_svg: icon,
                    title: "Composable",
                    body: "Mix any component with any other, regardless of language.",
                },
            ],
        );
        insta::assert_snapshot!(crate::components::ds::pretty_html(&html));
    }
}
