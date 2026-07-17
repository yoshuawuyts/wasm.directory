//! Detail page for a child module or component inside a Wasm component.

use crate::components::ds::metadata_table;
use crate::components::ds::page_header;
use crate::components::ds::wit_item::{self, WitItem};
use crate::components::page_sidebar::SidebarActive;
use wasm_meta_registry_client::{ComponentSummary, KnownPackage, PackageVersion};

use super::detail::{self, DetailSpec};

/// Render the detail page for a child module or component.
#[must_use]
pub(crate) fn render(
    pkg: &KnownPackage,
    version: &str,
    version_detail: Option<&PackageVersion>,
    child: &ComponentSummary,
    display_name: &str,
) -> String {
    let pkg_display = crate::components::page_shell::display_name_for(pkg);
    let kind = child.kind.as_deref().unwrap_or("module");
    let title = format!("{pkg_display} \u{2014} {display_name}");

    // Build the kicker: "v{version} · {Component|Module} · {size}"
    let kind_label = if kind == "component" {
        "Component"
    } else {
        "Module"
    };
    let mut kicker_parts = vec![format!("v{version}"), kind_label.to_owned()];
    if let Some(bytes) = child.size_bytes {
        kicker_parts.push(super::package::format_size(bytes));
    }
    let kicker = kicker_parts.join(" \u{00b7} ");

    let tagline = child
        .description
        .as_deref()
        .unwrap_or("No description available.")
        .to_owned();

    let header = page_header::page_header_block(&kicker, display_name, &tagline, None).to_string();

    let mut body = String::from("<div class=\"space-y-10 pt-8\">");

    // WIT exports
    if !child.exports.is_empty() {
        let entries: Vec<WitItem> = child
            .exports
            .iter()
            .map(wit_item::iface_ref_to_item)
            .collect();
        body.push_str(&wit_item::render_item_section("Exports", &entries).to_string());
    }

    // WIT imports
    if !child.imports.is_empty() {
        let entries: Vec<WitItem> = child
            .imports
            .iter()
            .map(wit_item::iface_ref_to_item)
            .collect();
        body.push_str(&wit_item::render_item_section("Imports", &entries).to_string());
    }

    // Metadata table (producers, dependencies, languages, size, etc.)
    if let Some(table) = metadata_table::render(child) {
        body.push_str(&table.to_string());
    }

    body.push_str("</div>");

    detail::render(&DetailSpec {
        pkg,
        version,
        version_detail,
        wit_doc: None,
        title: &title,
        header_html: &header,
        body_html: &body,
        sidebar_active: SidebarActive::Child(display_name),
        extra_crumbs: &[crate::components::ds::breadcrumb::Crumb {
            label: display_name.to_owned(),
            href: None,
        }],
        toc_html: None,
        importers: &[],
        exporters: &[],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_meta_registry_client::{BomEntry, ProducerEntry, WitInterfaceRef};

    fn sample_pkg() -> KnownPackage {
        KnownPackage {
            registry: "ghcr.io".to_string(),
            repository: "example/pkg".to_string(),
            kind: None,
            description: None,
            tags: vec!["1.0.0".to_string()],
            signature_tags: vec![],
            attestation_tags: vec![],
            last_seen_at: "2026-01-01T00:00:00Z".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            wit_namespace: Some("example".to_string()),
            wit_name: Some("pkg".to_string()),
            dependencies: vec![],
        }
    }

    fn sample_child(kind: &str) -> ComponentSummary {
        ComponentSummary {
            name: Some("child".into()),
            description: None,
            targets: vec![],
            producers: vec![
                ProducerEntry {
                    field: "language".into(),
                    name: "Rust".into(),
                    version: "1.82.0".into(),
                },
                ProducerEntry {
                    field: "processed-by".into(),
                    name: "wit-component".into(),
                    version: "0.220.0 (extra)".into(),
                },
            ],
            kind: Some(kind.into()),
            size_bytes: Some(4096),
            range_start: Some(0),
            range_end: Some(4096),
            languages: vec!["Rust".into()],
            children: vec![],
            source: None,
            homepage: None,
            licenses: None,
            authors: None,
            revision: None,
            component_version: None,
            bill_of_materials: vec![
                BomEntry {
                    name: "serde".into(),
                    version: "1.0.0".into(),
                    source: Some("crates.io".into()),
                },
                BomEntry {
                    name: "custom".into(),
                    version: "0.1.0".into(),
                    source: Some("git".into()),
                },
            ],
            imports: vec![WitInterfaceRef {
                package: "wasi:io".into(),
                interface: Some("streams".into()),
                version: Some("0.2.0".into()),
                docs: None,
                is_native: false,
            }],
            exports: vec![WitInterfaceRef {
                package: "wasi:http".into(),
                interface: Some("incoming-handler".into()),
                version: Some("0.2.0".into()),
                docs: None,
                is_native: false,
            }],
        }
    }

    #[test]
    fn render_module_uses_module_kicker() {
        let pkg = sample_pkg();
        let child = sample_child("module");
        let html = render(&pkg, "1.0.0", None, &child, "inner");
        assert!(html.contains("Module"));
        assert!(html.contains("inner"));
        assert!(html.contains("Imports"));
        assert!(html.contains("Exports"));
        assert!(html.contains("Metadata"));
        // Producer and dependency data are inside the metadata table
        assert!(html.contains("Processed By"));
        assert!(html.contains("wit-component"));
        assert!(html.contains("serde"));
    }

    #[test]
    fn render_component_uses_component_kicker() {
        let pkg = sample_pkg();
        let child = sample_child("component");
        let html = render(&pkg, "1.0.0", None, &child, "inner");
        assert!(html.contains("Component"));
    }

    #[test]
    fn render_empty_sections_are_skipped() {
        let pkg = sample_pkg();
        let mut child = sample_child("module");
        // Only language producers — filtered out of metadata table.
        child.producers = vec![ProducerEntry {
            field: "language".into(),
            name: "Rust".into(),
            version: String::new(),
        }];
        child.bill_of_materials = vec![];
        child.imports = vec![];
        child.exports = vec![];
        child.languages = vec![];
        child.size_bytes = None;
        let html = render(&pkg, "1.0.0", None, &child, "inner");
        // Producer/dependency sub-sections should be absent
        assert!(!html.contains("wit-component"));
        assert!(!html.contains(">Dependencies<"));
    }
}
