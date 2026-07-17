//! Interface detail page.

use crate::components::ds::wit_item::{self, TypeTag, WitItem, WitItemKind};
use crate::components::ds::{page_header, section_group};
use crate::components::page_sidebar::SidebarActive;
use crate::wit_doc::{FunctionDoc, InterfaceDoc, TypeDoc, TypeKind, WitDocument};
use html::text_content::Division;
use wasm_meta_registry_client::{KnownPackage, PackageVersion};

use super::detail::{self, DetailSpec};

/// Render the interface detail page.
#[must_use]
pub(crate) fn render(
    pkg: &KnownPackage,
    version: &str,
    version_detail: Option<&PackageVersion>,
    iface: &InterfaceDoc,
    doc: &WitDocument,
) -> String {
    let display_name = crate::components::page_shell::display_name_for(pkg);
    let title = format!("{display_name} — {}", iface.name);

    // Interface content — heading + docs in a two-column row

    let header_row = page_header::page_header_block(
        &format!("v{version} \u{00b7} Interface"),
        &iface.name,
        iface.docs.as_deref().unwrap_or("No description available."),
        None,
    )
    .to_string();

    // Grouped type and function sections
    let mut content = Division::builder();
    content.class("space-y-10 pt-8");
    let mut toc: Vec<(String, String)> = Vec::new();

    let resources: Vec<&TypeDoc> = iface
        .types
        .iter()
        .filter(|t| matches!(t.kind, TypeKind::Resource { .. }))
        .collect();
    let records: Vec<&TypeDoc> = iface
        .types
        .iter()
        .filter(|t| matches!(t.kind, TypeKind::Record { .. }))
        .collect();
    let variants: Vec<&TypeDoc> = iface
        .types
        .iter()
        .filter(|t| matches!(t.kind, TypeKind::Variant { .. }))
        .collect();
    let enums: Vec<&TypeDoc> = iface
        .types
        .iter()
        .filter(|t| matches!(t.kind, TypeKind::Enum { .. }))
        .collect();
    let flags: Vec<&TypeDoc> = iface
        .types
        .iter()
        .filter(|t| matches!(t.kind, TypeKind::Flags { .. }))
        .collect();
    let aliases: Vec<&TypeDoc> = iface
        .types
        .iter()
        .filter(|t| matches!(t.kind, TypeKind::Alias(_)))
        .collect();

    if !resources.is_empty() {
        toc.push(("#resources".to_owned(), "Resources".to_owned()));
        content.division(|d| {
            d.id("resources".to_owned()).push(render_type_section(
                "Resources",
                &resources,
                &display_name,
            ))
        });
    }
    if !records.is_empty() {
        toc.push(("#records".to_owned(), "Records".to_owned()));
        content.division(|d| {
            d.id("records".to_owned())
                .push(render_type_section("Records", &records, &display_name))
        });
    }
    if !variants.is_empty() {
        toc.push(("#variants".to_owned(), "Variants".to_owned()));
        content.division(|d| {
            d.id("variants".to_owned()).push(render_type_section(
                "Variants",
                &variants,
                &display_name,
            ))
        });
    }
    if !enums.is_empty() {
        toc.push(("#enums".to_owned(), "Enums".to_owned()));
        content.division(|d| {
            d.id("enums".to_owned())
                .push(render_type_section("Enums", &enums, &display_name))
        });
    }
    if !flags.is_empty() {
        toc.push(("#flags".to_owned(), "Flags".to_owned()));
        content.division(|d| {
            d.id("flags".to_owned())
                .push(render_type_section("Flags", &flags, &display_name))
        });
    }
    if !aliases.is_empty() {
        toc.push(("#type-aliases".to_owned(), "Type Aliases".to_owned()));
        content.division(|d| {
            d.id("type-aliases".to_owned()).push(render_type_section(
                "Type Aliases",
                &aliases,
                &display_name,
            ))
        });
    }
    if !iface.functions.is_empty() {
        toc.push(("#functions".to_owned(), "Functions".to_owned()));
        content.division(|d| {
            d.id("functions".to_owned())
                .push(render_function_section(&iface.functions, &display_name))
        });
    }

    let body_html = content.build().to_string();

    // Build "On this page" ToC
    let toc_html = if toc.is_empty() {
        None
    } else {
        use crate::components::ds::on_this_page::TocEntry;
        let links: Vec<TocEntry<'_>> = toc
            .iter()
            .map(|(href, label)| TocEntry {
                href: href.as_str(),
                label: label.as_str(),
                indent: false,
            })
            .collect();
        Some(crate::components::ds::on_this_page::on_this_page_nav(
            &links,
        ))
    };

    // Build nav card with interface items for the sidebar
    detail::render(&DetailSpec {
        pkg,
        version,
        version_detail,
        wit_doc: Some(doc),
        title: &title,
        header_html: &header_row,
        body_html: &body_html,
        sidebar_active: SidebarActive::Interface(&iface.name),
        extra_crumbs: &[],
        toc_html: toc_html.as_deref(),
        importers: &[],
        exporters: &[],
    })
}

/// Render a section of types grouped by kind.
fn render_type_section(heading: &str, types: &[&TypeDoc], pkg_name: &str) -> Division {
    let items: Vec<WitItem> = types
        .iter()
        .map(|ty| WitItem {
            kind: WitItemKind::Type(TypeTag::from_kind(&ty.kind)),
            name: ty.name.clone(),
            href: ty.url.clone(),
            docs: ty
                .docs
                .as_deref()
                .map(|d| crate::markdown::render_inline(&first_sentence(d))),
            version: String::new(),
            meta: ty.stability.meta_string(),
            meta_title: ty.stability.meta_title(pkg_name),
            deprecated: ty.stability.is_deprecated(),
            id: None,
        })
        .collect();
    wit_item::render_item_section(heading, &items)
}

/// Render the freestanding functions section.
fn render_function_section(functions: &[FunctionDoc], pkg_name: &str) -> Division {
    let items: Vec<WitItem> = functions
        .iter()
        .map(|func| WitItem {
            kind: WitItemKind::Function,
            name: func.name.clone(),
            href: func.url.clone(),
            docs: func
                .docs
                .as_deref()
                .map(|d| crate::markdown::render_inline(&first_sentence(d))),
            version: String::new(),
            meta: func.stability.meta_string(),
            meta_title: func.stability.meta_title(pkg_name),
            deprecated: func.stability.is_deprecated(),
            id: None,
        })
        .collect();
    wit_item::render_item_section("Functions", &items)
}

/// Convert a WIT stability to the component enum.
#[allow(dead_code)]
fn wit_stability(stability: &crate::wit_doc::Stability) -> section_group::Stability {
    match stability {
        crate::wit_doc::Stability::Stable { .. } => section_group::Stability::Stable,
        crate::wit_doc::Stability::Unstable { .. } => section_group::Stability::Unstable,
        crate::wit_doc::Stability::Unknown => section_group::Stability::Unknown,
    }
}

/// Extract the first sentence from a doc comment.
fn first_sentence(text: &str) -> String {
    // Split on paragraph break first, then on single newline for tighter excerpts
    let first_para = text.split_once("\n\n").map_or(text, |(first, _)| first);
    // Within that paragraph, take only the first line
    let first_line = first_para
        .split_once('\n')
        .map_or(first_para, |(first, _)| first);
    first_line.trim().to_owned()
}
/// Render the full interface definition as a WIT code block.
#[allow(dead_code)]
fn render_interface_definition(iface: &InterfaceDoc) -> Division {
    use crate::components::wit_render::{self, CODE_BLOCK_CLASS};

    Division::builder()
        .class("mb-8")
        .push(
            html::text_content::PreformattedText::builder()
                .class(CODE_BLOCK_CLASS)
                .code(|c| {
                    c.span(|s| s.class("text-ink-500").text("interface "))
                        .span(|s| {
                            s.class("text-wit-iface font-medium")
                                .text(iface.name.clone())
                        })
                        .text(" {\n".to_owned());

                    for ty in &iface.types {
                        wit_render::render_type_in_code(c, ty, "  ");
                        c.text("\n\n".to_owned());
                    }

                    for func in &iface.functions {
                        wit_render::render_func_in_code(c, func, "  ");
                        c.text("\n".to_owned());
                    }

                    c.text("}".to_owned())
                })
                .build(),
        )
        .build()
}
