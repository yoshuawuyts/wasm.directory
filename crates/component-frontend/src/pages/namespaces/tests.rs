use super::*;

fn page() -> RegistryPage<KnownNamespace> {
    RegistryPage {
        results: vec![
            KnownNamespace {
                name: "ba".into(),
                packages: 1,
            },
            KnownNamespace {
                name: "wasi".into(),
                packages: 12,
            },
        ],
        total: 5,
        offset: 2,
        limit: 2,
        has_next: true,
    }
}

#[test]
fn namespaces_share_all_packages_heading_and_pagination() {
    let html = render_namespaces(&page());
    assert!(html.contains("All Namespaces"));
    assert!(html.contains("showing 2 of 5 results"));
    assert!(html.contains("href=\"/ba\""));
    assert!(html.contains("href=\"/wasi\""));
    assert!(html.contains(">1 package</span>"));
    assert!(html.contains(">12 packages</span>"));
    assert!(html.contains("Showing 3\u{2013}4"));
    assert!(html.contains("href=\"/namespaces?offset=0&limit=2\""));
    assert!(html.contains("href=\"/namespaces?offset=4&limit=2\""));
    assert!(html.contains("motion-reduce:transition-none"));
    assert!(html.contains("[overflow-wrap:anywhere]"));
    assert!(!html.contains("<table"));
}

#[test]
fn empty_and_out_of_range_pages_keep_navigation() {
    let mut page = page();
    page.results.clear();
    page.has_next = false;
    let html = render_namespaces(&page);
    assert!(html.contains("showing 0 of 5 results"));
    assert!(html.contains("No namespaces on this page"));
    assert!(html.contains("href=\"/namespaces?offset=0&limit=2\""));
    assert!(!html.contains("href=\"/namespaces?offset=4&limit=2\""));
    page.total = 0;
    page.offset = 0;
    let html = render_namespaces(&page);
    assert!(html.contains("showing 0 of 0 results"));
    assert!(html.contains("No namespaces found. The registry may still be syncing."));
}

#[test]
fn last_full_page_does_not_invent_a_next_page() {
    let mut page = page();
    page.total = 4;
    page.has_next = false;
    let html = render_namespaces(&page);
    assert!(html.contains("href=\"/namespaces?offset=0&limit=2\""));
    assert!(!html.contains("href=\"/namespaces?offset=4&limit=2\""));
}

#[test]
fn namespace_names_are_escaped_and_encoded_as_single_path_segments() {
    let mut page = page();
    page.results[0].name = "<bad>/?\"&#".into();
    let html = render_namespaces(&page);
    assert!(html.contains("&lt;bad&gt;/?&quot;&amp;#"));
    assert!(html.contains("href=\"/%3Cbad%3E%2F%3F%22%26%23\""));
    assert!(!html.contains("<bad>"));
}

#[test]
fn landing_navigation_and_footer_link_to_namespaces() {
    let landing = layout::document_landing("Test", "");
    let nav = landing.split("</header>").next().expect("navigation");
    assert!(nav.contains("href=\"/namespaces\""));
    assert!(crate::footer::render().contains("href=\"/namespaces\""));
}
