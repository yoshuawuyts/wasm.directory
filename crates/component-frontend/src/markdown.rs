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

/// Render link-free phrasing content for a description inside a full-row link.
///
/// Retains text, code, and emphasis from the first text block, replacing links
/// and images with their labels. Raw HTML remains escaped.
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
    fn summaries_keep_inline_formatting_without_interactive_content() {
        let out = render_summary(
            "**bold** *italic* ~~old~~ `code` [link](https://example.com) ![alt](image.png)",
        );
        assert_eq!(
            out,
            "<strong>bold</strong> <em>italic</em> <del>old</del> <code>code</code> link alt"
        );
    }

    #[test]
    fn summaries_only_use_the_first_text_block() {
        for markdown in [
            "# First\n\nSecond",
            "First\n\nSecond",
            "- First\n- Second",
            "```\nFirst\n```\n\nSecond",
            "| First |\n| --- |\n| Second |",
        ] {
            assert_eq!(render_summary(markdown), "First");
        }
    }

    #[test]
    fn summaries_escape_raw_html_and_normalize_breaks() {
        assert_eq!(
            render_summary("<script>alert(1)</script>\n\nSecond"),
            "&lt;script&gt;alert(1)&lt;/script&gt;"
        );
        assert_eq!(
            render_summary("First\nsecond  \nthird"),
            "First second third"
        );
        assert_eq!(
            render_summary("```\nlist<string>\nlist<u8>\n```"),
            "list&lt;string&gt; list&lt;u8&gt;"
        );
        assert_eq!(render_summary("A &amp; B"), "A &amp; B");
        assert_eq!(render_summary(" \n "), "");
    }
}
