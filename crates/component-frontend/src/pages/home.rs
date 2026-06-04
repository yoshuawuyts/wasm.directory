//! Front page — landing experience matching `references/landing.html`.

// r[impl frontend.pages.home]

use component_meta_registry_client::{ApiError, KnownPackage, RegistryClient};

use crate::components::ds::{
    cta_strip::{self, CtaStrip},
    hero::{self, Hero},
    principles_grid::{self, Principle},
    search_bar,
};
use crate::layout;

/// Fetch recent packages and render the home page.
pub(crate) async fn render(client: &RegistryClient) -> String {
    match client.fetch_recent_packages(1000).await {
        Ok(packages) => render_packages(&packages),
        Err(err) => render_error(&err),
    }
}

/// Render the home page with a list of packages.
fn render_packages(packages: &[KnownPackage]) -> String {
    let body = compose_body(&Stats::from_packages(packages), None);
    layout::document_landing("Home", &body)
}

/// Render the home page with an API error message — keep the chrome but
/// surface a small notice so visitors know the live data is unavailable.
fn render_error(err: &ApiError) -> String {
    let notice = format!(
        r#"<div class="mx-auto mx-auto max-w-[1280px] w-full px-4 md:px-8 pt-4"><div role="status" class="flex items-start gap-2 rounded-md border border-line bg-surfaceMuted px-3 py-2 text-[12px] text-ink-700"><span class="mono uppercase tracking-wider text-ink-500">Registry offline</span><span>Live package data is temporarily unavailable, so search may return nothing right now. The <code class="px-1 py-0.5 rounded-sm bg-surface text-ink-900 mono text-[0.875em]">component</code> CLI still works locally without the registry — see the <a href="/docs" class="text-ink-900 hover:underline">docs</a> to get started. ({err})</span></div></div>"#,
        err = html_escape(&err.to_string()),
    );
    let body = compose_body(&Stats::default(), Some(&notice));
    layout::document_landing("Home", &body)
}

/// Minimal HTML escape for inline error text.
fn html_escape(s: &str) -> String {
    crate::escape::escape_html_text(s)
}

/// Aggregated landing-page statistics derived from the registry index.
#[derive(Default)]
struct Stats {
    /// Total number of indexed packages.
    packages: usize,
    /// Number of distinct WIT namespaces (or repository owners as fallback).
    namespaces: usize,
    /// Sum of release tag counts across all packages.
    versions: usize,
}

impl Stats {
    fn from_packages(packages: &[KnownPackage]) -> Self {
        use std::collections::BTreeSet;

        let package_count = packages.len();
        let version_count: usize = packages.iter().map(|p| p.tags.len()).sum();

        // Count distinct WIT namespaces (preferred) or fall back to the first
        // segment of the repository path.
        let mut ns_set: BTreeSet<String> = BTreeSet::new();
        for pkg in packages {
            let ns = pkg
                .wit_namespace
                .clone()
                .or_else(|| pkg.repository.split('/').next().map(str::to_owned))
                .unwrap_or_default();
            if ns.is_empty() {
                continue;
            }
            ns_set.insert(ns);
        }

        Self {
            packages: package_count,
            namespaces: ns_set.len(),
            versions: version_count,
        }
    }
}

/// Compose the full landing page body. `notice_html` is rendered above
/// the hero when present (for example, a registry-offline banner).
fn compose_body(stats: &Stats, notice_html: Option<&str>) -> String {
    let pkg_count = format_count(stats.packages);
    let ns_count = format_count(stats.namespaces);
    let version_count = format_count(stats.versions);

    let examples = [
        (
            "wasi:http",
            "Find components that can handle incoming HTTP requests",
        ),
        ("wasi", "Browse the reusable standard WASI interfaces"),
        (
            "wasi:keyvalue",
            "Discover key-value storage backends to plug into",
        ),
    ];
    let search_card = search_bar::landing_card(
        &search_bar::LandingStats {
            packages: &pkg_count,
            namespaces: &ns_count,
            versions: &version_count,
        },
        &examples,
    )
    .to_string();

    let hero_html = hero::render(&Hero {
        kicker: &[],
        title: "The package manager for wasm components.",
        lede: "Resolve, vendor, and compose WebAssembly components from any registry. \
               Reproducible builds and semantic versioning — so the dependency you \
               shipped is the dependency you keep.",
        ctas: &[],
        right: &search_card,
    });

    let principles_html = principles_grid::render(
        "",
        "Why Wasm Components?",
        "Wasm Components package core WebAssembly instructions into portable binaries and libraries with fully-typed interfaces.",
        PRINCIPLES,
    );

    let cta_html = cta_strip::render(&CtaStrip {
        kicker: "For maintainers",
        title: "Publish your component.",
        body_html: "Add your namespace to a registry config and run \
                    <code class=\"px-1 py-0.5 rounded-sm bg-surfaceMuted text-ink-900 mono text-[0.875em]\">component publish</code>. \
                    Every release is signed end-to-end.",
        primary_label: "Open the publishing guide",
        primary_href: "/docs",
        secondary_label: "Read the spec",
        secondary_href: "/docs",
    });

    let notice_html = notice_html.unwrap_or("");
    format!(
        r#"{notice_html}
{hero_html}
<div class="pb-16 md:pb-24">
{principles_html}
{cta_html}
</div>"#
    )
}

/// Format a count with a thin space as the thousands separator (e.g.
/// `1248 -> "1 248"`), matching the visual style in `landing.html`.
fn format_count(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push('\u{2009}');
        }
        out.push(c);
    }
    out.chars().rev().collect()
}

const STACK_SVG: &str = concat!(
    r#"<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">"#,
    include_str!("../../../../vendor/lucide/layers.svg"),
    "</svg>",
);
const GLOBE_SVG: &str = r#"<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="9"/><path d="M3 12h18"/><path d="M12 3a14 14 0 0 1 0 18a14 14 0 0 1 0-18z"/></svg>"#;
const GRID_SVG: &str = r#"<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="3" width="7" height="7" rx="1"/><rect x="3" y="14" width="7" height="7" rx="1"/><rect x="14" y="14" width="7" height="7" rx="1"/></svg>"#;
const CODE_SVG: &str = r#"<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><path d="m16 18 6-6-6-6"/><path d="m8 6-6 6 6 6"/></svg>"#;

const PRINCIPLES: &[Principle<'static>] = &[
    Principle {
        bg_class: "bg-cat-blue",
        fg_class: "text-cat-blueInk",
        icon_svg: STACK_SVG,
        title: "Compose sandboxes together",
        body: "Components can be linked together thanks to the strongly typed \
        WIT interfaces. Each component is always individually sandboxed, so a \
        problem in one component doesn't become a problem for all other \
        components.",
    },
    Principle {
        bg_class: "bg-cat-green",
        fg_class: "text-cat-greenInk",
        icon_svg: GRID_SVG,
        title: "Works with (almost) any language",
        body: "Choose the best language for the job — Rust, Go, JS, Python, C. \
               The interface is the contract, so if you ever need to switch languages, you can.",
    },
    Principle {
        bg_class: "bg-cat-peach",
        fg_class: "text-cat-peachInk",
        icon_svg: GLOBE_SVG,
        title: "Truly portable binaries",
        body: "Ship the same binary to your server, editor, or browser. As long as the host \
               provides the right imports, components will run anywhere.",
    },
    Principle {
        bg_class: "bg-cat-lilac",
        fg_class: "text-cat-lilacInk",
        icon_svg: CODE_SVG,
        title: "SDKs for free",
        body: "Define your interface once in WIT and every language gets a typed \
               binding — Rust, Go, JS, Python, C. No hand-written client library \
               to write and maintain per language.",
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    // r[verify frontend.pages.home]
    #[test]
    fn format_count_inserts_thin_spaces() {
        assert_eq!(format_count(73), "73");
        assert_eq!(format_count(1248), "1\u{2009}248");
        assert_eq!(format_count(1_234_567), "1\u{2009}234\u{2009}567");
    }

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

    #[test]
    fn stats_aggregate_counts_and_authors() {
        let packages = vec![
            pkg("wasi", "http", &["0.1.0", "0.2.0", "0.2.1"], Some("HTTP")),
            pkg("wasi", "io", &["0.2.0"], None),
            pkg("ba", "sqlite", &["0.1.0", "0.2.0"], Some("SQLite")),
        ];
        let stats = Stats::from_packages(&packages);
        assert_eq!(stats.packages, 3);
        assert_eq!(stats.namespaces, 2);
        assert_eq!(stats.versions, 6);
    }
}
