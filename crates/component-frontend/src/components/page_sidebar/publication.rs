//! Selected-version publication metadata for the shared package sidebar.

use chrono::{DateTime, Utc};

use super::{SVG_CLOCK, SidebarContext};
use crate::escape::{escape_html_attr, escape_html_text};
use crate::relative_time::Age;

pub(super) fn render(ctx: &SidebarContext<'_>, now: DateTime<Utc>) -> Option<String> {
    let timestamp = ctx.created_at?;
    let Some(age) = Age::parse("Published", timestamp.trim(), now) else {
        eprintln!(
            "component-frontend: invalid publication timestamp for {}/{}@{}",
            ctx.registry, ctx.repository, ctx.version
        );
        return None;
    };
    let exact = age.datetime.trim_end_matches('Z').replace('T', " ");
    let title = format!("Published {exact} UTC (publisher-provided OCI creation time)");
    let datetime = escape_html_attr(&age.datetime);
    let label = escape_html_text(&age.label);
    let accessible_label = escape_html_attr(&format!(
        "Published {}; {exact} UTC (publisher-provided OCI creation time)",
        age.label
    ));
    let title = escape_html_attr(&title);
    Some(format!(
        r#"<div class="tree-link" data-publication-date><span class="project-icon">{SVG_CLOCK}</span> <time datetime="{datetime}" title="{title}" aria-label="{accessible_label}">{label}</time></div>"#
    ))
}

#[cfg(test)]
mod tests;
