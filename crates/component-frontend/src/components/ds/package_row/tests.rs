//! Shared fixtures and regression checks for package listings.

use super::*;

/// Packages covering each kind, long text, and absent metadata.
pub(crate) fn packages() -> Vec<KnownPackage> {
    let interface = KnownPackage {
        registry: "ghcr.io".to_owned(),
        repository: "example/http".to_owned(),
        kind: Some(PackageKind::Interface),
        description: Some("HTTP **interfaces**".to_owned()),
        tags: vec!["0.2.0".to_owned(), "0.1.0".to_owned()],
        signature_tags: vec![],
        attestation_tags: vec![],
        last_seen_at: "2026-01-01T00:00:00Z".to_owned(),
        created_at: "2026-01-01T00:00:00Z".to_owned(),
        wit_namespace: Some("example".to_owned()),
        wit_name: Some("http".to_owned()),
        dependencies: vec![],
        dependents: Some(12),
        latest_release_at: Some("2026-09-21T08:30:00+02:00".to_owned()),
    };
    let mut component = interface.clone();
    component.repository = "example/atlassian".to_owned();
    component.wit_name = Some("atlassian".to_owned());
    component.kind = Some(PackageKind::Component);
    component.dependents = Some(1);
    component.tags = vec!["0.5.0_atlassian-1001.0.0-SNAPSHOT".to_owned()];
    component.description = Some(
        "WebAssembly component bindings for the atlassian API, generated from an OpenAPI document."
            .to_owned(),
    );
    let mut unlinked = interface.clone();
    unlinked.repository = format!("example/{}", "long-repository-name-".repeat(8));
    unlinked.wit_namespace = None;
    unlinked.wit_name = None;
    unlinked.kind = None;
    unlinked.dependents = None;
    unlinked.tags = vec![format!("1.0.0-rc.1+{}", "abcdefgh".repeat(24))];
    unlinked.description = Some("UnbrokenDescription".repeat(20));
    let mut missing_metadata = interface.clone();
    missing_metadata.repository = "example/empty".to_owned();
    missing_metadata.wit_name = Some("empty".to_owned());
    missing_metadata.kind = None;
    missing_metadata.tags.clear();
    missing_metadata.description = None;
    missing_metadata.dependents = Some(0);
    missing_metadata.latest_release_at = None;
    vec![interface, component, unlinked, missing_metadata]
}

/// Assert that a page uses ordered shared rows without table headings.
pub(crate) fn assert_listing(html: &str, packages: &[KnownPackage]) {
    let mut remaining = html;
    for pkg in packages {
        let row = render(pkg).to_string();
        let (_, rest) = remaining
            .split_once(&row)
            .expect("listing should contain each shared package row in order");
        remaining = rest;
    }
    for heading in ["Name", "Kind", "Version", "Description"] {
        assert!(
            !html.contains(&format!(">{heading}</span>")),
            "listing should not include a {heading} column heading"
        );
    }
}

#[test]
fn two_row_listing_snapshot() {
    let mut rows = Division::builder();
    rows.class("divide-y divide-lineSoft");
    for pkg in packages() {
        rows.push(render_at(&pkg, now()));
    }
    insta::assert_snapshot!(crate::components::ds::pretty_html(
        &rows.build().to_string()
    ));
}

#[test]
fn preserves_full_versions_and_first_tag_selection() {
    for pkg in packages() {
        let html = render(&pkg).to_string();
        let version = pkg
            .tags
            .first()
            .map_or_else(|| "\u{2014}".to_owned(), |tag| format!("v{tag}"));
        assert!(html.contains(&format!(">{version}</span>")));
        assert!(!html.contains(">v0.1.0</span>"));
    }
}

#[test]
fn adds_one_display_prefix_without_changing_version_tags() {
    for (tag, display) in [
        ("1.2.3", "v1.2.3"),
        ("v1.2.3", "v1.2.3"),
        ("V1.2.3", "v1.2.3"),
        ("1.0.0-rc.1_build.123", "v1.0.0-rc.1_build.123"),
    ] {
        let mut pkg = packages().remove(0);
        pkg.tags = vec![tag.to_owned()];
        let html = render(&pkg).to_string();
        assert!(html.contains(&format!(">{display}</span>")));
        assert_eq!(pkg.tags, [tag]);
    }
    let pkg = packages().remove(3);
    let html = render(&pkg).to_string();
    assert!(html.contains(">\u{2014}</span>"));
    assert!(!html.contains(">v\u{2014}</span>"));
}

#[test]
fn only_wit_packages_are_linked() {
    for pkg in packages() {
        let html = render(&pkg).to_string();
        match (&pkg.wit_namespace, &pkg.wit_name) {
            (Some(namespace), Some(name)) => {
                assert!(html.contains(&format!("href=\"/{namespace}/{name}\"")));
            }
            _ => {
                assert!(!html.contains("<a "));
                assert!(html.contains(&pkg.repository));
            }
        }
    }
}

#[test]
fn linked_and_unlinked_rows_share_the_same_flow() {
    let mut pkg = packages().remove(0);
    let linked = render(&pkg).to_string();
    pkg.wit_namespace = None;
    let unlinked = render(&pkg).to_string();
    assert!(linked.contains(&format!("class=\"{ROW_CLASS} ")));
    assert!(unlinked.contains(&format!("class=\"{ROW_CLASS}\"")));
    for html in [&linked, &unlinked] {
        assert!(!html.contains("sm:w-"));
        assert!(!html.contains("sm:shrink-0"));
        assert!(!html.contains("flex-nowrap"));
        assert!(html.contains(PRIMARY_CLASS));
        assert!(html.contains(META_CLASS));
        assert!(html.contains("[overflow-wrap:anywhere]"));
        let (_, metadata) = html.split_once(META_CLASS).expect("metadata group");
        assert!(!metadata.contains("truncate"));
    }
}

fn now() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-09-24T12:00:00Z")
        .expect("valid fixture timestamp")
        .with_timezone(&Utc)
}

#[test]
fn separates_identity_and_description_from_ordered_metadata() {
    let pkg = packages().remove(0);
    let html = render_at(&pkg, now()).to_string();
    let (primary, metadata) = html
        .split_once(META_CLASS)
        .expect("row has a metadata group");
    assert!(primary.contains(">example/http</span>"));
    assert!(primary.contains("HTTP <strong>interfaces</strong>"));
    assert!(!primary.contains(">v0.2.0</span>"));
    assert!(!metadata.contains("HTTP <strong>"));
    let version = metadata.find(">v0.2.0</span>").expect("version");
    let kind = metadata.find("interface").expect("kind");
    let dependents = metadata.find(">12 dependents</span>").expect("count");
    let updated = metadata.find(">Updated 3 days ago</span>").expect("age");
    assert!(kind < version && version < dependents && dependents < updated);
    assert!(metadata.contains("datetime=\"2026-09-21T06:30:00Z\""));
    assert!(metadata.contains("title=\"Updated 2026-09-21\""));
    assert!(metadata.contains(">3 days</span>"));
    assert!(!metadata.contains("rounded-pill"));
    assert!(metadata.contains(&kind_label(pkg.kind).to_string()));
    assert!(metadata.contains(">interface</span>"));
    assert_eq!(identity(&pkg).0, "example:http");
}

#[test]
fn kinds_use_the_style_guides_compact_inline_labels() {
    for (kind, background, ink, text) in [
        (
            Some(PackageKind::Interface),
            "bg-cat-blue",
            "text-cat-blueInk",
            "interface",
        ),
        (
            Some(PackageKind::Component),
            "bg-cat-green",
            "text-cat-greenInk",
            "component",
        ),
        (None, "bg-cat-slate", "text-cat-slateInk", "package"),
    ] {
        let mut pkg = packages().remove(0);
        pkg.kind = kind;
        let html = render(&pkg).to_string();
        let label = labels::inline_label(background, ink, text).to_string();
        assert!(html.contains(&label));
        let metadata = metadata_row(&pkg, now()).to_string();
        assert!(metadata.starts_with(&format!("<span class=\"{META_CLASS}\">{label}")));
        assert!(!metadata.contains("rounded-pill"));
    }
}

#[test]
fn distinguishes_zero_one_many_and_unknown_dependents() {
    for (count, expected) in [
        (Some(0), "0 dependents"),
        (Some(1), "1 dependent"),
        (Some(12), "12 dependents"),
        (None, "0 dependents (count unavailable)"),
    ] {
        let mut pkg = packages().remove(0);
        pkg.dependents = count;
        let html = render_at(&pkg, now()).to_string();
        assert!(html.contains(&format!("class=\"sr-only\">{expected}</span>")));
        assert!(html.contains(&format!("title=\"{expected}. {DEPENDENTS_DESCRIPTION}\"")));
        let visible = count.unwrap_or(0).to_string();
        assert!(html.contains(&format!("aria-hidden=\"true\">{visible}</span>")));
    }
}

#[test]
fn unavailable_count_defaults_only_in_the_display() {
    let mut pkg = packages().remove(0);
    pkg.dependents = None;
    let html = render_at(&pkg, now()).to_string();
    assert!(html.contains(r#"aria-hidden="true">0</span>"#));
    assert!(html.contains("class=\"sr-only\">0 dependents (count unavailable)</span>"));
    assert_eq!(pkg.dependents, None);
}

#[test]
fn missing_or_invalid_release_dates_do_not_use_scan_time() {
    for timestamp in [None, Some("not a timestamp"), Some("")] {
        let mut pkg = packages().remove(0);
        pkg.latest_release_at = timestamp.map(str::to_owned);
        let html = render_at(&pkg, now()).to_string();
        assert!(!html.contains("Update time unavailable"));
        assert!(!html.contains(&decorative_svg(&icons::icon(14, CLOCK_ICON)).to_string()));
        assert!(!html.contains("<time"));
        assert!(!html.contains(&pkg.last_seen_at));
        assert!(updated_at(timestamp, now()).is_none());
    }
}

#[test]
fn uses_existing_decorative_icons_and_accessible_metric_labels() {
    let html = render_at(&packages().remove(0), now()).to_string();
    assert!(html.contains(&decorative_svg(&icons::package_dependents(14)).to_string()));
    assert!(html.contains(&decorative_svg(&icons::icon(14, CLOCK_ICON)).to_string()));
    assert!(html.contains("class=\"sr-only\">12 dependents</span>"));
    assert!(html.contains("class=\"sr-only\">Updated 3 days ago</span>"));
    let fork = icons::icon(
        14,
        include_str!("../../../../../../vendor/lucide/git-fork.svg"),
    );
    assert!(!html.contains(&fork));
    let layers = icons::icon(
        14,
        include_str!("../../../../../../vendor/lucide/layers.svg"),
    );
    assert!(!html.contains(&layers));
}

#[test]
fn dependent_tooltip_explains_direction_and_distinct_counting() {
    for count in [Some(0), Some(1), Some(12), None] {
        let html = dependents(count).to_string();
        assert!(html.starts_with(&format!(
            "<span class=\"{METRIC_CLASS} cursor-help\" title=\""
        )));
        assert!(html.contains(
            "Other indexed repositories that directly depend on this package, counted once each."
        ));
        assert!(html.contains(&decorative_svg(&icons::package_dependents(14)).to_string()));
        assert!(html.contains("class=\"sr-only\""));
    }
}

#[test]
fn concise_release_ages_preserve_full_accessible_meaning() {
    for (days, visible, accessible) in [
        (0, "today", "Updated today"),
        (1, "1 day", "Updated 1 day ago"),
        (3, "3 days", "Updated 3 days ago"),
        (14, "2 weeks", "Updated 2 weeks ago"),
        (60, "2 months", "Updated 2 months ago"),
        (365, "1 year", "Updated 1 year ago"),
    ] {
        let mut pkg = packages().remove(0);
        pkg.latest_release_at = Some((now() - chrono::TimeDelta::days(days)).to_rfc3339());
        let html = render_at(&pkg, now()).to_string();
        assert!(html.contains(&format!("aria-hidden=\"true\">{visible}</span>")));
        assert!(html.contains(&format!("class=\"sr-only\">{accessible}</span>")));
    }
}

#[test]
fn absent_descriptions_do_not_add_placeholders() {
    for description in [None, Some(""), Some("  \n ")] {
        let mut pkg = packages().remove(0);
        pkg.description = description.map(str::to_owned);
        let html = render_at(&pkg, now()).to_string();
        assert!(!html.contains("basis-64"));
        assert!(!html.contains("No description"));
        assert!(html.contains(META_CLASS));
    }
}

#[test]
fn escapes_identity_version_and_link_attributes() {
    let mut pkg = packages().remove(0);
    pkg.wit_name = Some("http\"><script>alert(1)</script>".to_owned());
    pkg.tags = vec!["<img src=x onerror=alert(1)>".to_owned()];
    let html = render_at(&pkg, now()).to_string();
    assert!(html.contains("http&quot;&gt;&lt;script&gt;"));
    assert!(html.contains("&lt;img src=x onerror=alert(1)&gt;"));
    assert!(!html.contains("<script>"));
    assert!(!html.contains("<img "));
}

#[test]
fn descriptions_cannot_nest_links_or_blocks_in_the_row_link() {
    let mut pkg = packages().remove(0);
    pkg.description =
        Some("# **HTTP** [interfaces](https://example.com) ![icon](icon.png)".to_owned());
    let html = render_at(&pkg, now()).to_string();
    assert_eq!(html.matches("<a ").count(), 1);
    assert!(!html.contains("<h1"));
    assert!(!html.contains("<img"));
    assert!(html.contains("<strong>HTTP</strong> interfaces icon"));
}

#[test]
fn description_truncation_preserves_complete_accessible_text() {
    let mut pkg = packages().remove(0);
    let description = "Long description with no omitted words. ".repeat(30);
    pkg.description = Some(description.clone());
    let html = render_at(&pkg, now()).to_string();
    let (primary, metadata) = html.split_once(META_CLASS).expect("metadata group");
    assert!(primary.contains("text-ink-500 truncate"));
    assert!(primary.contains(&format!("title=\"{}\"", description.trim())));
    assert!(primary.contains(&format!(">{}</span>", description.trim())));
    assert!(!metadata.contains("truncate"));
}
