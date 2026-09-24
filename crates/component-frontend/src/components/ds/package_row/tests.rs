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
    };
    let mut component = interface.clone();
    component.repository = "example/atlassian".to_owned();
    component.wit_name = Some("atlassian".to_owned());
    component.kind = Some(PackageKind::Component);
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
    unlinked.tags = vec![format!("1.0.0-rc.1+{}", "abcdefgh".repeat(24))];
    unlinked.description = Some("UnbrokenDescription".repeat(20));
    let mut missing_metadata = interface.clone();
    missing_metadata.repository = "example/empty".to_owned();
    missing_metadata.wit_name = Some("empty".to_owned());
    missing_metadata.kind = None;
    missing_metadata.tags.clear();
    missing_metadata.description = None;
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
fn flowing_rows_snapshot() {
    let mut rows = Division::builder();
    rows.class("divide-y divide-lineSoft");
    for pkg in packages() {
        rows.push(render(&pkg));
    }
    insta::assert_snapshot!(crate::components::ds::pretty_html(
        &rows.build().to_string()
    ));
}

#[test]
fn preserves_full_versions_and_first_tag_selection() {
    for pkg in packages() {
        let html = render(&pkg).to_string();
        let version = pkg.tags.first().map_or("\u{2014}", String::as_str);
        assert!(html.contains(&format!(">{version}</span>")));
        assert!(!html.contains(">0.1.0</span>"));
    }
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
        assert!(html.contains("[overflow-wrap:anywhere]"));
    }
}
