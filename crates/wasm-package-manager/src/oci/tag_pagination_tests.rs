use std::future::ready;
use std::net::TcpListener;

use oci_client::Reference;
use oci_client::secrets::RegistryAuth;

use super::{PAGE_SIZE, collect_tags, http_client_builder, tags_url};

#[path = "tag_test_registry.rs"]
mod registry;

use registry::{Response, run_registry, run_with_auth};

fn tags(values: &[&str]) -> Vec<String> {
    values.iter().map(|tag| (*tag).to_owned()).collect()
}

async fn collect_pages(pages: &[&[&str]]) -> anyhow::Result<Vec<String>> {
    let mut pages = pages.iter();
    let result = collect_tags(|_| {
        ready(Ok(tags(
            pages.next().expect("unexpected pagination request"),
        )))
    })
    .await;
    assert!(
        pages.next().is_none(),
        "not all expected pages were fetched"
    );
    result
}

#[tokio::test]
async fn collects_every_page_including_short_pages() {
    let mut pages = [
        (None, tags(&["a", "b"])),
        (Some("b"), tags(&["c"])),
        (Some("c"), tags(&["d", "e"])),
        (Some("e"), Vec::new()),
    ]
    .into_iter();
    let result = collect_tags(|last| {
        let (expected_last, page) = pages.next().expect("unexpected pagination request");
        assert_eq!(last.as_deref(), expected_last);
        ready(Ok(page))
    })
    .await
    .expect("complete tag listing");
    assert_eq!(result, tags(&["a", "b", "c", "d", "e"]));
    assert!(pages.next().is_none());
}

#[tokio::test]
async fn accepts_successful_empty_listing() {
    assert!(
        collect_pages(&[&[]])
            .await
            .expect("empty tag listing")
            .is_empty()
    );
}

#[tokio::test]
async fn propagates_first_later_and_final_probe_errors() {
    for successful_pages in 0..3 {
        let mut calls = 0;
        let error = collect_tags(|_| {
            calls += 1;
            ready(if calls > successful_pages {
                Err(anyhow::anyhow!("page failed"))
            } else {
                Ok(vec![format!("tag-{calls}")])
            })
        })
        .await
        .expect_err("any page error must invalidate the listing");
        assert_eq!(calls, successful_pages + 1);
        assert_eq!(error.root_cause().to_string(), "page failed");
    }
}

#[tokio::test]
async fn deduplicates_tags_within_and_across_advancing_pages() {
    let result = collect_pages(&[&["a", "a", "b"], &["b", "c", "c"], &[]])
        .await
        .expect("overlapping pages can still advance");
    assert_eq!(result, tags(&["a", "b", "c"]));
}

#[tokio::test]
async fn rejects_ignored_cursor_even_when_page_contains_new_tags() {
    for pages in [
        vec![&["a", "b"][..], &["a", "b"][..]],
        vec![&["a", "b"][..], &["c", "b"][..]],
    ] {
        let error = collect_pages(&pages)
            .await
            .expect_err("a repeated cursor must fail instead of looping");
        assert!(error.to_string().contains("repeated cursor"));
    }
}

#[tokio::test]
async fn rejects_cursor_cycles() {
    let error = collect_pages(&[&["a"], &["b"], &["c", "a"]])
        .await
        .expect_err("a previously used cursor must fail");
    assert!(error.to_string().contains("repeated cursor"));
}

#[tokio::test]
async fn rejects_non_progress_with_a_different_cursor() {
    let error = collect_pages(&[&["a", "b"], &["a"]])
        .await
        .expect_err("reordering already-seen tags is not progress");
    assert!(error.to_string().contains("no progress"));
}

#[tokio::test]
async fn adapter_requests_explicit_pages_and_preserves_authentication() {
    let (result, paths) = run_registry(vec![
        Some(("200 OK", r#"{"name":"test","tags":["a","b"]}"#)),
        Some(("200 OK", r#"{"name":"test","tags":["c"]}"#)),
        Some(("200 OK", r#"{"name":"test","tags":[]}"#)),
    ])
    .await;
    assert_eq!(
        result.expect("complete tag listing"),
        tags(&["a", "b", "c"])
    );
    assert_eq!(
        paths,
        [
            format!("/v2/test/tags/list?n={PAGE_SIZE}"),
            format!("/v2/test/tags/list?n={PAGE_SIZE}&last=b"),
            format!("/v2/test/tags/list?n={PAGE_SIZE}&last=c"),
        ]
    );
}

#[tokio::test]
async fn adapter_continues_after_a_full_page_until_null_exhaustion() {
    let mut expected: Vec<_> = (0..PAGE_SIZE)
        .map(|index| format!("tag-{index:03}"))
        .collect();
    let full_page = serde_json::json!({"name": "test", "tags": expected}).to_string();
    let (result, requests) = run_with_auth(RegistryAuth::Bearer("stub-token".to_owned()), |_| {
        vec![
            Some(Response::new("200 OK", &full_page)),
            Some(Response::new("200 OK", r#"{"name":"test","tags":["z"]}"#)),
            Some(Response::new("200 OK", r#"{"name":"test","tags":null}"#)),
        ]
    })
    .await;
    expected.push("z".to_owned());
    assert_eq!(result.expect("complete listing"), expected);
    assert_eq!(requests.len(), 3);
    assert!(
        requests
            .get(1)
            .expect("second page")
            .target
            .ends_with("last=tag-099")
    );
}

#[tokio::test]
async fn adapter_accepts_empty_and_null_on_first_and_final_pages() {
    for body in [
        r#"{"name":"test","tags":[]}"#,
        r#"{"name":"test","tags":null}"#,
    ] {
        for after_page in [false, true] {
            let (result, _) =
                run_registry(response_script(after_page, Some(("200 OK", body)))).await;
            let expected = if after_page { tags(&["a"]) } else { Vec::new() };
            assert_eq!(result.expect("successful exhaustion"), expected);
        }
    }
}

#[tokio::test]
async fn adapter_rejects_untrusted_or_malformed_responses_on_every_page() {
    for body in [
        r#"{"name":"test","tags":[1]}"#,
        r#"{"name":"test","tags":[null]}"#,
        r#"{"name":"test","tags":"a"}"#,
        r#"{"name":"test","tags":{}}"#,
        r#"{"name":"test","tags":false}"#,
        r#"{"name":"test"}"#,
        r#"{"tags":[]}"#,
        r#"{"tags":null}"#,
        r#"{"name":null,"tags":null}"#,
        r#"{"name":42,"tags":[]}"#,
        r#"{"name":"evil","tags":null}"#,
        r#"{"name":"evil","tags":[]}"#,
        r#"{"name":"evil","tags":["a"]}"#,
        r#"{"name":"test","tags":null"#,
        r#"{"name":"test","tags":null} trailing"#,
        r#"{"name":"test","tags":null,"tags":["a"]}"#,
        "not JSON",
    ] {
        for after_page in [false, true] {
            let (result, _) =
                run_registry(response_script(after_page, Some(("200 OK", body)))).await;
            assert!(result.is_err(), "invalid body was accepted: {body}");
        }
    }
}

#[tokio::test]
async fn adapter_propagates_http_and_transport_errors_on_every_page() {
    for response in [
        Some(("201 Created", r#"{"name":"test","tags":null}"#)),
        Some(("204 No Content", "")),
        Some(("401 Unauthorized", r#"{"errors":[]}"#)),
        Some(("403 Forbidden", r#"{"errors":[]}"#)),
        Some(("404 Not Found", r#"{"name":"test","tags":null}"#)),
        Some(("429 Too Many Requests", r#"{"errors":[]}"#)),
        Some(("500 Internal Server Error", r#"{"errors":[]}"#)),
        None,
    ] {
        for after_page in [false, true] {
            let (result, _) = run_registry(response_script(after_page, response)).await;
            assert!(result.is_err(), "failed request was treated as complete");
        }
    }
}

#[tokio::test]
async fn adapter_rejects_redirects_without_contacting_the_target() {
    let target = TcpListener::bind("127.0.0.1:0").expect("bind redirect target");
    target.set_nonblocking(true).expect("nonblocking target");
    let location = format!(
        "http://{}/stolen-credentials",
        target.local_addr().expect("redirect address")
    );
    for status in [
        "301 Moved Permanently",
        "302 Found",
        "303 See Other",
        "307 Temporary Redirect",
        "308 Permanent Redirect",
    ] {
        for after_page in [false, true] {
            let (result, _) = run_with_auth(RegistryAuth::Bearer("stub-token".to_owned()), |_| {
                let mut responses = Vec::new();
                if after_page {
                    responses.push(Some(Response::new(
                        "200 OK",
                        r#"{"name":"test","tags":["a"]}"#,
                    )));
                }
                responses.push(Some(
                    Response::new(status, r#"{"name":"test","tags":null}"#)
                        .header("Location", location.clone()),
                ));
                responses
            })
            .await;
            assert!(result.is_err(), "redirect must not complete a listing");
            assert_eq!(
                target
                    .accept()
                    .expect_err("target must receive no connection")
                    .kind(),
                std::io::ErrorKind::WouldBlock
            );
        }
    }
}

#[tokio::test]
async fn adapter_uses_public_anonymous_auth_flow() {
    let (result, requests) = run_with_auth(RegistryAuth::Anonymous, |_| {
        vec![
            Some(Response::new("200 OK", "{}")),
            Some(Response::new("200 OK", r#"{"name":"test","tags":null}"#)),
        ]
    })
    .await;
    assert!(result.expect("anonymous listing").is_empty());
    assert_eq!(requests.first().expect("auth discovery").target, "/v2/");
    assert!(
        requests
            .iter()
            .all(|request| request.authorization.is_none())
    );
}

#[tokio::test]
async fn adapter_uses_public_basic_auth_flow() {
    let auth = RegistryAuth::Basic("user".to_owned(), "pass".to_owned());
    let (result, requests) = run_with_auth(auth, |_| {
        vec![
            Some(
                Response::new("401 Unauthorized", "")
                    .header("WWW-Authenticate", r#"Basic realm="registry""#.to_owned()),
            ),
            Some(Response::new("200 OK", r#"{"name":"test","tags":null}"#)),
        ]
    })
    .await;
    assert!(result.expect("basic-auth listing").is_empty());
    assert_eq!(requests.first().expect("auth discovery").target, "/v2/");
    assert!(
        requests
            .first()
            .expect("auth discovery")
            .authorization
            .is_none()
    );
    assert_eq!(
        requests
            .last()
            .expect("tag request")
            .authorization
            .as_deref(),
        Some("Basic dXNlcjpwYXNz")
    );
}

#[tokio::test]
async fn adapter_uses_public_bearer_challenge_flow() {
    for auth in [
        RegistryAuth::Anonymous,
        RegistryAuth::Basic("user".to_owned(), "pass".to_owned()),
    ] {
        let expected_auth = match &auth {
            RegistryAuth::Basic(..) => Some("Basic dXNlcjpwYXNz"),
            _ => None,
        };
        let (result, requests) = run_with_auth(auth, |address| {
            vec![
                Some(bearer_challenge(address)),
                Some(Response::new("200 OK", r#"{"token":"fetched-token"}"#)),
                Some(Response::new("200 OK", r#"{"name":"test","tags":null}"#)),
            ]
        })
        .await;
        assert!(result.expect("bearer challenge listing").is_empty());
        assert_eq!(requests.first().expect("auth discovery").target, "/v2/");
        let token_request = requests.get(1).expect("token exchange");
        assert!(token_request.target.starts_with("/token?"));
        assert!(
            token_request
                .target
                .contains("scope=repository%3Atest%3Apull")
        );
        assert_eq!(token_request.authorization.as_deref(), expected_auth);
        assert_eq!(
            requests
                .last()
                .expect("tag request")
                .authorization
                .as_deref(),
            Some("Bearer fetched-token")
        );
    }
}

#[tokio::test]
async fn adapter_propagates_upstream_authentication_and_transport_errors() {
    for token_response in [
        Some(Response::new("401 Unauthorized", "bad credentials")),
        Some(Response::new("200 OK", "malformed token")),
        None,
    ] {
        let (result, requests) = run_with_auth(RegistryAuth::Anonymous, |address| {
            vec![Some(bearer_challenge(address)), token_response]
        })
        .await;
        assert!(result.is_err(), "auth failure must invalidate listing");
        assert_eq!(requests.len(), 2, "no tag request after failed auth");
    }
    let (result, requests) = run_with_auth(RegistryAuth::Anonymous, |_| vec![None]).await;
    assert!(
        result.is_err(),
        "auth discovery transport failure must propagate"
    );
    assert_eq!(requests.len(), 1);
}

fn bearer_challenge(address: std::net::SocketAddr) -> Response {
    Response::new("401 Unauthorized", "").header(
        "WWW-Authenticate",
        format!(r#"Bearer realm="http://{address}/token",service="stub""#),
    )
}

#[test]
fn production_url_is_https_and_preserves_registry_namespace() {
    let reference: Reference = "ghcr.io/example/test:latest"
        .parse()
        .expect("valid reference");
    assert_eq!(
        tags_url(&reference).expect("HTTPS tag URL").as_str(),
        "https://ghcr.io/v2/example/test/tags/list"
    );
    let mut mirrored = reference;
    mirrored.set_mirror_registry("mirror.example".to_owned());
    assert_eq!(
        tags_url(&mirrored).expect("mirror tag URL").as_str(),
        "https://mirror.example/v2/example/test/tags/list?ns=ghcr.io"
    );
}

#[tokio::test]
async fn production_http_client_refuses_plaintext_requests() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind plaintext target");
    listener.set_nonblocking(true).expect("nonblocking target");
    let address = listener.local_addr().expect("plaintext target address");
    let client = http_client_builder().build().expect("production client");
    assert!(
        client
            .get(format!("http://{address}/v2/test/tags/list"))
            .send()
            .await
            .is_err()
    );
    assert_eq!(
        listener
            .accept()
            .expect_err("plaintext request must not be sent")
            .kind(),
        std::io::ErrorKind::WouldBlock
    );
}

#[tokio::test]
async fn adapter_rejects_a_registry_that_ignores_the_cursor() {
    let page = Some(("200 OK", r#"{"name":"test","tags":["a","b"]}"#));
    let (result, paths) = run_registry(vec![page, page]).await;
    assert!(
        result
            .expect_err("repeated page must not loop")
            .to_string()
            .contains("repeated cursor")
    );
    assert_eq!(paths.len(), 2);
}

type StubResponse = Option<(&'static str, &'static str)>;

fn response_script(after_page: bool, response: StubResponse) -> Vec<StubResponse> {
    let mut responses = Vec::new();
    if after_page {
        responses.push(Some(("200 OK", r#"{"name":"test","tags":["a"]}"#)));
    }
    responses.push(response);
    responses
}
