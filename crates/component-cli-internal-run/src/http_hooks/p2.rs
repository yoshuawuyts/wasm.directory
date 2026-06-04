//! WASI HTTP P2 native-cert hooks.
//!
//! Mirrors [`super::p3`] for the WASI P2 `wasi:http/outgoing-handler` interface used by
//! TinyGo-compiled components.

use tokio::net::TcpStream;
use tracing::warn;
use wasmtime_wasi_http::p2::bindings::http::types::{
    DnsErrorPayload as P2DnsErrorPayload, ErrorCode as P2ErrorCode,
};
use wasmtime_wasi_http::p2::{
    self as p2, WasiHttpHooks as WasiHttpHooksP2,
    body::{HyperIncomingBody, HyperOutgoingBody},
    types::{HostFutureIncomingResponse, IncomingResponse, OutgoingRequestConfig},
};

use super::{RwStream, native_root_tls_config};

fn p2_dns_error(rcode: &str) -> P2ErrorCode {
    P2ErrorCode::DnsError(P2DnsErrorPayload {
        rcode: Some(rcode.to_string()),
        info_code: Some(0),
    })
}

/// WASI HTTP P2 [`WasiHttpHooksP2`] implementation that trusts native OS CA
/// certificates in addition to the built-in [`webpki_roots`] bundle.
///
/// Mirrors [`super::NativeCertHooks`] for the WASI P3 path but targets the P2
/// `wasi:http/outgoing-handler` interface used by TinyGo-compiled components.
pub(crate) struct NativeCertHooksP2;

impl WasiHttpHooksP2 for NativeCertHooksP2 {
    fn send_request(
        &mut self,
        request: hyper::Request<HyperOutgoingBody>,
        config: OutgoingRequestConfig,
    ) -> p2::HttpResult<HostFutureIncomingResponse> {
        let handle =
            wasmtime_wasi::runtime::spawn(
                async move { Ok(p2_native_cert_send(request, config).await) },
            );
        Ok(HostFutureIncomingResponse::pending(handle))
    }
}

/// Async inner implementation of the P2 native-cert request sender.
/// Mirrors `wasmtime_wasi_http::p2::default_send_request_handler` but builds
/// the TLS root store from both `webpki_roots` and the OS native cert store.
async fn p2_native_cert_send(
    mut request: hyper::Request<HyperOutgoingBody>,
    OutgoingRequestConfig {
        use_tls,
        connect_timeout,
        first_byte_timeout,
        between_bytes_timeout,
    }: OutgoingRequestConfig,
) -> Result<IncomingResponse, P2ErrorCode> {
    use http_body_util::BodyExt as _;
    use tokio::time::timeout;
    use wasmtime_wasi_http::io::TokioIo;
    use wasmtime_wasi_http::p2::hyper_request_error;

    let uri_authority = request
        .uri()
        .authority()
        .ok_or(P2ErrorCode::HttpRequestUriInvalid)?
        .clone();
    // Host without the port, used for TLS SNI. Using `Authority::host()` keeps IPv6
    // literals like `[::1]` intact instead of truncating at the first colon.
    let host = uri_authority.host().to_string();
    let authority = if uri_authority.port().is_some() {
        uri_authority.to_string()
    } else {
        let port = if use_tls { 443 } else { 80 };
        format!("{uri_authority}:{port}")
    };

    let tcp_stream = timeout(connect_timeout, TcpStream::connect(&authority))
        .await
        .map_err(|_| P2ErrorCode::ConnectionTimeout)?
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::AddrNotAvailable => p2_dns_error("address not available"),
            _ => {
                if e.to_string()
                    .starts_with("failed to lookup address information")
                {
                    p2_dns_error("address not available")
                } else {
                    P2ErrorCode::ConnectionRefused
                }
            }
        })?;

    // Build a common stream abstraction so both branches produce the same sender type.
    #[allow(clippy::items_after_statements)]
    type Sender = hyper::client::conn::http1::SendRequest<HyperOutgoingBody>;

    let (mut sender, worker): (Sender, _) = if use_tls {
        use rustls::pki_types::ServerName;

        let connector = tokio_rustls::TlsConnector::from(native_root_tls_config());
        let domain = ServerName::try_from(host.as_str())
            .map_err(|e| {
                warn!("invalid DNS name (p2): {e:?}");
                p2_dns_error("invalid dns name")
            })?
            .to_owned();
        let tls_stream = connector.connect(domain, tcp_stream).await.map_err(|e| {
            warn!("TLS protocol error (p2): {e:?}");
            P2ErrorCode::TlsProtocolError
        })?;

        // Erase the concrete TLS stream type behind a boxed trait object so
        // both branches produce the same `(Sender, worker)` tuple type.
        let boxed: Box<dyn RwStream> = Box::new(tls_stream);
        let (sender, conn) = timeout(
            connect_timeout,
            hyper::client::conn::http1::handshake(TokioIo::new(boxed)),
        )
        .await
        .map_err(|_| P2ErrorCode::ConnectionTimeout)?
        .map_err(hyper_request_error)?;
        let worker = wasmtime_wasi::runtime::spawn(async move {
            match conn.await {
                Ok(()) => {}
                Err(e) => warn!("p2 tls connection error: {e}"),
            }
        });
        (sender, worker)
    } else {
        let boxed: Box<dyn RwStream> = Box::new(tcp_stream);
        let (sender, conn) = timeout(
            connect_timeout,
            hyper::client::conn::http1::handshake(TokioIo::new(boxed)),
        )
        .await
        .map_err(|_| P2ErrorCode::ConnectionTimeout)?
        .map_err(hyper_request_error)?;
        let worker = wasmtime_wasi::runtime::spawn(async move {
            match conn.await {
                Ok(()) => {}
                Err(e) => warn!("p2 plain connection error: {e}"),
            }
        });
        (sender, worker)
    };

    // Strip scheme + authority — HTTP/1.1 request-line must not include them.
    *request.uri_mut() = http::Uri::builder()
        .path_and_query(
            request
                .uri()
                .path_and_query()
                .map_or("/", http::uri::PathAndQuery::as_str),
        )
        .build()
        .expect("comes from valid request");

    let resp = timeout(first_byte_timeout, sender.send_request(request))
        .await
        .map_err(|_| P2ErrorCode::ConnectionReadTimeout)?
        .map_err(hyper_request_error)?
        .map(|body| -> HyperIncomingBody { body.map_err(hyper_request_error).boxed_unsync() });

    Ok(IncomingResponse {
        resp,
        worker: Some(worker),
        between_bytes_timeout,
    })
}
