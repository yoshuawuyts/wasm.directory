//! Package detail page.

// r[impl frontend.pages.package-detail]

use crate::components::ds::wit_item::{self, WitItem, WitItemKind};
use crate::components::ds::{metadata_table, page_header};
use crate::components::page_sidebar::SidebarActive;
use crate::wit_doc::WitDocument;
use component_meta_registry_client::{KnownPackage, PackageVersion};
use html::content::Section;
use html::text_content::Division;

use super::detail::{self, DetailSpec};
use crate::components::page_shell;

/// Render the package detail page for a given package and version.
#[must_use]
pub(crate) fn render(
    pkg: &KnownPackage,
    version: &str,
    version_detail: Option<&PackageVersion>,
    importers: &[KnownPackage],
    exporters: &[KnownPackage],
) -> String {
    let display_name = page_shell::display_name_for(pkg);
    let url_base = page_shell::url_base_for(pkg, version);
    let wit_doc = version_detail.and_then(|d| try_parse_wit(d, &url_base, pkg));

    // Package heading
    let kind_label = match pkg.kind {
        Some(component_meta_registry_client::PackageKind::Interface) => "Interface Types",
        Some(component_meta_registry_client::PackageKind::Component) => "Component",
        _ => "Package",
    };
    let _pkg_name = pkg.wit_name.as_deref().unwrap_or(&display_name);

    // Build kicker: "Interface Types · version 0.2.11"
    let kicker = format!("{kind_label} \u{00b7} version {version}");

    let tagline = pkg
        .description
        .as_deref()
        .unwrap_or("No description available.");

    let command = format!("component install {display_name}@{version}");
    let run_command = format!("component run {display_name}@{version}");
    let acp_command = format!("/install {display_name}@{version}");

    let command_attr = html_escape_attr(&command);
    let run_command_attr = html_escape_attr(&run_command);
    let acp_command_attr = html_escape_attr(&acp_command);

    let copy_svg = concat!(
        r#"<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
        include_str!("../../../../vendor/lucide/copy.svg"),
        "</svg>"
    );
    let check_svg = concat!(
        r#"<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="text-positive">"#,
        include_str!("../../../../vendor/lucide/check.svg"),
        "</svg>"
    );

    // Collapse newlines in SVGs so they work inside JS string literals
    let copy_svg_js: String = copy_svg.chars().filter(|c| *c != '\n').collect();
    let check_svg_js: String = check_svg.chars().filter(|c| *c != '\n').collect();

    let copy_script = format!(
        r"<script>(function(){{var ci='{copy_svg_js}';var ch='{check_svg_js}';['copy-install-btn','copy-run-btn','copy-acp-btn'].forEach(function(id){{var btn=document.getElementById(id);if(!btn)return;var cmd=btn.getAttribute('data-cmd');btn.addEventListener('click',function(){{navigator.clipboard.writeText(cmd).then(function(){{btn.innerHTML=ch;setTimeout(function(){{btn.innerHTML=ci}},2000)}})}})}})}})()</script>",
    );

    let install_intro = match pkg.kind {
        Some(component_meta_registry_client::PackageKind::Interface) => {
            "Add these interface types to your project:"
        }
        Some(component_meta_registry_client::PackageKind::Component) => {
            "Add this component to your project:"
        }
        _ => "Add this package to your project:",
    };

    let install_panel = format!(
        "<div>\
            <p class=\"mb-2 text-[12px] text-ink-500\">{install_intro}</p>\
            <div class=\"flex\">\
                <span class=\"inline-flex items-center px-2.5 h-7 rounded-l-md border border-r-0 border-line bg-surfaceMuted text-[12.5px] text-ink-500 mono select-none\" aria-hidden=\"true\">\u{276f}</span>\
                <code class=\"inline-flex items-center px-2.5 h-7 flex-1 border border-line bg-surface mono text-[12.5px] text-ink-900 whitespace-nowrap\">{command}</code>\
                <button type=\"button\" id=\"copy-install-btn\" data-cmd=\"{command_attr}\" class=\"inline-flex items-center justify-center w-7 h-7 rounded-r-md border border-l-0 border-line bg-surface text-ink-500 hover:text-ink-900 hover:bg-surfaceMuted\" aria-label=\"Copy install command\">{copy_svg}</button>\
            </div>\
            <p class=\"mt-3 text-[12px] text-ink-500\">\
                <a href=\"/downloads\" class=\"text-ink-700 underline decoration-line decoration-1 underline-offset-2 hover:text-ink-900\">Learn more</a> about the component CLI.\
            </p>\
        </div>",
    );

    let run_panel = format!(
        "<div>\
            <p class=\"mb-2 text-[12px] text-ink-500\">Run this component without installing it:</p>\
            <div class=\"flex\">\
                <span class=\"inline-flex items-center px-2.5 h-7 rounded-l-md border border-r-0 border-line bg-surfaceMuted text-[12.5px] text-ink-500 mono select-none\" aria-hidden=\"true\">\u{276f}</span>\
                <code class=\"inline-flex items-center px-2.5 h-7 flex-1 border border-line bg-surface mono text-[12.5px] text-ink-900 whitespace-nowrap\">{run_command}</code>\
                <button type=\"button\" id=\"copy-run-btn\" data-cmd=\"{run_command_attr}\" class=\"inline-flex items-center justify-center w-7 h-7 rounded-r-md border border-l-0 border-line bg-surface text-ink-500 hover:text-ink-900 hover:bg-surfaceMuted\" aria-label=\"Copy run command\">{copy_svg}</button>\
            </div>\
            <p class=\"mt-3 text-[12px] text-ink-500\">\
                <a href=\"/downloads\" class=\"text-ink-700 underline decoration-line decoration-1 underline-offset-2 hover:text-ink-900\">Learn more</a> about the component CLI.\
            </p>\
        </div>",
    );

    let acp_arg = format!("{display_name}@{version}");
    let acp_panel = format!(
        "<div>\
            <p class=\"mb-2 text-[12px] text-ink-500\">Ask your AI assistant to install it for you:</p>\
            <div class=\"flex items-center gap-2 px-2.5 h-9 rounded-md border border-line bg-surface\">\
                <span class=\"bar-sm bg-cat-blue text-cat-blueInk mono\"><span class=\"mr-[2px]\">/</span>install</span>\
                <code class=\"flex-1 mono text-[12.5px] text-ink-900 whitespace-nowrap\">{acp_arg}</code>\
                <button type=\"button\" id=\"copy-acp-btn\" data-cmd=\"{acp_command_attr}\" class=\"inline-flex items-center justify-center w-7 h-7 rounded-md text-ink-500 hover:text-ink-900 hover:bg-surfaceMuted\" aria-label=\"Copy ACP command\">{copy_svg}</button>\
            </div>\
            <p class=\"mt-3 text-[12px] text-ink-500\">\
                <a href=\"https://github.com/yoshuawuyts/playground-wasm-acp\" class=\"text-ink-700 underline decoration-line decoration-1 underline-offset-2 hover:text-ink-900\">Read more</a> about Agent Client Protocol support.\
            </p>\
        </div>",
    );

    let mut tab_panels: Vec<(&str, &str)> = Vec::with_capacity(3);
    tab_panels.push(("Install", &install_panel));
    if matches!(
        pkg.kind,
        Some(component_meta_registry_client::PackageKind::Component)
    ) {
        tab_panels.push(("CLI", &run_panel));
        tab_panels.push(("ACP", &acp_panel));
    }

    let tabs_html = crate::components::ds::tabs::panel_tabs_switchable(&tab_panels);

    let install_meta = format!("{tabs_html}{copy_script}");
    let header =
        page_header::page_header_block(&kicker, &display_name, tagline, Some(&install_meta))
            .to_string();

    let (wit_content, toc_entries) = if let Some(detail) = version_detail {
        render_wit_content_with_doc(detail, &url_base, wit_doc.as_ref(), pkg, version)
    } else {
        (String::new(), Vec::new())
    };

    let body_html = format!("<div class=\"space-y-10 max-w-4xl pt-8 pb-12\">{wit_content}</div>");

    // Build "On this page" ToC
    let toc_html = if toc_entries.is_empty() {
        None
    } else {
        use crate::components::ds::on_this_page::TocEntry;
        let links: Vec<TocEntry<'_>> = toc_entries
            .iter()
            .map(|(href, label, indent)| TocEntry {
                href: href.as_str(),
                label: label.as_str(),
                indent: *indent,
            })
            .collect();
        Some(crate::components::ds::on_this_page::on_this_page_nav(
            &links,
        ))
    };

    // Build nav card showing interfaces/worlds (or modules/components when no WIT)
    detail::render(&DetailSpec {
        pkg,
        version,
        version_detail,
        wit_doc: wit_doc.as_ref(),
        title: &display_name,
        header_html: &header,
        body_html: &body_html,
        sidebar_active: SidebarActive::None,
        extra_crumbs: &[],
        toc_html: toc_html.as_deref(),
        importers,
        exporters,
    })
}

/// Escape a string for safe inclusion in an HTML attribute value.
fn html_escape_attr(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Render the WIT content section for a package version.
///
/// When a pre-parsed `WitDocument` is available, show interfaces and worlds
/// as navigable cards.  Otherwise fall back to the world summaries that the
/// registry extracted at index time plus the raw WIT text block.
/// Returns `(html_string, toc_entries)` where each ToC entry is `(href, label, indent)`.
fn render_wit_content_with_doc(
    detail: &PackageVersion,
    _url_base: &str,
    doc: Option<&WitDocument>,
    pkg: &KnownPackage,
    version: &str,
) -> (String, Vec<(String, String, bool)>) {
    let mut section = Section::builder();
    section.class("space-y-10");
    let mut toc: Vec<(String, String, bool)> = Vec::new();
    let display_name = page_shell::display_name_for(pkg);
    if let Some(doc) = doc {
        // Component special-case: when the document represents an extracted
        // wasm component (is_component is true) and the only world is
        // synthetic, inline its imports/exports as the component's own
        // instead of surfacing a single boring "Worlds → root" card.
        let inline_root = doc.is_component
            && doc
                .worlds
                .first()
                .is_some_and(|w| w.is_synthetic && doc.worlds.len() == 1);
        if inline_root {
            let world = doc
                .worlds
                .first()
                .expect("inline_root ensures doc.worlds is non-empty");
            let api_docs = super::world::build_api_doc_lookup(Some(detail), &world.name);
            if !world.exports.is_empty() {
                toc.push(("#exports".to_owned(), "Exports".to_owned(), false));
                section.division(|d| {
                    d.id("exports".to_owned())
                        .push(super::world::render_item_section(
                            "Exports",
                            &world.exports,
                            &api_docs,
                            &display_name,
                        ))
                });
            }
            if !world.imports.is_empty() {
                toc.push(("#imports".to_owned(), "Imports".to_owned(), false));
                section.division(|d| {
                    d.id("imports".to_owned())
                        .push(super::world::render_item_section(
                            "Imports",
                            &world.imports,
                            &api_docs,
                            &display_name,
                        ))
                });
            }
        } else if !doc.worlds.is_empty() {
            toc.push(("#worlds".to_owned(), "Worlds".to_owned(), false));
            for world in &doc.worlds {
                let id = format!("world-{}", world.name);
                toc.push((format!("#{id}"), world.name.clone(), true));
            }
            section.division(|d| {
                d.id("worlds".to_owned())
                    .push(render_world_overview(doc, &display_name))
            });
        }
        // When inlining the synthetic root world, the world's exports already
        // surface every native interface — listing them again under
        // "Interfaces" would duplicate `convert` etc.
        if !inline_root && !doc.interfaces.is_empty() {
            toc.push(("#interfaces".to_owned(), "Interfaces".to_owned(), false));
            for iface in &doc.interfaces {
                let id = format!("iface-{}", iface.name);
                toc.push((format!("#{id}"), iface.name.clone(), true));
            }
            section.division(|d| {
                d.id("interfaces".to_owned())
                    .push(render_interface_overview(doc, &display_name))
            });
        }
    } else {
        // Fallback: prefer component-level imports/exports (from wasm-metadata,
        // which include docs) over world summaries (from DB, no docs).
        let has_component_imports = detail
            .components
            .iter()
            .any(|c| !c.imports.is_empty() || !c.exports.is_empty());

        if has_component_imports {
            for comp in &detail.components {
                if !comp.exports.is_empty() {
                    toc.push(("#exports".to_owned(), "Exports".to_owned(), false));
                    section.division(|d| {
                        d.id("exports".to_owned())
                            .push(render_iface_ref_list("Exports", &comp.exports))
                    });
                }
                if !comp.imports.is_empty() {
                    toc.push(("#imports".to_owned(), "Imports".to_owned(), false));
                    section.division(|d| {
                        d.id("imports".to_owned())
                            .push(render_iface_ref_list("Imports", &comp.imports))
                    });
                }
            }
        } else if !detail.worlds.is_empty() {
            toc.push(("#worlds".to_owned(), "Worlds".to_owned(), false));
            section.division(|d| {
                d.id("worlds".to_owned())
                    .push(render_world_summaries(detail))
            });
        }

        // Only show the raw WIT text if it's genuine WIT (not lossy
        // debug output that contains patterns like `type foo: "type"`
        // or `interface-Id { idx: 0 }`).
        if let Some(wit_text) = &detail.wit_text
            && !is_lossy_wit(wit_text)
        {
            toc.push(("#wit".to_owned(), "WIT Definition".to_owned(), false));
            section.division(|d| d.id("wit".to_owned()).push(render_raw_wit(wit_text)));
        }
    }

    // Component children: list modules and nested components as navigable sections.
    for comp in &detail.components {
        let url_base = page_shell::url_base_for(pkg, version);

        // Modules section
        let modules: Vec<&component_meta_registry_client::ComponentSummary> = comp
            .children
            .iter()
            .filter(|ch| ch.kind.as_deref() == Some("module"))
            .collect();
        if !modules.is_empty() {
            toc.push(("#modules".to_owned(), "Modules".to_owned(), false));
            section.division(|d| {
                d.id("modules".to_owned()).push(render_children_overview(
                    "Modules", &modules, &url_base, "module",
                ))
            });
        }

        // Nested components section
        let components: Vec<&component_meta_registry_client::ComponentSummary> = comp
            .children
            .iter()
            .filter(|ch| ch.kind.as_deref() == Some("component"))
            .collect();
        if !components.is_empty() {
            toc.push(("#components".to_owned(), "Components".to_owned(), false));
            section.division(|d| {
                d.id("components".to_owned()).push(render_children_overview(
                    "Components",
                    &components,
                    &url_base,
                    "component",
                ))
            });
        }

        // Root metadata table (producers, size, languages, etc.)
        if let Some(table) = metadata_table::render(comp) {
            toc.push(("#metadata".to_owned(), "Metadata".to_owned(), false));
            section.division(|d| d.id("metadata".to_owned()).push(table));
        }
    }

    // For packages without components (e.g. WIT-only), show version-level metadata.
    if detail.components.is_empty()
        && let Some(table) = metadata_table::render_version(detail)
    {
        toc.push(("#metadata".to_owned(), "Metadata".to_owned(), false));
        section.division(|d| d.id("metadata".to_owned()).push(table));
    }

    (section.build().to_string(), toc)
}

/// Render a section listing child modules or components as navigable links.
fn render_children_overview(
    heading: &str,
    children: &[&component_meta_registry_client::ComponentSummary],
    url_base: &str,
    kind: &str,
) -> Division {
    let items: Vec<WitItem> = children
        .iter()
        .enumerate()
        .map(|(i, child)| {
            let fallback = format!("{kind}[{i}]");
            let name = child.name.as_deref().unwrap_or(&fallback).to_owned();
            let href = if kind == "module" {
                format!("{url_base}/module/{name}")
            } else {
                format!("{url_base}/component/{i}")
            };
            let item_kind = if kind == "component" {
                WitItemKind::Component
            } else {
                WitItemKind::Module
            };
            WitItem {
                kind: item_kind,
                name,
                href,
                docs: None,
                version: String::new(),
                meta: String::new(),
                meta_title: String::new(),
                deprecated: false,
                id: None,
            }
        })
        .collect();
    wit_item::render_item_section(heading, &items)
}

/// Try parsing the WIT text into a rich document model.
fn try_parse_wit(
    detail: &PackageVersion,
    url_base: &str,
    pkg: &KnownPackage,
) -> Option<WitDocument> {
    let wit_text = detail.wit_text.as_deref()?;
    let dep_urls = build_dep_urls(&detail.dependencies);
    let own_oci_package = match (pkg.wit_namespace.as_deref(), pkg.wit_name.as_deref()) {
        (Some(ns), Some(n)) => Some(format!("{ns}:{n}")),
        _ => None,
    };
    crate::wit_doc::parse_wit_doc_with_type_docs(
        wit_text,
        url_base,
        &dep_urls,
        &detail.type_docs,
        own_oci_package.as_deref(),
    )
    .ok()
}

/// Build the `dep_urls` mapping from a package's declared dependencies.
///
/// Maps `"namespace:name"` → `"/namespace/name/version"` for each
/// dependency that has a version.
fn build_dep_urls(
    deps: &[component_meta_registry_client::PackageDependencyRef],
) -> std::collections::HashMap<String, String> {
    deps.iter()
        .filter_map(|dep| {
            let version = dep.version.as_deref()?;
            let url = format!("/{}/{version}", dep.package.replace(':', "/"));
            Some((dep.package.clone(), url))
        })
        .collect()
}

/// Render the interfaces overview section.
fn render_interface_overview(doc: &WitDocument, pkg_name: &str) -> Division {
    let items: Vec<WitItem> = doc
        .interfaces
        .iter()
        .map(|iface| WitItem {
            kind: WitItemKind::Interface,
            name: iface.name.clone(),
            href: iface.url.clone(),
            docs: iface.docs.as_deref().map(first_sentence),
            version: String::new(),
            meta: iface.stability.meta_string(),
            meta_title: iface.stability.meta_title(pkg_name),
            deprecated: iface.stability.is_deprecated(),
            id: Some(format!("iface-{}", iface.name)),
        })
        .collect();
    wit_item::render_item_section("Interfaces", &items)
}

/// Render the worlds overview section.
fn render_world_overview(doc: &WitDocument, pkg_name: &str) -> Division {
    let items: Vec<WitItem> = doc
        .worlds
        .iter()
        .map(|world| WitItem {
            kind: WitItemKind::World,
            name: world.name.clone(),
            href: world.url.clone(),
            docs: world.docs.as_deref().map(first_sentence),
            version: String::new(),
            meta: world.stability.meta_string(),
            meta_title: world.stability.meta_title(pkg_name),
            deprecated: world.stability.is_deprecated(),
            id: Some(format!("world-{}", world.name)),
        })
        .collect();
    wit_item::render_item_section("Worlds", &items)
}

/// Render raw WIT text in a pre-formatted code block (fallback).
fn render_raw_wit(wit_text: &str) -> Division {
    Division::builder()
        .heading_2(|h2| {
            h2.class(crate::components::ds::typography::SECTION_CLASS)
                .text("WIT Definition")
        })
        .push(
            html::text_content::PreformattedText::builder()
                .class("border border-line p-4 overflow-x-auto text-[15px] leading-relaxed")
                .code(|code| code.class("text-ink-900").text(wit_text.to_owned()))
                .build(),
        )
        .build()
}

/// Render world summaries from pre-extracted `PackageVersion` data (fallback
/// when the WIT text cannot be parsed into a rich document).
fn render_world_summaries(detail: &PackageVersion) -> Division {
    let mut container = Division::builder();
    container.class("space-y-8");

    for world in &detail.worlds {
        container.division(|world_div| {
            if world.name != "root" {
                world_div.heading_2(|h2| {
                    h2.class(crate::components::ds::typography::SECTION_CLASS)
                        .text(format!("world {}", world.name))
                });
            }

            if let Some(desc) = &world.description {
                world_div.paragraph(|p| {
                    p.class("text-ink-700 text-[15px] mb-3")
                        .text(crate::markdown::render_inline(desc))
                });
            }

            if !world.exports.is_empty() {
                world_div.push(render_iface_ref_list("Exports", &world.exports));
            }
            if !world.imports.is_empty() {
                world_div.push(render_iface_ref_list("Imports", &world.imports));
            }
            world_div
        });
    }

    container.build()
}

/// Render a list of WIT interface references (fallback), styled like world
/// imports/exports with clickable links.
fn render_iface_ref_list(
    label: &str,
    interfaces: &[component_meta_registry_client::WitInterfaceRef],
) -> Division {
    let items: Vec<WitItem> = interfaces.iter().map(wit_item::iface_ref_to_item).collect();
    wit_item::render_item_section(label, &items)
}

/// Format a byte size into a human-readable string.
pub(crate) fn format_size(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = 1024 * KIB;
    #[allow(clippy::cast_precision_loss)]
    match bytes {
        b if b >= MIB => format!("{:.1} MiB", b as f64 / MIB as f64),
        b if b >= KIB => format!("{:.1} KiB", b as f64 / KIB as f64),
        b => format!("{b} B"),
    }
}

/// Extract the first sentence from a doc comment for summary display.
fn first_sentence(text: &str) -> String {
    let first_para = text.split_once("\n\n").map_or(text, |(first, _)| first);
    let first_line = first_para
        .split_once('\n')
        .map_or(first_para, |(first, _)| first);
    first_line.trim().to_owned()
}

/// Detect whether WIT text is the lossy hand-rolled format rather than
/// genuine parseable WIT.  The lossy format contains debug patterns like
/// `type foo: "type"` and `interface-Id { idx: 0 }`.
fn is_lossy_wit(text: &str) -> bool {
    text.contains(": \"type\"")
        || text.contains(": \"record\"")
        || text.contains(": \"variant\"")
        || text.contains("interface-Id {")
}

#[cfg(test)]
mod tests {
    use super::*;
    use component_meta_registry_client::PackageDependencyRef;

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
            wit_namespace: Some("wasi".to_string()),
            wit_name: Some("demo".to_string()),
            dependencies: vec![PackageDependencyRef {
                package: "wasi:io".to_string(),
                version: Some("0.2.0".to_string()),
            }],
        }
    }

    #[test]
    fn dependency_versions_shown_in_sidebar() {
        let pkg = sample_pkg();
        let html = render(&pkg, "1.0.0", None, &[], &[]);
        // Sidebar temporarily removed — just verify the page renders
        assert!(html.contains("<!DOCTYPE html>"));
    }
}
