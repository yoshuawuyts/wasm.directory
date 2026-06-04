//! Custom `WasiHttpHooks` that augment TLS root certificates with native system CAs.
//!
//! The default [`wasmtime_wasi_http`] implementation only trusts the [`webpki_roots`] bundle,
//! which breaks in environments that use a TLS inspection proxy with a private CA (e.g.
//! corporate proxies or cloud sandbox environments). This module provides drop-in
//! replacements — one for WASI HTTP P3 ([`NativeCertHooks`]) and one for WASI HTTP P2
//! ([`NativeCertHooksP2`]) — that each load the OS certificate store via
//! [`rustls_native_certs`] in addition to the standard webpki roots.
//!
//! The two hook implementations live in their own modules ([`p3`] and [`p2`]); the shared
//! TLS root store / [`rustls::ClientConfig`] construction lives here so it is built only
//! once and reused across requests.
//!
//! # Alternative approach
//!
//! The same behaviour can be achieved without any extra code by patching
//! `wasmtime-wasi-http` directly: replace the two lines in `src/p3/request.rs` that
//! construct the `RootCertStore` with code that also calls
//! `rustls_native_certs::load_native_certs()`, and add `rustls-native-certs` to
//! `default-send-request` in `Cargo.toml`. This requires vendoring the upstream crate
//! (adding a `[patch.crates-io]` entry and a `vendor/wasmtime-wasi-http/` directory).
//! The hooks approach avoids that maintenance burden at the cost of duplicating ~100 lines
//! of connection logic from `default_send_request`.

mod p2;
mod p3;

pub(crate) use p2::NativeCertHooksP2;
pub(crate) use p3::NativeCertHooks;

use std::sync::{Arc, OnceLock};

use tokio::io::{AsyncRead, AsyncWrite};
use tracing::warn;

/// Async I/O stream abstraction covering both plain TCP and TLS connections.
pub(super) trait RwStream: AsyncRead + AsyncWrite + Send + Unpin + 'static {}
impl<T: AsyncRead + AsyncWrite + Send + Unpin + 'static> RwStream for T {}

/// Shared [`rustls::ClientConfig`] trusting both the [`webpki_roots`] bundle and the OS
/// native CA certificates.
///
/// Loading the native cert store and rebuilding the config is relatively expensive, so it
/// is constructed once on first use and cached for the lifetime of the process. The config
/// carries no per-request state (SNI is supplied per connection), so a single shared
/// instance is safe to reuse for every outbound request.
pub(super) fn native_root_tls_config() -> Arc<rustls::ClientConfig> {
    static CONFIG: OnceLock<Arc<rustls::ClientConfig>> = OnceLock::new();
    let config = CONFIG.get_or_init(|| {
        let mut roots = rustls::RootCertStore {
            roots: webpki_roots::TLS_SERVER_ROOTS.into(),
        };
        let native = rustls_native_certs::load_native_certs();
        for err in &native.errors {
            warn!("native cert load error: {err:?}");
        }
        for cert in native.certs {
            let _ = roots.add(cert);
        }
        Arc::new(
            rustls::ClientConfig::builder()
                .with_root_certificates(roots)
                .with_no_client_auth(),
        )
    });
    Arc::clone(config)
}
