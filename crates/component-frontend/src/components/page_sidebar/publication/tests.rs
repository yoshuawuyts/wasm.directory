use chrono::{DateTime, Utc};
use wasm_meta_registry_client::{ComponentSummary, KnownPackage, OciAnnotations, PackageVersion};

use super::super::{SidebarActive, SidebarContext, render_sidebar_at};
use crate::pages;

const CREATED: &str = "2026-09-16T10:42:09.820719295Z";
const PREVIOUS: &str = "2026-09-02T15:11:52.178946721Z";
const SOURCE: &str =
    "?registry=ghcr.io&amp;repository=andreiltd%2Fcomponentize-qjs%2Fcomponentize-qjs-runtime";

fn now() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-09-24T12:00:00Z")
        .expect("fixed render clock")
        .with_timezone(&Utc)
}

fn context<'a>(version: &'a str, created_at: Option<&'a str>) -> SidebarContext<'a> {
    SidebarContext {
        display_name: "andreiltd:componentize-qjs-runtime",
        version,
        versions: &[],
        doc: None,
        components: &[],
        url_base: "/andreiltd/componentize-qjs-runtime/0.4.5",
        active: SidebarActive::None,
        annotations: None,
        created_at,
        kind_label: "Component",
        description: Some("QuickJS runtime for componentize-qjs"),
        registry: "ghcr.io",
        repository: "andreiltd/componentize-qjs/componentize-qjs-runtime",
        digest: Some("sha256:f9b8a1f133b7f9eb888a8e5d30073d8e0f820107e77850bcae403a25fc6942f1"),
        dependencies: &[],
    }
}

#[test]
fn published_row_uses_selected_date_and_exact_utc_tooltip() {
    let tags = vec!["0.4.5".to_owned(), "0.4.4".to_owned()];
    for (tag, date, datetime, age) in [
        ("0.4.5", CREATED, "2026-09-16T10:42:09Z", "8 days ago"),
        ("0.4.4", PREVIOUS, "2026-09-02T15:11:52Z", "3 weeks ago"),
    ] {
        let mut ctx = context(tag, Some(date));
        ctx.versions = &tags;
        let html = render_sidebar_at(&ctx, now()).to_string();
        assert!(html.contains(&format!(r#"datetime="{datetime}""#)));
        assert!(html.contains(&format!(">{age}</time>")));
        assert!(html.contains("UTC (publisher-provided OCI creation time)"));
        assert!(html.contains(&format!(r#"aria-label="Published {age};"#)));
        assert_eq!(html.matches("data-publication-date").count(), 1);
        assert!(html.contains(&format!(r#"/{tag}{SOURCE}" selected"#)));
        let date_position = html.find("data-publication-date").expect("Published row");
        assert!(html.find("</select>").expect("version selector") < date_position);
        assert!(html.find("Image Digest").expect("digest label") < date_position);
        assert!(html.find(">Project<").expect("Project section") < date_position);
        assert!(date_position < html.find(">Items<").expect("Items section"));
        assert!(
            !html.contains(">Published<"),
            "no separate Published heading"
        );
        assert!(html.contains(r#"class="tree-link" data-publication-date"#));
        assert!(html.contains(super::super::SVG_CLOCK));
        assert!(super::super::SVG_CLOCK.contains(r#"stroke-width="1.75""#));
        assert!(
            super::super::SVG_CLOCK
                .contains(include_str!("../../../../../../vendor/lucide/clock.svg"))
        );
    }
}

#[test]
fn missing_invalid_and_non_ascii_dates_are_omitted_not_replaced() {
    let annotations = OciAnnotations {
        created: Some(CREATED.to_owned()),
        source: Some("https://github.com/andreiltd/componentize-qjs".to_owned()),
        licenses: Some("Apache-2.0".to_owned()),
        ..Default::default()
    };
    for date in [
        None,
        Some(""),
        Some("garbage"),
        Some("日本語"),
        Some("<script>"),
    ] {
        let mut ctx = context("0.4.5", date);
        ctx.annotations = Some(&annotations);
        let html = render_sidebar_at(&ctx, now()).to_string();
        assert!(!html.contains("data-publication-date"));
        assert!(
            !html.contains("<time"),
            "do not fall back to raw annotations"
        );
        assert!(!html.contains("Publication date unavailable"));
        assert!(html.contains("Image Digest"));
        assert!(html.contains("Repository"));
        assert!(html.contains("Apache-2.0"));
    }
}

#[test]
fn annotation_and_config_date_share_one_project_row() {
    let annotations = OciAnnotations {
        created: Some(CREATED.to_owned()),
        revision: Some("abcdef1234567890".to_owned()),
        ..Default::default()
    };
    let mut ctx = context("0.4.5", Some(CREATED));
    ctx.annotations = Some(&annotations);
    let html = render_sidebar_at(&ctx, now()).to_string();
    assert_eq!(html.matches("<time").count(), 1);
    assert!(!html.contains("Sep 16, 2026"));
    assert!(html.contains("abcdef123456"));
    assert!(html.contains("ghcr.io/andreiltd/componentize-qjs/componentize-qjs-runtime"));
}

#[test]
fn date_without_other_version_metadata_stays_in_project() {
    let mut ctx = context("0.4.5", Some(CREATED));
    ctx.digest = None;
    let html = render_sidebar_at(&ctx, now()).to_string();
    assert!(html.contains("data-publication-date"));
    assert!(html.contains("8 days ago"));
    assert!(!html.contains("pb-5 border-b-[1.5px] border-rule space-y-3"));
    assert!(
        html.find(">Project<").expect("Project section")
            < html.find("data-publication-date").expect("publication row")
    );
}

#[test]
fn offsets_and_whitespace_normalize_to_utc() {
    let ctx = context("0.4.5", Some(" \t2026-09-16T12:42:09.820719295+02:00\n"));
    let html = render_sidebar_at(&ctx, now()).to_string();
    assert!(html.contains(r#"datetime="2026-09-16T10:42:09Z""#));
    assert!(html.contains("Published 2026-09-16 10:42:09 UTC"));
    assert!(html.contains(">8 days ago</time>"));
}

fn package() -> KnownPackage {
    serde_json::from_value(serde_json::json!({
        "registry": "ghcr.io",
        "repository": "andreiltd/componentize-qjs/componentize-qjs-runtime",
        "description": "QuickJS runtime for componentize-qjs",
        "tags": ["0.4.5", "0.4.4"],
        "last_seen_at": "2026-09-24T20:00:00Z",
        "created_at": "2026-09-24T18:51:34Z",
        "wit_namespace": "andreiltd",
        "wit_name": "componentize-qjs-runtime"
    }))
    .expect("valid package fixture")
}

fn version(tag: &str, created: Option<&str>) -> PackageVersion {
    serde_json::from_value(serde_json::json!({
        "tag": tag,
        "digest": "sha256:b9c9d698e5f5f9e712e88e2304bff1f6e18b6e1d09ecb1f3beb21f1279d7b7f0",
        "created_at": created,
        "synced_at": "2026-09-24T18:51:32.644261Z"
    }))
    .expect("valid version fixture")
}

fn sidebar(html: &str) -> &str {
    let marker = html.find(r#"id="package-sidebar""#).expect("sidebar id");
    let start = html[..marker].rfind("<aside").expect("sidebar element");
    let end = html[start..].find("</aside>").expect("sidebar end");
    &html[start..start + end]
}

#[test]
fn package_kinds_use_selected_version_not_latest_or_indexed_dates() {
    use wasm_meta_registry_client::PackageKind;

    let mut pkg = package();
    let detail = version("0.4.4", Some(PREVIOUS));
    for kind in [
        None,
        Some(PackageKind::Component),
        Some(PackageKind::Interface),
    ] {
        pkg.kind = kind;
        let html = pages::package::render(&pkg, "0.4.4", Some(&detail));
        let nav = sidebar(&html);
        assert!(nav.contains(r#"datetime="2026-09-02T15:11:52Z""#));
        assert!(!nav.contains("2026-09-16"));
        assert!(!nav.contains("2026-09-24"));
        assert!(nav.contains(&format!(r#"/0.4.4{SOURCE}" selected"#)));
    }
}

#[test]
fn child_components_and_modules_inherit_the_containing_version_date() {
    let pkg = package();
    let detail = version("0.4.4", Some(PREVIOUS));
    for kind in ["module", "component"] {
        let child: ComponentSummary =
            serde_json::from_value(serde_json::json!({"name": "child", "kind": kind}))
                .expect("child fixture");
        let html = pages::child_component::render(&pkg, "0.4.4", Some(&detail), &child, "child");
        assert!(sidebar(&html).contains(r#"datetime="2026-09-02T15:11:52Z""#));
    }
}

#[test]
fn missing_version_or_publication_metadata_never_uses_package_created_at() {
    let pkg = package();
    let detail = version("0.4.4", None);
    for detail in [None, Some(&detail)] {
        let html = pages::package::render(&pkg, "0.4.4", detail);
        assert!(!sidebar(&html).contains("data-publication-date"));
    }
}
