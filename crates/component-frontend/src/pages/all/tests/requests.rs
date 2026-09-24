use super::*;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::time::{Duration, timeout};

async fn render_with_responses(responses: Vec<(&str, String)>) -> (String, Vec<String>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind registry fixture");
    let address = listener.local_addr().expect("get registry fixture address");
    let client = RegistryClient::new(format!("http://{address}"));
    timeout(Duration::from_secs(10), async {
        let serve = async {
            let mut requests = Vec::new();
            for (status, body) in responses {
                let (stream, _) = listener.accept().await.expect("accept registry request");
                let mut stream = BufReader::new(stream);
                let mut request = String::new();
                stream.read_line(&mut request).await.expect("read request");
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream.get_mut().write_all(response.as_bytes()).await.expect("write registry response");
                requests.push(request.trim().to_owned());
            }
            requests
        };
        tokio::join!(render(&client, 100, 100), serve)
    })
    .await
    .expect("registry fixture requests should finish")
}

fn package_response() -> (&'static str, String) {
    (
        "200 OK",
        serde_json::to_string(&package_row::tests::packages())
            .expect("serialize registry fixture packages"),
    )
}

#[tokio::test]
async fn fetches_both_page_and_registry_total() {
    let (html, requests) = render_with_responses(vec![
        package_response(),
        (
            "200 OK",
            r#"{"packages":245,"namespaces":2,"versions":1000}"#.to_owned(),
        ),
    ])
    .await;
    assert!(html.contains("showing 4 of 245 results"));
    assert_eq!(
        requests,
        [
            "GET /v1/packages?offset=100&limit=100 HTTP/1.1",
            "GET /v1/stats HTTP/1.1",
        ]
    );
}

#[tokio::test]
async fn failed_or_malformed_stats_do_not_hide_packages() {
    for response in [
        (
            "503 Service Unavailable",
            r#"{"error":"stats unavailable"}"#.to_owned(),
        ),
        (
            "503 Service Unavailable",
            r#"{"packages":245,"namespaces":2,"versions":1000}"#.to_owned(),
        ),
        ("200 OK", "not valid stats JSON".to_owned()),
    ] {
        let (html, requests) = render_with_responses(vec![package_response(), response]).await;
        package_row::tests::assert_listing(&html, &package_row::tests::packages());
        assert!(html.contains("showing 4 results (total unavailable)"));
        assert!(!html.contains("Unable to load packages"));
        assert_eq!(requests.len(), 2);
    }
}

#[tokio::test]
async fn package_failure_keeps_error_page_without_fetching_stats() {
    let (html, requests) = render_with_responses(vec![(
        "503 Service Unavailable",
        r#"{"error":"packages unavailable"}"#.to_owned(),
    )])
    .await;
    assert!(html.contains("Unable to load packages"));
    assert!(!html.contains("total unavailable"));
    assert_eq!(requests, ["GET /v1/packages?offset=100&limit=100 HTTP/1.1"]);
}
