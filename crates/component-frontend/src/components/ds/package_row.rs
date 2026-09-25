//! Package row component.
//!
//! Two grouped rows for search results, namespaces, and all-packages pages:
//! identity and description, then kind, version, dependents, and release age.
//! Identities and metadata wrap when needed; version tags remain fully visible.
//! Descriptions stay on one line and truncate visually, retaining their text.

use chrono::{DateTime, Utc};
use html::inline_text::Span;
use html::text_content::Division;
use wasm_meta_registry_client::{KnownPackage, PackageKind};

use super::{icons, labels};
use crate::escape::{escape_html_attr, escape_html_text};
use crate::relative_time::Age;

mod matching;
pub(crate) use matching::{render_matching_package, render_matching_world};

const ROW_CLASS: &str = "flex flex-col gap-1 py-3 -mx-2 px-2";
const PRIMARY_CLASS: &str = "flex flex-wrap items-baseline gap-x-3 gap-y-1 min-w-0";
const META_CLASS: &str =
    "flex flex-wrap items-center gap-x-3 gap-y-1 min-w-0 text-[12px] sm:text-[13px] text-ink-500";
const METRIC_CLASS: &str = "inline-flex items-center gap-1";
const DEPENDENTS_DESCRIPTION: &str =
    "Other indexed repositories that directly depend on this package, counted once each.";
const CLOCK_ICON: &str = include_str!("../../../../../vendor/lucide/clock.svg");

/// Render a package's identity and description above its release metadata.
pub(crate) fn render(pkg: &KnownPackage) -> Division {
    render_at(pkg, Utc::now())
}

fn render_at(pkg: &KnownPackage, now: DateTime<Utc>) -> Division {
    let (display_name, href) = listing_identity(pkg);
    let primary = primary_row(&display_name, pkg.description.as_deref());
    let metadata = metadata_row(pkg, now);

    let mut row = Division::builder();
    match href {
        Some(href) => {
            let class = format!(
                "{ROW_CLASS} hover:bg-surfaceMuted focus-visible:bg-surfaceMuted transition-colors motion-reduce:transition-none"
            );
            row.anchor(|a| {
                a.href(escape_html_attr(&href))
                    .class(class)
                    .push(primary)
                    .push(metadata)
            });
        }
        None => {
            row.class(ROW_CLASS).push(primary).push(metadata);
        }
    }
    row.build()
}

/// Extract display name and optional href from a package.
pub(crate) fn identity(pkg: &KnownPackage) -> (String, Option<String>) {
    match (&pkg.wit_namespace, &pkg.wit_name) {
        (Some(ns), Some(name)) => (format!("{ns}:{name}"), Some(format!("/{ns}/{name}"))),
        _ => (pkg.repository.clone(), None),
    }
}

fn listing_identity(pkg: &KnownPackage) -> (String, Option<String>) {
    match (&pkg.wit_namespace, &pkg.wit_name) {
        (Some(ns), Some(name)) => (format!("{ns}/{name}"), Some(format!("/{ns}/{name}"))),
        _ => (pkg.repository.clone(), None),
    }
}

fn primary_row(display_name: &str, description: Option<&str>) -> Span {
    let mut row = Span::builder();
    row.class(PRIMARY_CLASS).span(|name| {
        name.class(
            "min-w-0 max-w-full mono text-[14px] font-medium text-ink-900 [overflow-wrap:anywhere]",
        )
        .text(escape_html_text(display_name))
    });
    if let Some(description) = description.map(str::trim).filter(|text| !text.is_empty()) {
        row.span(|desc| {
            desc.class("flex-1 basis-64 min-w-0 max-w-full text-[13px] text-ink-500 truncate")
                .title(escape_html_attr(description))
                .text(crate::markdown::render_summary(description))
        });
    }
    row.build()
}

fn metadata_row(pkg: &KnownPackage, now: DateTime<Utc>) -> Span {
    let version = version_label(pkg.tags.first().map(String::as_str));
    let mut row = Span::builder();
    row.class(META_CLASS)
        .push(kind_label(pkg.kind))
        .span(|s| {
            s.class("min-w-0 max-w-full mono [overflow-wrap:anywhere]")
                .text(escape_html_text(&version))
        })
        .push(dependents(pkg.dependents));
    if let Some(age) = updated_at(pkg.latest_release_at.as_deref(), now) {
        row.push(age);
    }
    row.build()
}

fn version_label(version: Option<&str>) -> String {
    match version {
        Some(version) => {
            let number = version.trim_start_matches(['v', 'V']);
            format!("v{number}")
        }
        None => "\u{2014}".to_owned(),
    }
}

fn dependents(count: Option<u64>) -> Span {
    let label = match count {
        Some(1) => "1 dependent".to_owned(),
        Some(count) => format!("{count} dependents"),
        None => "0 dependents (count unavailable)".to_owned(),
    };
    let visible = count.unwrap_or(0).to_string();
    Span::builder()
        .class(format!("{METRIC_CLASS} cursor-help"))
        .title(format!("{label}. {DEPENDENTS_DESCRIPTION}"))
        .push(decorative_svg(&icons::package_dependents(14)))
        .push(accessible_text(&visible, &label))
        .build()
}

fn decorative_svg(trusted_svg: &str) -> Span {
    Span::builder()
        .class("inline-flex shrink-0")
        .text(hidden_content(trusted_svg))
        .build()
}

fn accessible_text(visible: &str, label: &str) -> Span {
    Span::builder()
        .text(hidden_content(&escape_html_text(visible)))
        .span(|s| s.class("sr-only").text(escape_html_text(label)))
        .build()
}

fn hidden_content(trusted_html: &str) -> String {
    // The html crate's boolean ARIA builder emits `aria-hidden` without a value.
    format!(r#"<span aria-hidden="true">{trusted_html}</span>"#)
}

fn updated_at(timestamp: Option<&str>, now: DateTime<Utc>) -> Option<Span> {
    let Some(age) = Age::parse("Updated", timestamp?, now) else {
        eprintln!("component-frontend: invalid latest release timestamp");
        return None;
    };
    let visible = age.label.strip_suffix(" ago").unwrap_or(&age.label);
    let label = format!("Updated {}", age.label);
    let text = accessible_text(visible, &label);
    Some(
        Span::builder()
            .class(METRIC_CLASS)
            .push(decorative_svg(&icons::icon(14, CLOCK_ICON)))
            .time(|time| {
                time.date_time(age.datetime)
                    .title(escape_html_attr(&age.title))
                    .push(text)
            })
            .build(),
    )
}

fn kind_label(kind: Option<PackageKind>) -> Span {
    let (background, ink, text) = match kind {
        Some(PackageKind::Component) => ("bg-cat-green", "text-cat-greenInk", "component"),
        Some(PackageKind::Interface) => ("bg-cat-blue", "text-cat-blueInk", "interface"),
        None => ("bg-cat-slate", "text-cat-slateInk", "package"),
    };
    labels::inline_label(background, ink, text)
}

#[cfg(test)]
pub(crate) mod tests;
