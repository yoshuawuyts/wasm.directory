#![allow(clippy::print_stdout, clippy::print_stderr)]

//! Execute a Wasm Component via Wasmtime.
//!
//! Runs a Wasm Component from a local file or OCI reference. The component is
//! sandboxed by default — WASI capabilities (env, filesystem, network, stdio)
//! are only granted through CLI flags or layered config.
//!
//! Both `wasi:cli/command` and `wasi:http/proxy` worlds are supported.
//! Components that export `wasi:http/incoming-handler` are served as HTTP
//! servers; all others are executed as CLI commands.

mod errors;
mod http;

use std::net::SocketAddr;
use std::path::PathBuf;

use errors::RunError;
use miette::{Context, IntoDiagnostic};

use component_manifest::RunPermissions;
use component_package_manager::manager::Manager;
use wasmparser::{Parser, Payload};

use wit2cli::{
    LibraryExtractError, build_clap, extract_library_surface, parse_invocation, print_results,
};

/// Options for the `component run` command.
#[derive(clap::Parser)]
#[allow(clippy::struct_excessive_bools)]
pub(crate) struct Opts {
    /// Local file path, OCI reference, or manifest key (scope:component)
    /// for a Wasm Component.
    #[arg(value_name = "INPUT")]
    input: String,

    /// Pass an environment variable to the guest (repeatable).
    #[arg(long = "env", value_name = "KEY=VAL", num_args = 1)]
    envs: Vec<String>,

    /// Pre-open a host directory for the guest (repeatable).
    #[arg(long = "dir", value_name = "HOST_PATH")]
    dirs: Vec<PathBuf>,

    /// Inherit all host environment variables.
    #[arg(long)]
    inherit_env: bool,

    /// Allow the guest to access the network.
    #[arg(long)]
    inherit_network: bool,

    /// Suppress stdin/stdout/stderr inheritance.
    #[arg(long)]
    no_stdio: bool,

    /// Address to bind the HTTP server to when running a `wasi:http/proxy`
    /// component.
    #[arg(long, value_name = "ADDR", default_value = "127.0.0.1:8080")]
    listen: SocketAddr,

    /// Run from the global cache, bypassing local installation.
    #[arg(long, short = 'g')]
    global: bool,

    /// Trailing arguments forwarded to the guest. For
    /// `wasi:cli/command` components these become `argv`; for
    /// library-style components they are parsed by a dynamically
    /// generated sub-CLI built from the component's WIT exports.
    ///
    /// Note: host-side flags (such as `--global`, `--env`, `--dir`)
    /// must be specified BEFORE the `<INPUT>` argument; everything
    /// after `<INPUT>` is forwarded to the guest.
    // r[impl run.host-flags-before-input]
    #[arg(
        last = false,
        trailing_var_arg = true,
        allow_hyphen_values = true,
        num_args = 0..,
        value_name = "GUEST_ARGS"
    )]
    extra: Vec<String>,
}

impl Opts {
    /// Execute the `run` command.
    pub(crate) async fn run(self, offline: bool) -> miette::Result<()> {
        let input = self.input.as_str();

        // 1. Resolve input — local files take priority, then manifest keys,
        //    then OCI references.
        let local_path = PathBuf::from(input);
        let is_local = local_path.exists();

        // Manifest keys use `scope:component` syntax; an optional `@version`
        // suffix (e.g. `yoshuawuyts:wordmark@2.0.6`) is part of the input
        // grammar but is not part of the key stored in `wasm.toml`. Strip the
        // version when consulting the manifest/lockfile, but pass the original
        // input — which still carries the version — to install/global-cache
        // resolution so the requested version is honored.
        let manifest_key = strip_at_version(input);

        // Try manifest key lookup (scope:component syntax).
        let mut manifest_path = if is_local {
            None
        } else {
            resolve_manifest_key(manifest_key)?
        };

        // For inputs that look like manifest keys (`scope:component`) but are
        // not yet installed in the local project, auto-install into a local
        // manifest + lockfile by default. The `--global` flag bypasses local
        // installation and runs from the global cache instead.
        let global_bytes =
            if !is_local && manifest_path.is_none() && looks_like_manifest_key(manifest_key) {
                if self.global {
                    Some(load_from_global_cache(input, offline).await?)
                } else {
                    auto_install(input, offline).await?;
                    // Re-resolve the manifest key now that the install has
                    // populated `wasm.toml`, `wasm.lock.toml`, and the
                    // vendored Wasm file.
                    manifest_path = resolve_manifest_key(manifest_key)?;
                    None
                }
            } else {
                None
            };

        // Only try OCI when the input is not a local file and not a manifest key.
        let reference = if is_local || manifest_path.is_some() || global_bytes.is_some() {
            None
        } else {
            crate::util::parse_reference(input).ok()
        };

        // 2. Get Wasm bytes.
        let bytes = if let Some(bytes) = global_bytes {
            bytes
        } else if let Some(ref vendored) = manifest_path {
            tokio::fs::read(vendored)
                .await
                .into_diagnostic()
                .wrap_err_with(|| format!("failed to read {}", vendored.display()))?
        } else {
            match reference {
                Some(ref oci_ref) => fetch_oci_bytes(oci_ref, offline).await?,
                None => tokio::fs::read(&local_path)
                    .await
                    .into_diagnostic()
                    .wrap_err_with(|| format!("failed to read {}", local_path.display()))?,
            }
        };

        // 3. Validate — must be a Wasm Component.
        component_cli_internal_run::validate_component(&bytes)?;

        // 4. Resolve permissions (4-layer merge).
        let permissions = self.resolve_permissions(reference.as_ref());

        // 5. Detect world and execute.
        if http::exports_http_incoming_handler(&bytes) {
            // wasi:http/proxy — start an HTTP server.
            // r[impl run.host-flags-before-input]
            if !self.extra.is_empty() {
                return Err(miette::miette!(
                    "trailing arguments are not allowed for HTTP-proxy components: {:?}",
                    self.extra
                ));
            }
            http::serve(&bytes, &permissions, self.listen).await?;
        } else if exports_cli_run(&bytes) {
            // wasi:cli/command — run as a CLI program, forwarding
            // trailing args as guest argv.
            let argv = self.extra.clone();
            let result = tokio::task::spawn_blocking(move || {
                component_cli_internal_run::execute_cli_component(&bytes, &permissions, &argv)
            })
            .await
            .into_diagnostic()
            .wrap_err("runtime task panicked")??;

            // 6. Map exit.
            if let Err(()) = result {
                std::process::exit(1);
            }
        } else {
            // Library-style component: build a clap CLI from the
            // component's WIT and dynamically dispatch.
            // r[impl run.library-detection]
            return run_library_component(&bytes, &permissions, &self.extra).await;
        }
        Ok(())
    }

    /// Build a [`RunPermissions`] from CLI flags (only the explicitly
    /// provided flags are `Some`).
    fn cli_permissions(&self) -> RunPermissions {
        let mut perms = RunPermissions::default();

        if self.inherit_env {
            perms.inherit_env = Some(true);
        }
        if !self.envs.is_empty() {
            perms.allow_env = Some(self.envs.clone());
        }
        if !self.dirs.is_empty() {
            perms.allow_dirs = Some(self.dirs.clone());
        }
        if self.no_stdio {
            perms.inherit_stdio = Some(false);
        }
        if self.inherit_network {
            perms.inherit_network = Some(true);
        }

        perms
    }

    /// Resolve permissions through the 4-layer merge:
    ///
    /// 1. Global defaults from `config.toml` → `[run.permissions]`
    /// 2. Global per-component from `components.toml`
    /// 3. Local per-component from `wasm.toml`
    /// 4. CLI flags
    fn resolve_permissions(
        &self,
        reference: Option<&component_package_manager::Reference>,
    ) -> component_manifest::ResolvedPermissions {
        let cli = self.cli_permissions();
        component_package_manager::permissions::resolve_permissions(reference, cli)
    }
}

/// Resolve a `scope:component` manifest key to a vendored file path.
///
/// Reads the lockfile to find the matching component entry, then
/// reconstructs the vendor filename from registry, version, and digest.
/// Returns `None` if the input doesn't match any manifest entry.
fn resolve_manifest_key(input: &str) -> miette::Result<Option<PathBuf>> {
    let lockfile_path = PathBuf::from("wasm.lock.toml");
    let manifest_path = PathBuf::from("wasm.toml");

    let Ok(manifest_str) = std::fs::read_to_string(&manifest_path) else {
        return Ok(None);
    };
    let Ok(manifest) = toml::from_str::<component_manifest::Manifest>(&manifest_str) else {
        return Ok(None);
    };

    // Check if the input matches a manifest component key
    if !manifest.dependencies.components.contains_key(input) {
        return Ok(None);
    }

    let Ok(lockfile_str) = std::fs::read_to_string(&lockfile_path) else {
        return Ok(None);
    };
    let Ok(lockfile) = toml::from_str::<component_manifest::Lockfile>(&lockfile_str) else {
        return Ok(None);
    };

    // Find the matching lockfile entry
    let package = lockfile
        .components
        .iter()
        .find(|p| p.name == input)
        .ok_or_else(|| RunError::NotInLockfile {
            name: input.to_string(),
        })?;

    // Reconstruct the vendor filename from lockfile data.  The on-disk
    // file is named after the `namespace:package@version` declared in
    // the WIT metadata (e.g. `yoshuawuyts-acp-3.0.0.wasm`).
    let filename = component_package_manager::manager::vendor_filename(
        &package.name,
        Some(package.version.as_str()),
    );

    let vendored_path = PathBuf::from("vendor/wasm").join(filename);
    if !vendored_path.exists() {
        return Err(RunError::VendoredFileMissing {
            path: vendored_path.display().to_string(),
            name: input.to_string(),
        }
        .into());
    }

    Ok(Some(vendored_path))
}

/// Load component bytes from the global cache for a `scope:component` key.
///
/// When online, resolves the manifest key through the known-package index
/// and pulls the latest version from the remote registry first, ensuring
/// the cache is up to date before running. When offline, falls back to
/// whatever copy is already present in the local cache.
async fn load_from_global_cache(input: &str, offline: bool) -> miette::Result<Vec<u8>> {
    let manager = if offline {
        Manager::open_offline().await
    } else {
        Manager::open().await
    }
    .map_err(crate::util::into_miette)?;

    // Refresh the known-package index so WIT-style name resolution can find
    // packages that haven't been installed locally yet. Failures here are
    // non-fatal — fall through to the local cache lookup below.
    if !offline {
        let _ = manager
            .sync_from_meta_registry(
                Manager::DEFAULT_REGISTRY_URL,
                Manager::DEFAULT_SYNC_INTERVAL,
                component_package_manager::manager::SyncPolicy::IfStale,
            )
            .await;
    }

    // Try resolving through the known-package index and pulling the latest
    // version from the remote registry. Falls back to fuzzy search when no
    // exact WIT-name match exists, then to the local cache as a last resort.
    if !offline && component_package_manager::manager::install::looks_like_wit_name(input) {
        if let Ok(reference) =
            component_package_manager::manager::install::resolve_wit_name(input, &manager).await
        {
            return fetch_oci_bytes(&reference, offline).await;
        }
        if let Some(reference) = fuzzy_resolve_from_registry(&manager, input).await? {
            return fetch_oci_bytes(&reference, offline).await;
        }
    }

    let pattern = strip_at_version(input).replace(':', "/");
    let suffix = format!("/{pattern}");

    let entries = manager.list_all().await.map_err(crate::util::into_miette)?;
    let entry = entries
        .iter()
        .find(|e| e.ref_repository == pattern || e.ref_repository.ends_with(&suffix))
        .ok_or_else(|| RunError::NotInGlobalCache {
            name: input.to_string(),
        })?;

    let wasm_layers = component_package_manager::oci::filter_wasm_layers(&entry.manifest.layers);
    let layer = wasm_layers.first().ok_or(RunError::NoWasmLayer)?;
    manager
        .get(&layer.digest)
        .await
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to read cached component for {}", layer.digest))
}

/// Search the known-package index for entries matching `input` and, if a
/// unique component matches, return its OCI reference. When multiple
/// candidates match, returns an error listing the alternatives so the user
/// can pick the right one.
async fn fuzzy_resolve_from_registry(
    manager: &Manager,
    input: &str,
) -> miette::Result<Option<component_package_manager::Reference>> {
    // Split `scope:name-prefix` so we can search by the name fragment and
    // filter by the namespace separately. The known-package index stores
    // `wit_namespace` (e.g. `ba`) and `wit_name` (e.g. `sample-wasi-http-rust`)
    // as distinct columns, so a `scope/name` substring search would never
    // match.
    let Some((scope, name_prefix)) = input.split_once(':') else {
        return Ok(None);
    };

    let matches = manager
        .search_packages(name_prefix, 0, 64)
        .await
        .map_err(crate::util::into_miette)?;

    let candidates: Vec<&component_package_manager::storage::KnownPackage> = matches
        .iter()
        .filter(|p| {
            let Some(ns) = p.wit_namespace.as_deref() else {
                return false;
            };
            let Some(name) = p.wit_name.as_deref() else {
                return false;
            };
            ns == scope && (name == name_prefix || name.starts_with(&format!("{name_prefix}-")))
        })
        .collect();

    match candidates.as_slice() {
        [] => Ok(None),
        [pkg] => {
            let reference_str = format!("{}/{}", pkg.registry, pkg.repository);
            let reference =
                component_package_manager::parse_reference(&reference_str).map_err(|e| {
                    miette::miette!("failed to build OCI reference for '{reference_str}': {e}")
                })?;
            Ok(Some(reference))
        }
        many => {
            let names: Vec<String> = many
                .iter()
                .filter_map(|p| {
                    let ns = p.wit_namespace.as_deref()?;
                    let name = p.wit_name.as_deref()?;
                    Some(format!("{ns}:{name}"))
                })
                .collect();
            Err(miette::miette!(
                help = format!(
                    "multiple packages match '{input}': {}. Specify the full name.",
                    names.join(", ")
                ),
                "ambiguous package name '{input}'"
            ))
        }
    }
}

/// Fetch component bytes from an OCI registry.
async fn fetch_oci_bytes(
    oci_ref: &component_package_manager::Reference,
    offline: bool,
) -> miette::Result<Vec<u8>> {
    let manager = if offline {
        Manager::open_offline().await
    } else {
        Manager::open().await
    }
    .map_err(crate::util::into_miette)?;
    let pull_result = manager
        .pull(oci_ref.clone())
        .await
        .map_err(crate::util::into_miette)?;
    let manifest = pull_result.manifest.as_ref().ok_or(RunError::NoManifest)?;
    let wasm_layers = component_package_manager::oci::filter_wasm_layers(&manifest.layers);
    let layer = wasm_layers.first().ok_or(RunError::NoWasmLayer)?;
    let key = &layer.digest;
    manager
        .get(key)
        .await
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to read cached component for {key}"))
}

/// Strip an optional `@version` suffix from a `scope:name@version` input,
/// returning just the manifest-key portion (`scope:name`). Inputs without an
/// `@` are returned unchanged.
fn strip_at_version(input: &str) -> &str {
    input.split_once('@').map_or(input, |(name, _version)| name)
}

/// Check whether `input` looks like a manifest key (`scope:component`).
///
/// Manifest keys use `scope:component` syntax (e.g. `wasi:http`, `test:hello`)
/// without dots or slashes, which distinguishes them from OCI references
/// (e.g. `ghcr.io/user/repo:tag`). WIT-style names never contain dots or
/// slashes, so rejecting those characters safely separates manifest keys
/// from OCI references.
///
/// Callers that accept an optional `@version` suffix (e.g.
/// `yoshuawuyts:wordmark@2.0.6`) must strip the suffix before invoking this
/// check, since the version may legitimately contain `.` characters.
fn looks_like_manifest_key(input: &str) -> bool {
    let Some((scope, component)) = input.split_once(':') else {
        return false;
    };
    !scope.is_empty() && !component.is_empty() && !input.contains('/') && !input.contains('.')
}

/// Auto-install a manifest key into the local project so that `component run`
/// can execute it directly without requiring an explicit `component install` step.
///
/// Creates `wasm.toml`, `wasm.lock.toml`, and the standard vendor directories
/// when they are not already present, then delegates to the install command
/// to fetch the component, vendor it, and update the manifest + lockfile.
async fn auto_install(input: &str, offline: bool) -> miette::Result<()> {
    use miette::{IntoDiagnostic, WrapErr};

    // Ensure the minimal project skeleton exists so that `install` can
    // succeed. This creates the manifest, lockfile, and vendor directories
    // needed here; it does not attempt to fully replicate `component init`.
    tokio::fs::create_dir_all("vendor/wit")
        .await
        .into_diagnostic()
        .wrap_err("failed to create vendor/wit directory")?;
    tokio::fs::create_dir_all("vendor/wasm")
        .await
        .into_diagnostic()
        .wrap_err("failed to create vendor/wasm directory")?;

    let manifest_path = std::path::Path::new("wasm.toml");
    if !manifest_path.exists() {
        let manifest = component_manifest::Manifest::default();
        let manifest_str = toml::to_string_pretty(&manifest).into_diagnostic()?;
        tokio::fs::write(manifest_path, manifest_str.as_bytes())
            .await
            .into_diagnostic()
            .wrap_err("failed to write wasm.toml")?;
    }

    let lockfile_path = std::path::Path::new("wasm.lock.toml");
    if !lockfile_path.exists() {
        let lockfile = component_manifest::Lockfile::default();
        crate::util::write_lock_file(lockfile_path, &lockfile)
            .await
            .into_diagnostic()
            .wrap_err("failed to write wasm.lock.toml")?;
    }

    // Delegate the actual fetch + vendor + manifest update to the install
    // command. This keeps the auto-install behavior identical to running
    // `component install <input>` directly.
    let opts = crate::install::Opts::with_inputs(vec![input.to_string()]);
    opts.run(offline).await
}

/// Check whether a component exports `wasi:cli/run`, which is the
/// canonical hint that it targets `wasi:cli/command`.
///
/// Mirrors [`http::exports_http_incoming_handler`]; only top-level
/// component exports are considered.
// r[impl run.library-detection]
fn exports_cli_run(bytes: &[u8]) -> bool {
    let parser = Parser::new(0);
    let mut depth: u32 = 0;
    for payload in parser.parse_all(bytes) {
        let Ok(payload) = payload else { continue };
        match payload {
            Payload::Version { .. } => {
                depth += 1;
            }
            Payload::End(_) => {
                depth = depth.saturating_sub(1);
            }
            Payload::ComponentExportSection(reader) if depth == 1 => {
                for export in reader.into_iter().flatten() {
                    if export.name.name.starts_with("wasi:cli/run") {
                        return true;
                    }
                }
            }
            _ => {}
        }
    }
    false
}

/// Library-style dispatch: extract the component's WIT surface,
/// build a dynamic clap CLI, parse the user's trailing args, invoke
/// the matching function, and render the result.
// r[impl run.library-dispatch]
// r[impl run.library-help]
async fn run_library_component(
    bytes: &[u8],
    permissions: &component_manifest::ResolvedPermissions,
    extra: &[String],
) -> miette::Result<()> {
    // 1. Extract the dispatch surface.
    let surface = match extract_library_surface(bytes) {
        Ok(s) => s,
        // r[impl run.library-resources-rejected]
        Err(LibraryExtractError::Resource { name }) => {
            return Err(miette::miette!(
                help = "library-style invocation does not support resources; \
                        use a CLI or HTTP component instead",
                "component exports the resource `{name}`, which is not supported by `component run`"
            ));
        }
        // r[impl run.library-resources-rejected]
        // Every export was skipped because it uses an unsupported type
        // (resources, streams, futures, …), so there is nothing to invoke.
        Err(LibraryExtractError::NoInvocableFunctions { reasons }) => {
            return Err(miette::miette!(
                help = "library-style invocation does not support these types; \
                        use a CLI or HTTP component instead",
                "no invocable functions: every export uses an unsupported type ({reasons})"
            ));
        }
        Err(e) => return Err(miette::miette!("{e}")),
    };

    // 2. Build the dynamic clap CLI and parse the user's args.
    let cmd = build_clap(&surface, "component run").map_err(|e| miette::miette!("{e}"))?;
    let cli_args: Vec<&str> = std::iter::once("component run")
        .chain(extra.iter().map(String::as_str))
        .collect();
    let matches = match cmd.try_get_matches_from(cli_args) {
        Ok(m) => m,
        Err(e) => {
            // Clap prints its own formatted help/error for usage errors.
            // Use clap's exit-code mapping so missing-arg → 2 and
            // --help → 0.
            e.exit();
        }
    };
    let invocation = parse_invocation(&matches, &surface).map_err(|e| miette::miette!("{e}"))?;

    // 3. Invoke off-thread (sync wasmtime).
    let bytes_owned = bytes.to_vec();
    let permissions_owned = permissions.clone();
    let interface = invocation.path.interface.clone();
    let func = invocation.path.func.clone();
    let func_args = invocation.args;
    let expected_results = invocation.expected_results;
    let results = tokio::task::spawn_blocking(move || {
        component_cli_internal_run::execute_library_function(
            &bytes_owned,
            &permissions_owned,
            interface.as_deref(),
            &func,
            &func_args,
        )
    })
    .await
    .into_diagnostic()
    .wrap_err("runtime task panicked")??;

    // Sanity-check that wasmtime returned the number of values the
    // WIT signature declared. A mismatch would indicate either a
    // wasmtime/wit-parser drift or a corrupt component.
    if results.len() != expected_results.len() {
        return Err(miette::miette!(
            "result count mismatch: WIT declares {} value(s), runtime returned {}",
            expected_results.len(),
            results.len()
        ));
    }

    // 4. Render results to stdout/stderr and propagate exit code.
    let stdout = std::io::stdout();
    let stderr = std::io::stderr();
    let mut stdout_lock = stdout.lock();
    let mut stderr_lock = stderr.lock();
    let outcome = print_results(&results, &mut stdout_lock, &mut stderr_lock)
        .into_diagnostic()
        .wrap_err("rendering results")?;
    if outcome.exit_code != 0 {
        std::process::exit(outcome.exit_code);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{looks_like_manifest_key, strip_at_version};

    #[test]
    fn manifest_key_basic() {
        assert!(looks_like_manifest_key("wasi:http"));
        assert!(looks_like_manifest_key("yoshuawuyts:wordmark"));
    }

    #[test]
    fn manifest_key_rejects_oci_references() {
        assert!(!looks_like_manifest_key("ghcr.io/user/repo:tag"));
        assert!(!looks_like_manifest_key("docker.io/library/nginx:latest"));
    }

    #[test]
    fn strip_at_version_no_at_sign_is_passthrough() {
        assert_eq!(
            strip_at_version("yoshuawuyts:wordmark"),
            "yoshuawuyts:wordmark"
        );
    }

    /// A `scope:name@version` input must be stripped of its `@version` suffix
    /// before being checked, since versions may legitimately contain `.`
    /// characters that would otherwise be rejected.
    #[test]
    fn strip_at_version_then_manifest_key_check() {
        let stripped = strip_at_version("yoshuawuyts:wordmark@2.0.6");
        assert_eq!(stripped, "yoshuawuyts:wordmark");
        assert!(looks_like_manifest_key(stripped));
    }
}
