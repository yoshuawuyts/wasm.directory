//! Markdown-to-HTML rendering for documentation text.

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd, html};

/// Render a markdown string to HTML.
///
/// Uses `pulldown-cmark` to convert doc comments into HTML. Registry
/// documentation is untrusted, so raw HTML in the source is downgraded to
/// escaped text (instead of being passed through verbatim) and link/image
/// destinations with dangerous URL schemes (e.g. `javascript:`) are
/// neutralized. See [`crate::escape::sanitize_url`].
#[must_use]
pub(crate) fn render(input: &str) -> String {
    let mut output = String::new();
    html::push_html(&mut output, sanitized_events(input));
    output
}

fn sanitized_events(input: &str) -> impl Iterator<Item = Event<'_>> {
    let options = Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES;
    Parser::new_ext(input, options).map(sanitize_event)
}

/// Neutralize unsafe events emitted by the markdown parser.
///
/// Raw HTML is turned into text (so `push_html` escapes it), and link/image
/// destinations are passed through [`crate::escape::sanitize_url`].
fn sanitize_event(event: Event<'_>) -> Event<'_> {
    match event {
        Event::Html(html) | Event::InlineHtml(html) => Event::Text(html),
        Event::Start(Tag::Link {
            link_type,
            dest_url,
            title,
            id,
        }) => Event::Start(Tag::Link {
            link_type,
            dest_url: crate::escape::sanitize_url(&dest_url).into(),
            title,
            id,
        }),
        Event::Start(Tag::Image {
            link_type,
            dest_url,
            title,
            id,
        }) => Event::Start(Tag::Image {
            link_type,
            dest_url: crate::escape::sanitize_url(&dest_url).into(),
            title,
            id,
        }),
        other => other,
    }
}

/// Standard CSS class for rendered doc comment blocks.
pub(crate) const DOC_CLASS: &str = "text-base text-ink-500 leading-relaxed prose-doc";

/// Render markdown and wrap in a styled `<div>`.
///
/// Applies prose-like styling classes for rendered documentation.
#[must_use]
pub(crate) fn render_block(input: &str, class: &str) -> String {
    let html = render(input);
    format!(r#"<div class="{class}">{html}</div>"#)
}

/// Render a short markdown string as inline HTML.
///
/// Strips the outer `<p>` wrapper that pulldown-cmark adds for single
/// paragraphs, making the result safe for use inside table cells,
/// list items, and other inline contexts.
#[must_use]
pub(crate) fn render_inline(input: &str) -> String {
    let html = render(input);
    // Extract content from the first <p>...</p> only, stripping any
    // subsequent paragraphs that would break inline contexts.
    if let Some(start) = html.find("<p>") {
        let content_start = start + 3;
        if let Some(end) = html[content_start..].find("</p>") {
            return html[content_start..content_start + end].to_owned();
        }
    }
    // Fallback: return as-is if no <p> found
    html
}

/// Render a raw Markdown excerpt inside a whole-row link.
///
/// Keeps the first text block's inline formatting, but not block containers,
/// links, or images: links contribute their labels and images their alt text.
/// Source HTML remains escaped, and line breaks become spaces.
#[must_use]
pub(crate) fn render_summary(input: &str) -> String {
    let events = sanitized_events(input)
        .take_while(|event| !ends_summary(event))
        .filter_map(summary_event);
    let mut output = String::new();
    html::push_html(&mut output, events);
    output.trim().to_owned()
}

fn ends_summary(event: &Event<'_>) -> bool {
    matches!(
        event,
        Event::End(
            TagEnd::Paragraph
                | TagEnd::Heading(_)
                | TagEnd::CodeBlock
                | TagEnd::HtmlBlock
                | TagEnd::Item
                | TagEnd::TableCell
        )
    )
}

fn summary_event(event: Event<'_>) -> Option<Event<'_>> {
    match event {
        Event::Text(text) => Some(Event::Text(text.replace(['\r', '\n'], " ").into())),
        Event::SoftBreak | Event::HardBreak => Some(Event::Text(" ".into())),
        Event::Code(_)
        | Event::Start(Tag::Emphasis | Tag::Strong | Tag::Strikethrough)
        | Event::End(TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough) => Some(event),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_raw_html_in_markdown() {
        let out = render("hello <script>alert(1)</script> world");
        assert!(!out.contains("<script>"));
        assert!(out.contains("&lt;script&gt;"));
    }

    #[test]
    fn neutralizes_javascript_links() {
        let out = render("[click](javascript:alert(1))");
        assert!(!out.contains("javascript:"));
        assert!(out.contains(r##"href="#""##));
    }

    #[test]
    fn preserves_safe_markdown() {
        let out = render("**bold** and [link](https://example.com)");
        assert!(out.contains("<strong>bold</strong>"));
        assert!(out.contains(r#"href="https://example.com""#));
    }

    #[test]
    fn preserves_soft_wraps_paragraphs_and_explicit_hard_breaks() {
        assert_eq!(
            render("Soft\nwrap.\n\nHard  \nbreak.\\\nAnother."),
            "<p>Soft\nwrap.</p>\n<p>Hard<br />\nbreak.<br />\nAnother.</p>\n"
        );
    }

    #[test]
    fn summary_preserves_inline_formatting_without_nested_links_or_images() {
        let out = render_summary(
            "Use `body` with **care**, *emphasis*, ~~old~~, \
             [the **spec**](https://example.com), and ![an image](https://example.com/image.png).",
        );
        assert_eq!(
            out,
            "Use <code>body</code> with <strong>care</strong>, <em>emphasis</em>, \
             <del>old</del>, the <strong>spec</strong>, and an image."
        );
    }

    #[test]
    fn summary_keeps_only_first_text_block_and_collapses_line_breaks() {
        for (source, expected) in [
            (
                "First\nline.  \nHard break.\n\nSecond paragraph.",
                "First line. Hard break.",
            ),
            ("# A `heading`\n\nLater.", "A <code>heading</code>"),
            (
                "- First **item**.\n- Second item.",
                "First <strong>item</strong>.",
            ),
            ("> A quote.\n>\n> Later.", "A quote."),
            (
                "```wit\nlist<string>\nlist<u8>\n```\n\nLater.",
                "list&lt;string&gt; list&lt;u8&gt;",
            ),
            ("| Header |\n| --- |\n| Cell |", "Header"),
            ("", ""),
            (" \n\n ", ""),
            (
                "Unclosed `code and **emphasis",
                "Unclosed `code and **emphasis",
            ),
        ] {
            assert_eq!(render_summary(source), expected, "{source}");
        }
    }

    #[test]
    fn summary_escapes_source_html_and_metacharacters_once() {
        let out = render_summary(
            "A &amp; B <code>literal</code> <script>alert(1)</script> <img src=x onerror=alert(1)>.",
        );
        assert!(out.contains("A &amp; B"));
        assert!(!out.contains("&amp;amp;"));
        assert!(out.contains("&lt;code&gt;literal&lt;/code&gt;"));
        assert!(out.contains("&lt;script&gt;"));
        assert!(out.contains("&lt;img src=x"));
        assert!(!out.contains("<script>"));
        assert!(!out.contains("<img"));
    }

    #[test]
    fn unsafe_links_and_images_remain_neutralized_in_blocks_and_summaries() {
        for url in [
            "javascript:alert(1)",
            "JaVaScRiPt:alert(1)",
            "data:text/html,payload",
            "vbscript:payload",
        ] {
            let source = format!("[label]({url}) and ![alt]({url})");
            let block = render_block(&source, DOC_CLASS);
            assert!(block.contains(r##"href="#""##));
            assert!(block.contains(r##"src="#""##));
            assert!(!block.contains(url));
            assert_eq!(render_summary(&source), "label and alt");
        }
    }

    #[test]
    fn safe_links_survive_block_rendering_and_attribute_escaping() {
        let out = render_block(
            r#"[relative](/guide?a=1&b=2 "A &quot;title&quot;") and [mail](mailto:hello@example.com)"#,
            DOC_CLASS,
        );
        assert!(out.contains(r#"href="/guide?a=1&amp;b=2""#));
        assert!(out.contains(r#"title="A &quot;title&quot;""#));
        assert!(out.contains(r#"href="mailto:hello@example.com""#));
    }
}
