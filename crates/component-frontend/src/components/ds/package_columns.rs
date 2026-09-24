//! Three-column package highlights for the landing page: new releases, new
//! packages, and popular packages.
//!
//! Each column has a mono kicker heading, a heavy top rule, and a hairline
//! between rows. A row shows the package name with a muted mono detail on the
//! right (version or dependents count) and an optional one-line description.
//! Columns stack on narrow viewports and sit side by side from `md` up.

use std::fmt::Write as _;

use wasm_meta_registry_client::{KnownPackage, PackageRelease, PopularPackage};

use super::package_row;
use crate::escape::{escape_html_attr, escape_html_text};

/// A single row in a highlight column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ColumnRow {
    /// Display name (e.g. `wasi:http`, or the repository path).
    pub name: String,
    /// Link target, when the package has a WIT identity.
    pub href: Option<String>,
    /// Muted detail shown on the right (version, dependents count).
    pub detail: String,
    /// Optional one-line description.
    pub description: Option<String>,
}

impl ColumnRow {
    /// Row for a package, showing its latest version.
    #[must_use]
    pub(crate) fn package(pkg: &KnownPackage) -> Self {
        let detail = pkg.tags.first().cloned().unwrap_or_default();
        Self::with_detail(pkg, detail)
    }

    /// Row for a single release, showing the released version.
    #[must_use]
    pub(crate) fn release(release: &PackageRelease) -> Self {
        Self::with_detail(&release.package, release.version.clone())
    }

    /// Row for a popular package, showing how many packages depend on it.
    #[must_use]
    pub(crate) fn popular(popular: &PopularPackage) -> Self {
        let noun = if popular.dependents == 1 {
            "dependent"
        } else {
            "dependents"
        };
        Self::with_detail(&popular.package, format!("{} {noun}", popular.dependents))
    }

    fn with_detail(pkg: &KnownPackage, detail: String) -> Self {
        let (name, href) = package_row::identity(pkg);
        let description = pkg
            .description
            .as_deref()
            .map(str::trim)
            .filter(|d| !d.is_empty())
            .map(str::to_owned);
        Self {
            name,
            href,
            detail,
            description,
        }
    }
}

/// What a column has to show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ColumnState {
    /// Rows loaded successfully (possibly empty).
    Rows(Vec<ColumnRow>),
    /// The data could not be loaded.
    Unavailable,
}

impl Default for ColumnState {
    fn default() -> Self {
        Self::Rows(Vec::new())
    }
}

impl ColumnState {
    /// Build a column state from a fallible fetch, mapping each item to a row.
    #[must_use]
    pub(crate) fn from_result<T, E>(result: Result<Vec<T>, E>, row: fn(&T) -> ColumnRow) -> Self {
        match result {
            Ok(items) => Self::Rows(items.iter().map(row).collect()),
            Err(_) => Self::Unavailable,
        }
    }
}

/// A titled highlight column.
pub(crate) struct Column<'a> {
    /// Column heading (e.g. "New releases").
    pub title: &'a str,
    /// Column content.
    pub state: &'a ColumnState,
}

const HEADING_CLASS: &str = "text-[12px] mono uppercase tracking-wider text-ink-500";
const LIST_CLASS: &str = "mt-4 border-t-[1.5px] border-lineSoft";
const ROW_CLASS: &str = "group block py-3 no-underline";
const NAME_CLASS: &str = "mono text-[14px] font-medium text-ink-900 truncate group-hover:underline decoration-1 underline-offset-4";
const DETAIL_CLASS: &str = "mono text-[12px] text-ink-500 tabular-nums shrink-0";
const DESC_CLASS: &str = "mt-0.5 block text-[13px] text-ink-500 truncate";
const NOTE_CLASS: &str = "mt-4 border-t-[1.5px] border-lineSoft pt-3 text-[13px] text-ink-500";

/// Render the highlight columns as a full-width landing-page band.
#[must_use]
pub(crate) fn render(columns: &[Column<'_>]) -> String {
    let mut cols = String::new();
    for column in columns {
        push_column(&mut cols, column);
    }
    format!(
        r#"<section aria-label="Package highlights" data-package-columns class="mx-auto max-w-[1280px] w-full px-4 md:px-8 mt-12 md:mt-16"><div class="grid grid-cols-1 md:grid-cols-3 gap-10 md:gap-8">{cols}</div></section>"#
    )
}

/// Append one column (heading + list or note) to `out`.
fn push_column(out: &mut String, column: &Column<'_>) {
    let title = escape_html_text(column.title);
    let body = match &column.state {
        ColumnState::Rows(rows) if rows.is_empty() => {
            format!(r#"<p class="{NOTE_CLASS}">Nothing here yet.</p>"#)
        }
        ColumnState::Rows(rows) => render_list(rows),
        ColumnState::Unavailable => {
            format!(r#"<p role="status" class="{NOTE_CLASS}">Unavailable right now.</p>"#)
        }
    };
    let _ = write!(
        out,
        r#"<div class="min-w-0"><h2 class="{HEADING_CLASS}">{title}</h2>{body}</div>"#
    );
}

/// Render the row list for a column.
fn render_list(rows: &[ColumnRow]) -> String {
    let mut items = String::new();
    for row in rows {
        let _ = write!(
            items,
            r#"<li class="border-b border-lineSoft">{}</li>"#,
            render_row(row)
        );
    }
    format!(r#"<ol class="{LIST_CLASS}">{items}</ol>"#)
}

/// Render a single row, linked when the package has a WIT identity.
fn render_row(row: &ColumnRow) -> String {
    let name = escape_html_text(&row.name);
    let detail = escape_html_text(&row.detail);
    let description = row.description.as_deref().map_or_else(String::new, |d| {
        format!(
            r#"<span class="{DESC_CLASS}">{}</span>"#,
            escape_html_text(d)
        )
    });
    let inner = format!(
        r#"<span class="flex items-baseline justify-between gap-4"><span class="{NAME_CLASS}">{name}</span><span class="{DETAIL_CLASS}">{detail}</span></span>{description}"#
    );
    match &row.href {
        Some(href) => format!(
            r#"<a href="{}" class="{ROW_CLASS}">{inner}</a>"#,
            escape_html_attr(href)
        ),
        None => format!(r#"<div class="block py-3">{inner}</div>"#),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pkg(ns: &str, name: &str, tags: &[&str], description: Option<&str>) -> KnownPackage {
        KnownPackage {
            registry: "ghcr.io".into(),
            repository: format!("{ns}/{name}"),
            kind: None,
            description: description.map(str::to_owned),
            tags: tags.iter().map(|s| (*s).to_owned()).collect(),
            signature_tags: vec![],
            attestation_tags: vec![],
            last_seen_at: String::new(),
            created_at: String::new(),
            wit_namespace: Some(ns.into()),
            wit_name: Some(name.into()),
            dependencies: vec![],
        }
    }

    fn sample() -> String {
        let http = pkg("wasi", "http", &["0.2.1", "0.2.0"], Some("HTTP <types>"));
        let io = pkg("wasi", "io", &["0.2.0"], None);
        let mut raw = pkg("x", "y", &["1.0.0"], Some("  "));
        raw.wit_namespace = None;
        raw.wit_name = None;
        render(&[
            Column {
                title: "New releases",
                state: &ColumnState::Rows(vec![ColumnRow::release(&PackageRelease {
                    package: http.clone(),
                    version: "0.2.0".into(),
                    released_at: String::new(),
                })]),
            },
            Column {
                title: "New packages",
                state: &ColumnState::Rows(vec![ColumnRow::package(&raw)]),
            },
            Column {
                title: "Popular packages",
                state: &ColumnState::Rows(vec![
                    ColumnRow::popular(&PopularPackage {
                        package: io,
                        dependents: 12,
                    }),
                    ColumnRow::popular(&PopularPackage {
                        package: http,
                        dependents: 1,
                    }),
                ]),
            },
        ])
    }

    #[test]
    fn snapshot() {
        insta::assert_snapshot!(crate::components::ds::pretty_html(&sample()));
    }

    #[test]
    fn rows_show_details_and_escape_text() {
        let html = sample();
        assert!(html.contains(r#"href="/wasi/http""#));
        assert!(html.contains(">0.2.0</span>"), "release shows its version");
        assert!(html.contains(">12 dependents</span>"));
        assert!(html.contains(">1 dependent</span>"));
        assert!(
            html.contains("HTTP &lt;types&gt;"),
            "description is escaped"
        );
        // Packages without a WIT identity render unlinked, and blank
        // descriptions are dropped.
        assert!(html.contains(">x/y</span>"));
        assert!(!html.contains(r#"href="/x"#));
        assert_eq!(html.matches(DESC_CLASS).count(), 2);
    }

    #[test]
    fn empty_and_unavailable_columns_render_notes() {
        let html = render(&[
            Column {
                title: "New releases",
                state: &ColumnState::Rows(vec![]),
            },
            Column {
                title: "Popular packages",
                state: &ColumnState::from_result::<PopularPackage, ()>(Err(()), ColumnRow::popular),
            },
        ]);
        assert!(html.contains("Nothing here yet."));
        assert!(html.contains(r#"role="status""#) && html.contains("Unavailable right now."));
        assert!(!html.contains("<ol"));
    }
}
