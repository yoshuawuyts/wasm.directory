use std::collections::HashMap;

use super::fixtures::{WIT, package, version};

const SOURCE: &str = "?registry=mirror.test&amp;repository=mirrors%2Fhttp";

pub(super) fn package_destinations(html: &str) -> Vec<&str> {
    ["href=\"", "value=\""]
        .into_iter()
        .flat_map(|attribute| {
            html.split(attribute)
                .skip(1)
                .filter_map(|value| value.split('"').next())
        })
        .filter(|url| url.starts_with("/example/http/"))
        .collect()
}

pub(super) fn assert_source_destinations(html: &str, expected_paths: &[&str]) {
    let urls = package_destinations(html);
    assert!(!urls.is_empty(), "detail page must have package navigation");
    for url in &urls {
        assert!(
            url.ends_with(SOURCE),
            "source missing or misplaced in {url}"
        );
        assert_eq!(url.matches("?registry=").count(), 1, "{url}");
    }
    for path in expected_paths {
        assert!(
            urls.contains(&format!("{path}{SOURCE}").as_str()),
            "missing source-pinned destination {path}"
        );
    }
}

#[test]
fn package_navigation_preserves_the_selected_mirror() {
    let html = crate::pages::package::render(
        &package(),
        "0.1.0",
        Some(&version(Some(WIT))),
        crate::relationship_counts::RelationshipCounts::default(),
    );
    assert_source_destinations(
        &html,
        &[
            "/example/http/0.1.0",
            "/example/http/0.2.0",
            "/example/http/0.1.0/interface/types",
            "/example/http/0.1.0/interface/types/request",
            "/example/http/0.1.0/interface/types/send",
            "/example/http/0.1.0/world/proxy",
        ],
    );
    assert!(html.contains("href=\"/other/types\""));
    assert!(html.contains("href=\"/search/dependents?package=example%3Ahttp\""));
    assert!(html.contains("href=\"/search/imported-by?package=example%3Ahttp\""));
    assert!(html.contains("href=\"/search/exported-by?package=example%3Ahttp\""));
}

#[test]
fn world_navigation_preserves_the_selected_mirror() {
    let pkg = package();
    let version = version(Some(WIT));
    let doc = crate::wit_doc::parse_wit_doc(
        WIT,
        &crate::components::page_shell::url_base_for(&pkg, "0.1.0"),
        &HashMap::new(),
    )
    .expect("parse world navigation fixture");
    let html = crate::pages::world::render(
        &pkg,
        "0.1.0",
        Some(&version),
        &doc.worlds[0],
        &doc,
        crate::relationship_counts::RelationshipCounts::default(),
    );
    assert_source_destinations(
        &html,
        &[
            "/example/http/0.1.0/interface/types",
            "/example/http/0.1.0/world/proxy/function/run",
        ],
    );
}

#[test]
fn child_navigation_preserves_the_selected_mirror() {
    let html = crate::pages::package::render(
        &package(),
        "0.1.0",
        Some(&version(None)),
        crate::relationship_counts::RelationshipCounts::default(),
    );
    assert_source_destinations(
        &html,
        &[
            "/example/http/0.1.0",
            "/example/http/0.2.0",
            "/example/http/0.1.0/module/tool",
            "/example/http/0.1.0/component/0",
        ],
    );
}

#[test]
fn source_values_and_release_tags_are_encoded_for_their_url_context() {
    let mut pkg = package();
    pkg.registry = "mirror.test:5000".to_owned();
    pkg.repository = "mirrors/a b?x&y".to_owned();
    let url = crate::components::page_shell::url_base_for(&pkg, "1.0.0+build.1");
    assert_eq!(
        url,
        "/example/http/1.0.0%2Bbuild.1?registry=mirror.test%3A5000&repository=mirrors%2Fa+b%3Fx%26y"
    );
    let uri = url.parse().expect("valid encoded source URI");
    let axum::extract::Query(source) =
        axum::extract::Query::<crate::package_source::PackageSource>::try_from_uri(&uri)
            .expect("decode the selected source");
    assert_eq!(source.registry.as_deref(), Some("mirror.test:5000"));
    assert_eq!(source.repository.as_deref(), Some("mirrors/a b?x&y"));
}

#[test]
fn cross_package_wit_links_do_not_inherit_the_current_mirror() {
    let wit = r"
        package example:http@0.1.0;
        world proxy { import other:types/data@1.0.0; }
        package other:types@1.0.0 {
            interface data { type number = u32; }
        }
    ";
    let doc = crate::wit_doc::parse_wit_doc(
        wit,
        "/example/http/0.1.0?registry=mirror.test&repository=mirrors%2Fhttp",
        &HashMap::from([("other:types".to_owned(), "/other/types/1.0.0".to_owned())]),
    )
    .expect("parse cross-package interface fixture");
    let crate::wit_doc::WorldItemDoc::Interface { url, .. } = &doc.worlds[0].imports[0] else {
        panic!("expected an external interface reference");
    };
    assert_eq!(url.as_deref(), Some("/other/types/1.0.0/interface/data"));
}

#[test]
fn wit_navigation_appends_paths_before_source_parameters() {
    let doc = crate::wit_doc::parse_wit_doc(
        WIT,
        "/example/http/0.1.0?registry=mirror.test&repository=mirrors%2Fhttp",
        &HashMap::new(),
    )
    .expect("parse source-pinned WIT");
    assert_eq!(
        doc.interfaces[0].url,
        "/example/http/0.1.0/interface/types?registry=mirror.test&repository=mirrors%2Fhttp"
    );
    assert_eq!(
        doc.interfaces[0].types[0].url,
        "/example/http/0.1.0/interface/types/request?registry=mirror.test&repository=mirrors%2Fhttp"
    );
    assert_eq!(
        doc.worlds[0].url,
        "/example/http/0.1.0/world/proxy?registry=mirror.test&repository=mirrors%2Fhttp"
    );
}
