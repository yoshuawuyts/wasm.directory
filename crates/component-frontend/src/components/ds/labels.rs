//! 09 — Labels.

use html::text_content::Division;

/// Label bar entries: (bg class, ink class, text).
pub(crate) const BARS: &[(&str, &str, &str)] = &[
    ("bg-cat-blue", "text-cat-blueInk", "Lorem ipsum dolor"),
    ("bg-cat-pink", "text-cat-pinkInk", "Sit amet"),
    ("bg-cat-cream", "text-cat-creamInk", "Consectetur"),
    ("bg-cat-green", "text-cat-greenInk", "Adipiscing elit"),
    ("bg-cat-peach", "text-cat-peachInk", "Sed do eiusmod"),
    ("bg-cat-lilac", "text-cat-lilacInk", "Tempor incididunt"),
    ("bg-cat-teal", "text-cat-tealInk", "Ut labore"),
    ("bg-cat-rust", "text-cat-rustInk", "Et dolore magna"),
    ("bg-cat-plum", "text-cat-plumInk", "Aliqua enim"),
    ("bg-cat-slate", "text-cat-slateInk", "Ad minim veniam"),
];

#[allow(dead_code)]
/// Render the labels section.
/// Render a single label bar.
pub(crate) fn label_bar(bg_class: &str, ink_class: &str, text: &str) -> Division {
    let class = format!("bar {bg_class} {ink_class}");
    let text = text.to_owned();
    Division::builder().class(class).text(text).build()
}

pub(crate) fn render(
    section_id: &str,
    num: &str,
    title: &str,
    desc: &str,
    bars: &[(&str, &str, &str)],
) -> String {
    let mut col = Division::builder();
    col.class("flex flex-col items-start gap-4");
    col.division(|group| {
        let mut group = group.class("flex flex-col items-start gap-2");
        for (bg, ink, text) in bars {
            let class = format!("bar {bg} {ink}");
            let text = (*text).to_owned();
            group = group.division(|d| d.class(class).text(text));
        }
        group
    });
    col.division(|group| {
        let mut group = group.class("flex flex-col items-start gap-2");
        group = group.division(|d| {
            d.class("text-[12px] text-ink-500 mb-1")
                .text("Small \u{00b7} for inline use inside compact rows")
        });
        for (bg, ink, text) in bars {
            let class = format!("bar-sm {bg} {ink}");
            let text = (*text).to_owned();
            group = group.division(|d| d.class(class).text(text));
        }
        group
    });
    let content = col.build().to_string();

    super::section(section_id, num, title, desc, &content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot() {
        insta::assert_snapshot!(crate::components::ds::pretty_html(&render(
            "bars",
            "09",
            "Labels",
            "28px tall, 6px radius, label inset 12px. Pastel fill with paired ink for text \u{2014} 4.5:1 contrast minimum.",
            BARS,
        )));
    }
}
