//! Shared headings and counts for flowing result lists.

use html::{content::Heading1, text_content::Division};

use crate::escape::escape_html_text;

/// Render a page count without inventing an unavailable or inconsistent total.
pub(crate) fn result_summary(page_count: usize, total: Option<u64>) -> String {
    let page_count = u64::try_from(page_count).expect("result page count should fit in u64");
    let total = match total {
        Some(total) if total < page_count => {
            eprintln!(
                "component-frontend: listing total {total} is smaller than page count {page_count}"
            );
            None
        }
        total => total,
    };
    match total {
        Some(1) => format!("showing {page_count} of 1 result"),
        Some(total) => format!("showing {page_count} of {total} results"),
        None if page_count == 1 => "showing 1 result (total unavailable)".to_owned(),
        None => format!("showing {page_count} results (total unavailable)"),
    }
}

/// Render a wrapping page heading and the actual number of displayed results.
pub(crate) fn header(title: &str, page_count: usize, total: Option<u64>) -> Division {
    Division::builder()
        .class("pt-8 flex flex-wrap items-baseline justify-between gap-x-4 gap-y-2 pb-6 border-b-[1.5px] border-rule mb-6")
        .push(heading(title))
        .span(|s| {
            s.class(super::typography::SUBTITLE_CLASS)
                .text(result_summary(page_count, total))
        })
        .build()
}

/// Keep long package and interface identities within the available width.
pub(crate) fn heading(title: &str) -> Heading1 {
    Heading1::builder()
        .class(format!(
            "{} min-w-0 max-w-full [overflow-wrap:anywhere]",
            super::typography::H1_CLASS
        ))
        .text(escape_html_text(title))
        .build()
}
