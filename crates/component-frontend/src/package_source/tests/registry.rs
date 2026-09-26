use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request};
use axum::response::Response;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use tower::ServiceExt;
use wasm_meta_registry_client::{KnownPackage, RegistryClient};

use super::fixtures::{package, version};

/// Every relationship-count lookup reports this total.
pub(super) const RELATIONSHIP_TOTAL: u64 = 7;

pub(super) struct RegistryFixture {
    app: Router,
    requests: Arc<Mutex<Vec<String>>>,
    relationship_requests: Arc<Mutex<Vec<String>>>,
    task: JoinHandle<()>,
}

impl RegistryFixture {
    pub(super) async fn new(wit: Option<&str>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind source fixture");
        let address = listener.local_addr().expect("source fixture address");
        let mut other_mirror = package();
        other_mirror.registry = "canonical.test".to_owned();
        other_mirror.repository = "canonical/http".to_owned();
        other_mirror.tags = vec!["9.0.0".to_owned()];
        let search = serde_json::to_string(&[&other_mirror]).expect("serialize other mirror");
        let mut responses = HashMap::new();
        add_mirror(&mut responses, &package(), wit);
        add_mirror(&mut responses, &other_mirror, wit);
        let requests = Arc::new(Mutex::new(Vec::new()));
        let relationship_requests = Arc::new(Mutex::new(Vec::new()));
        let captured = Captured {
            requests: Arc::clone(&requests),
            relationship_requests: Arc::clone(&relationship_requests),
        };
        let task = tokio::spawn(async move {
            loop {
                let (stream, _) = listener.accept().await.expect("accept source request");
                respond(stream, &responses, &search, &captured).await;
            }
        });
        Self {
            app: crate::app_with_client(RegistryClient::new(format!("http://{address}"))),
            requests,
            relationship_requests,
            task,
        }
    }

    pub(super) async fn request(&self, method: Method, uri: &str) -> Response {
        self.send(
            Request::builder()
                .method(method)
                .uri(uri)
                .body(Body::empty())
                .expect("detail request"),
        )
        .await
    }

    pub(super) async fn send(&self, request: Request<Body>) -> Response {
        self.app
            .clone()
            .oneshot(request)
            .await
            .expect("detail response")
    }

    /// Package and release lookups; version-independent relationship
    /// counts are recorded separately in [`Self::relationship_requests`].
    pub(super) fn requests(&self) -> Vec<String> {
        self.requests.lock().expect("fixture requests lock").clone()
    }

    pub(super) fn relationship_requests(&self) -> Vec<String> {
        self.relationship_requests
            .lock()
            .expect("fixture relationship requests lock")
            .clone()
    }
}

struct Captured {
    requests: Arc<Mutex<Vec<String>>>,
    relationship_requests: Arc<Mutex<Vec<String>>>,
}

impl Drop for RegistryFixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn add_mirror(responses: &mut HashMap<String, String>, pkg: &KnownPackage, wit: Option<&str>) {
    responses.insert(
        format!("/v1/packages/{}/{}", pkg.registry, pkg.repository),
        serde_json::to_string(pkg).expect("serialize mirror"),
    );
    for tag in &pkg.tags {
        let mut release = version(wit);
        release.tag = Some(tag.clone());
        responses.insert(
            format!(
                "/v1/packages/version/{}/{tag}/{}",
                pkg.registry, pkg.repository
            ),
            serde_json::to_string(&release).expect("serialize mirror version"),
        );
    }
}

async fn respond(
    stream: TcpStream,
    responses: &HashMap<String, String>,
    search: &str,
    captured: &Captured,
) {
    let mut stream = BufReader::new(stream);
    let mut request = String::new();
    stream.read_line(&mut request).await.expect("read request");
    let path = request.split_whitespace().nth(1).expect("request target");
    let mut header = String::new();
    loop {
        header.clear();
        stream.read_line(&mut header).await.expect("read header");
        if header == "\r\n" || header.is_empty() {
            break;
        }
    }
    let relationship_page = format!(
        r#"{{"results":[],"total":{RELATIONSHIP_TOTAL},"offset":0,"limit":1,"has_next":true}}"#
    );
    let is_relationship = path.starts_with("/v1/relationships/");
    let log = if is_relationship {
        &captured.relationship_requests
    } else {
        &captured.requests
    };
    log.lock().expect("capture request").push(path.to_owned());
    let (status, body) = match responses.get(path) {
        Some(body) => ("200 OK", body.as_str()),
        None if is_relationship => ("200 OK", relationship_page.as_str()),
        None if path.starts_with("/v1/search?") => ("200 OK", search),
        None => ("404 Not Found", r#"{"error":"fixture source not found"}"#),
    };
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream
        .get_mut()
        .write_all(response.as_bytes())
        .await
        .expect("write source response");
}
