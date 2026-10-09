use std::collections::HashSet;
use std::future::Future;

use anyhow::Context;
use oci_client::secrets::RegistryAuth;
use oci_client::{Reference, RegistryOperation};
use serde::Deserialize;

const PAGE_SIZE: usize = 100;

/// Enumerate tags using OCI's `n` and `last` query parameters.
///
/// The upstream tag decoder rejects legitimate `tags: null` empty listings.
/// Use its authentication flow, but decode tag responses ourselves so only
/// validated HTTP 200 responses for this repository can end pagination.
pub(super) async fn list_tags(
    client: &oci_client::Client,
    reference: &Reference,
    auth: &RegistryAuth,
) -> anyhow::Result<Vec<String>> {
    let http = http_client_builder().build()?;
    list_tags_with_http(client, &http, reference, auth, &tags_url(reference)?).await
}

fn http_client_builder() -> reqwest::ClientBuilder {
    reqwest::Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
}

fn tags_url(reference: &Reference) -> anyhow::Result<reqwest::Url> {
    let mut url = reqwest::Url::parse(&format!(
        "https://{}/v2/{}/tags/list",
        reference.resolve_registry(),
        reference.repository()
    ))?;
    if let Some(namespace) = reference.namespace() {
        url.query_pairs_mut().append_pair("ns", namespace);
    }
    Ok(url)
}

async fn list_tags_with_http(
    client: &oci_client::Client,
    http: &reqwest::Client,
    reference: &Reference,
    auth: &RegistryAuth,
    url: &reqwest::Url,
) -> anyhow::Result<Vec<String>> {
    let token = client
        .auth(reference, auth, RegistryOperation::Pull)
        .await
        .context("failed to authenticate OCI tag listing")?;
    collect_tags(|last| {
        fetch_page(
            http,
            url,
            reference.repository(),
            auth,
            token.as_deref(),
            last,
        )
    })
    .await
}

async fn fetch_page(
    http: &reqwest::Client,
    url: &reqwest::Url,
    repository: &str,
    auth: &RegistryAuth,
    token: Option<&str>,
    last: Option<String>,
) -> anyhow::Result<Vec<String>> {
    let mut request = http.get(url.clone()).query(&[("n", PAGE_SIZE)]);
    if let Some(last) = last {
        request = request.query(&[("last", last)]);
    }
    let response = authenticate_request(request, auth, token).send().await?;
    anyhow::ensure!(
        response.status() == reqwest::StatusCode::OK,
        "OCI tag listing returned HTTP {}",
        response.status()
    );
    let page: TagPage = serde_json::from_slice(&response.bytes().await?)?;
    anyhow::ensure!(
        page.name == repository,
        "OCI tag listing returned repository {:?}, expected {repository:?}",
        page.name
    );
    Ok(page.tags)
}

fn authenticate_request(
    request: reqwest::RequestBuilder,
    auth: &RegistryAuth,
    token: Option<&str>,
) -> reqwest::RequestBuilder {
    match (token, auth) {
        (Some(token), _) => request.bearer_auth(token),
        (None, RegistryAuth::Bearer(token)) => request.bearer_auth(token),
        (None, RegistryAuth::Basic(username, password)) => {
            request.basic_auth(username, Some(password))
        }
        (None, RegistryAuth::Anonymous) => request,
    }
}

#[derive(Deserialize)]
struct TagPage {
    name: String,
    // No serde(default): missing tags is invalid, while an explicit null is empty.
    #[serde(deserialize_with = "deserialize_tags")]
    tags: Vec<String>,
}

fn deserialize_tags<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Option::<Vec<String>>::deserialize(deserializer)?.unwrap_or_default())
}

async fn collect_tags<F, Fut>(mut fetch_page: F) -> anyhow::Result<Vec<String>>
where
    F: FnMut(Option<String>) -> Fut,
    Fut: Future<Output = anyhow::Result<Vec<String>>>,
{
    let mut tags = Vec::new();
    let mut seen_tags = HashSet::new();
    let mut cursors = HashSet::new();
    let mut last = None;

    loop {
        let page = fetch_page(last.clone())
            .await
            .with_context(|| format!("failed to list OCI tags after cursor {last:?}"))?;
        let Some(next) = page.last().cloned() else {
            return Ok(tags);
        };
        anyhow::ensure!(
            cursors.insert(next.clone()),
            "OCI tag pagination did not advance: repeated cursor {next:?}"
        );
        let previous_count = tags.len();
        append_unique_tags(&mut tags, &mut seen_tags, page);
        anyhow::ensure!(
            tags.len() > previous_count,
            "OCI tag pagination made no progress after cursor {last:?}"
        );
        last = Some(next);
    }
}

fn append_unique_tags(tags: &mut Vec<String>, seen: &mut HashSet<String>, page: Vec<String>) {
    for tag in page {
        if seen.insert(tag.clone()) {
            tags.push(tag);
        }
    }
}

#[cfg(test)]
#[path = "tag_pagination_tests.rs"]
mod tests;
