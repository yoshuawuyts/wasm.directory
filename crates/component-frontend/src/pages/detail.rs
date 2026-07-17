//! Unified detail-page renderer.
//!
//! All package-family pages (package root, world, interface, item,
//! child component) share the same outer shape:
//!
//! - Two-column layout from `page_shell::render_page_with_crumbs`.
//! - Left sidebar from `page_sidebar::render_sidebar` (WIT nav + modules +
//!   metadata).
//! - Optional right "On this page" ToC.
//! - Breadcrumbs after namespace/name.
//!
//! Only the header HTML and body HTML differ per page, plus which sidebar
//! entry is active. `DetailSpec` captures those differences and
//! [`render`] handles all the wiring.

use crate::components::page_shell;
use crate::components::page_sidebar::{self, SidebarActive, SidebarContext};
use crate::wit_doc::WitDocument;
use wasm_meta_registry_client::{KnownPackage, PackageVersion};

/// Everything needed to render a package-family detail page.
pub(crate) struct DetailSpec<'a> {
    /// Package being displayed.
    pub pkg: &'a KnownPackage,
    /// Version string.
    pub version: &'a str,
    /// Version detail for sidebar metadata (annotations, digest, components).
    pub version_detail: Option<&'a PackageVersion>,
    /// Parsed WIT document for sidebar WIT navigation. `None` for component
    /// packages without WIT.
    pub wit_doc: Option<&'a WitDocument>,
    /// Full `<title>` string.
    pub title: &'a str,
    /// Pre-rendered page header HTML (usually from `page_header_block`).
    pub header_html: &'a str,
    /// Pre-rendered body HTML.
    pub body_html: &'a str,
    /// Which sidebar entry is active.
    pub sidebar_active: SidebarActive<'a>,
    /// Extra breadcrumb segments after namespace / name.
    pub extra_crumbs: &'a [crate::components::ds::breadcrumb::Crumb],
    /// Optional "On this page" ToC HTML.
    pub toc_html: Option<&'a str>,
    /// Packages that import this one (root page only).
    pub importers: &'a [KnownPackage],
    /// Packages that export this one (root page only).
    pub exporters: &'a [KnownPackage],
}

/// Render a package-family detail page.
///
/// Handles the left sidebar, breadcrumbs, and two-column shell. Callers
/// only build their header + body HTML.
#[must_use]
pub(crate) fn render(spec: &DetailSpec<'_>) -> String {
    let display_name = page_shell::display_name_for(spec.pkg);
    let url_base = page_shell::url_base_for(spec.pkg, spec.version);
    let components = spec
        .version_detail
        .map_or(&[][..], |d| d.components.as_slice());

    let nav = page_sidebar::render_sidebar(&SidebarContext {
        display_name: &display_name,
        version: spec.version,
        versions: &spec.pkg.tags,
        doc: spec.wit_doc,
        components,
        url_base: &url_base,
        active: spec.sidebar_active,
        annotations: spec.version_detail.and_then(|d| d.annotations.as_ref()),
        kind_label: page_shell::kind_label_for(spec.pkg),
        description: spec.pkg.description.as_deref(),
        registry: &spec.pkg.registry,
        repository: &spec.pkg.repository,
        digest: spec.version_detail.map(|d| d.digest.as_str()),
        dependencies: &spec.pkg.dependencies,
    });

    let shell_ctx = page_shell::SidebarContext {
        pkg: spec.pkg,
        version: spec.version,
        version_detail: spec.version_detail,
        importers: spec.importers,
        exporters: spec.exporters,
        nav_html: Some(nav.to_string()),
    };
    page_shell::render_page_with_crumbs(
        &shell_ctx,
        spec.title,
        spec.header_html,
        spec.body_html,
        spec.extra_crumbs,
        spec.toc_html,
    )
}
