use std::collections::HashMap;

use wasm_meta_registry_client::{
    KnownPackage, PackageKind, PackageVersion, WitInterfaceRef, WitWorldSummary,
};

use super::{interface, item, package, world};
use crate::components::ds::wit_item;
use crate::wit_doc::{Stability, TypeKind, WitDocument, WorldItemDoc};

const DOCS: &str = concat!(
    "Use `body` with **care** and [the spec](https://example.com/spec?a=1&b=2).\n",
    "A soft-wrapped line.\n\n",
    "## Usage\n\n",
    "Second paragraph.\\\n",
    "A hard break.\n\n",
    "- First item.\n",
    "- Second item.\n\n",
    "1. First step.\n",
    "2. Second step.\n\n",
    "```wit\n",
    "list<string>\n",
    "list<u8>\n",
    "send: func(headers: list<tuple<string, string>>, body: list<u8>) -> result<list<u8>, string>;\n",
    "```\n\n",
    "[Unsafe link](javascript:alert(1))\n\n",
    "<script>alert('wit-doc')</script>\n",
    "<img src=x onerror=alert('wit-doc')>\n",
);

const WIT_TEMPLATE: &str = r"
package test:markdown@1.0.0;

DOCS
interface transport {
    DOCS
    type body = list<u8>;

    DOCS
    record request {
        DOCS
        content: body,
    }

    DOCS
    variant response {
        DOCS
        complete(body),
    }

    DOCS
    enum status {
        DOCS
        ready,
    }

    DOCS
    flags request-flags {
        DOCS
        urgent,
    }

    DOCS
    resource client {
        DOCS
        constructor();
        DOCS
        send: func();
        DOCS
        is-available: static func() -> bool;
    }

    DOCS
    send: func(value: request) -> response;
}

DOCS
world proxy {
    import transport;
    export transport;
    DOCS
    import check: func();
    DOCS
    export run: func();
}
";

struct Fixture {
    pkg: KnownPackage,
    version: PackageVersion,
    doc: WitDocument,
}

impl Fixture {
    fn new() -> Self {
        let comments = format!("/// {}", DOCS.replace('\n', "\n/// "));
        let wit = WIT_TEMPLATE.replace("DOCS", &comments);
        let doc = crate::wit_doc::parse_wit_doc(&wit, "/test/markdown/1.0.0", &HashMap::new())
            .expect("Markdown documentation fixture should be valid WIT");
        let pkg = KnownPackage {
            registry: "ghcr.io".to_owned(),
            repository: "test/markdown".to_owned(),
            kind: Some(PackageKind::Interface),
            description: None,
            tags: vec!["1.0.0".to_owned()],
            signature_tags: vec![],
            attestation_tags: vec![],
            last_seen_at: "2026-01-01T00:00:00Z".to_owned(),
            created_at: "2026-01-01T00:00:00Z".to_owned(),
            wit_namespace: Some("test".to_owned()),
            wit_name: Some("markdown".to_owned()),
            dependencies: vec![],
        };
        let version = PackageVersion {
            tag: Some("1.0.0".to_owned()),
            digest: "sha256:abc123".to_owned(),
            size_bytes: None,
            created_at: None,
            synced_at: None,
            annotations: None,
            worlds: vec![],
            components: vec![],
            dependencies: vec![],
            referrers: vec![],
            layers: vec![],
            wit_text: Some(wit),
            type_docs: HashMap::new(),
        };
        Self { pkg, version, doc }
    }
}

fn assert_full_docs(html: &str, count: usize) {
    for fragment in [
        "A soft-wrapped line.</p>",
        "<h2>Usage</h2>",
        "<p>Second paragraph.<br />\nA hard break.</p>",
        "<ul>\n<li>First item.</li>\n<li>Second item.</li>\n</ul>",
        "<ol>\n<li>First step.</li>\n<li>Second step.</li>\n</ol>",
        concat!(
            "<pre><code class=\"language-wit\">list&lt;string&gt;\nlist&lt;u8&gt;\n",
            "send: func(headers: list&lt;tuple&lt;string, string&gt;&gt;, body: list&lt;u8&gt;) ",
            "-&gt; result&lt;list&lt;u8&gt;, string&gt;;\n</code></pre>",
        ),
        "href=\"https://example.com/spec?a=1&amp;b=2\"",
        "&lt;script&gt;",
        "&lt;img src=x",
    ] {
        assert_eq!(html.matches(fragment).count(), count, "{fragment}");
    }
    assert!(html.contains("<code>body</code>"));
    assert!(html.contains("<strong>care</strong>"));
    assert!(!html.contains("&lt;code&gt;body"));
    assert!(!html.contains("<script>alert('wit-doc')"));
    assert!(!html.contains("<img src=x"));
    assert!(!html.contains("href=\"javascript:"));
}

fn assert_summary_rows(html: &str) {
    let rows: Vec<_> = html
        .split("class=\"item-row\"")
        .skip(1)
        .map(|tail| tail.split_once("</a>").expect("row should be a link").0)
        .collect();
    assert!(!rows.is_empty(), "expected at least one WIT summary row");
    for row in rows {
        assert!(row.contains("<code>body</code>"));
        assert!(row.contains("<strong>care</strong>"));
        assert!(row.contains("the spec"));
        assert!(!row.contains("<a "), "summary must not nest links: {row}");
        assert!(!row.contains("<p>"));
        assert!(!row.contains("<pre>"));
        assert!(!row.contains("Second paragraph."));
        assert!(!row.contains("&lt;code&gt;"));
    }
}

#[test]
fn interface_and_world_docs_are_full_blocks_but_rows_are_summaries() {
    let fixture = Fixture::new();
    let iface = fixture.doc.interfaces.first().expect("fixture interface");
    let world = fixture.doc.worlds.first().expect("fixture world");
    for html in [
        interface::render(&fixture.pkg, "1.0.0", None, iface, &fixture.doc),
        world::render(&fixture.pkg, "1.0.0", None, world, &fixture.doc),
    ] {
        assert_full_docs(&html, 1);
        assert_summary_rows(&html);
    }
}

#[test]
fn type_and_member_docs_are_not_truncated() {
    let fixture = Fixture::new();
    let iface = fixture.doc.interfaces.first().expect("fixture interface");
    for ty in &iface.types {
        let count = match &ty.kind {
            TypeKind::Alias(_) => 1,
            TypeKind::Resource { .. } => 4,
            _ => 2,
        };
        let html = item::render_type(&fixture.pkg, "1.0.0", None, &iface.name, ty, &fixture.doc);
        assert_full_docs(&html, count);
        if matches!(ty.kind, TypeKind::Resource { .. }) {
            assert_eq!(
                html.matches("class=\"id-page-tagline mt-3 prose-doc\"")
                    .count(),
                3,
            );
        }
        if matches!(
            ty.kind,
            TypeKind::Record { .. }
                | TypeKind::Variant { .. }
                | TypeKind::Enum { .. }
                | TypeKind::Flags { .. }
        ) {
            assert!(
                html.contains(
                    r#"<td class="py-2 align-top text-ink-700"><div class="prose-doc"><p>"#
                )
            );
        }
    }
}

#[test]
fn interface_and_world_function_docs_are_not_truncated() {
    let fixture = Fixture::new();
    let iface = fixture.doc.interfaces.first().expect("fixture interface");
    let world = fixture.doc.worlds.first().expect("fixture world");
    let world_functions =
        world
            .imports
            .iter()
            .chain(&world.exports)
            .filter_map(|item| match item {
                WorldItemDoc::Function(func) => Some((func, &world.name, &world.url)),
                _ => None,
            });
    for (func, owner, url) in iface
        .functions
        .iter()
        .map(|func| (func, &iface.name, &iface.url))
        .chain(world_functions)
    {
        let html =
            item::render_function(&fixture.pkg, "1.0.0", None, owner, url, func, &fixture.doc);
        assert_full_docs(&html, 1);
    }
}

#[test]
fn package_overviews_render_markdown_once_and_keep_first_line_excerpts() {
    let fixture = Fixture::new();
    let html = package::render(&fixture.pkg, "1.0.0", Some(&fixture.version));
    assert_summary_rows(&html);
    assert!(!html.contains("A soft-wrapped line."));
    assert!(!html.contains("Second paragraph."));
    assert!(html.contains("No description available."));
}

#[test]
fn api_enriched_and_fallback_summaries_render_markdown_once() {
    let mut fixture = Fixture::new();
    let iface = WitInterfaceRef {
        package: "test:markdown".to_owned(),
        interface: Some("transport".to_owned()),
        version: Some("1.0.0".to_owned()),
        docs: Some(DOCS.to_owned()),
        is_native: false,
    };
    fixture.version.wit_text = None;
    fixture.version.worlds = vec![WitWorldSummary {
        name: "proxy".to_owned(),
        description: Some(DOCS.to_owned()),
        imports: vec![iface.clone()],
        exports: vec![iface.clone()],
    }];
    let html = package::render(&fixture.pkg, "1.0.0", Some(&fixture.version));
    assert_summary_rows(&html);
    assert!(!html.contains("Second paragraph."));
    assert!(html.contains("href=\"https://example.com/spec?a=1&amp;b=2\""));

    let item = wit_item::iface_ref_to_item(&iface);
    assert_summary_rows(&wit_item::render_item_section("Imports", &[item]).to_string());

    let api_docs = world::build_api_doc_lookup(Some(&fixture.version), "proxy");
    let items = [WorldItemDoc::Interface {
        name: "test:markdown/transport@1.0.0".to_owned(),
        url: Some("/test/markdown/1.0.0/interface/transport".to_owned()),
        docs: None,
        stability: Stability::Unknown,
    }];
    let html =
        world::render_item_section("Imports", &items, &api_docs, "test:markdown").to_string();
    assert_summary_rows(&html);
    assert!(html.contains("href=\"/test/markdown/1.0.0/interface/transport\""));
}

#[test]
fn detail_pages_keep_the_missing_documentation_fallback() {
    let fixture = Fixture::new();
    let mut iface = fixture
        .doc
        .interfaces
        .first()
        .expect("fixture interface")
        .clone();
    let mut world = fixture.doc.worlds.first().expect("fixture world").clone();
    let mut ty = iface.types.first().expect("fixture alias").clone();
    let mut func = iface.functions.first().expect("fixture function").clone();
    iface.docs = None;
    world.docs = None;
    ty.docs = None;
    func.docs = None;
    for html in [
        interface::render(&fixture.pkg, "1.0.0", None, &iface, &fixture.doc),
        world::render(&fixture.pkg, "1.0.0", None, &world, &fixture.doc),
        item::render_type(&fixture.pkg, "1.0.0", None, &iface.name, &ty, &fixture.doc),
        item::render_function(
            &fixture.pkg,
            "1.0.0",
            None,
            &iface.name,
            &iface.url,
            &func,
            &fixture.doc,
        ),
    ] {
        assert!(html.contains("<p>No description available.</p>"));
    }
}
