//! World detail page.

use crate::components::ds::page_header;
use crate::components::ds::wit_item::{self, TypeTag, WitItem, WitItemKind};
use crate::components::page_sidebar::SidebarActive;
use crate::wit_doc::{WitDocument, WorldDoc, WorldItemDoc};
use html::text_content::Division;
use wasm_meta_registry_client::{KnownPackage, PackageVersion};

use super::detail::{self, DetailSpec};

/// Render the world detail page.
#[must_use]
pub(crate) fn render(
    pkg: &KnownPackage,
    version: &str,
    version_detail: Option<&PackageVersion>,
    world: &WorldDoc,
    doc: &WitDocument,
) -> String {
    let display_name = crate::components::page_shell::display_name_for(pkg);
    let title = format!("{display_name} \u{2014} {}", world.name);

    let header = page_header::page_header_block(
        &format!("v{version} \u{00b7} World"),
        &world.name,
        world.docs.as_deref().unwrap_or("No description available."),
        None,
    )
    .to_string();

    // Body sections: Exports + Imports (grouped by package).
    let api_docs = build_api_doc_lookup(version_detail, &world.name);
    let mut body = Division::builder();
    body.class("space-y-10 pt-8");
    if !world.exports.is_empty() {
        body.push(render_item_section(
            "Exports",
            &world.exports,
            &api_docs,
            &display_name,
        ));
    }
    if !world.imports.is_empty() {
        body.push(render_item_section(
            "Imports",
            &world.imports,
            &api_docs,
            &display_name,
        ));
    }
    let body_html = body.build().to_string();

    detail::render(&DetailSpec {
        pkg,
        version,
        version_detail,
        wit_doc: Some(doc),
        title: &title,
        header_html: &header,
        body_html: &body_html,
        sidebar_active: SidebarActive::World(&world.name),
        extra_crumbs: &[crate::components::ds::breadcrumb::Crumb {
            label: world.name.clone(),
            href: None,
        }],
        toc_html: None,
        importers: &[],
        exporters: &[],
    })
}

/// Build a lookup map of interface name → doc string from the API's enriched
/// world data. This provides cross-package docs that the WIT parser can't.
pub(crate) fn build_api_doc_lookup(
    version_detail: Option<&PackageVersion>,
    world_name: &str,
) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    let Some(detail) = version_detail else {
        return map;
    };
    for world in &detail.worlds {
        if world.name != world_name {
            continue;
        }
        for iface in world.imports.iter().chain(world.exports.iter()) {
            if let Some(docs) = &iface.docs {
                let mut key = iface.package.clone();
                if let Some(name) = &iface.interface {
                    key.push('/');
                    key.push_str(name);
                }
                map.insert(key, docs.clone());
            }
        }
    }
    map
}

/// Render an imports or exports section, grouped by package namespace.
pub(crate) fn render_item_section(
    heading: &str,
    items: &[WorldItemDoc],
    api_docs: &std::collections::HashMap<String, String>,
    pkg_name: &str,
) -> Division {
    let rows: Vec<WitItem> = items
        .iter()
        .map(|item| match item {
            WorldItemDoc::Interface {
                name,
                url,
                docs,
                stability,
            } => {
                let name_no_ver = strip_version(name);
                let ver_suffix = extract_version(name)
                    .map(ToOwned::to_owned)
                    .unwrap_or_default();
                let desc = docs.clone().or_else(|| api_docs.get(name_no_ver).cloned());
                WitItem {
                    kind: WitItemKind::Interface,
                    name: name_no_ver.to_owned(),
                    href: url.clone().unwrap_or_default(),
                    docs: desc,
                    version: ver_suffix,
                    meta: stability.meta_string(),
                    meta_title: stability.meta_title(pkg_name),
                    deprecated: stability.is_deprecated(),
                    id: None,
                }
            }
            WorldItemDoc::Function(func) => WitItem {
                kind: WitItemKind::Function,
                name: func.name.clone(),
                href: func.url.clone(),
                docs: func.docs.clone(),
                version: String::new(),
                meta: func.stability.meta_string(),
                meta_title: func.stability.meta_title(pkg_name),
                deprecated: func.stability.is_deprecated(),
                id: None,
            },
            WorldItemDoc::Type(ty) => WitItem {
                kind: WitItemKind::Type(TypeTag::from_kind(&ty.kind)),
                name: ty.name.clone(),
                href: ty.url.clone(),
                docs: ty.docs.clone(),
                version: String::new(),
                meta: ty.stability.meta_string(),
                meta_title: ty.stability.meta_title(pkg_name),
                deprecated: ty.stability.is_deprecated(),
                id: None,
            },
        })
        .collect();

    wit_item::render_item_section(heading, &rows)
}

/// Strip version suffix from a qualified name.
///
/// `"wasi:cli/environment@0.2.11"` → `"wasi:cli/environment"`
fn strip_version(name: &str) -> &str {
    name.split('@').next().unwrap_or(name)
}

/// Extract the version suffix from a qualified name.
///
/// `"wasi:cli/environment@0.2.11"` → `Some("0.2.11")`
fn extract_version(name: &str) -> Option<&str> {
    name.split_once('@').map(|(_, ver)| ver)
}
