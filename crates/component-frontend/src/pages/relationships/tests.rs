use super::*;

fn package() -> KnownPackage {
    package_row::tests::packages().remove(0)
}

fn page<T>(results: Vec<T>) -> RelationshipPage<T> {
    RelationshipPage {
        total: Some(u64::try_from(results.len()).expect("fixture length fits")),
        results,
        offset: 0,
        limit: 100,
        has_next: false,
    }
}

fn target() -> RelationshipTarget {
    RelationshipTarget::new("wasi:io", Some("streams")).expect("valid fixture target")
}

fn world() -> MatchingWorld {
    MatchingWorld {
        package: package(),
        name: "proxy".to_owned(),
        description: Some("A **streaming** proxy.".to_owned()),
        version: "0.1.0".to_owned(),
    }
}

#[test]
fn dependents_link_the_matching_release_not_the_latest() {
    let target = RelationshipTarget::new("wasi:io", None).expect("valid target");
    let results = page(vec![DependentPackage {
        package: package(),
        version: "0.1.0".to_owned(),
    }]);
    let html = render_page(
        Relationship::Dependents,
        &target,
        &results,
        render_dependent,
    );
    assert!(html.contains("Dependents of wasi:io"));
    assert!(html.contains("showing 1 of 1 result"));
    assert!(html.contains("Direct and transitive dependents"));
    assert!(
        html.contains(
            r#"href="/example/http/0.1.0?registry=ghcr.io&amp;repository=example%2Fhttp""#
        )
    );
    assert!(html.contains(">0.1.0</span>"));
    assert!(!html.contains(">0.2.0</span>"));
}

#[test]
fn world_results_have_distinct_identity_direction_and_matching_version() {
    for (relation, heading) in [
        (Relationship::ImportedBy, "Worlds importing wasi:io/streams"),
        (Relationship::ExportedBy, "Worlds exporting wasi:io/streams"),
    ] {
        let html = render_page(relation, &target(), &page(vec![world()]), render_world);
        assert!(html.contains(heading));
        assert!(html.contains(">example:http/proxy</span>"));
        assert!(html.contains(">World</span>"));
        assert!(html.contains("/example/http/0.1.0/world/proxy?registry=ghcr.io"));
        assert!(!html.contains("sm:w-"));
        assert!(!html.contains("sm:flex-nowrap"));
        assert!(!html.contains(">Version</span>"));
        assert!(html.contains("[overflow-wrap:anywhere]"));
    }
}

#[test]
fn synthetic_worlds_link_to_their_component_page() {
    let mut world = world();
    world.package.kind = Some(PackageKind::Component);
    world.name = "root".to_owned();
    let html = render_world(&world).to_string();
    assert!(html.contains("/example/http/0.1.0?registry=ghcr.io"));
    assert!(!html.contains("/world/root"));
    assert!(html.contains(">example:http/root</span>"));
}

#[test]
fn matching_rows_use_link_free_markdown_summaries() {
    let mut world = world();
    world.description = Some(
        "Use `streams` for **flow**; see [the spec](https://example.test/spec).\n\nMore details."
            .to_owned(),
    );
    let html = render_world(&world).to_string();
    assert_eq!(html.matches("<a ").count(), 1);
    assert!(html.contains("<code>streams</code>"));
    assert!(html.contains("<strong>flow</strong>"));
    assert!(html.contains("the spec"));
    assert!(!html.contains("https://example.test/spec"));
    assert!(!html.contains("More details."));
}

#[test]
fn pagination_preserves_scope_and_does_not_infer_next_from_total() {
    let mut results = page(vec![world()]);
    results.offset = 100;
    results.limit = 100;
    results.total = None;
    results.has_next = true;
    let html = render_page(Relationship::ImportedBy, &target(), &results, render_world);
    assert!(html.contains("showing 1 result (total unavailable)"));
    assert!(html.contains("Showing 101\u{2013}101"));
    assert!(html.contains(
        "/search/imported-by?package=wasi%3Aio&amp;interface=streams&amp;offset=0&amp;limit=100"
    ));
    assert!(html.contains(
        "/search/imported-by?package=wasi%3Aio&amp;interface=streams&amp;offset=200&amp;limit=100"
    ));
    results.total = Some(900);
    results.has_next = false;
    let html = render_page(Relationship::ImportedBy, &target(), &results, render_world);
    assert!(!html.contains("&amp;offset=200"));
}

#[test]
fn empty_and_out_of_range_results_remain_navigable() {
    let mut results: RelationshipPage<MatchingWorld> = page(vec![]);
    let html = render_page(Relationship::ExportedBy, &target(), &results, render_world);
    assert!(html.contains("showing 0 of 0 results"));
    assert!(html.contains("No worlds with matching exports found in the index."));
    results.offset = 200;
    results.total = Some(1);
    let html = render_page(Relationship::ExportedBy, &target(), &results, render_world);
    assert!(html.contains("No results on this page."));
    assert!(html.contains("Back to first page"));
    assert!(html.contains("&amp;offset=100&amp;limit=100"));
}

#[test]
fn row_metadata_and_errors_are_escaped() {
    let mut world = world();
    world.name = "proxy\"><script>unsafe</script>".to_owned();
    world.version = "1.0.0_abc&test".to_owned();
    let html = render_world(&world).to_string();
    assert!(!html.contains("<script>unsafe"));
    assert!(html.contains("proxy&quot;&gt;&lt;script&gt;"));
    assert!(html.contains("1.0.0_abc%26test/world"));
    assert!(html.contains(">1.0.0_abc&amp;test</span>"));
    let html = render_error(
        Relationship::ImportedBy,
        &target(),
        "<script>failed</script>",
        100,
        20,
    );
    assert!(html.contains("Unable to load relationship results"));
    assert!(html.contains("&lt;script&gt;failed&lt;/script&gt;"));
    assert!(html.contains("Try again"));
    assert!(html.contains("&amp;offset=100&amp;limit=20"));
    assert!(!html.contains("showing 0"));
}

#[test]
fn unaddressable_packages_remain_plain_rows() {
    let mut pkg = package();
    pkg.wit_namespace = None;
    let html = render_dependent(&DependentPackage {
        package: pkg,
        version: "0.1.0".to_owned(),
    })
    .to_string();
    assert!(!html.contains("<a "));
}

#[test]
fn long_identities_wrap_in_success_and_error_navigation() {
    let name = format!("example:{}", "p".repeat(160));
    let target = RelationshipTarget::new(&name, None).expect("valid long package identity");
    for html in [
        render_page(
            Relationship::Dependents,
            &target,
            &page(vec![]),
            render_dependent,
        ),
        render_error(
            Relationship::Dependents,
            &target,
            "Lookup unavailable",
            0,
            100,
        ),
    ] {
        let heading = html
            .split_once("<h1")
            .expect("page heading")
            .1
            .split_once("</h1>")
            .expect("closed page heading")
            .0;
        assert!(heading.contains("min-w-0 max-w-full [overflow-wrap:anywhere]"));
        assert!(heading.contains(&name));
        assert!(html.contains(
            r#"class="text-accent hover:underline [overflow-wrap:anywhere]">Back to example:"#
        ));
    }
}
