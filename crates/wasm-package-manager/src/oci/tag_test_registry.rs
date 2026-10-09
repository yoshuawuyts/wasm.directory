use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::thread;
use std::time::{Duration, Instant};

use oci_client::Reference;
use oci_client::client::{ClientConfig, ClientProtocol};
use oci_client::secrets::RegistryAuth;

use super::super::{http_client_builder, list_tags_with_http, tags_url};

#[derive(Debug)]
pub(super) struct Response {
    status: &'static str,
    body: String,
    headers: Vec<(&'static str, String)>,
}

impl Response {
    pub(super) fn new(status: &'static str, body: &str) -> Self {
        Self {
            status,
            body: body.to_owned(),
            headers: Vec::new(),
        }
    }

    pub(super) fn header(mut self, name: &'static str, value: String) -> Self {
        self.headers.push((name, value));
        self
    }
}

#[derive(Debug)]
pub(super) struct Request {
    pub(super) target: String,
    pub(super) authorization: Option<String>,
}

pub(super) async fn run_registry(
    responses: Vec<Option<(&'static str, &'static str)>>,
) -> (anyhow::Result<Vec<String>>, Vec<String>) {
    let (result, requests) = run_with_auth(RegistryAuth::Bearer("stub-token".to_owned()), |_| {
        responses
            .into_iter()
            .map(|response| response.map(|(status, body)| Response::new(status, body)))
            .collect()
    })
    .await;
    assert!(
        requests
            .iter()
            .all(|request| { request.authorization.as_deref() == Some("Bearer stub-token") })
    );
    (
        result,
        requests.into_iter().map(|request| request.target).collect(),
    )
}

pub(super) async fn run_with_auth(
    auth: RegistryAuth,
    responses: impl FnOnce(SocketAddr) -> Vec<Option<Response>>,
) -> (anyhow::Result<Vec<String>>, Vec<Request>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local registry");
    listener
        .set_nonblocking(true)
        .expect("nonblocking listener");
    let address = listener.local_addr().expect("local registry address");
    let responses = responses(address);
    let server = thread::spawn(move || serve_responses(&listener, responses));
    let client = oci_client::Client::new(ClientConfig {
        protocol: ClientProtocol::Http,
        no_proxy: Some("*".to_owned()),
        read_timeout: Some(Duration::from_secs(5)),
        connect_timeout: Some(Duration::from_secs(5)),
        ..Default::default()
    });
    let http = http_client_builder()
        .https_only(false)
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("local tag HTTP client");
    let reference: Reference = format!("{address}/test:latest")
        .parse()
        .expect("valid local registry reference");
    let mut url = tags_url(&reference).expect("local tag URL");
    url.set_scheme("http").expect("local HTTP stub scheme");
    let result = list_tags_with_http(&client, &http, &reference, &auth, &url).await;
    (result, server.join().expect("registry stub succeeded"))
}

fn serve_responses(listener: &TcpListener, responses: Vec<Option<Response>>) -> Vec<Request> {
    let mut requests = Vec::new();
    for response in responses {
        let mut stream = accept_request(listener);
        requests.push(read_request(&stream));
        if let Some(response) = response {
            write_response(&mut stream, response);
        }
    }
    requests
}

fn read_request(stream: &TcpStream) -> Request {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).expect("read request line");
    let target = line
        .split_whitespace()
        .nth(1)
        .expect("request target")
        .to_owned();
    let mut authorization = None;
    loop {
        line.clear();
        reader.read_line(&mut line).expect("read request header");
        if line == "\r\n" || line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("authorization")
        {
            authorization = Some(value.trim().to_owned());
        }
    }
    Request {
        target,
        authorization,
    }
}

fn write_response(stream: &mut TcpStream, response: Response) {
    let Response {
        status,
        body,
        headers,
    } = response;
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    )
    .expect("write response headers");
    for (name, value) in headers {
        write!(stream, "{name}: {value}\r\n").expect("write additional header");
    }
    write!(stream, "\r\n{body}").expect("write response body");
}

fn accept_request(listener: &TcpListener) -> TcpStream {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                stream
                    .set_nonblocking(false)
                    .expect("blocking request stream");
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .expect("request timeout");
                return stream;
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(Instant::now() < deadline, "registry request timed out");
                thread::sleep(Duration::from_millis(1));
            }
            Err(error) => panic!("failed accepting registry request: {error}"),
        }
    }
}
