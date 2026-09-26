//! Shared search results page layout.
//!
//! Every result listing (all packages, search, namespaces, dependents,
//! imported-by and exported-by) uses the same layout, modelled on `/all`: a
//! wrapping heading with a result summary, optional intro content, the result
//! rows (or an empty notice), and optional pagination. Failures reuse the
//! same shell with a visible error notice instead of an empty list.

use html::text_content::Division;

use super::{listing, typography};
use crate::escape::{escape_html_attr, escape_html_text};
use crate::layout;

/// A centered notice shown instead of result rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Notice {
    message: String,
    detail: Option<String>,
    action: Option<(String, String)>,
}

impl Notice {
    /// A notice with a plain-text message.
    #[must_use]
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            detail: None,
            action: None,
        }
    }

    /// Add a secondary plain-text line, such as an error description.
    #[must_use]
    pub(crate) fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Add a follow-up link; `href` is escaped when rendered.
    #[must_use]
    pub(crate) fn action(mut self, label: impl Into<String>, href: impl Into<String>) -> Self {
        self.action = Some((label.into(), href.into()));
        self
    }

    fn render(&self, message_class: &str) -> Division {
        let mut div = Division::builder();
        div.class("py-16 text-center").paragraph(|p| {
            p.class(message_class.to_owned())
                .text(escape_html_text(&self.message))
        });
        if let Some(detail) = &self.detail {
            div.paragraph(|p| {
                p.class(typography::SUBTITLE_CLASS)
                    .text(escape_html_text(detail))
            });
        }
        if let Some((label, href)) = &self.action {
            div.paragraph(|p| {
                p.class("mt-4").anchor(|a| {
                    a.href(escape_html_attr(href))
                        .class("text-[13px] text-accent hover:underline")
                        .text(escape_html_text(label))
                })
            });
        }
        div.build()
    }
}

/// Builder for a results page; see the module documentation.
#[derive(Debug)]
pub(crate) struct ResultsPage<'a> {
    title: &'a str,
    document_title: &'a str,
    total: Option<u64>,
    intro: Vec<Division>,
    rows: Vec<Division>,
    empty: Notice,
    pagination: Option<Division>,
}

impl<'a> ResultsPage<'a> {
    /// Start a page with a plain-text heading, also used as document title.
    #[must_use]
    pub(crate) fn new(title: &'a str) -> Self {
        Self {
            title,
            document_title: title,
            total: None,
            intro: Vec::new(),
            rows: Vec::new(),
            empty: Notice::new("No results found."),
            pagination: None,
        }
    }

    /// Override the `<title>` text when it differs from the heading.
    #[must_use]
    pub(crate) fn document_title(mut self, document_title: &'a str) -> Self {
        self.document_title = document_title;
        self
    }

    /// Set the total number of results across all pages, when known.
    #[must_use]
    pub(crate) fn total(mut self, total: Option<u64>) -> Self {
        self.total = total;
        self
    }

    /// Append content shown between the heading and the results.
    #[must_use]
    pub(crate) fn intro(mut self, intro: Division) -> Self {
        self.intro.push(intro);
        self
    }

    /// Set the result rows shown on this page.
    #[must_use]
    pub(crate) fn rows(mut self, rows: impl IntoIterator<Item = Division>) -> Self {
        self.rows = rows.into_iter().collect();
        self
    }

    /// Set the notice shown when this page has no rows.
    #[must_use]
    pub(crate) fn empty(mut self, empty: Notice) -> Self {
        self.empty = empty;
        self
    }

    /// Set the pagination controls shown below the results.
    #[must_use]
    pub(crate) fn pagination(mut self, pagination: Division) -> Self {
        self.pagination = Some(pagination);
        self
    }

    /// Render the full document with the result rows.
    #[must_use]
    pub(crate) fn render(self) -> String {
        let mut body = Division::builder();
        body.push(listing::header(self.title, self.rows.len(), self.total));
        for intro in self.intro {
            body.push(intro);
        }
        if self.rows.is_empty() {
            body.push(self.empty.render("text-ink-500"));
        } else {
            body.push(result_rows(self.rows));
        }
        if let Some(pagination) = self.pagination {
            body.push(pagination);
        }
        layout::document_with_nav(self.document_title, &body.build().to_string())
    }

    /// Render the full document with a visible failure instead of results.
    #[must_use]
    pub(crate) fn render_error(self, error: &Notice) -> String {
        let mut body = Division::builder();
        body.division(|div| {
            div.class("pt-8 pb-6 border-b-[1.5px] border-rule mb-6")
                .push(listing::heading(self.title))
        });
        for intro in self.intro {
            body.push(intro);
        }
        body.push(error.render("text-ink-900 font-medium"));
        if let Some(pagination) = self.pagination {
            body.push(pagination);
        }
        layout::document_with_nav(self.document_title, &body.build().to_string())
    }
}

fn result_rows(rows: Vec<Division>) -> Division {
    let mut list = Division::builder();
    list.class("divide-y divide-lineSoft");
    for row in rows {
        list.push(row);
    }
    list.build()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(text: &str) -> Division {
        Division::builder().text(text.to_owned()).build()
    }

    #[test]
    fn results_share_header_intro_rows_and_pagination() {
        let html = ResultsPage::new("Title <x>")
            .total(Some(9))
            .intro(row("INTRO-MARK"))
            .rows([row("ROW-ONE"), row("ROW-TWO")])
            .pagination(row("PAGER-MARK"))
            .render();
        assert!(html.contains("Title &lt;x&gt;</h1>"));
        assert!(html.contains("showing 2 of 9 results"));
        assert!(html.contains("divide-y divide-lineSoft"));
        let order = ["</h1>", "INTRO-MARK", "ROW-ONE", "ROW-TWO", "PAGER-MARK"]
            .map(|needle| html.find(needle).expect("rendered section"));
        assert!(order.is_sorted());
    }

    #[test]
    fn empty_pages_show_the_notice_and_its_action() {
        let html = ResultsPage::new("Title")
            .empty(Notice::new("Nothing here").action("Go <back>", "/a?b=1&c=2"))
            .render();
        assert!(html.contains("showing 0 results (total unavailable)"));
        assert!(html.contains("Nothing here"));
        assert!(html.contains(r#"href="/a?b=1&amp;c=2""#));
        assert!(html.contains("Go &lt;back&gt;"));
        assert!(!html.contains("divide-y divide-lineSoft"));
    }

    #[test]
    fn errors_are_escaped_and_omit_the_result_summary() {
        let html = ResultsPage::new("Title")
            .document_title("Doc")
            .render_error(&Notice::new("Unable to load").detail("<script>bad</script>"));
        assert!(html.contains("<title>Doc"));
        assert!(html.contains("Unable to load"));
        assert!(html.contains("&lt;script&gt;bad&lt;/script&gt;"));
        assert!(!html.contains("showing"));
    }
}
