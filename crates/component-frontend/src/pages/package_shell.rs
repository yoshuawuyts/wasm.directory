//! Shared page shell for the package detail page and its sub-pages
//! (interface, world, item).
//!
//! Provides a two-column layout: main content on the left, and a sidebar
//! on the right with version selector, install command, metadata,
//! dependencies, and dependents.

use html::text_content::Division;
use component_meta_registry_client::{KnownPackage, PackageVersion};

use crate::layout;

/// Context for rendering the package page sidebar.
#[allow(dead_code)]
pub(crate) struct SidebarContext<'a> {
    /// Package being displayed.
    pub pkg: &'a KnownPackage,
    /// Current version string.
    pub version: &'a str,
    /// Version detail (annotations, size, etc.) if available.
    pub version_detail: Option<&'a PackageVersion>,
    /// Packages that import this one.
    pub importers: &'a [KnownPackage],
    /// Packages that export this one.
    pub exporters: &'a [KnownPackage],
    /// Optional navigation card HTML (interfaces/worlds/items list).
    pub nav_html: Option<String>,
}

/// Render the shared page shell: two-column layout with sidebar,
/// wrapped in the HTML document layout.
#[must_use]
pub(crate) fn render_page(
    ctx: &SidebarContext<'_>,
    title: &str,
    header: &str,
    body_content: &str,
    toc_html: Option<&str>,
) -> String {
    render_page_inner(ctx, title, header, body_content, &[], true, toc_html)
}

/// Render the page shell with extra breadcrumb segments after the package name.
#[must_use]
pub(crate) fn render_page_with_crumbs(
    ctx: &SidebarContext<'_>,
    title: &str,
    header: &str,
    body_content: &str,
    extra_crumbs: &[crate::components::ds::breadcrumb::Crumb],
    toc_html: Option<&str>,
) -> String {
    render_page_inner(
        ctx,
        title,
        header,
        body_content,
        extra_crumbs,
        false,
        toc_html,
    )
}

/// Inner page shell renderer.
///
/// Uses a "golden layout": left sidebar with navigation and metadata,
/// right column for main content. The top nav bar is replaced by the
/// sidebar's own logo, breadcrumbs, and search.
fn render_page_inner(
    ctx: &SidebarContext<'_>,
    title: &str,
    header: &str,
    body_content: &str,
    extra_crumbs: &[crate::components::ds::breadcrumb::Crumb],
    _is_root: bool,
    toc_html: Option<&str>,
) -> String {
    use crate::components::ds::breadcrumb::Crumb;
    use crate::components::ds::navbar::{self, NavLink};

    let pkg = ctx.pkg;
    let version = ctx.version;

    // Build breadcrumbs: namespace > name > extra
    let mut crumbs = Vec::new();
    let pkg_url = url_base_for(pkg, version);
    if let Some(ns) = &pkg.wit_namespace {
        crumbs.push(Crumb {
            label: ns.clone(),
            href: Some(format!("/{ns}")),
        });
    }
    if let Some(name) = &pkg.wit_name {
        crumbs.push(Crumb {
            label: name.clone(),
            href: Some(pkg_url),
        });
    }
    for c in extra_crumbs {
        crumbs.push(Crumb {
            label: c.label.clone(),
            href: c.href.clone(),
        });
    }

    #[allow(clippy::items_after_statements)]
    const LINKS: &[NavLink] = &[
        NavLink {
            label: "Downloads",
            href: "/downloads",
        },
    ];
    let nav = navbar::render_bar(&crumbs, LINKS);

    // Sidebar navigation (interfaces/worlds tree)
    let sidebar_html = ctx.nav_html.as_deref().unwrap_or("");

    let toc_column = match toc_html {
        Some(toc) => format!(
            r#"<aside class="hidden lg:block lg:sticky lg:top-16 lg:self-start lg:max-h-[calc(100vh-5rem)] lg:overflow-y-auto">{toc}</aside>"#
        ),
        None => String::new(),
    };

    let grid_class = if toc_html.is_some() {
        "max-w-[1440px] px-4 md:px-6 grid grid-cols-1 lg:grid-cols-[240px_1fr_200px] gap-8 lg:gap-10 pt-8 pb-24"
    } else {
        "max-w-[1440px] px-4 md:px-6 grid grid-cols-1 md:grid-cols-[240px_1fr] gap-8 lg:gap-10 pt-8 pb-24"
    };

    let body = format!(
        r#"{nav}
<div class="{grid_class}">
  <aside class="hidden md:block md:sticky md:top-16 md:self-start md:max-h-[calc(100vh-5rem)] md:overflow-y-auto -mx-2 px-2">{sidebar_html}</aside>
  <article class="min-w-0">{header}{body_content}</article>
  {toc_column}
</div>"#,
    );

    layout::document_full_width(title, &body)
}

/// Render breadcrumb segments as inline HTML.
#[allow(dead_code)]
fn render_breadcrumb_path(crumbs: &[crate::components::ds::breadcrumb::Crumb]) -> String {
    use std::fmt::Write;
    let mut html = String::new();
    for crumb in crumbs {
        html.push_str(r#" <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="inline-block text-ink-300 mx-1 align-[-1px]"><path d="m9 18 6-6-6-6"/></svg> "#);
        if let Some(href) = &crumb.href {
            write!(
                html,
                r#"<a href="{href}" class="text-ink-500 hover:text-ink-900 transition-colors">{label}</a>"#,
                label = crumb.label
            )
            .unwrap();
        } else {
            write!(
                html,
                r#"<span class="text-ink-900">{label}</span>"#,
                label = crumb.label
            )
            .unwrap();
        }
    }
    html
}

/// Sidebar section label class matching the design system Details (section 23).
#[allow(dead_code)]
const SIDEBAR_LABEL: &str = crate::components::ds::typography::SECTION_LABEL_CLASS;

/// Render the right sidebar with all package metadata.
#[allow(dead_code)]
fn render_sidebar(ctx: &SidebarContext<'_>, display_name: &str) -> Division {
    let pkg = ctx.pkg;
    let version = ctx.version;
    let version_detail = ctx.version_detail;
    let annotations = version_detail.and_then(|d| d.annotations.as_ref());

    let mut sidebar = Division::builder();
    sidebar.class("space-y-4");

    // ── Version selector ─────────────────────────────────
    if !pkg.tags.is_empty() {
        let url_name = match (&pkg.wit_namespace, &pkg.wit_name) {
            (Some(ns), Some(name)) => format!("{ns}/{name}"),
            _ => pkg.repository.clone(),
        };
        sidebar.push(render_version_select(pkg, version, &url_name));
    }

    // ── Metadata detail rows ─────────────────────────────
    sidebar.division(|meta| {
        {
            let registry_url = format!("https://{}/{}", pkg.registry, pkg.repository);
            let registry_display = friendly_registry_name(&pkg.registry);
            meta.push(meta_link_row("Registry", &registry_display, &registry_url));
        }
        if let Some(source) = annotations.and_then(|a| a.source.as_deref()) {
            meta.push(meta_link_row(
                "Repository",
                &friendly_repo_name(source),
                source,
            ));
        } else {
            let repo_url = format!("https://{}/{}", pkg.registry, pkg.repository);
            let repo_display = friendly_repo_name(&repo_url);
            meta.push(meta_link_row("Repository", &repo_display, &repo_url));
        }
        if let Some(license) = annotations.and_then(|a| a.licenses.as_deref()) {
            meta.push(meta_row("License", license));
        }
        if let Some(size) = version_detail.and_then(|d| d.size_bytes) {
            meta.push(meta_row("Size", &format_size(size)));
        }
        if let Some(created) = version_detail.and_then(|d| d.created_at.as_deref()) {
            meta.push(meta_row("Published", &format_date(created)));
        }
        if let Some(docs_url) = annotations.and_then(|a| a.documentation.as_deref()) {
            meta.push(meta_link_row("Docs", &abbreviate_url(docs_url), docs_url));
        }
        let authors = annotations.and_then(|a| a.authors.as_deref()).or_else(|| {
            version_detail.and_then(|d| d.components.first().and_then(|c| c.authors.as_deref()))
        });
        if let Some(authors) = authors {
            meta.push(meta_row("Authors", authors));
        }
        let oci_source = annotations.and_then(|a| a.source.as_deref());
        let homepage = annotations.and_then(|a| a.url.as_deref()).or_else(|| {
            version_detail.and_then(|d| d.components.first().and_then(|c| c.homepage.as_deref()))
        });
        if let Some(url) = homepage
            && oci_source != Some(url)
        {
            meta.push(meta_link_row("Homepage", &abbreviate_url(url), url));
        }
        if oci_source.is_none()
            && let Some(src) =
                version_detail.and_then(|d| d.components.first().and_then(|c| c.source.as_deref()))
        {
            meta.push(meta_link_row("Source", &abbreviate_url(src), src));
        }
        let revision = annotations.and_then(|a| a.revision.as_deref()).or_else(|| {
            version_detail.and_then(|d| d.components.first().and_then(|c| c.revision.as_deref()))
        });
        if let Some(rev) = revision {
            let display = if rev.len() > 12 { &rev[..12] } else { rev };
            meta.push(meta_row("Revision", display));
        }
        meta
    });

    // ── Navigation card (interfaces/worlds/items) ────────
    if let Some(nav) = &ctx.nav_html {
        sidebar.text(nav.clone());
    }

    // ── Dependencies ─────────────────────────────────────
    if !pkg.dependencies.is_empty() {
        sidebar.division(|wrapper| {
            wrapper
                .class("my-3 border-t-[1.5px] border-rule pt-3")
                .heading_3(|h3| h3.class(SIDEBAR_LABEL).text("Dependencies"));
            let mut ul = html::text_content::UnorderedList::builder();
            ul.class("space-y-1");
            for dep in &pkg.dependencies {
                ul.list_item(|li| {
                    li.class("text-[12px]");
                    match dep.package.split_once(':') {
                        Some((ns, name)) => {
                            li.anchor(|a| {
                                a.href(format!("/{ns}/{name}"))
                                    .class("text-accent hover:underline")
                                    .text(dep.package.clone())
                            });
                        }
                        None => {
                            li.span(|s| s.class("text-ink-900").text(dep.package.clone()));
                        }
                    }
                    if let Some(v) = &dep.version {
                        li.span(|s| s.class("text-ink-400 ml-1").text(format!("@{v}")));
                    }
                    li
                });
            }
            wrapper.push(ul.build());
            wrapper
        });
    }

    // ── Dependents ───────────────────────────────────────
    let total_dependents = ctx.importers.len() + ctx.exporters.len();
    if total_dependents > 0 {
        sidebar.division(|wrapper| {
            wrapper
                .class("my-3 border-t-[1.5px] border-rule pt-3")
                .heading_3(|h3| h3.class(SIDEBAR_LABEL).text("Dependents"));
            wrapper.anchor(|a| {
                a.href(format!("/search?q={display_name}"))
                    .class("text-[13px] text-accent hover:underline")
                    .text("Search for dependent packages \u{2192}")
            });
            wrapper
        });
    }

    sidebar.build()
}

/// Compute the display name from package WIT metadata.
pub(crate) fn display_name_for(pkg: &KnownPackage) -> String {
    match (&pkg.wit_namespace, &pkg.wit_name) {
        (Some(ns), Some(name)) => format!("{ns}:{name}"),
        _ => pkg.repository.clone(),
    }
}

/// Get a human-readable kind label for a package.
pub(crate) fn kind_label_for(pkg: &KnownPackage) -> &'static str {
    match pkg.kind {
        Some(component_meta_registry_client::PackageKind::Interface) => "Interface Types",
        Some(component_meta_registry_client::PackageKind::Component) => "Component",
        _ => "Package",
    }
}

/// Compute the URL base for sub-page links.
pub(crate) fn url_base_for(pkg: &KnownPackage, version: &str) -> String {
    match (&pkg.wit_namespace, &pkg.wit_name) {
        (Some(ns), Some(name)) => format!("/{ns}/{name}/{version}"),
        _ => format!("/{}/{version}", pkg.repository),
    }
}

/// Render the version selector dropdown.
#[allow(dead_code)]
fn render_version_select(pkg: &KnownPackage, current_version: &str, url_name: &str) -> Division {
    let script_body = format!(
        "document.getElementById('version-select').addEventListener('change',function(){{\
        var p=window.location.pathname;\
        var base='/{url_name}/';\
        var rest=p.indexOf(base)===0?p.slice(base.length):'';\
        var slash=rest.indexOf('/');\
        var sub=slash>=0?rest.slice(slash):'';\
        window.location.href=base+this.value+sub\
        }})"
    );

    Division::builder()
        .class("flex items-center justify-between gap-3")
        .span(|s| s.class("text-ink-500 text-[13px]").text("Version"))
        .push({
            let mut s = html::forms::Select::builder();
            s.id("version-select").name("version").class(
                "bg-transparent text-ink-900 text-[13px] cursor-pointer border-0 outline-none text-right",
            );
            for tag in &pkg.tags {
                let is_current = tag == current_version;
                if is_current {
                    s.option(|opt| opt.value(tag.clone()).text(tag.clone()).selected(true));
                } else {
                    s.option(|opt| opt.value(tag.clone()).text(tag.clone()));
                }
            }
            s.build()
        })
        .script(|s| s.text(script_body))
        .build()
}

/// Render the install command section with a copy button.
#[allow(dead_code)]
pub(crate) fn render_install_command(display_name: &str, version: &str) -> Division {
    let command = format!("component install {display_name}@{version}");

    let copy_icon = "<svg xmlns='http://www.w3.org/2000/svg' width='16' height='16' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><rect x='9' y='9' width='13' height='13' rx='2' ry='2'/><path d='M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1'/></svg>";
    let check_icon = "<svg xmlns='http://www.w3.org/2000/svg' width='16' height='16' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><polyline points='20 6 9 17 4 12'/></svg>";

    let script = format!(
        "(function(){{\
        var btn=document.getElementById('copy-install-btn');\
        var copyIcon=\"{copy_icon}\";\
        var checkIcon=\"{check_icon}\";\
        btn.innerHTML=copyIcon;\
        btn.addEventListener('click',function(){{\
        navigator.clipboard.writeText('{command}').then(function(){{\
        btn.innerHTML=checkIcon;\
        setTimeout(function(){{btn.innerHTML=copyIcon}},2000)\
        }})}})}})()",
    );

    Division::builder()
        .division(|div| {
            div.class(
                "flex items-center gap-2 rounded-md border border-line \
                 px-3 py-2 text-[12px] text-ink-900",
            )
            .code(|code| {
                code.class("flex-1 select-all overflow-hidden whitespace-nowrap text-ellipsis")
                    .text(command)
            })
            .button(|btn| {
                btn.id("copy-install-btn").class(
                    "shrink-0 text-ink-500 hover:text-ink-900 transition-opacity cursor-pointer",
                )
            })
            .script(|s| s.text(script))
        })
        .build()
}

/// Render a label: value metadata row.
#[allow(dead_code)]
fn meta_row(label: &str, value: &str) -> Division {
    crate::components::ds::detail_row::row(
        label,
        crate::components::ds::detail_row::Value::Text(value.to_owned()),
    )
}

/// Render a label: linked-value metadata row.
#[allow(dead_code)]
fn meta_link_row(label: &str, text: &str, href: &str) -> Division {
    crate::components::ds::detail_row::row(
        label,
        crate::components::ds::detail_row::Value::Link {
            text: text.to_owned(),
            href: href.to_owned(),
        },
    )
}

/// Format a byte count as a human-readable size string.
#[allow(clippy::cast_precision_loss)]
#[allow(dead_code)]
fn format_size(bytes: i64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;

    let bytes = bytes as f64;
    if bytes < KIB {
        format!("{bytes} B")
    } else if bytes < MIB {
        format!("{:.1} KiB", bytes / KIB)
    } else if bytes < GIB {
        format!("{:.1} MiB", bytes / MIB)
    } else {
        format!("{:.1} GiB", bytes / GIB)
    }
}

/// Abbreviate a URL for display (strip scheme and trailing slash).
#[allow(dead_code)]
fn abbreviate_url(url: &str) -> String {
    url.strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url)
        .trim_end_matches('/')
        .to_owned()
}

/// Return a friendly display name for a known OCI registry, or the full host/path.
#[allow(dead_code)]
fn friendly_registry_name(registry: &str) -> String {
    match registry {
        "ghcr.io" => "GitHub Packages".to_owned(),
        "registry-1.docker.io" | "docker.io" => "Docker Hub".to_owned(),
        "mcr.microsoft.com" => "Microsoft MCR".to_owned(),
        _ => registry.to_owned(),
    }
}

/// Return a friendly display name for a known repository host, or the abbreviated URL.
#[allow(dead_code)]
fn friendly_repo_name(url: &str) -> String {
    let stripped = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);

    if stripped.starts_with("github.com/") {
        "GitHub".to_owned()
    } else if stripped.starts_with("gitlab.com/") {
        "GitLab".to_owned()
    } else if stripped.starts_with("codeberg.org/") {
        "Codeberg".to_owned()
    } else {
        abbreviate_url(url)
    }
}

/// Format an ISO 8601 timestamp as a short date (YYYY-MM-DD).
#[allow(dead_code)]
fn format_date(iso: &str) -> String {
    iso.split('T').next().unwrap_or(iso).to_owned()
}
