//! Three-column package highlights for the landing page: new releases, new
//! packages, and popular packages.
//!
//! Each column is a standalone card using the landing page's card shell
//! (hairline border on `surface`, card elevation — as the hero search card
//! and install card): a header strip with the sans-serif column title, then rows
//! divided by `lineSoft` hairlines. A row shows the package name in the site's
//! sans-serif font and an optional description of up to two lines on the left.
//! On the right, two lines show the version and either date or dependents.
//! Release rows also show how long ago they were published, and new-package
//! rows how long ago the registry first indexed them. Clock and dependents
//! icons accompany compact labels; version labels have a single `v` prefix.
//! Cards stack on narrow viewports and sit side by side from `md` up.

use std::fmt::Write as _;

use chrono::{DateTime, SecondsFormat, Utc};
use wasm_meta_registry_client::{KnownPackage, NewPackage, PackageRelease, PopularPackage};

use super::{icons, package_row};
use crate::escape::{escape_html_attr, escape_html_text};
use crate::relative_time::relative_age;

/// A single row in a highlight column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ColumnRow {
    /// Display name (e.g. `wasi:http`, or the repository path).
    pub name: String,
    /// Link target, when the package has a WIT identity.
    pub href: Option<String>,
    /// The original version tag, before display formatting.
    pub version: String,
    /// Optional description, clamped to two lines.
    pub description: Option<String>,
    /// The final metadata line: event age or dependents count.
    pub metric: Option<ColumnMetric>,
}

/// The metric shown below a highlight's version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ColumnMetric {
    /// When the release was published or the package was first indexed.
    Age(Age),
    /// The number of distinct other repositories directly depending on it.
    Dependents(u64),
}

/// When something happened, rendered as a relative age with the event and
/// exact date on hover.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Age {
    /// Machine-readable timestamp for the `<time datetime>` attribute.
    pub datetime: String,
    /// Tooltip naming the event and its date (e.g. `Released 2026-07-08`).
    pub title: String,
    /// Relative age (e.g. `3 days ago`).
    pub label: String,
}

impl Age {
    /// Describe an RFC 3339 timestamp of `event` (e.g. `"Released"`)
    /// relative to `now`. Returns `None` when the timestamp can't be parsed.
    #[must_use]
    pub(crate) fn parse(event: &str, rfc3339: &str, now: DateTime<Utc>) -> Option<Self> {
        let then = DateTime::parse_from_rfc3339(rfc3339)
            .ok()?
            .with_timezone(&Utc);
        Some(Self {
            datetime: then.to_rfc3339_opts(SecondsFormat::Secs, true),
            title: format!("{event} {}", then.format("%Y-%m-%d")),
            label: relative_age(then, now),
        })
    }
}

impl ColumnRow {
    /// Row for a newly added package, showing its latest version and how
    /// long before `now` the registry first indexed it.
    #[must_use]
    pub(crate) fn new_package(new: &NewPackage, now: DateTime<Utc>) -> Self {
        Self::with_metadata(
            &new.package,
            new.package.tags.first().cloned().unwrap_or_default(),
            Age::parse("First indexed", &new.first_indexed_at, now).map(ColumnMetric::Age),
        )
    }

    /// Row for a single release, showing the released version and how long
    /// before `now` it was published.
    #[must_use]
    pub(crate) fn release(release: &PackageRelease, now: DateTime<Utc>) -> Self {
        Self::with_metadata(
            &release.package,
            release.version.clone(),
            Age::parse("Released", &release.released_at, now).map(ColumnMetric::Age),
        )
    }

    /// Row for a popular package, showing how many packages depend on it.
    #[must_use]
    pub(crate) fn popular(popular: &PopularPackage) -> Self {
        Self::with_metadata(
            &popular.package,
            popular.package.tags.first().cloned().unwrap_or_default(),
            Some(ColumnMetric::Dependents(popular.dependents)),
        )
    }

    fn with_metadata(pkg: &KnownPackage, version: String, metric: Option<ColumnMetric>) -> Self {
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
            version,
            description,
            metric,
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
    pub(crate) fn from_result<T, E>(
        result: Result<Vec<T>, E>,
        row: impl Fn(&T) -> ColumnRow,
    ) -> Self {
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

const CARD_CLASS: &str =
    "min-w-0 flex flex-col rounded-lg border border-line bg-surface shadow-card overflow-hidden";
const HEADER_CLASS: &str = "flex items-center h-10 px-4 border-b border-lineSoft";
const HEADING_CLASS: &str = "text-[12px] font-sans uppercase tracking-wider text-ink-500";
const LIST_CLASS: &str = "divide-y divide-lineSoft";
// Each row is a three-line grid: the name (one line) and description (two
// lines, clamped) share a left column, and version / metric sit in a
// right column the description can never run into. Every
// cell has a fixed height and the description cell is always present, so
// all rows are the same height however long their description is, or
// whether they have one at all.
const ROW_GRID: &str = "grid grid-cols-[minmax(0,1fr)_auto] grid-rows-[repeat(3,1.25rem)] items-center gap-x-4 gap-y-0.5 px-4 py-3";
const ROW_CLASS: &str = "no-underline hover:bg-surfaceMuted focus-visible:bg-surfaceMuted";
const NAME_CLASS: &str = "h-5 leading-5 font-sans text-[14px] font-medium text-ink-900 truncate";
const VERSION_CLASS: &str = "col-start-2 row-start-1 h-5 leading-5 max-w-[20ch] mono text-[12px] text-ink-500 tabular-nums text-right truncate";
const DESC_CLASS: &str = "col-start-1 row-start-2 row-span-2 self-start h-10 leading-5 text-[13px] text-ink-500 line-clamp-2 break-words";
const METRIC_CLASS: &str = "col-start-2 row-start-2 h-5 leading-5 max-w-[20ch] mono text-[12px] text-ink-500 tabular-nums text-right whitespace-nowrap inline-flex items-center justify-end gap-1";
const DESC_MISSING: &str = "No description";
const NOTE_CLASS: &str = "px-4 py-3 text-[13px] text-ink-500";
const CLOCK_ICON: &str = include_str!("../../../../../vendor/lucide/clock.svg");
const DEPENDENTS_DESCRIPTION: &str =
    "Other indexed repositories that directly depend on this package, counted once each.";

/// Render the highlight columns as a full-width landing-page band.
#[must_use]
pub(crate) fn render(columns: &[Column<'_>]) -> String {
    let mut cols = String::new();
    for column in columns {
        push_column(&mut cols, column);
    }
    format!(
        r#"<section aria-label="Package highlights" data-package-columns class="mx-auto max-w-[1280px] w-full px-4 md:px-8 mt-12 md:mt-16"><div class="grid grid-cols-1 md:grid-cols-3 gap-4 md:gap-6">{cols}</div></section>"#
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
        r#"<div class="{CARD_CLASS}"><div class="{HEADER_CLASS}"><h2 class="{HEADING_CLASS}">{title}</h2></div>{body}</div>"#
    );
}

/// Render the row list for a column.
fn render_list(rows: &[ColumnRow]) -> String {
    let mut items = String::new();
    for row in rows {
        let _ = write!(items, "<li>{}</li>", render_row(row));
    }
    format!(r#"<ol class="{LIST_CLASS}">{items}</ol>"#)
}

/// Render a single row, linked when the package has a WIT identity.
fn render_row(row: &ColumnRow) -> String {
    let name = escape_html_text(&row.name);
    let version = render_version(&row.version);
    let description = render_description(row.description.as_deref());
    let metric = row.metric.as_ref().map_or_else(String::new, render_metric);
    let inner =
        format!(r#"<span class="{NAME_CLASS}">{name}</span>{version}{description}{metric}"#);
    match &row.href {
        Some(href) => format!(
            r#"<a href="{}" class="{ROW_GRID} {ROW_CLASS}">{inner}</a>"#,
            escape_html_attr(href)
        ),
        None => format!(r#"<div class="{ROW_GRID}">{inner}</div>"#),
    }
}

fn render_version(version: &str) -> String {
    let title = escape_html_attr(version);
    let label = escape_html_text(&version_label(version));
    format!(r#"<span title="{title}" class="{VERSION_CLASS}">{label}</span>"#)
}

fn render_metric(metric: &ColumnMetric) -> String {
    match metric {
        ColumnMetric::Age(age) => render_age(age),
        ColumnMetric::Dependents(count) => {
            let noun = if *count == 1 {
                "dependent"
            } else {
                "dependents"
            };
            let label = format!("{count} {noun}");
            let title = format!("{label}. {DEPENDENTS_DESCRIPTION}");
            let content =
                metric_contents(&icons::package_dependents(14), &count.to_string(), &label);
            format!(r#"<span title="{title}" class="{METRIC_CLASS} cursor-help">{content}</span>"#)
        }
    }
}

fn version_label(version: &str) -> String {
    if version.is_empty() {
        return String::new();
    }
    let number = version.trim_start_matches(['v', 'V']);
    format!("v{number}")
}

fn metric_contents(icon: &str, visible: &str, label: &str) -> String {
    format!(
        r#"<span aria-hidden="true" class="inline-flex shrink-0">{icon}</span><span aria-hidden="true" class="truncate">{}</span><span class="sr-only">{}</span>"#,
        escape_html_text(visible),
        escape_html_text(label),
    )
}

/// Reserve two lines for the description, even when it is missing.
fn render_description(description: Option<&str>) -> String {
    match description {
        Some(d) => format!(
            r#"<span class="{DESC_CLASS}">{}</span>"#,
            escape_html_text(d)
        ),
        None => format!(r#"<span class="{DESC_CLASS} italic">{DESC_MISSING}</span>"#),
    }
}

fn render_age(age: &Age) -> String {
    let visible = age.label.strip_suffix(" ago").unwrap_or(&age.label);
    let content = metric_contents(
        &icons::icon(14, CLOCK_ICON),
        visible,
        &format!("{}, {}", age.title, age.label),
    );
    format!(
        r#"<time datetime="{}" title="{}" class="{METRIC_CLASS}">{content}</time>"#,
        escape_html_attr(&age.datetime),
        escape_html_attr(&age.title),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_meta_registry_client::PackageKind;

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

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-09-24T12:00:00Z")
            .expect("valid timestamp")
            .with_timezone(&Utc)
    }

    fn sample() -> String {
        let mut http = pkg("wasi", "http", &["0.2.1", "0.2.0"], Some("HTTP <types>"));
        http.kind = Some(PackageKind::Interface);
        let io = pkg("wasi", "io", &["0.2.0"], None);
        let mut raw = pkg("x", "y", &["1.0.0"], Some("  "));
        raw.wit_namespace = None;
        raw.wit_name = None;
        raw.kind = Some(PackageKind::Component);
        render(&[
            Column {
                title: "New releases",
                state: &ColumnState::Rows(vec![
                    ColumnRow::release(
                        &PackageRelease {
                            package: http.clone(),
                            version: "0.2.0".into(),
                            released_at: "2026-09-21T08:30:00.123456+02:00".into(),
                        },
                        now(),
                    ),
                    ColumnRow::release(
                        &PackageRelease {
                            package: io.clone(),
                            version: "0.2.0".into(),
                            released_at: "not a date".into(),
                        },
                        now(),
                    ),
                ]),
            },
            Column {
                title: "New packages",
                state: &ColumnState::Rows(vec![ColumnRow::new_package(
                    &NewPackage {
                        package: raw,
                        first_indexed_at: "2026-08-24T12:00:00Z".into(),
                    },
                    now(),
                )]),
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
    fn headings_use_sans_while_metadata_remains_monospace() {
        let html = sample();
        assert_eq!(
            html.matches(
                r#"<h2 class="text-[12px] font-sans uppercase tracking-wider text-ink-500">"#
            )
            .count(),
            3,
            "all three column titles use sans"
        );
        assert_eq!(
            html.matches(
                r#"class="h-5 leading-5 font-sans text-[14px] font-medium text-ink-900 truncate""#
            )
            .count(),
            5,
            "linked and repository-only names across all three columns use sans"
        );
        for class in [VERSION_CLASS, METRIC_CLASS] {
            assert!(class.split_whitespace().any(|token| token == "mono"));
            assert!(html.contains(&format!(r#"class="{class}""#)));
        }
    }

    #[test]
    fn rows_show_details_and_escape_text() {
        let html = sample();
        assert!(html.contains(r#"href="/wasi/http""#));
        assert!(html.contains(">v0.2.0</span>"), "release shows its version");
        assert!(
            html.contains(r#"title="0.2.0""#),
            "truncatable labels keep their full text"
        );
        assert!(html.contains(r#"class="sr-only">12 dependents</span>"#));
        assert!(html.contains(r#"class="sr-only">1 dependent</span>"#));
        assert!(
            html.contains("HTTP &lt;types&gt;"),
            "description is escaped"
        );
        // Packages without a WIT identity render unlinked, and blank
        // descriptions are dropped in favour of an italic placeholder.
        assert!(html.contains(">x/y</span>"));
        assert!(!html.contains(r#"href="/x"#));
        let rows = html.matches(ROW_GRID).count();
        assert_eq!(html.matches(DESC_CLASS).count(), rows);
        assert_eq!(
            html.matches(&format!(
                r#"class="{DESC_CLASS} italic">{DESC_MISSING}</span>"#
            ))
            .count(),
            rows - 2
        );
    }

    #[test]
    fn releases_and_new_packages_show_how_long_ago() {
        let html = sample();
        assert!(html.contains(
            r#"<time datetime="2026-09-21T06:30:00Z" title="Released 2026-09-21" class="#
        ));
        assert!(html.contains(r#"aria-hidden="true" class="truncate">3 days</span>"#));
        assert!(html.contains(r#"class="sr-only">Released 2026-09-21, 3 days ago</span>"#));
        assert!(html.contains(
            r#"<time datetime="2026-08-24T12:00:00Z" title="First indexed 2026-08-24" class="#
        ));
        assert!(html.contains(r#"aria-hidden="true" class="truncate">4 weeks</span>"#));
        assert!(html.contains(r#"class="sr-only">First indexed 2026-08-24, 4 weeks ago</span>"#));
        assert_eq!(
            html.matches("<time").count(),
            2,
            "unparseable timestamps and popular rows show no age"
        );
    }

    #[test]
    fn version_prefix_is_display_only_and_never_doubled() {
        for (tag, label) in [
            ("0.2.0", "v0.2.0"),
            ("v0.2.0", "v0.2.0"),
            ("vv1.2.3", "v1.2.3"),
            ("vV1.2.3", "v1.2.3"),
            ("VVv1.2.3", "v1.2.3"),
            ("V1.0.0-rc.1+build.42", "v1.0.0-rc.1+build.42"),
            ("", ""),
            ("<tag>", "v&lt;tag&gt;"),
        ] {
            let package = pkg("wasi", "http", &[tag], None);
            let release = ColumnRow::release(
                &PackageRelease {
                    package: package.clone(),
                    version: tag.to_owned(),
                    released_at: String::new(),
                },
                now(),
            );
            let new = ColumnRow::new_package(
                &NewPackage {
                    package,
                    first_indexed_at: String::new(),
                },
                now(),
            );
            let popular = ColumnRow::popular(&PopularPackage {
                package: pkg("wasi", "http", &[tag], None),
                dependents: 3,
            });
            for row in [release, new, popular] {
                assert_eq!(row.version, tag);
                let html = render_row(&row);
                assert!(html.contains(&format!(r#"class="{VERSION_CLASS}">{label}</span>"#)));
                assert!(html.contains(r#"href="/wasi/http""#));
            }
        }
    }

    #[test]
    fn missing_version_and_date_stay_absent() {
        let row = ColumnRow::new_package(
            &NewPackage {
                package: pkg("wasi", "http", &[], None),
                first_indexed_at: String::new(),
            },
            now(),
        );
        let html = render_row(&row);
        assert!(html.contains(&format!(r#"class="{VERSION_CLASS}"></span>"#)));
        assert!(!html.contains("<time"));
        assert!(!html.contains("<svg"));
    }

    #[test]
    fn metric_icons_are_decorative_and_counts_keep_their_meaning() {
        let html = sample();
        assert_eq!(html.matches(&icons::icon(14, CLOCK_ICON)).count(), 2);
        assert_eq!(html.matches(&icons::package_dependents(14)).count(), 2);
        assert_eq!(
            html.matches(r#"aria-hidden="true" class="inline-flex shrink-0"><svg"#)
                .count(),
            4
        );
        let zero = render_metric(&ColumnMetric::Dependents(0));
        assert!(zero.contains(r#"aria-hidden="true" class="truncate">0</span>"#));
        assert!(zero.contains(r#"class="sr-only">0 dependents</span>"#));
        assert!(zero.contains(&format!(
            r#"title="0 dependents. {DEPENDENTS_DESCRIPTION}""#
        )));
    }

    #[test]
    fn package_kinds_do_not_render_labels() {
        let html = sample();
        assert!(!html.contains("bar-sm"));
        assert!(!html.contains("bg-cat-"));
        assert!(!html.contains(">component</span>"));
        assert!(!html.contains(">interface</span>"));
        assert!(!html.contains(">package</span>"));
        assert_eq!(html.matches(VERSION_CLASS).count(), 5);
    }

    #[test]
    fn versions_and_metrics_match_each_columns_semantics() {
        let package = pkg("wasi", "http", &["0.2.1", "0.2.0"], None);
        let release = ColumnRow::release(
            &PackageRelease {
                package: package.clone(),
                version: "0.2.0".to_owned(),
                released_at: "2026-09-21T12:00:00Z".to_owned(),
            },
            now(),
        );
        let new = ColumnRow::new_package(
            &NewPackage {
                package: package.clone(),
                first_indexed_at: "2026-09-20T12:00:00Z".to_owned(),
            },
            now(),
        );
        let popular = ColumnRow::popular(&PopularPackage {
            package,
            dependents: 12,
        });
        assert_eq!(release.version, "0.2.0", "release uses its selected tag");
        assert_eq!(new.version, "0.2.1", "new package uses the first tag");
        assert_eq!(
            popular.version, "0.2.1",
            "popular package uses the first tag"
        );
        assert_eq!(popular.metric, Some(ColumnMetric::Dependents(12)));
        assert!(render_row(&release).contains("Released 2026-09-21"));
        assert!(render_row(&new).contains("First indexed 2026-09-20"));
        assert!(!render_row(&popular).contains("<time"));
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
        assert_eq!(
            html.matches(CARD_CLASS).count(),
            2,
            "each column renders as its own card, even when empty"
        );
        assert!(html.contains("Nothing here yet."));
        assert!(html.contains(r#"role="status""#) && html.contains("Unavailable right now."));
        assert!(!html.contains("<ol"));
    }
}
