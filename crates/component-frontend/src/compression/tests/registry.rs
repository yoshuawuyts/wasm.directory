use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use wasm_meta_registry_client::RegistryClient;

pub(super) struct Registry {
    address: std::net::SocketAddr,
    task: JoinHandle<()>,
}

impl Registry {
    pub(super) async fn new() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind fixture");
        let address = listener.local_addr().expect("fixture address");
        let task = tokio::spawn(async move {
            loop {
                let (stream, _) = listener.accept().await.expect("accept request");
                respond(stream).await;
            }
        });
        Self { address, task }
    }

    pub(super) fn client(&self) -> RegistryClient {
        RegistryClient::new(format!("http://{}", self.address))
    }
}

impl Drop for Registry {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn respond(stream: TcpStream) {
    let mut stream = BufReader::new(stream);
    let mut line = String::new();
    stream.read_line(&mut line).await.expect("read request");
    let body = match line.split_whitespace().nth(1).expect("request path") {
        "/v1/stats" => r#"{"packages":0,"namespaces":0,"versions":0}"#,
        "/v1/releases/recent?limit=10"
        | "/v1/packages/new?limit=10"
        | "/v1/packages/popular?limit=10" => "[]",
        path => panic!("unexpected fixture path: {path}"),
    };
    loop {
        line.clear();
        stream.read_line(&mut line).await.expect("read header");
        if line == "\r\n" {
            break;
        }
        assert!(!line.is_empty(), "request headers ended early");
    }
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream
        .get_mut()
        .write_all(response.as_bytes())
        .await
        .expect("write fixture");
}
