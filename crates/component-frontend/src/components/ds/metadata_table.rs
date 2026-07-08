//! Metadata table component.
//!
//! Renders all available metadata for a component or module in a structured
//! key-value table at the bottom of the page. Consolidates producers,
//! dependencies, languages, size, and other fields into a single section.

use html::tables::{Table, TableBody, TableRow};
use html::text_content::Division;
use wasm_meta_registry_client::{BomEntry, ComponentSummary, PackageVersion};

/// Chevron SVG for collapsible sections — points left when collapsed, down when open.
const CHEVRON_SVG: &str = r#"<svg class="h-4 w-4 text-ink-400 transition-transform duration-200 rotate-90 group-open:rotate-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="m6 9 6 6 6-6"/></svg>"#;

/// Render a metadata table for a component or module.
///
/// Returns `None` if there is no metadata to display.
#[must_use]
pub(crate) fn render(child: &ComponentSummary) -> Option<Division> {
    let rows = collect_rows(child);
    if rows.is_empty() {
        return None;
    }

    let mut div = Division::builder();

    let mut tbody = TableBody::builder();
    tbody.class("text-ink-900");
    for row in &rows {
        tbody.push(render_row(row));
    }

    let table = Table::builder()
        .class("w-full text-[13px]")
        .push(tbody.build())
        .build();
    let table_html = format!(r#"<div class="rounded-lg bg-surface overflow-hidden">{table}</div>"#);

    div.text(format!(
        r#"<details open class="group space-y-3"><summary class="flex items-center justify-between cursor-pointer list-none [&::-webkit-details-marker]:hidden"><h2 class="text-[22px] font-semibold tracking-tight text-ink-700">Metadata</h2>{CHEVRON_SVG}</summary>{table_html}</details>"#
    ));
    Some(div.build())
}

/// Render a metadata table for a package version (WIT-only packages).
///
/// Returns `None` if there is no metadata to display.
#[must_use]
pub(crate) fn render_version(version: &PackageVersion) -> Option<Division> {
    let rows = collect_version_rows(version);
    if rows.is_empty() {
        return None;
    }

    let mut div = Division::builder();

    let mut tbody = TableBody::builder();
    tbody.class("text-ink-900");
    for row in &rows {
        tbody.push(render_row(row));
    }

    let table = Table::builder()
        .class("w-full text-[13px]")
        .push(tbody.build())
        .build();
    let table_html = format!(r#"<div class="rounded-lg bg-surface overflow-hidden">{table}</div>"#);

    div.text(format!(
        r#"<details open class="group space-y-3"><summary class="flex items-center justify-between cursor-pointer list-none [&::-webkit-details-marker]:hidden"><h2 class="text-[22px] font-semibold tracking-tight text-ink-700">Metadata</h2>{CHEVRON_SVG}</summary>{table_html}</details>"#
    ));
    Some(div.build())
}

/// A single metadata row.
#[allow(dead_code)]
enum MetadataRow {
    /// Simple key-value text.
    Text { label: String, value: String },
    /// Key-value with monospace value.
    Range {
        label: String,
        start: String,
        end: String,
    },
    /// Key-value with a link.
    Link {
        label: String,
        text: String,
        href: String,
    },
    /// A producer entry (field → name@version).
    Producer {
        field: String,
        name: String,
        version: String,
    },
    /// A dependency entry (name@version with optional link).
    Dependency {
        name: String,
        version: String,
        href: Option<String>,
    },
}

/// Collect all metadata rows from a component summary.
fn collect_rows(child: &ComponentSummary) -> Vec<MetadataRow> {
    let mut rows = Vec::new();

    if let Some(bytes) = child.size_bytes {
        rows.push(MetadataRow::Text {
            label: "Size".into(),
            value: crate::pages::package::format_size(bytes),
        });
    }

    if let (Some(start), Some(end)) = (child.range_start, child.range_end) {
        rows.push(MetadataRow::Range {
            label: "Range".into(),
            start: format_hex_offset(start),
            end: format_hex_offset(end),
        });
    }

    if !child.languages.is_empty() {
        rows.push(MetadataRow::Text {
            label: "Languages".into(),
            value: child.languages.join(", "),
        });
    }

    if let Some(v) = &child.component_version {
        rows.push(MetadataRow::Text {
            label: "Version".into(),
            value: v.clone(),
        });
    }

    if let Some(authors) = &child.authors {
        rows.push(MetadataRow::Text {
            label: "Authors".into(),
            value: authors.clone(),
        });
    }

    if let Some(rev) = &child.revision {
        rows.push(MetadataRow::Text {
            label: "Revision".into(),
            value: rev.clone(),
        });
    }

    // Producers (excluding language — already shown above)
    for entry in child.producers.iter().filter(|e| e.field != "language") {
        rows.push(MetadataRow::Producer {
            field: title_case(&entry.field),
            name: entry.name.clone(),
            version: entry.version.clone(),
        });
    }

    // Dependencies (bill of materials)
    for dep in &child.bill_of_materials {
        rows.push(dependency_row(dep));
    }

    rows
}

/// Collect metadata rows from a package version.
fn collect_version_rows(version: &PackageVersion) -> Vec<MetadataRow> {
    let mut rows = Vec::new();

    #[allow(clippy::cast_sign_loss)]
    if let Some(bytes) = version.size_bytes
        && bytes > 0
    {
        rows.push(MetadataRow::Text {
            label: "Size".into(),
            value: crate::pages::package::format_size(bytes as u64),
        });
    }

    // Pull range from the root component if available.
    if let Some(comp) = version.components.first()
        && let (Some(start), Some(end)) = (comp.range_start, comp.range_end)
    {
        rows.push(MetadataRow::Range {
            label: "Range".into(),
            start: format_hex_offset(start),
            end: format_hex_offset(end),
        });
    }

    if let Some(created) = &version.created_at {
        rows.push(MetadataRow::Text {
            label: "Created".into(),
            value: created.clone(),
        });
    }

    if let Some(ann) = &version.annotations {
        if let Some(authors) = &ann.authors {
            rows.push(MetadataRow::Text {
                label: "Authors".into(),
                value: authors.clone(),
            });
        }
        if let Some(vendor) = &ann.vendor {
            rows.push(MetadataRow::Text {
                label: "Vendor".into(),
                value: vendor.clone(),
            });
        }
        if let Some(rev) = &ann.revision {
            rows.push(MetadataRow::Text {
                label: "Revision".into(),
                value: rev.clone(),
            });
        }
    }

    // Producers from the root component, if any.
    for comp in &version.components {
        for entry in comp.producers.iter().filter(|e| e.field != "language") {
            rows.push(MetadataRow::Producer {
                field: title_case(&entry.field),
                name: entry.name.clone(),
                version: entry.version.clone(),
            });
        }
    }

    rows
}

/// Format a byte offset as a hex string (e.g. `0x0000`, `0x1A3F`).
fn format_hex_offset(offset: u64) -> String {
    if offset <= 0xFFFF {
        format!("0x{offset:04X}")
    } else if offset <= 0xFF_FFFF {
        format!("0x{offset:06X}")
    } else {
        format!("0x{offset:08X}")
    }
}

/// Title-case a hyphenated field name (e.g. `"processed-by"` → `"Processed By"`).
fn title_case(s: &str) -> String {
    s.split('-')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => {
                    let mut out = c.to_uppercase().to_string();
                    out.extend(chars);
                    out
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Build a dependency row from a BOM entry.
fn dependency_row(dep: &BomEntry) -> MetadataRow {
    let source = dep.source.as_deref().unwrap_or("crates.io");
    let href = match source {
        "crates.io" | "registry" => Some(format!(
            "https://crates.io/crates/{}/{}",
            dep.name, dep.version
        )),
        _ => None,
    };
    MetadataRow::Dependency {
        name: dep.name.clone(),
        version: dep.version.clone(),
        href,
    }
}

/// Render a single metadata row as a table row.
fn render_row(row: &MetadataRow) -> TableRow {
    match row {
        MetadataRow::Text { label, value } => TableRow::builder()
            .class("border-b border-surfaceMuted last:border-b-0")
            .table_cell(|td| {
                td.class("py-2.5 px-4 pr-4 align-baseline text-ink-500 whitespace-nowrap w-[120px]")
                    .text(label.clone())
            })
            .table_cell(|td| td.class("py-2.5 px-4 align-baseline").text(value.clone()))
            .build(),
        MetadataRow::Range { label, start, end } => TableRow::builder()
            .class("border-b border-surfaceMuted last:border-b-0")
            .table_cell(|td| {
                td.class("py-2.5 px-4 pr-4 align-baseline text-ink-500 whitespace-nowrap w-[120px]")
                    .text(label.clone())
            })
            .table_cell(|td| {
                td.class("py-2.5 px-4 align-baseline flex items-center gap-1.5")
                    .code(|c| {
                        c.class("mono text-[11px] text-ink-700 bg-surfaceMuted px-1.5 py-0.5 rounded-sm")
                            .text(start.clone())
                    })
                    .span(|s| s.class("text-ink-400 text-[11px]").text("\u{2013}"))
                    .code(|c| {
                        c.class("mono text-[11px] text-ink-700 bg-surfaceMuted px-1.5 py-0.5 rounded-sm")
                            .text(end.clone())
                    })
            })
            .build(),
        MetadataRow::Link { label, text, href } => TableRow::builder()
            .class("border-b border-surfaceMuted last:border-b-0")
            .table_cell(|td| {
                td.class("py-2.5 px-4 pr-4 align-baseline text-ink-500 whitespace-nowrap w-[120px]")
                    .text(label.clone())
            })
            .table_cell(|td| {
                td.class("py-2.5 px-4 align-baseline").anchor(|a| {
                    a.href(href.clone())
                        .class("text-accent hover:underline truncate")
                        .text(text.clone())
                })
            })
            .build(),
        MetadataRow::Producer {
            field,
            name,
            version,
        } => {
            let display_version = version
                .split_once(" (")
                .map_or_else(|| version.clone(), |(before, _)| before.to_owned());
            TableRow::builder()
                .class("border-b border-surfaceMuted last:border-b-0")
                .table_cell(|td| {
                    td.class(
                        "py-2.5 px-4 pr-4 align-baseline text-ink-500 whitespace-nowrap w-[120px]",
                    )
                    .text(field.clone())
                })
                .table_cell(|td| {
                    let cell = td
                        .class("py-2.5 px-4 align-baseline")
                        .span(|s| s.class("text-ink-900").text(name.clone()));
                    if !display_version.is_empty() {
                        let tag_text = if display_version.starts_with('v')
                            || display_version.starts_with('V')
                        {
                            display_version.clone()
                        } else {
                            format!("v{display_version}")
                        };
                        cell.code(|c| {
                            c.class("ml-2 mono text-[11px] text-ink-700 bg-surfaceMuted px-1.5 py-0.5 rounded-sm")
                                .text(tag_text)
                        });
                    }
                    cell
                })
                .build()
        }
        MetadataRow::Dependency {
            name,
            version,
            href,
        } => {
            let purl_type = if href.is_some() { "cargo" } else { "generic" };
            let dep_url = format!("pkg:{purl_type}/{name}");
            TableRow::builder()
                .class("border-b border-surfaceMuted last:border-b-0")
                .table_cell(|td| {
                    td.class(
                        "py-2.5 px-4 pr-4 align-baseline text-ink-500 whitespace-nowrap w-[120px]",
                    )
                    .text("Dependency".to_owned())
                })
                .table_cell(|td| {
                    let cell = if let Some(url) = href {
                        td.class("py-2.5 px-4 align-baseline").anchor(|a| {
                            a.href(url.clone())
                                .class("text-accent hover:underline")
                                .text(dep_url)
                        })
                    } else {
                        td.class("py-2.5 px-4 align-baseline").text(dep_url)
                    };
                    if !version.is_empty() {
                        cell.span(|s| {
                            s.class("ml-2 text-[11px] text-ink-500")
                                .text(format!("v{version}"))
                        });
                    }
                    cell
                })
                .build()
        }
    }
}
