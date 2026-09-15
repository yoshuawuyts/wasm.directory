//! Native-cert [`WasiHttpHooks`] implementation.

use bytes::Bytes;
use core::pin::Pin;
use core::task::{Context, Poll, ready};
use core::time::Duration;
use http::uri::Scheme;
use std::future::Future;
use tokio::net::TcpStream;
use tracing::warn;
use wasmtime_wasi_http::{Error, RequestOptions, WasiBody, WasiHttpHooks, io::TokioIo};

use super::{RwStream, native_root_tls_config};

/// Default timeout applied when the guest does not configure one.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(600);

/// [`WasiHttpHooks`] implementation that trusts native OS CA certificates in addition to
/// the built-in [`webpki_roots`] bundle.
///
/// The same hooks serve both the WASI HTTP P2 (`wasi:http/outgoing-handler`) and P3
/// (`wasi:http/handler`) paths, which share a single outgoing-request hook.
pub(crate) struct NativeCertHooks;

impl WasiHttpHooks for NativeCertHooks {
    fn send_request(
        &mut self,
        request: http::Request<WasiBody>,
        options: Option<RequestOptions>,
        _fut: Box<dyn Future<Output = Result<(), Error>> + Send>,
    ) -> Box<
        dyn Future<
                Output = Result<
                    (
                        http::Response<WasiBody>,
                        Box<dyn Future<Output = Result<(), Error>> + Send>,
                    ),
                    Error,
                >,
            > + Send,
    > {
        Box::new(async move {
            let (res, io) = send(request, options).await?;
            Ok((
                res.map(http_body_util::BodyExt::boxed_unsync),
                Box::new(io) as Box<dyn Future<Output = Result<(), Error>> + Send>,
            ))
        })
    }
}

/// Async inner implementation of the native-cert request sender.
///
/// Mirrors [`wasmtime_wasi_http::default_send_request`] but builds the TLS root store from
/// both [`webpki_roots`] and the OS native cert store.
async fn send(
    mut req: http::Request<WasiBody>,
    options: Option<RequestOptions>,
) -> Result<
    (
        http::Response<ResponseBody>,
        impl Future<Output = Result<(), Error>> + Send,
    ),
    Error,
> {
    use core::future::poll_fn;
    use core::pin::pin;

    let uri = req.uri();
    let authority = uri.authority().ok_or(Error::HttpRequestUriInvalid)?.clone();
    let use_tls = uri.scheme() == Some(&Scheme::HTTPS);
    let addr = if authority.port().is_some() {
        authority.to_string()
    } else {
        format!("{}:{}", authority, if use_tls { 443 } else { 80 })
    };

    let connect_timeout = options
        .and_then(|o| o.connect_timeout)
        .unwrap_or(DEFAULT_TIMEOUT);
    let first_byte_timeout = options
        .and_then(|o| o.first_byte_timeout)
        .unwrap_or(DEFAULT_TIMEOUT);
    let between_bytes_timeout = options
        .and_then(|o| o.between_bytes_timeout)
        .unwrap_or(DEFAULT_TIMEOUT);

    let tcp = match tokio::time::timeout(connect_timeout, TcpStream::connect(&addr)).await {
        Ok(Ok(s)) => s,
        Ok(Err(e)) if e.kind() == std::io::ErrorKind::AddrNotAvailable => {
            return Err(dns_error());
        }
        Ok(Err(e))
            if e.to_string()
                .starts_with("failed to lookup address information") =>
        {
            return Err(dns_error());
        }
        Ok(Err(_)) => return Err(Error::ConnectionRefused),
        Err(_) => return Err(Error::ConnectionTimeout),
    };

    let stream: Box<dyn RwStream> = if use_tls {
        Box::new(tls_connect(&authority, tcp).await?)
    } else {
        Box::new(tcp)
    };

    let (mut sender, conn) = tokio::time::timeout(
        connect_timeout,
        hyper::client::conn::http1::Builder::new().handshake(TokioIo::new(stream)),
    )
    .await
    .map_err(|_| Error::ConnectionTimeout)??;

    // HTTP/1.1 must not include scheme or authority in the request URI.
    *req.uri_mut() = http::Uri::builder()
        .path_and_query(
            req.uri()
                .path_and_query()
                .map_or("/", http::uri::PathAndQuery::as_str),
        )
        .build()
        .expect("comes from valid request");

    let send_fut = async move {
        let res = tokio::time::timeout(first_byte_timeout, sender.send_request(req))
            .await
            .map_err(|_| Error::ConnectionReadTimeout)?
            .map_err(Error::from)?;
        let mut timeout = tokio::time::interval(between_bytes_timeout);
        timeout.reset();
        Ok(res.map(|incoming| ResponseBody { incoming, timeout }))
    };

    let mut send_fut = pin!(send_fut);
    let mut conn = Some(conn);

    // Drive connection I/O alongside the send future.
    let res = poll_fn(|cx| match send_fut.as_mut().poll(cx) {
        Poll::Ready(v) => Poll::Ready(v),
        Poll::Pending => {
            let Some(c) = conn.as_mut() else {
                return Poll::Pending;
            };
            match ready!(Pin::new(c).poll(cx)) {
                Ok(()) => {
                    conn = None;
                    send_fut.as_mut().poll(cx)
                }
                Err(err) => Poll::Ready(Err(Error::from(err))),
            }
        }
    })
    .await?;

    // Wait for connection close after the response body is consumed.
    let io_fut = async move {
        let Some(c) = conn else {
            return Ok(());
        };
        c.await.map_err(|err| {
            if err.is_timeout() {
                Error::HttpResponseTimeout
            } else {
                Error::from(err)
            }
        })
    };

    Ok((res, io_fut))
}

/// Perform the TLS handshake for an outgoing request, using the shared
/// native-cert-augmented client configuration.
async fn tls_connect(
    authority: &http::uri::Authority,
    tcp: TcpStream,
) -> Result<tokio_rustls::client::TlsStream<TcpStream>, Error> {
    use rustls::pki_types::ServerName;

    let connector = tokio_rustls::TlsConnector::from(native_root_tls_config());
    let domain = ServerName::try_from(authority.host())
        .map_err(|e| {
            warn!("invalid DNS name: {e:?}");
            dns_error_with("invalid dns name")
        })?
        .to_owned();
    connector.connect(domain, tcp).await.map_err(|e| {
        warn!("TLS protocol error: {e:?}");
        Error::TlsProtocolError
    })
}

/// DNS failure reported when an address could not be resolved.
fn dns_error() -> Error {
    dns_error_with("address not available")
}

/// DNS failure carrying `rcode` as the failure reason.
fn dns_error_with(rcode: &str) -> Error {
    Error::DnsError {
        rcode: Some(rcode.to_string()),
        info_code: Some(0),
    }
}

/// Response body that enforces the between-bytes read timeout.
struct ResponseBody {
    incoming: hyper::body::Incoming,
    timeout: tokio::time::Interval,
}

impl http_body::Body for ResponseBody {
    type Data = Bytes;
    type Error = Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
        match Pin::new(&mut self.as_mut().incoming).poll_frame(cx) {
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Ready(Some(Err(err))) => Poll::Ready(Some(Err(if err.is_timeout() {
                Error::HttpResponseTimeout
            } else {
                Error::from(err)
            }))),
            Poll::Ready(Some(Ok(frame))) => {
                self.timeout.reset();
                Poll::Ready(Some(Ok(frame)))
            }
            Poll::Pending => {
                ready!(self.timeout.poll_tick(cx));
                Poll::Ready(Some(Err(Error::ConnectionReadTimeout)))
            }
        }
    }

    fn is_end_stream(&self) -> bool {
        self.incoming.is_end_stream()
    }

    fn size_hint(&self) -> http_body::SizeHint {
        self.incoming.size_hint()
    }
}
