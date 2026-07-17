//! Shared sidebar components for detail pages.
//!
//! Provides a navigation sidebar showing sibling interfaces/worlds and
//! package metadata, using the DS nested sidebar component (C01).

use crate::components::ds::sidebar::{self, SidebarEntry, SidebarGroup, SidebarItem};
use crate::components::ds::sigil as s;
use crate::wit_doc::WitDocument;
use html::content::Aside;
use wasm_meta_registry_client::{ComponentSummary, OciAnnotations};

/// GitHub logo SVG icon (14px, ink-500).
const SVG_GITHUB: &str = r#"<svg class="h-3.5 w-3.5 text-ink-500 flex-shrink-0" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true"><path d="M8 .2a8 8 0 0 0-2.5 15.6c.4 0 .55-.17.55-.38v-1.4c-2.22.48-2.69-1.07-2.69-1.07-.36-.92-.89-1.17-.89-1.17-.73-.5.05-.49.05-.49.8.06 1.23.83 1.23.83.71 1.23 1.87.87 2.33.66.07-.52.28-.87.5-1.07-1.77-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.83-2.15-.08-.2-.36-1.02.08-2.13 0 0 .67-.22 2.2.82A7.6 7.6 0 0 1 8 4.04c.68 0 1.37.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.11.16 1.93.08 2.13.52.56.83 1.28.83 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.74.54 1.49v2.21c0 .21.15.46.55.38A8 8 0 0 0 8 .2Z" /></svg>"#;

/// Lucide book-open icon (14px, ink-500).
const SVG_BOOK: &str = concat!(
    r#"<svg class="h-3.5 w-3.5 text-ink-500 flex-shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">"#,
    include_str!("../../../../vendor/lucide/book-open.svg"),
    "</svg>"
);

/// Lucide house icon (14px, ink-500).
const SVG_HOUSE: &str = concat!(
    r#"<svg class="h-3.5 w-3.5 text-ink-500 flex-shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">"#,
    include_str!("../../../../vendor/lucide/house.svg"),
    "</svg>"
);

/// Lucide scale icon (14px, ink-500).
const SVG_SCALE: &str = concat!(
    r#"<svg class="h-3.5 w-3.5 text-ink-500 flex-shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">"#,
    include_str!("../../../../vendor/lucide/scale.svg"),
    "</svg>"
);

/// Lucide calendar icon (14px, ink-500).
const SVG_CALENDAR: &str = concat!(
    r#"<svg class="h-3.5 w-3.5 text-ink-500 flex-shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">"#,
    include_str!("../../../../vendor/lucide/calendar.svg"),
    "</svg>"
);

/// Lucide git-fork icon (14px, ink-500) — generic VCS icon.
const SVG_GIT_FORK: &str = concat!(
    r#"<svg class="h-3.5 w-3.5 text-ink-500 flex-shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">"#,
    include_str!("../../../../vendor/lucide/git-fork.svg"),
    "</svg>"
);

/// Context needed to render the detail page sidebar.
pub(crate) struct SidebarContext<'a> {
    /// The package display name (e.g. `"wasi:cli"`).
    pub display_name: &'a str,
    /// The current version string.
    pub version: &'a str,
    /// All available version tags (newest first).
    pub versions: &'a [String],
    /// The parsed WIT document for navigation links. `None` for component
    /// packages without WIT.
    pub doc: Option<&'a WitDocument>,
    /// Top-level components in the package version. Used to build a
    /// Modules/Components nav when no `doc` is available (and as a child
    /// list under the package on detail pages).
    pub components: &'a [ComponentSummary],
    /// Base URL for child-component links (e.g. `/wasi/http/0.2.11`).
    pub url_base: &'a str,
    /// Which sidebar item is currently active.
    pub active: SidebarActive<'a>,
    /// OCI annotations for the current version (optional).
    pub annotations: Option<&'a OciAnnotations>,
    /// Package kind label (e.g. "Interface Types", "Component").
    pub kind_label: &'a str,
    /// Package description.
    pub description: Option<&'a str>,
    /// OCI registry hostname (e.g. "ghcr.io").
    pub registry: &'a str,
    /// OCI repository path (e.g. "wasi/http").
    pub repository: &'a str,
    /// OCI image digest (e.g. "sha256:abc123...").
    pub digest: Option<&'a str>,
}

/// Which item in the sidebar is currently active.
pub(crate) enum SidebarActive<'a> {
    /// An interface page (name of the interface).
    Interface(&'a str),
    /// An item within an interface (interface name, item name).
    Item(&'a str, #[allow(dead_code)] &'a str),
    /// A world page (name of the world).
    World(&'a str),
    /// A child module/component page (display name).
    #[allow(dead_code)]
    Child(&'a str),
}

/// Render the sidebar for a detail page using the DS nested sidebar.
pub(crate) fn render_sidebar(ctx: &SidebarContext<'_>) -> Aside {
    let mut items: Vec<SidebarItem> = Vec::new();

    if let Some(doc) = ctx.doc {
        push_wit_nav(&mut items, ctx, doc);
    }

    // Modules / Components — for component packages without WIT, expose the
    // child modules and components as direct nav entries.
    push_component_children_nav(&mut items, ctx);

    let version_strs: Vec<&str> = ctx.versions.iter().map(String::as_str).collect();
    let base_url = format!("/{}/", ctx.display_name.replace(':', "/"));
    let header_html = build_sidebar_header(ctx);
    let project_html = build_project_section(ctx);
    let version_html = sidebar::render_version_selector(ctx.version, &version_strs, &base_url);
    let items_html = sidebar::render_items_nav(Some("Items"), &items);

    let mut aside = Aside::builder();
    aside.class("space-y-4");
    aside.text(header_html);

    // Version + Digest + Revision block (single bordered section)
    let revision = ctx.annotations.and_then(|a| a.revision.as_deref());
    let has_version = version_html.is_some();
    let has_digest = ctx.digest.is_some();
    let has_revision = revision.is_some();
    if has_version || has_digest || has_revision {
        let mut block =
            String::from(r#"<div class="pb-5 border-b-[1.5px] border-rule space-y-3">"#);
        if let Some(version) = &version_html {
            block.push_str(version);
        }
        if let Some(digest) = ctx.digest {
            block.push_str(&build_digest_row(digest));
        }
        if let Some(rev) = revision {
            block.push_str(&build_revision_row(rev));
        }
        block.push_str("</div>");
        aside.text(block);
    }

    if let Some(project) = &project_html {
        aside.text(project.clone());
    }
    aside.text(items_html);
    aside.build()
}

/// Lucide `package` icon (14px, for Component packages).
const SVG_PACKAGE: &str = concat!(
    r#"<svg class="h-3.5 w-3.5 flex-shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">"#,
    include_str!("../../../../vendor/lucide/package.svg"),
    "</svg>"
);

/// Lucide `layers` icon (14px, for Interface Types packages).
const SVG_LAYERS: &str = concat!(
    r#"<svg class="h-3.5 w-3.5 flex-shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">"#,
    include_str!("../../../../vendor/lucide/layers.svg"),
    "</svg>"
);

/// Lucide `box` icon (14px, fallback for unknown package kinds).
const SVG_BOX: &str = concat!(
    r#"<svg class="h-3.5 w-3.5 flex-shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">"#,
    include_str!("../../../../vendor/lucide/box.svg"),
    "</svg>"
);

/// Build the sidebar header with icon, title, version subtitle, and description.
fn build_sidebar_header(ctx: &SidebarContext<'_>) -> String {
    let ns = ctx.display_name.replace(':', "/");
    let desc = ctx.description.unwrap_or("No description available.");
    let icon = match ctx.kind_label {
        "Component" => SVG_PACKAGE,
        "Interface Types" => SVG_LAYERS,
        _ => SVG_BOX,
    };

    format!(
        r#"<div class="pb-4 border-b-[1.5px] border-rule"><div class="flex items-center gap-2.5"><span class="sigil" style="background:{};color:{};width:28px;height:28px;">{icon}</span><div><a href="/{ns}/{}" class="text-[15px] font-semibold text-ink-900 hover:underline no-underline">{}</a><div class="text-[11px] text-ink-500 mono">v{} · {}</div></div></div><p class="mt-2 text-[12px] text-ink-700 leading-relaxed">{desc}</p></div>"#,
        s::ROOT.bg,
        s::ROOT.color,
        ctx.version,
        ctx.display_name,
        ctx.version,
        ctx.kind_label,
    )
}

/// Build a "Project" section from OCI annotations and registry info.
///
/// Uses tree-link rows with icons matching the DS C01 "Project" section:
/// GitHub logo for github.com/ghcr.io URLs, book for docs, house for homepage.
fn build_project_section(ctx: &SidebarContext<'_>) -> Option<String> {
    let mut rows = Vec::new();

    // Registry link
    let registry_url = format!("https://{}/{}", ctx.registry, ctx.repository);
    let (registry_icon, _) = icon_and_label_for_url(&registry_url, "Registry");
    rows.push(project_link(&registry_url, registry_icon, "Registry"));

    if let Some(ann) = ctx.annotations {
        // Link rows
        if let Some(url) = &ann.url
            && ann.source.as_deref() != Some(url)
        {
            let icon = if is_github_url(url) {
                SVG_GITHUB
            } else {
                SVG_HOUSE
            };
            rows.push(project_link(url, icon, "Homepage"));
        }
        if let Some(source) = &ann.source {
            rows.push(project_link(source, SVG_GIT_FORK, "Repository"));
        }
        if let Some(docs) = &ann.documentation {
            let icon = if is_github_url(docs) {
                SVG_GITHUB
            } else {
                SVG_BOOK
            };
            rows.push(project_link(docs, icon, "Documentation"));
        }

        // Metadata rows
        if let Some(license) = &ann.licenses {
            let base = strip_with_clause(license);
            rows.push(project_icon_row(SVG_SCALE, &base));
        }
        if let Some(created) = &ann.created {
            let date = format_date(created);
            rows.push(project_icon_row(SVG_CALENDAR, &date));
        }
        if let Some(authors) = &ann.authors {
            rows.push(detail_row("Authors", authors));
        }
        if let Some(vendor) = &ann.vendor {
            rows.push(detail_row("Vendor", vendor));
        }
    }

    if rows.is_empty() {
        return None;
    }

    let items = rows.join("");
    Some(format!(
        r#"<div class="pb-4 mb-4 border-b-[1.5px] border-rule"><div class="mono uppercase tracking-wider text-[10px] text-ink-500 mb-2">Project</div><nav class="space-y-px">{items}</nav></div>"#
    ))
}

/// Render a project link as a tree-link with an icon and label.
fn project_link(href: &str, icon: &str, label: &str) -> String {
    format!(
        r#"<a href="{href}" class="tree-link" target="_blank" rel="noopener">{icon} {label}</a>"#
    )
}

/// Render a non-link row with an icon and text (tree-link styling, no href).
fn project_icon_row(icon: &str, text: &str) -> String {
    format!(r#"<div class="tree-link">{icon} {text}</div>"#)
}

/// Render a key-value detail row for the project section.
fn detail_row(label: &str, value: &str) -> String {
    format!(
        r#"<div class="flex items-baseline justify-between gap-4 py-1 text-[12px]"><span class="text-ink-500">{label}</span><span class="text-ink-700 mono text-right truncate">{value}</span></div>"#
    )
}

/// Check if a URL points to GitHub or GHCR.
fn is_github_url(url: &str) -> bool {
    url.contains("github.com") || url.contains("ghcr.io")
}

/// Return the appropriate icon and label for a URL.
///
/// GitHub/GHCR URLs get the GitHub logo; others get a house icon.
fn icon_and_label_for_url<'a>(url: &str, fallback_label: &'a str) -> (&'static str, &'a str) {
    let icon = if is_github_url(url) {
        SVG_GITHUB
    } else {
        SVG_HOUSE
    };
    (icon, fallback_label)
}

/// Extract the short interface name from a fully-qualified WIT name.
///
/// `"wasi:http/types@0.2.11"` → `"types"`
/// `"types"` → `"types"`
fn short_interface_name(name: &str) -> String {
    let without_version = name.split('@').next().unwrap_or(name);
    let short = without_version
        .rsplit('/')
        .next()
        .unwrap_or(without_version);
    short.to_owned()
}

/// Strip a `WITH` clause from an SPDX license expression.
///
/// `"Apache-2.0 WITH LLVM-Exception"` → `"Apache-2.0"`
/// `"MIT"` → `"MIT"`
fn strip_with_clause(license: &str) -> String {
    match license.find(" WITH ") {
        Some(pos) => license[..pos].to_owned(),
        None => license.to_owned(),
    }
}

/// Lucide copy icon (13px, for digest copy button).
const SVG_COPY_SM: &str = concat!(
    r#"<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
    include_str!("../../../../vendor/lucide/copy.svg"),
    "</svg>"
);

/// Build the digest row matching the install command copy button pattern.
fn build_digest_row(digest: &str) -> String {
    // Split "sha256:abcdef..." into prefix "sha256" and full hash
    let (prefix, hash) = digest.split_once(':').unwrap_or(("", digest));

    // Strip newlines from SVGs for JS embedding
    let copy_svg: String = SVG_COPY_SM.chars().filter(|c| *c != '\n').collect();
    let check_svg_raw = concat!(
        r#"<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="text-positive">"#,
        include_str!("../../../../vendor/lucide/check.svg"),
        "</svg>"
    );
    let check_svg: String = check_svg_raw.chars().filter(|c| *c != '\n').collect();

    let prefix_box = if prefix.is_empty() {
        String::new()
    } else {
        format!(
            r#"<span class="inline-flex items-center px-2.5 h-7 rounded-l-md border border-r-0 border-line bg-surfaceMuted text-[11px] text-ink-500 mono select-none">{prefix}</span>"#
        )
    };
    let code_rounding = if prefix.is_empty() {
        "rounded-l-md"
    } else {
        ""
    };

    format!(
        r#"<div><div class="mono uppercase tracking-wider text-[10px] text-ink-500 mb-1 flex items-center gap-1">Image Digest {info}</div><div class="flex">{prefix_box}<code class="inline-flex items-center px-2.5 h-7 flex-1 min-w-0 border border-line bg-surface mono text-[11px] text-ink-700 truncate {code_rounding}" title="{digest}">{hash}</code><button type="button" id="copy-digest-btn" class="inline-flex items-center justify-center w-7 h-7 rounded-r-md border border-l-0 border-line bg-surface text-ink-500 hover:text-ink-900 hover:bg-surfaceMuted" aria-label="Copy digest">{copy_svg}</button></div></div><script>(function(){{var b=document.getElementById('copy-digest-btn');var ci='{copy_svg}';var ch='{check_svg}';b.addEventListener('click',function(){{navigator.clipboard.writeText('{digest}').then(function(){{b.innerHTML=ch;setTimeout(function(){{b.innerHTML=ci}},2000)}})}})}})()</script>"#,
        info = sidebar::INFO_BUBBLE_DIGEST,
    )
}

/// Build the revision row matching the digest copy button pattern.
fn build_revision_row(revision: &str) -> String {
    let short = if revision.len() > 12 {
        format!("{}…", &revision[..12])
    } else {
        revision.to_owned()
    };
    let copy_svg: String = SVG_COPY_SM.chars().filter(|c| *c != '\n').collect();
    let check_svg_raw = concat!(
        r#"<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="text-positive">"#,
        include_str!("../../../../vendor/lucide/check.svg"),
        "</svg>"
    );
    let check_svg: String = check_svg_raw.chars().filter(|c| *c != '\n').collect();

    format!(
        r#"<div><div class="mono uppercase tracking-wider text-[10px] text-ink-500 mb-1 flex items-center gap-1">Revision {info}</div><div class="flex"><code class="inline-flex items-center px-2.5 h-7 flex-1 min-w-0 border border-line bg-surface mono text-[11px] text-ink-700 truncate rounded-l-md" title="{revision}">{short}</code><button type="button" id="copy-revision-btn" class="inline-flex items-center justify-center w-7 h-7 rounded-r-md border border-l-0 border-line bg-surface text-ink-500 hover:text-ink-900 hover:bg-surfaceMuted" aria-label="Copy revision">{copy_svg}</button></div></div><script>(function(){{var b=document.getElementById('copy-revision-btn');var ci='{copy_svg}';var ch='{check_svg}';b.addEventListener('click',function(){{navigator.clipboard.writeText('{revision}').then(function(){{b.innerHTML=ch;setTimeout(function(){{b.innerHTML=ci}},2000)}})}})}})()</script>"#,
        info = sidebar::INFO_BUBBLE_REVISION,
    )
}

/// Format an ISO 8601 date string to a human-friendly form.
///
/// `"2025-03-15T10:30:00Z"` → `"Mar 15, 2025"`
/// Falls back to the first 10 characters if parsing fails.
fn format_date(iso: &str) -> String {
    // Extract YYYY-MM-DD
    let date_part = if iso.len() >= 10 { &iso[..10] } else { iso };
    let parts: Vec<&str> = date_part.split('-').collect();
    if let [year, mm, dd] = parts.as_slice() {
        let month = match *mm {
            "01" => "Jan",
            "02" => "Feb",
            "03" => "Mar",
            "04" => "Apr",
            "05" => "May",
            "06" => "Jun",
            "07" => "Jul",
            "08" => "Aug",
            "09" => "Sep",
            "10" => "Oct",
            "11" => "Nov",
            "12" => "Dec",
            _ => return date_part.to_owned(),
        };
        let day = dd.trim_start_matches('0');
        format!("{month} {day}, {year}")
    } else {
        date_part.to_owned()
    }
}

/// Push WIT worlds and interfaces nav groups into `items`.
fn push_wit_nav(items: &mut Vec<SidebarItem>, ctx: &SidebarContext<'_>, doc: &WitDocument) {
    // Worlds — each world is a group with its imports/exports as children.
    for world in &doc.worlds {
        let is_active = matches!(ctx.active, SidebarActive::World(name) if name == world.name);
        let mut children = Vec::new();
        for item in world.imports.iter().chain(world.exports.iter()) {
            if let crate::wit_doc::WorldItemDoc::Interface {
                name,
                url: Some(url),
                ..
            } = item
            {
                children.push(SidebarEntry {
                    sigil_bg: s::IFACE.bg,
                    sigil_color: s::IFACE.color,
                    sigil_text: s::IFACE.text,
                    name: short_interface_name(name),
                    href: url.clone(),
                    meta: String::new(),
                    active: false,
                });
            }
        }
        items.push(SidebarItem::Group(SidebarGroup {
            label: world.name.clone(),
            href: Some(world.url.clone()),
            sigil_bg: Some(s::WORLD.bg),
            sigil_color: Some(s::WORLD.color),
            sigil_text: Some(s::WORLD.text),
            open: is_active,
            count: None,
            children,
        }));
    }

    // Interfaces — each interface is a group with its types and functions as children.
    for iface in &doc.interfaces {
        let is_active = matches!(
            ctx.active,
            SidebarActive::Interface(name) if name == iface.name
        ) || matches!(
            ctx.active,
            SidebarActive::Item(iface_name, _) if iface_name == iface.name
        );
        let mut children = Vec::new();
        for ty in &iface.types {
            let sigil = s::for_type_kind(&ty.kind);
            children.push(SidebarEntry {
                sigil_bg: sigil.bg,
                sigil_color: sigil.color,
                sigil_text: sigil.text,
                name: ty.name.clone(),
                href: ty.url.clone(),
                meta: String::new(),
                active: matches!(
                    ctx.active,
                    SidebarActive::Item(iface_name, item_name) if iface_name == iface.name && item_name == ty.name
                ),
            });
        }
        for func in &iface.functions {
            children.push(SidebarEntry {
                sigil_bg: s::FUNC.bg,
                sigil_color: s::FUNC.color,
                sigil_text: s::FUNC.text,
                name: func.name.clone(),
                href: func.url.clone(),
                meta: String::new(),
                active: matches!(
                    ctx.active,
                    SidebarActive::Item(iface_name, item_name) if iface_name == iface.name && item_name == func.name
                ),
            });
        }
        items.push(SidebarItem::Group(SidebarGroup {
            label: iface.name.clone(),
            href: Some(iface.url.clone()),
            sigil_bg: Some(s::IFACE.bg),
            sigil_color: Some(s::IFACE.color),
            sigil_text: Some(s::IFACE.text),
            open: is_active,
            count: None,
            children,
        }));
    }
}

/// Push Modules / Components nav groups built from `ctx.components` children
/// into `items`. Used for component packages without a parsed WIT document.
fn push_component_children_nav(items: &mut Vec<SidebarItem>, ctx: &SidebarContext<'_>) {
    // Flatten one level of children — top-level component(s) usually wrap the
    // children we want to show in nav.
    let mut child_idx = 0usize;
    let mut modules: Vec<(usize, &ComponentSummary)> = Vec::new();
    let mut components: Vec<(usize, &ComponentSummary)> = Vec::new();
    for comp in ctx.components {
        for child in &comp.children {
            let kind = child.kind.as_deref().unwrap_or("module");
            if kind == "component" {
                components.push((child_idx, child));
            } else {
                modules.push((child_idx, child));
            }
            child_idx += 1;
        }
    }

    let push_group = |items: &mut Vec<SidebarItem>,
                      label: &str,
                      sigil: &crate::components::ds::sigil::Sigil,
                      entries: &[(usize, &ComponentSummary)]| {
        if entries.is_empty() {
            return;
        }
        let kind_url = if label == "Components" {
            "component"
        } else {
            "module"
        };
        let children: Vec<SidebarEntry> = entries
            .iter()
            .map(|(idx, child)| {
                let display = child
                    .name
                    .clone()
                    .unwrap_or_else(|| format!("{kind_url} {idx}"));
                let active = matches!(ctx.active, SidebarActive::Child(name) if name == display);
                SidebarEntry {
                    sigil_bg: sigil.bg,
                    sigil_color: sigil.color,
                    sigil_text: sigil.text,
                    name: display,
                    href: format!("{}/{}/{}", ctx.url_base, kind_url, idx),
                    meta: String::new(),
                    active,
                }
            })
            .collect();
        let any_active = children.iter().any(|c| c.active);
        items.push(SidebarItem::Group(SidebarGroup {
            label: label.to_owned(),
            href: None,
            sigil_bg: Some(sigil.bg),
            sigil_color: Some(sigil.color),
            sigil_text: Some(sigil.text),
            open: any_active || ctx.doc.is_none(),
            count: None,
            children,
        }));
    };

    push_group(items, "Modules", &s::MODULE, &modules);
    push_group(items, "Components", &s::COMPONENT, &components);
}
