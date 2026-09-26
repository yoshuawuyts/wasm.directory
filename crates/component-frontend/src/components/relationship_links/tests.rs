use super::*;
use crate::relationship_counts::RelationshipCounts;
use crate::wit_doc::WitDocument;

fn document() -> WitDocument {
    crate::wit_doc::parse_wit_doc(
        "package wasi:io; interface streams {} world proxy {}",
        "/wasi/io/0.2.0",
        &std::collections::HashMap::<String, String>::new(),
    )
    .expect("parse navigation fixture")
}

fn context<'a>(
    doc: &'a WitDocument,
    active: SidebarActive<'a>,
    kind_label: &'a str,
) -> SidebarContext<'a> {
    SidebarContext {
        display_name: "wasi:io",
        version: "0.2.0",
        versions: &[],
        doc: Some(doc),
        components: &[],
        url_base: "/wasi/io/0.2.0",
        active,
        annotations: None,
        kind_label,
        description: None,
        registry: "ghcr.io",
        repository: "wasi/io",
        digest: None,
        dependencies: &[],
        relationship_counts: RelationshipCounts::default(),
    }
}

fn link_destinations(html: &str) -> Vec<&str> {
    html.split("href=\"")
        .skip(1)
        .map(|rest| rest.split('"').next().expect("link destination"))
        .collect()
}

#[test]
fn component_navigation_always_includes_dependents_without_count_fetches() {
    let doc = document();
    let mut ctx = context(&doc, SidebarActive::None, "Component");
    ctx.doc = None;
    let html = crate::components::page_sidebar::render_sidebar(&ctx).to_string();
    assert!(html.contains(&render(&ctx)));
    assert_eq!(
        link_destinations(&render(&ctx)),
        ["/search/dependents?package=wasi%3Aio"]
    );
    assert!(!html.contains("imported-by"));
    assert!(!html.contains("exported-by"));
    assert_eq!(html.matches("/search/dependents?").count(), 1);
}

#[test]
fn interface_package_and_member_links_keep_their_scope() {
    let doc = document();
    for active in [SidebarActive::None, SidebarActive::World("proxy")] {
        let root = render(&context(&doc, active, "Interface Types"));
        assert_eq!(
            link_destinations(&root),
            [
                "/search/dependents?package=wasi%3Aio",
                "/search/imported-by?package=wasi%3Aio",
                "/search/exported-by?package=wasi%3Aio",
            ]
        );
    }
    for active in [
        SidebarActive::Interface("streams"),
        SidebarActive::Item("streams", "read"),
    ] {
        let html = render(&context(&doc, active, "Interface Types"));
        assert_eq!(
            link_destinations(&html),
            [
                "/search/dependents?package=wasi%3Aio",
                "/search/imported-by?package=wasi%3Aio&amp;interface=streams",
                "/search/exported-by?package=wasi%3Aio&amp;interface=streams",
            ]
        );
    }
}

#[test]
fn component_interfaces_keep_their_import_and_export_links() {
    let doc = document();
    let ctx = context(&doc, SidebarActive::Interface("streams"), "Component");
    assert_eq!(
        link_destinations(&render(&ctx)),
        [
            "/search/dependents?package=wasi%3Aio",
            "/search/imported-by?package=wasi%3Aio&amp;interface=streams",
            "/search/exported-by?package=wasi%3Aio&amp;interface=streams",
        ]
    );
}

#[test]
fn relationships_have_an_unlabeled_group_with_shared_trailing_arrows() {
    let doc = document();
    let ctx = context(&doc, SidebarActive::Interface("streams"), "Interface Types");
    let section = render(&ctx);
    let sidebar = crate::components::page_sidebar::render_sidebar(&ctx).to_string();
    assert!(sidebar.contains(r#"id="package-sidebar""#));
    assert!(sidebar.contains(r#"aria-label="Package navigation""#));
    assert!(sidebar.contains(&section));
    assert!(section.contains(r#"aria-label="Relationships""#));
    assert!(!section.contains("<h2"));
    assert!(!section.contains(">Relationships<"));
    assert!(section.contains("pt-4 border-t-[1.5px] border-rule"));
    assert_eq!(section.matches(r#"class="tree-link""#).count(), 3);
    assert_eq!(
        section
            .matches(r#"class="inline-flex items-center gap-1""#)
            .count(),
        3
    );
    assert_eq!(section.matches(icons::ARROW_UP_RIGHT).count(), 3);
    assert_eq!(section.matches(r#"aria-hidden="true""#).count(), 3);
    assert_eq!(
        section
            .matches(r#"class="shrink-0 inline-flex items-center h-[18px]""#)
            .count(),
        3
    );
    assert!(!section.contains("ml-auto"));
    assert!(!section.contains("sigil"));
    assert!(!section.contains("<img"));
    assert!(!section.contains(r#"class="hidden"#));
    for label in ["Dependents", "Imported by", "Exported by"] {
        assert!(section.contains(&format!(">{label}<span")));
    }
    let ordinary_navigation = sidebar
        .split_once(&section)
        .expect("separate relationships section")
        .0;
    assert!(!ordinary_navigation.contains("/search/"));
}

#[test]
fn destination_arrows_preserve_same_tab_links_not_disclosure_controls() {
    let doc = document();
    let ctx = context(&doc, SidebarActive::Interface("streams"), "Interface Types");
    let section = render(&ctx);
    assert!(!section.contains(" target="));
    assert!(!section.contains(" onclick="));
    assert!(!section.contains(r#"role="button""#));
    assert!(!section.contains("aria-expanded"));
    assert!(!section.contains("<details"));
    assert!(!section.contains("<summary"));
    assert!(!section.contains("chev"));
    assert_eq!(
        link_destinations(&section),
        [
            "/search/dependents?package=wasi%3Aio",
            "/search/imported-by?package=wasi%3Aio&amp;interface=streams",
            "/search/exported-by?package=wasi%3Aio&amp;interface=streams",
        ]
    );
}

#[test]
fn detail_pages_render_relationships_once_and_never_in_main_content() {
    use crate::pages::detail::{DetailSpec, render};
    use wasm_meta_registry_client::{KnownPackage, PackageKind};

    let doc = document();
    let pkg = KnownPackage {
        registry: "ghcr.io".to_owned(),
        repository: "wasi/io".to_owned(),
        kind: Some(PackageKind::Interface),
        description: None,
        tags: vec!["0.2.0".to_owned()],
        signature_tags: vec![],
        attestation_tags: vec![],
        last_seen_at: "2026-01-01T00:00:00Z".to_owned(),
        created_at: "2026-01-01T00:00:00Z".to_owned(),
        wit_namespace: Some("wasi".to_owned()),
        wit_name: Some("io".to_owned()),
        dependents: None,
        latest_release_at: None,
        dependencies: vec![],
    };
    let html = render(&DetailSpec {
        pkg: &pkg,
        version: "0.2.0",
        version_detail: None,
        wit_doc: Some(&doc),
        title: "Streams",
        header_html: "<h1>Streams</h1>",
        body_html: "<p>Stream reference.</p>",
        sidebar_active: SidebarActive::Interface("streams"),
        extra_crumbs: &[],
        toc_html: None,
        relationship_counts: RelationshipCounts::default(),
    });
    assert_eq!(html.matches(r#"aria-label="Relationships""#).count(), 1);
    for slug in ["dependents", "imported-by", "exported-by"] {
        assert_eq!(html.matches(&format!("/search/{slug}?")).count(), 1);
    }
    let main = html
        .split_once(r#"<main id="content""#)
        .expect("detail main")
        .1
        .split_once("</main>")
        .expect("main closing tag")
        .0;
    assert!(main.contains("<article><h1>Streams</h1><p>Stream reference.</p></article>"));
    assert!(!main.contains("/search/"));
    assert!(!main.contains(r#"aria-label="Relationships""#));
}

#[test]
fn invalid_package_identity_does_not_render_an_empty_section() {
    let doc = document();
    let mut ctx = context(&doc, SidebarActive::None, "Component");
    ctx.display_name = "unregistered/repository";
    assert!(render(&ctx).is_empty());
}

#[test]
fn loaded_totals_trail_their_links_and_unavailable_totals_are_omitted() {
    let doc = document();
    let mut ctx = context(&doc, SidebarActive::Interface("streams"), "Interface Types");
    ctx.relationship_counts = RelationshipCounts {
        dependents: Some(12),
        imported_by: Some(0),
        exported_by: None,
    };
    let section = render(&ctx);
    let meta = r#"<span class="ml-auto mono text-[10.5px] text-ink-400">"#;
    assert_eq!(section.matches(meta).count(), 2);
    assert!(section.contains(&format!("{meta}12</span></a>")));
    assert!(section.contains(&format!("{meta}0</span></a>")));
    let exported = section
        .split_once("/search/exported-by?")
        .expect("exported-by link")
        .1;
    assert!(!exported.contains(meta));
}
