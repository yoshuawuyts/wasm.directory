//! Front page — landing experience matching `references/landing.html`.

// r[impl frontend.pages.home]

use component_meta_registry_client::{ApiError, KnownPackage, RegistryClient};

use crate::components::ds::{
    cta_strip::{self, CtaStrip},
    hero::{self, Hero},
    install_widget::{self, InstallOption},
    navbar,
    quick_start::{self, QuickStart, QuickStartStep},
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
        r#"<div class="mx-auto mx-auto max-w-[1280px] w-full px-4 md:px-8 pt-4"><div role="status" class="flex items-start gap-2 rounded-md border border-line bg-surfaceMuted px-3 py-2 text-[12px] text-ink-700"><span class="mono uppercase tracking-wider text-ink-500">Registry offline</span><span>Live package data is temporarily unavailable, so search may return nothing right now. The <code class="px-1 py-0.5 rounded-sm bg-surface text-ink-900 mono text-[0.875em]">component</code> CLI still works locally without the registry. See the <a href="/docs" class="text-ink-900 hover:underline">docs</a> to get started. ({err})</span></div></div>"#,
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

/// Persistent call-out marking the registry as alpha. Slotted into the hero's
/// left column directly below the lede (and above the search card in reading
/// order) on every visit, so it is clear the project is early and its data and
/// APIs are not yet stable. It reuses the navbar's alpha badge for a consistent
/// header treatment, sits on the white `surface` paper, and borrows the badge's
/// orange border so the two read as one family without shouting.
fn alpha_notice() -> String {
    format!(
        r#"<div role="note" class="mt-8 max-w-2xl flex items-start gap-3 rounded-lg border border-orange-200 bg-surface px-4 py-3">{badge}<span class="text-[13px] text-ink-700 leading-relaxed">This registry is in <strong class="font-semibold text-ink-900">alpha</strong>. Indexed data may be incomplete or reset without notice, and both the site and APIs can still change or break. Explore freely and <a href="https://github.com/yoshuawuyts/component-registry/issues" class="font-medium text-ink-900 underline underline-offset-2 hover:no-underline">share feedback</a>, but don't depend on it in production yet.</span></div>"#,
        badge = navbar::ALPHA_BADGE,
    )
}

/// Compose the full landing page body. The alpha call-out lives inside the
/// hero (below the lede); `notice_html` is rendered above the hero when
/// present (for example, a registry-offline banner).
fn compose_body(stats: &Stats, notice_html: Option<&str>) -> String {
    let pkg_count = format_count(stats.packages);
    let ns_count = format_count(stats.namespaces);
    let version_count = format_count(stats.versions);

    let examples = [
        search_bar::Example::link("/wasi", "Browse the reusable standard WASI interfaces"),
        search_bar::Example::link("/autostamp", "Discover HTTP client programs"),
        // Not working yet: crossed out and pointed at a dead link for now.
        search_bar::Example::link(
            "#",
            "Find components that can handle incoming HTTP requests",
        )
        .struck(),
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

    let alpha_notice = alpha_notice();
    let hero_html = hero::render(&Hero {
        kicker: &[],
        title: "A meta-registry for WebAssembly",
        lede: "Find WebAssembly applications, libraries, and interface types published to any OCI 1.1-compliant registry. \
               This includes GitHub Packages, AWS ECR, JFrog Artifactory, and more. \
               Wasm Directory never serves packages directly: its only job is to serve metadata and resolve names.",
        note: &alpha_notice,
        ctas: &[],
        right: &search_card,
    });

    let cta_html = cta_strip::render(&CtaStrip {
        kicker: "For maintainers",
        title: "Publish your first component.",
        body_html: "Add your namespace to a registry config and run \
                    <code class=\"px-1 py-0.5 rounded-sm bg-surfaceMuted text-ink-900 mono text-[0.875em]\">component publish</code>.",
        primary_label: "Open the publishing guide",
        primary_href: "/docs",
        secondary_label: "Read the spec",
        secondary_href: "/docs",
    });

    let install_widget_html = install_widget::widget(&[
        InstallOption {
            id: "linux",
            label: "Linux",
            command: "curl --proto '=https' --tlsv1.2 -LsSf https://github.com/yoshuawuyts/component-cli/releases/latest/download/install.sh | sh",
        },
        InstallOption {
            id: "macos",
            label: "macOS",
            command: "curl --proto '=https' --tlsv1.2 -LsSf https://github.com/yoshuawuyts/component-cli/releases/latest/download/install.sh | sh",
        },
        InstallOption {
            id: "windows",
            label: "Windows",
            command: "irm https://github.com/yoshuawuyts/component-cli/releases/latest/download/install.ps1 | iex",
        },
    ]);

    let quickstart_html = quick_start::render(&QuickStart {
        kicker: "Get started",
        title: "Run your first component",
        intro_html: "Install the CLI, then go from an empty directory to a running \
                     <code class=\"px-1 py-0.5 rounded-sm bg-surfaceMuted text-ink-900 mono text-[0.875em]\">wasi:http</code> server.",
        steps: &[
            QuickStartStep {
                title: "Install the CLI",
                body_html: "Grab the <code class=\"px-1 py-0.5 rounded-sm bg-surfaceMuted text-ink-900 mono text-[0.875em]\">component</code> CLI for your platform. \
                            We\u{2019}ve picked the command that matches your system. Copy it and run.",
                command: "",
                command_html: &install_widget_html,
            },
            QuickStartStep {
                title: "Initialize a project",
                body_html: "Scaffold a project in the current directory. This writes a \
                            <code class=\"px-1 py-0.5 rounded-sm bg-surfaceMuted text-ink-900 mono text-[0.875em]\">wasm.toml</code> manifest and a \
                            <code class=\"px-1 py-0.5 rounded-sm bg-surfaceMuted text-ink-900 mono text-[0.875em]\">wasm.lock.toml</code> lockfile so dependencies resolve to exact, reproducible versions.",
                command: "component init",
                command_html: "",
            },
            QuickStartStep {
                title: "Add a dependency",
                body_html: "Add a dependency from any OCI registry. Here we pull in \
                            <code class=\"px-1 py-0.5 rounded-sm bg-surfaceMuted text-ink-900 mono text-[0.875em]\">ba:sample-wasi-http-rust</code> (a ready-made \
                            <code class=\"px-1 py-0.5 rounded-sm bg-surfaceMuted text-ink-900 mono text-[0.875em]\">wasi:http</code> server) and record it in your manifest and lockfile.",
                command: "component install ba:sample-wasi-http-rust",
                command_html: "",
            },
            QuickStartStep {
                title: "Run the server",
                body_html: "Run the server you just installed. \
                            <code class=\"px-1 py-0.5 rounded-sm bg-surfaceMuted text-ink-900 mono text-[0.875em]\">component run</code> fetches the component if needed and serves it on \
                            <code class=\"px-1 py-0.5 rounded-sm bg-surfaceMuted text-ink-900 mono text-[0.875em]\">localhost:8080</code>.",
                command: "component run ba:sample-wasi-http-rust",
                command_html: "",
            },
            QuickStartStep {
                title: "Send a request",
                body_html: "From another terminal, send it a request and watch it respond.",
                command: "curl localhost:8080",
                command_html: "",
            },
        ],
    });

    let offline_notice = notice_html.unwrap_or("");
    format!(
        r#"{offline_notice}
{hero_html}
<div class="pb-16 md:pb-24">
{quickstart_html}
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

    #[test]
    fn body_includes_alpha_notice() {
        let body = compose_body(&Stats::default(), None);
        assert!(
            body.contains(r#"role="note""#),
            "alpha call-out should render as a note landmark"
        );
        // Reuses the navbar's alpha badge styling.
        assert!(
            body.contains(">alpha</span>") && body.contains("text-orange-700"),
            "alpha call-out should reuse the header alpha badge"
        );
        // Border matches the header badge (orange) on a white surface.
        assert!(
            body.contains("border-orange-200 bg-surface "),
            "alpha call-out border should match the badge on a white background"
        );
        // Left-aligned and capped to the hero text width.
        assert!(
            body.contains("max-w-2xl"),
            "alpha call-out should be left-aligned at the hero text width"
        );
        // The notice sits inside the hero: below the lede and above the
        // search card in reading order.
        let lede_at = body.find("resolve names").expect("hero lede present");
        let notice_at = body.find(r#"role="note""#).expect("notice present");
        let search_at = body
            .find("Search the meta-registry.")
            .expect("search card present");
        assert!(
            lede_at < notice_at && notice_at < search_at,
            "alpha notice should sit below the lede and before the search box"
        );
    }

    #[test]
    fn quick_start_sits_between_search_and_publish() {
        let body = compose_body(&Stats::default(), None);
        assert!(
            body.contains("data-quickstart"),
            "quick start should render"
        );
        // Reading order: search card -> quick start -> publish strip.
        let search_at = body
            .find("Search the meta-registry.")
            .expect("search card present");
        let quickstart_at = body.find("data-quickstart").expect("quick start present");
        let publish_at = body
            .find("Publish your first component.")
            .expect("publish strip present");
        assert!(
            search_at < quickstart_at && quickstart_at < publish_at,
            "quick start should sit between the search section and the publish section"
        );
    }

    #[test]
    fn quick_start_folds_in_install_and_all_commands() {
        let body = compose_body(&Stats::default(), None);
        // Step one is the install widget: a platform dropdown with the real
        // install command for each supported OS.
        assert!(
            body.contains("data-install-cta"),
            "install widget should render as the first step"
        );
        for id in ["linux", "macos", "windows"] {
            assert!(
                body.contains(&format!(r#"data-ig-option="{id}""#)),
                "install step should offer a {id} option"
            );
        }
        assert!(body.contains("install.sh | sh") && body.contains("install.ps1 | iex"));
        // The remaining steps render their copyable commands.
        for cmd in [
            "component init",
            "component install ba:sample-wasi-http-rust",
            "component run ba:sample-wasi-http-rust",
            "curl localhost:8080",
        ] {
            assert!(
                body.contains(&format!(">{cmd}</code>")),
                "quick start should show the {cmd} command"
            );
        }
        // The install step precedes the shell steps within the walkthrough.
        let install_at = body.find("data-install-cta").expect("install present");
        let init_at = body.find(">component init</code>").expect("init present");
        assert!(
            install_at < init_at,
            "install should be the first quick-start step"
        );
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
