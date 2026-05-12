#![allow(clippy::print_stderr)]

//! Internal crate for executing WebAssembly components via Wasmtime.
//!
//! This crate is **not** intended for third-party consumption — it is an
//! implementation detail of `component-cli`. The API may change without notice.
//!
//! It provides three entry points:
//!
//! - [`validate_component`] — checks that a byte slice is a Wasm Component
//!   (not a core module or WIT-only package).
//! - [`execute_cli_component`] — builds the Wasmtime runtime, wires WASI
//!   permissions, instantiates the component, and invokes
//!   `wasi:cli/run@0.2.0#run`.
//! - [`execute_library_function`] — invokes an arbitrary exported
//!   function on a "library-style" component using wasmtime's untyped
//!   `Func::call` API.

mod errors;

use miette::Context;
use wasmparser::{Encoding, Parser, Payload};
use wasmtime::component::{Component, Linker, Val};
use wasmtime::{Engine, Store};
use wasmtime_wasi::p2::bindings::sync::Command;
use wasmtime_wasi::{DirPerms, FilePerms, ResourceTable, WasiCtxBuilder, WasiCtxView, WasiView};
use wasmtime_wasi_http::WasiHttpCtx;
use wasmtime_wasi_http::p2::{WasiHttpCtxView, WasiHttpView};

pub use errors::RunError;

/// Host state wired into `Store<WasiState>`.
struct WasiState {
    ctx: wasmtime_wasi::WasiCtx,
    http: WasiHttpCtx,
    table: ResourceTable,
}

impl WasiView for WasiState {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.ctx,
            table: &mut self.table,
        }
    }
}

impl WasiHttpView for WasiState {
    fn http(&mut self) -> WasiHttpCtxView<'_> {
        WasiHttpCtxView {
            ctx: &mut self.http,
            table: &mut self.table,
            hooks: Default::default(),
        }
    }
}

/// Confirm the bytes are a Wasm Component (not a core module or WIT-only package).
///
/// # Errors
///
/// Returns a [`RunError`] if the bytes are a core module, invalid binary, or
/// have no version header.
pub fn validate_component(bytes: &[u8]) -> miette::Result<()> {
    let parser = Parser::new(0);
    for payload in parser.parse_all(bytes) {
        match payload {
            Ok(Payload::Version { encoding, .. }) => {
                return match encoding {
                    Encoding::Component => Ok(()),
                    Encoding::Module => Err(RunError::CoreModule.into()),
                };
            }
            Err(e) => {
                return Err(RunError::InvalidBinary {
                    reason: e.to_string(),
                }
                .into());
            }
            _ => {}
        }
    }
    Err(RunError::NoVersionHeader.into())
}

/// Build a [`WasiState`] from the resolved CLI permissions, plus
/// optional `argv` to forward to the guest's `wasi:cli/environment#get-arguments`.
fn build_wasi_state(
    permissions: &component_manifest::ResolvedPermissions,
    argv: &[String],
) -> miette::Result<WasiState> {
    let mut builder = WasiCtxBuilder::new();

    if permissions.inherit_stdio {
        builder.inherit_stdio();
    }
    if permissions.inherit_env {
        builder.inherit_env();
    }
    // Forward explicitly allowed env vars.
    // Entries containing '=' are treated as KEY=VAL pairs (from --env flags);
    // entries without '=' are treated as variable names to look up from the host.
    for entry in &permissions.allow_env {
        if let Some((k, v)) = entry.split_once('=') {
            builder.env(k, v);
        } else if let Ok(v) = std::env::var(entry) {
            builder.env(entry, &v);
        }
    }
    // Pre-open directories with full read/write permissions.
    for dir in &permissions.allow_dirs {
        builder
            .preopened_dir(
                dir,
                dir.to_string_lossy(),
                DirPerms::all(),
                FilePerms::all(),
            )
            .map_err(into_miette)
            .wrap_err_with(|| format!("failed to pre-open directory: {}", dir.display()))?;
    }
    if permissions.inherit_network {
        builder.inherit_network();
    }
    if !argv.is_empty() {
        builder.args(argv);
    }

    let wasi_ctx = builder.build();
    Ok(WasiState {
        ctx: wasi_ctx,
        http: WasiHttpCtx::new(),
        table: ResourceTable::new(),
    })
}

/// Build the Wasmtime runtime, instantiate the component, and invoke
/// `wasi:cli/run@0.2.0#run`.
///
/// `argv` is forwarded to the guest as `wasi:cli/environment#get-arguments`.
///
/// Returns `Ok(Ok(()))` on success, `Ok(Err(()))` when the guest returns a
/// non-zero exit code, or a [`miette::Report`] on runtime failures.
///
/// # Errors
///
/// Returns a [`miette::Report`] when compilation, instantiation, or WASI
/// context setup fails.
pub fn execute_cli_component(
    bytes: &[u8],
    permissions: &component_manifest::ResolvedPermissions,
    argv: &[String],
) -> miette::Result<Result<(), ()>> {
    let engine = build_engine()?;
    let component = Component::new(&engine, bytes)
        .map_err(into_miette)
        .wrap_err("failed to compile Wasm Component")?;

    let state = build_wasi_state(permissions, argv)?;
    let mut store = Store::new(&engine, state);

    let mut linker = Linker::new(&engine);
    wasmtime_wasi::p2::add_to_linker_sync(&mut linker).map_err(into_miette)?;

    let command = Command::instantiate(&mut store, &component, &linker)
        .map_err(into_miette)
        .wrap_err("failed to instantiate Wasm Component")?;

    let result = command.wasi_cli_run().call_run(&mut store);
    match result {
        Ok(Ok(())) => Ok(Ok(())),
        Ok(Err(())) => {
            eprintln!("Error: guest returned a non-zero exit code");
            Ok(Err(()))
        }
        Err(e) => {
            eprintln!("Error: {e:#}");
            Ok(Err(()))
        }
    }
}

/// Invoke an arbitrary exported function on a "library-style"
/// component using wasmtime's untyped `Func::call` API.
///
/// `interface` selects an exported interface (e.g. `"math"`); pass
/// `None` for free world-level exports. `func` is the function name.
///
/// # Errors
///
/// Returns a [`miette::Report`] on compilation, instantiation, lookup,
/// or invocation failures.
// r[impl run.library-detection]
// r[impl run.library-dispatch]
pub fn execute_library_function(
    bytes: &[u8],
    permissions: &component_manifest::ResolvedPermissions,
    interface: Option<&str>,
    func: &str,
    args: &[Val],
) -> miette::Result<Vec<Val>> {
    let engine = build_engine()?;
    let component = Component::new(&engine, bytes)
        .map_err(into_miette)
        .wrap_err("failed to compile Wasm Component")?;

    let state = build_wasi_state(permissions, &[])?;
    let mut store = Store::new(&engine, state);

    let mut linker: Linker<WasiState> = Linker::new(&engine);
    wasmtime_wasi::p2::add_to_linker_sync(&mut linker).map_err(into_miette)?;
    // Make wasi:http/outgoing-handler available so library functions
    // can opportunistically perform outbound HTTP. Use the
    // http-only variant to avoid colliding with `wasi:io/error` etc.
    // already provided by `wasmtime_wasi::p2::add_to_linker_sync`.
    wasmtime_wasi_http::p2::add_only_http_to_linker_sync(&mut linker).map_err(into_miette)?;

    let instance = linker.instantiate(&mut store, &component).map_err(|e| {
        RunError::LibraryInstantiationFailed {
            cause: format!("{e:#}"),
        }
    })?;

    // Look up the function. Two-phase via `Component::get_export_index`
    // for interface-nested functions; direct `instance.get_func` for
    // free world-level functions.
    let target =
        match interface {
            None => instance.get_func(&mut store, func).ok_or_else(|| {
                RunError::LibraryExportMissing {
                    path: func.to_string(),
                }
            })?,
            Some(iface) => {
                let iface_idx = component.get_export_index(None, iface).ok_or_else(|| {
                    RunError::LibraryExportMissing {
                        path: iface.to_string(),
                    }
                })?;
                let func_idx = component
                    .get_export_index(Some(&iface_idx), func)
                    .ok_or_else(|| RunError::LibraryExportMissing {
                        path: format!("{iface}#{func}"),
                    })?;
                instance.get_func(&mut store, func_idx).ok_or_else(|| {
                    RunError::LibraryExportMissing {
                        path: format!("{iface}#{func}"),
                    }
                })?
            }
        };

    // Pre-size the results buffer using `Func::ty(&store).results().len()`.
    // Initial values are ignored — wasmtime overwrites them.
    let result_count = target.ty(&store).results().len();
    let mut results = vec![Val::Bool(false); result_count];

    target
        .call(&mut store, args, &mut results)
        .map_err(into_miette)
        .wrap_err_with(|| format!("invoking {}", display_path(interface, func)))?;
    // CRITICAL: do NOT call `Func::post_return` — automatic in wasmtime 43+.

    Ok(results)
}

fn display_path(interface: Option<&str>, func: &str) -> String {
    match interface {
        Some(iface) => format!("{iface}#{func}"),
        None => func.to_string(),
    }
}

/// Build a [`wasmtime::Engine`] with component-model-async enabled so that
/// WASI 0.3 components using `stream` / `future` types can be compiled.
///
/// In wasmtime 44, async support is unconditional (the legacy
/// `Config::async_support` toggle is a no-op), so the same engine is usable
/// from the HTTP run path which relies on `add_to_linker_async` and
/// `instantiate_async`.
pub fn build_engine() -> miette::Result<Engine> {
    let mut config = wasmtime::Config::new();
    config.wasm_component_model_async(true);
    Engine::new(&config)
        .map_err(into_miette)
        .wrap_err("failed to create Wasmtime engine")
}

/// Convert an error into a [`miette::Report`], preserving the cause chain.
fn into_miette(err: impl std::fmt::Display) -> miette::Report {
    miette::miette!("{err:#}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression test for WASI 0.3 `stream` / `future` support.
    ///
    /// Without `wasm_component_model_async(true)` on the engine, parsing a
    /// component that contains a `stream` type fails with:
    ///
    /// > failed to parse WebAssembly module: `stream` requires the component
    /// > model async feature
    ///
    /// See PR #359.
    #[test]
    fn build_engine_compiles_component_with_stream_type() {
        let engine = build_engine().expect("build_engine should succeed");
        // Minimal component containing a `stream` type — the construct that
        // requires the component-model-async feature to even parse.
        let wat = r#"(component (type (stream u8)))"#;
        Component::new(&engine, wat)
            .expect("component using `stream` should compile when async features are enabled");
    }
}
