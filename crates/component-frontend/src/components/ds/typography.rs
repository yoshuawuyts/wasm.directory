//! 02 — Typography.

use html::text_content::Division;

/// Type sample: (label, text_class, sample_text, spec).
pub(crate) const SAMPLES: &[(&str, &str, &str, &str)] = &[
    (
        "Display",
        "text-[44px] leading-[1.05] font-semibold tracking-tight",
        "Aa Display",
        "44 / 1.05 / -0.01em / 600",
    ),
    (
        "H1",
        "text-[28px] leading-[1.15] font-semibold tracking-tight",
        "Lorem ipsum dolor",
        "28 / 1.15 / 600",
    ),
    (
        "H2",
        "text-[22px] font-semibold tracking-tight",
        "Sit amet consectetur",
        "22 / 600",
    ),
    (
        "Lead",
        "text-[20px] font-semibold tracking-tight leading-tight",
        "42.7 k",
        "20 / tight / 600 \u{2014} metric value",
    ),
    (
        "Body",
        "text-[15px] leading-relaxed text-ink-700",
        "The quick brown fox jumps over the lazy dog.",
        "15 / 1.6 / 400",
    ),
    (
        "UI",
        "text-[14px]",
        "Navigation item \u{00b7} Table cell",
        "14 / 400 \u{2014} 13 / 500 (medium)",
    ),
    (
        "Caption",
        "text-[12px] text-ink-500",
        "Aenean lectus \u{00b7} Vivamus aliquet",
        "12 / 400 / ink-500",
    ),
    (
        "Micro",
        "text-[11px] text-ink-500",
        "Tempor incididunt \u{00b7} ut labore",
        "11 / 400",
    ),
];

#[allow(dead_code)]
pub(crate) fn type_row(label: &str, text_class: &str, sample: &str, spec: &str) -> Division {
    let label = label.to_owned();
    let text_class = text_class.to_owned();
    let sample = sample.to_owned();
    let spec = spec.to_owned();
    Division::builder()
        .class("py-5 grid grid-cols-[120px_1fr] gap-6 items-baseline")
        .division(|l| l.class("text-[12px] text-ink-500 mono").text(label))
        .division(|c| {
            c.division(|d| d.class(text_class).text(sample))
                .division(|d| d.class("text-[12px] text-ink-500 mt-1 mono").text(spec))
        })
        .build()
}

/// Class string for the display-sized page heading.
#[allow(dead_code)]
pub(crate) const DISPLAY_CLASS: &str =
    "text-[36px] md:text-[44px] leading-[1.05] font-semibold tracking-tight";

/// Class string for the primary page heading (h1).
pub(crate) const H1_CLASS: &str = "text-[28px] leading-[1.15] font-semibold tracking-tight";

/// Class string for a page sub-heading (h2).
pub(crate) const H2_CLASS: &str = "text-[22px] font-semibold tracking-tight mt-10 mb-4";

/// Class string for a subtitle line below the heading.
pub(crate) const SUBTITLE_CLASS: &str = "text-[13px] text-ink-500 mt-2";

/// Class string for body text paragraphs.
pub(crate) const BODY_CLASS: &str = "text-ink-700 leading-relaxed";

/// Class string for a section heading.
pub(crate) const SECTION_CLASS: &str = "text-[16px] font-semibold tracking-tight mb-3";

/// Class string for a section heading with bottom border.
#[allow(dead_code)]
pub(crate) const SECTION_BORDERED_CLASS: &str =
    "text-[16px] font-semibold tracking-tight mb-3 pb-2 border-b border-lineSoft";

/// Class string for a section label (eyebrow heading).
pub(crate) const SECTION_LABEL_CLASS: &str =
    "text-[11px] uppercase tracking-wider text-ink-500 mb-2";

/// Class string for a rule divider between detail sections.
#[allow(dead_code)]
pub(crate) const SECTION_RULE_CLASS: &str = "my-3 border-t-[1.5px] border-rule";

/// Render this section.
pub(crate) fn render(
    section_id: &str,
    num: &str,
    title: &str,
    desc: &str,
    samples: &[(&str, &str, &str, &str)],
) -> String {
    let mut rows = Division::builder();
    rows.class("divide-y divide-lineSoft");
    for (label, cls, sample, spec) in samples {
        rows.push(type_row(label, cls, sample, spec));
    }

    // Inline row — contains raw HTML for mixed inline elements
    let inline_html = r##"<div class="text-[15px] leading-relaxed text-ink-700">
                Read the <a href="#"
                  class="text-ink-900 underline decoration-line decoration-1 underline-offset-[3px] hover:decoration-ink-900">installation
                  guide</a>,
                then run <code
                  class="px-1 py-0.5 rounded-sm bg-surfaceMuted text-ink-900 mono text-[0.875em]">component install</code>.
                Use <strong class="font-semibold text-ink-900">--strict</strong> for <em
                  class="italic">reproducible</em> builds.
                Press <kbd
                  class="inline-flex items-center px-1.5 h-5 rounded-sm border border-line bg-surface text-ink-700 mono text-[11px] align-[1px]">⌘K</kbd>
                to search.
                <del class="text-ink-500 line-through decoration-1">Deprecated since v0.4</del> &mdash;
                see <a href="#"
                  class="text-ink-900 underline decoration-line decoration-1 underline-offset-[3px] hover:decoration-ink-900">migration
                  notes</a>.
              </div>"##;

    rows.push(
        Division::builder()
            .class("py-5 grid grid-cols-[120px_1fr] gap-6 items-baseline")
            .division(|l| l.class("text-[12px] text-ink-500 mono").text("Inline"))
            .division(|c| {
                c.text(inline_html.to_owned()).division(|d| {
                    d.class("text-[12px] text-ink-500 mt-2 mono").text(
                        "link \u{00b7} code \u{00b7} strong \u{00b7} em \u{00b7} kbd \u{00b7} del",
                    )
                })
            })
            .build(),
    );

    // Markdown row — complex nested content
    let markdown_html = r##"<article class="text-[15px] leading-relaxed text-ink-700 space-y-4">
                <h3 class="text-[20px] font-semibold tracking-tight text-ink-900 leading-tight">Configuring the registry
                </h3>
                <p>
                  The <code
                    class="px-1 py-0.5 rounded-sm bg-surfaceMuted text-ink-900 mono text-[0.875em]">wasm.toml</code>
                  manifest lives at the root of every package. It declares the package
                  identity, its <a href="#"
                    class="text-ink-900 underline decoration-line decoration-1 underline-offset-[3px] hover:decoration-ink-900">dependencies</a>,
                  and the registries it pulls from.
                </p>
                <h4 class="text-[16px] font-semibold tracking-tight text-ink-900 leading-snug pt-2">Manifest fields</h4>
                <ul class="list-disc pl-5 space-y-1 marker:text-ink-400">
                  <li>
                    <p><code class="px-1 py-0.5 rounded-sm bg-surfaceMuted text-ink-900 mono text-[0.875em]">name</code>
                      &mdash; reverse-DNS package identifier</p>
                  </li>
                  <li>
                    <p><code
                        class="px-1 py-0.5 rounded-sm bg-surfaceMuted text-ink-900 mono text-[0.875em]">version</code>
                      &mdash; semantic version, must be unique per registry</p>
                  </li>
                  <li>
                    <p><code
                        class="px-1 py-0.5 rounded-sm bg-surfaceMuted text-ink-900 mono text-[0.875em]">authors</code>
                      &mdash; one or more contact strings</p>
                  </li>
                </ul>
                <h4 class="text-[16px] font-semibold tracking-tight text-ink-900 leading-snug pt-2">Resolution order
                </h4>
                <ol class="list-decimal pl-5 space-y-1 marker:text-ink-400 marker:tabular-nums">
                  <li>
                    <p>Local cache at <code
                        class="px-1 py-0.5 rounded-sm bg-surfaceMuted text-ink-900 mono text-[0.875em]">~/.wasm/store</code>
                    </p>
                  </li>
                  <li>
                    <p>Registries declared in the manifest, in order</p>
                  </li>
                  <li>
                    <p>The default registry, unless <code
                        class="px-1 py-0.5 rounded-sm bg-surfaceMuted text-ink-900 mono text-[0.875em]">--offline</code>
                      is set</p>
                  </li>
                </ol>
                <blockquote class="border-l-2 border-ink-900 pl-4 text-ink-700 italic">
                  Every dependency is locked by content hash, so a build today
                  resolves byte-for-byte tomorrow.
                </blockquote>
                <pre class="id-code text-[13px] leading-relaxed"><span class="h">[package]</span>
<span class="k">name</span>    <span class="p">=</span> <span class="s">"example.com/hello-world"</span>
<span class="k">version</span> <span class="p">=</span> <span class="s">"0.1.0"</span>
<span class="k">authors</span> <span class="p">=</span> <span class="p">[</span><span class="s">"Lorem Ipsum &lt;lorem@example.com&gt;"</span><span class="p">]</span>

<span class="h">[dependencies]</span>
<span class="k">"wasi:http"</span>   <span class="p">=</span> <span class="s">"0.2"</span>
<span class="k">"wasi:cli"</span>    <span class="p">=</span> <span class="s">"0.2"</span></pre>
                <p class="text-[13px] text-ink-500">
                  See the <a href="#"
                    class="text-ink-700 underline decoration-line decoration-1 underline-offset-[3px] hover:text-ink-900">manifest
                    reference</a>
                  for the complete schema, including optional fields like
                  <code
                    class="px-1 py-0.5 rounded-sm bg-surfaceMuted text-ink-700 mono text-[0.875em]">[targets.*]</code>
                  and <code
                    class="px-1 py-0.5 rounded-sm bg-surfaceMuted text-ink-700 mono text-[0.875em]">[features]</code>.
                </p>
              </article>"##;

    rows.push(
        Division::builder()
            .class("py-5 grid grid-cols-[120px_1fr] gap-6 items-start")
            .division(|l| l.class("text-[12px] text-ink-500 mono pt-1").text("Markdown"))
            .division(|c| {
                c.text(markdown_html.to_owned())
                    .division(|d| {
                        d.class("text-[12px] text-ink-500 mt-3 mono")
                            .text("h3 / h4 \u{00b7} p \u{00b7} ul \u{00b7} ol \u{00b7} blockquote \u{00b7} pre")
                    })
            })
            .build(),
    );

    let content = rows.build().to_string();

    super::section(section_id, num, title, desc, &content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot() {
        insta::assert_snapshot!(crate::components::ds::pretty_html(&render(
            "typography",
            "02",
            "Typography",
            "System UI stack for native rendering across platforms. Tight tracking on display sizes; relaxed for body.",
            SAMPLES,
        )));
    }
}
