//! `component registry publish` subcommand.
//!
//! Reads the project's `wasm.toml` `[package]` section, checks whether the
//! package is already in the component registry, and—when it is not—opens a
//! prefilled "Registry entry" issue on the registry's GitHub repository so a
//! maintainer's automation can turn it into a pull request.
//!
//! This only reads the local package index and constructs a URL (optionally
//! launching the system browser); it never publishes artifacts itself.

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::fmt::Write as _;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use component_manifest::Manifest;
use component_package_manager::manager::Manager;

/// Default GitHub repository hosting the component registry.
///
/// This mirrors the manager's compile-time default registry: the registry's
/// issue forms live in this repository.
const DEFAULT_REGISTRY_REPO: &str = "yoshuawuyts/component-registry";

/// File name of the registry-entry issue form template.
const ISSUE_TEMPLATE: &str = "registry-entry.yml";

/// Open a prefilled registry-entry issue from a project's `wasm.toml`.
///
/// Reads the `[package]` section of the manifest, and—unless the package is
/// already present in the registry—opens the registry's issue form prefilled
/// with the package's namespace, name, kind, repository, and registry base so
/// the entry can be reviewed and merged.
#[derive(clap::Args)]
pub(crate) struct PublishOpts {
    /// Path to the project directory containing `wasm.toml`. Defaults to the
    /// current directory.
    #[arg(long, default_value = ".")]
    manifest_path: PathBuf,

    /// GitHub repository hosting the registry, as `owner/name`.
    #[arg(long, default_value = DEFAULT_REGISTRY_REPO)]
    repo: String,

    /// Do not launch a browser; the issue URL is always printed to stdout.
    #[arg(long)]
    no_open: bool,
}

impl PublishOpts {
    pub(crate) async fn run(self, store: &Manager) -> Result<()> {
        let entry = self.load_entry()?;

        if package_already_registered(store, &entry).await? {
            println!(
                "{}:{} is already in the registry; nothing to publish.",
                entry.namespace, entry.package
            );
            return Ok(());
        }

        let url = build_issue_url(&self.repo, &entry);
        println!("{url}");

        if !self.no_open
            && let Err(err) = open_in_browser(&url)
        {
            eprintln!("Could not open a browser ({err}); open the URL above manually.");
        }
        Ok(())
    }

    /// Read and validate the `[package]` section into a [`RegistryEntry`].
    fn load_entry(&self) -> Result<RegistryEntry> {
        validate_repo(&self.repo)?;

        let manifest_file = self.manifest_path.join("wasm.toml");
        let text = std::fs::read_to_string(&manifest_file)
            .with_context(|| format!("failed to read `{}`", manifest_file.display()))?;
        let manifest: Manifest = toml::from_str(&text)
            .with_context(|| format!("failed to parse `{}`", manifest_file.display()))?;

        let package = manifest.package.with_context(|| {
            format!(
                "`{}` has no `[package]` section; add one (name, kind, \
                 version, registry) before publishing to the registry",
                manifest_file.display()
            )
        })?;
        package.validate()?;

        RegistryEntry::from_package(&package)
    }
}

/// The registry-entry fields derived from a manifest's `[package]` section.
struct RegistryEntry {
    /// WIT namespace (e.g. `wasi`).
    namespace: String,
    /// WIT package name (e.g. `http`).
    package: String,
    /// `"component"` or `"interface"`.
    kind: &'static str,
    /// Catalog path within the namespace's registry (e.g. `wasi/http`).
    repository: String,
    /// Namespace's OCI registry base (e.g. `ghcr.io/webassembly`).
    registry: String,
}

impl RegistryEntry {
    fn from_package(package: &component_manifest::Package) -> Result<Self> {
        let (namespace, name) = parse_wit_name(&package.name)?;
        let (registry, repository) = split_registry_ref(&package.registry)?;

        Ok(Self {
            namespace: namespace.to_string(),
            package: name.to_string(),
            kind: package.kind.as_str(),
            repository,
            registry,
        })
    }
}

/// Return whether the package is already present in the local registry index.
///
/// Uses an exact `(wit_namespace, wit_name)` identity match. A lookup error
/// (e.g. an unsynced or offline index) is treated as "unknown": it is
/// reported and the caller proceeds to open the issue rather than silently
/// suppressing it.
async fn package_already_registered(store: &Manager, entry: &RegistryEntry) -> Result<bool> {
    let wit_name = format!("{}:{}", entry.namespace, entry.package);
    match store.find_known_package_by_wit_name(&wit_name).await {
        Ok(Some(found)) => Ok(
            found.wit_namespace.as_deref() == Some(entry.namespace.as_str())
                && found.wit_name.as_deref() == Some(entry.package.as_str()),
        ),
        Ok(None) => Ok(false),
        Err(err) => {
            eprintln!(
                "warning: could not check the registry index ({err}); \
                 proceeding to open an issue."
            );
            Ok(false)
        }
    }
}

/// Split a WIT-style `namespace:package` name into its two parts.
///
/// Rejects versions (`@`), path separators (`/`), and anything that is not
/// exactly one non-empty namespace and one non-empty package.
fn parse_wit_name(name: &str) -> Result<(&str, &str)> {
    let Some((namespace, package)) = name.split_once(':') else {
        bail!(
            "`[package].name` '{name}' is not a WIT-style name; expected \
             `namespace:package` (e.g., `wasi:http`)"
        );
    };
    if namespace.is_empty() || package.is_empty() {
        bail!("`[package].name` '{name}' must have a non-empty namespace and package");
    }
    if package.contains(':') {
        bail!("`[package].name` '{name}' has too many ':' separators");
    }
    if name.contains('@') {
        bail!("`[package].name` '{name}' must not include a version");
    }
    if namespace.contains('/') || package.contains('/') {
        bail!("`[package].name` '{name}' must not contain '/'");
    }
    Ok((namespace, package))
}

/// Split the manifest's full OCI `registry` reference into the registry
/// schema's two parts: the namespace's registry base (the issue form's
/// `registry` field, e.g. `ghcr.io/webassembly`) and the catalog path
/// (the issue form's `repository` field, e.g. `wasi/http`).
///
/// The base is taken as the first two `/`-separated segments (host plus
/// namespace org) and the repository is everything after. This matches the
/// `<host>/<org>` + `<repository>` convention used by every existing
/// `registry/<namespace>.toml`. The result only ever prefills a
/// human-reviewed issue form, so the split does not need to be authoritative.
///
/// Mirrors `REGISTRY_RE`/`REPO_RE` in `.github/scripts/registry-entry.mjs`
/// (the base allows `:` for a host `:port`; the repository does not).
fn split_registry_ref(reference: &str) -> Result<(String, String)> {
    if reference.is_empty() {
        bail!("`[package].registry` must not be empty");
    }
    if reference.contains('@') {
        bail!("`[package].registry` '{reference}' must not pin a digest");
    }
    if reference.starts_with('/') || reference.ends_with('/') {
        bail!("`[package].registry` '{reference}' must not start or end with '/'");
    }
    let segments: Vec<&str> = reference.split('/').collect();
    if segments.iter().any(|s| s.is_empty()) {
        bail!("`[package].registry` '{reference}' must not contain empty path segments");
    }
    let [host, org, rest @ ..] = segments.as_slice() else {
        bail!(
            "`[package].registry` '{reference}' must include a host, namespace, \
             and at least one repository segment (e.g. `ghcr.io/my-org/my-package`)"
        );
    };
    if rest.is_empty() {
        bail!(
            "`[package].registry` '{reference}' must include a host, namespace, \
             and at least one repository segment (e.g. `ghcr.io/my-org/my-package`)"
        );
    }
    // Only the host (first segment) may carry a `:port`; a `:` elsewhere is a tag.
    if org.contains(':') || rest.iter().any(|s| s.contains(':')) {
        bail!("`[package].registry` '{reference}' must not include a tag");
    }

    let registry = format!("{host}/{org}");
    let repository = rest.join("/");
    if !is_valid_registry(&registry) {
        bail!(
            "`[package].registry` base '{registry}' must start with an alphanumeric \
             and contain only ASCII letters, digits, `.`, `_`, `-`, `:`, and `/`"
        );
    }
    if !is_valid_repository_path(&repository) {
        bail!(
            "`[package].registry` repository path '{repository}' must start with an \
             alphanumeric and contain only ASCII letters, digits, `.`, `_`, `-`, and `/`"
        );
    }
    Ok((registry, repository))
}

fn is_valid_repository_path(repository: &str) -> bool {
    repository
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphanumeric())
        && repository
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/'))
}

fn is_valid_registry(registry: &str) -> bool {
    registry
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphanumeric())
        && registry
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | ':' | '/'))
}

/// Validate a `owner/name` GitHub repository slug used in the URL path.
fn validate_repo(repo: &str) -> Result<()> {
    let Some((owner, name)) = repo.split_once('/') else {
        bail!(
            "--repo '{repo}' must be in `owner/name` form (e.g., `yoshuawuyts/component-registry`)"
        );
    };
    if owner.is_empty() || name.is_empty() || name.contains('/') {
        bail!(
            "--repo '{repo}' must be in `owner/name` form (e.g., `yoshuawuyts/component-registry`)"
        );
    }
    if !valid_repo_segment(owner) || !valid_repo_segment(name) {
        bail!("--repo '{repo}' contains characters that are not valid in a GitHub repository slug");
    }
    Ok(())
}

/// Whether `segment` is a valid GitHub `owner` or repository-name path
/// segment: an allowlist of ASCII alphanumerics plus `-`, `_`, and `.`.
fn valid_repo_segment(segment: &str) -> bool {
    segment
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// Build the prefilled GitHub issue-form URL.
///
/// Each query key matches a field `id` in `registry-entry.yml`, so GitHub
/// populates the corresponding form fields. Query values are percent-encoded.
/// The `registry` base is always included: the automation ignores it when the
/// namespace already exists and uses it when creating a new namespace.
#[must_use]
fn build_issue_url(repo: &str, entry: &RegistryEntry) -> String {
    let mut url = format!("https://github.com/{repo}/issues/new?template={ISSUE_TEMPLATE}");
    write!(url, "&kind={}", encode(entry.kind)).expect("writing to a String cannot fail");
    write!(url, "&namespace={}", encode(&entry.namespace))
        .expect("writing to a String cannot fail");
    write!(url, "&package={}", encode(&entry.package)).expect("writing to a String cannot fail");
    write!(url, "&repository={}", encode(&entry.repository))
        .expect("writing to a String cannot fail");
    write!(url, "&registry={}", encode(&entry.registry)).expect("writing to a String cannot fail");
    url
}

/// Percent-encode a query value, passing through only RFC 3986 unreserved
/// characters and encoding every other UTF-8 byte as `%XX`.
#[must_use]
fn encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for &byte in value.as_bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(byte as char);
        } else {
            write!(out, "%{byte:02X}").expect("writing to a String cannot fail");
        }
    }
    out
}

/// Launch the system browser for `url` using a platform-native handler.
///
/// Uses `Command` directly (never a shell) so URL metacharacters such as `&`
/// and `%` cannot be reinterpreted.
fn open_in_browser(url: &str) -> Result<()> {
    use std::process::Command;

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut c = Command::new("open");
        c.arg(url);
        c
    };

    #[cfg(target_os = "windows")]
    let mut command = {
        // Avoid `cmd /C start`, which reinterprets `&` and `%` in the URL.
        let mut c = Command::new("rundll32.exe");
        c.args(["url.dll,FileProtocolHandler", url]);
        c
    };

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let mut command = {
        let mut c = Command::new("xdg-open");
        c.arg(url);
        c
    };

    let status = command.status()?;
    if !status.success() {
        bail!("browser launcher exited with status {status}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use component_manifest::{Package, PackageKind};

    fn sample_package() -> Package {
        Package {
            name: "wasi:http".into(),
            version: "0.1.0".into(),
            registry: "ghcr.io/webassembly/wasi/http".into(),
            kind: PackageKind::Interface,
            file: None,
            wit: None,
            description: None,
            source: None,
            homepage: None,
            documentation: None,
            license: None,
            authors: vec![],
        }
    }

    #[test]
    fn entry_from_package_derives_fields() {
        let entry = RegistryEntry::from_package(&sample_package()).unwrap();
        assert_eq!(entry.namespace, "wasi");
        assert_eq!(entry.package, "http");
        assert_eq!(entry.kind, "interface");
        assert_eq!(entry.repository, "wasi/http");
        assert_eq!(entry.registry, "ghcr.io/webassembly");
    }

    #[test]
    fn entry_requires_registry() {
        let mut pkg = sample_package();
        pkg.registry = String::new();
        assert!(RegistryEntry::from_package(&pkg).is_err());
    }

    #[test]
    fn entry_rejects_registry_without_repository_segment() {
        let mut pkg = sample_package();
        pkg.registry = "ghcr.io/webassembly".into();
        assert!(RegistryEntry::from_package(&pkg).is_err());
    }

    #[test]
    fn entry_rejects_registry_with_digest() {
        let mut pkg = sample_package();
        pkg.registry = "ghcr.io/webassembly/wasi/http@sha256:abc".into();
        assert!(RegistryEntry::from_package(&pkg).is_err());
    }

    #[test]
    fn entry_rejects_tagged_registry() {
        let mut pkg = sample_package();
        pkg.registry = "ghcr.io/webassembly/wasi/http:0.2.0".into();
        assert!(RegistryEntry::from_package(&pkg).is_err());
    }

    #[test]
    fn parses_wit_name() {
        assert_eq!(parse_wit_name("wasi:http").unwrap(), ("wasi", "http"));
    }

    #[test]
    fn rejects_invalid_wit_names() {
        assert!(parse_wit_name("nocolon").is_err());
        assert!(parse_wit_name(":http").is_err());
        assert!(parse_wit_name("wasi:").is_err());
        assert!(parse_wit_name("wasi:http@0.2.0").is_err());
        assert!(parse_wit_name("wasi:comp/onents").is_err());
    }

    #[test]
    fn splits_registry_ref() {
        // Splits host+org base from the catalog path, matching the
        // conventions used by every existing registry/<namespace>.toml.
        assert_eq!(
            split_registry_ref("ghcr.io/webassembly/wasi/http").unwrap(),
            ("ghcr.io/webassembly".to_string(), "wasi/http".to_string())
        );
        assert_eq!(
            split_registry_ref("ghcr.io/microsoft/fetch-rs").unwrap(),
            ("ghcr.io/microsoft".to_string(), "fetch-rs".to_string())
        );
        assert_eq!(
            split_registry_ref("ghcr.io/fermyon/wasm-pkg/fermyon/hello-world").unwrap(),
            (
                "ghcr.io/fermyon".to_string(),
                "wasm-pkg/fermyon/hello-world".to_string()
            )
        );
        assert_eq!(
            split_registry_ref("localhost:5000/team/my-pkg").unwrap(),
            ("localhost:5000/team".to_string(), "my-pkg".to_string())
        );

        // Rejections.
        assert!(split_registry_ref("").is_err());
        assert!(split_registry_ref("ghcr.io/webassembly").is_err()); // no repo segment
        assert!(split_registry_ref("/leading/foo/bar").is_err());
        assert!(split_registry_ref("ghcr.io/webassembly/wasi/http/").is_err());
        assert!(split_registry_ref("ghcr.io//wasi/http").is_err());
        assert!(split_registry_ref("ghcr.io/webassembly/wasi/http:0.2.0").is_err()); // tag
        assert!(split_registry_ref("ghcr.io/webassembly/wasi/http@sha256:abc").is_err());
        assert!(split_registry_ref("ghcr.io/web assembly/wasi/http").is_err());
        assert!(split_registry_ref("ghcr.io/webassembly/wasi/ht%2Ftp").is_err());
        assert!(split_registry_ref("ghcr.io/webassembly/wasi/ht\ttp").is_err());
        assert!(split_registry_ref("ghcr.io/webassembly/wasi/caf\u{e9}").is_err());
    }

    #[test]
    fn validates_repo() {
        assert!(validate_repo("yoshuawuyts/component-registry").is_ok());
        assert!(validate_repo("owner-1/repo_2.name").is_ok());
        assert!(validate_repo("o/%2Fetc").is_err());
        assert!(validate_repo("owner/re%2Fpo").is_err());
        assert!(validate_repo("owner/re po").is_err());
        assert!(validate_repo("owner/re\tpo").is_err());
        assert!(validate_repo("owner/re\npo").is_err());
        assert!(validate_repo("noslash").is_err());
        assert!(validate_repo("/name").is_err());
        assert!(validate_repo("owner/").is_err());
        assert!(validate_repo("a/b/c").is_err());
    }

    #[test]
    fn encodes_query_values() {
        assert_eq!(encode("components/wordmark"), "components%2Fwordmark");
        assert_eq!(encode("ghcr.io/my-org"), "ghcr.io%2Fmy-org");
        assert_eq!(encode("plain-name_1.0"), "plain-name_1.0");
    }

    #[test]
    fn builds_url() {
        let entry = RegistryEntry::from_package(&sample_package()).unwrap();
        let url = build_issue_url("yoshuawuyts/component-registry", &entry);
        assert_eq!(
            url,
            "https://github.com/yoshuawuyts/component-registry/issues/new\
             ?template=registry-entry.yml&kind=interface&namespace=wasi\
             &package=http&repository=wasi%2Fhttp&registry=ghcr.io%2Fwebassembly"
        );
    }
}
