//! Pure logic extracted from the `Manager` and `Store` implementations.
//!
//! These functions contain no IO and can be unit-tested in isolation.

use std::collections::HashSet;

/// Compute the vendor filename for a cached layer from a WIT-style
/// `namespace:package` name and an optional version.
///
/// The filename has the form `<namespace>-<package>-<version>.wasm`
/// (or `<namespace>-<package>.wasm` when no version is known).  Any
/// `:` or `/` characters in the name are replaced with `-` so the
/// result is a single path segment.  A trailing `@version` suffix in
/// `name` (as produced by `wit_parser::PackageName`'s `Display` impl)
/// takes precedence over the explicit `version` argument.
///
/// The content digest is intentionally **not** included — it lives in
/// the lockfile, so the on-disk filenames stay short and readable.
///
/// # Example
///
/// ```
/// use component_package_manager::manager::vendor_filename;
///
/// let name = vendor_filename("yoshuawuyts:acp", Some("3.0.0"));
/// assert_eq!(name, "yoshuawuyts-acp-3.0.0.wasm");
/// ```
// r[impl manager.vendor-filename.basic]
// r[impl manager.vendor-filename.no-version]
// r[impl manager.vendor-filename.embedded-version]
#[must_use]
pub fn vendor_filename(name: &str, version: Option<&str>) -> String {
    // `wit_parser::PackageName`'s Display impl appends `@<version>` when a
    // version is present.  Prefer that embedded version over the explicit
    // argument so callers can pass the WIT package name verbatim.
    let (bare_name, embedded_version) = match name.rsplit_once('@') {
        Some((n, v)) => (n, Some(v)),
        None => (name, None),
    };
    let version = embedded_version.or(version).filter(|v| !v.is_empty());
    let base: String = bare_name
        .chars()
        .map(|c| if matches!(c, ':' | '/') { '-' } else { c })
        .collect();
    match version {
        Some(v) => format!("{base}-{v}.wasm"),
        None => format!("{base}.wasm"),
    }
}

/// Determine whether a sync from the meta-registry should proceed.
///
/// Returns `true` when enough time has elapsed since `last_synced_epoch`,
/// or when the last-sync timestamp is unknown.
///
/// # Example
///
/// ```
/// use component_package_manager::manager::should_sync;
///
/// // No previous sync — always sync.
/// assert!(should_sync(None, 3600, 1000));
///
/// // Last synced long ago — sync again.
/// assert!(should_sync(Some(1000), 3600, 5000));
///
/// // Recently synced — skip.
/// assert!(!should_sync(Some(1000), 3600, 2000));
/// ```
// r[impl manager.sync.fresh]
// r[impl manager.sync.stale]
// r[impl manager.sync.no-previous]
#[must_use]
pub fn should_sync(last_synced_epoch: Option<i64>, sync_interval: u64, now_epoch: i64) -> bool {
    match last_synced_epoch {
        // `sync_interval` is a config-supplied second count; saturating to
        // `i64::MAX` only makes the threshold unreachable, never syncing early.
        Some(last) => now_epoch - last >= i64::try_from(sync_interval).unwrap_or(i64::MAX),
        None => true,
    }
}

/// Sanitize a string into a valid WIT identifier.
///
/// WIT identifiers must match `[a-z][a-z0-9]*(-[a-z][a-z0-9]*)*`.
/// Returns `None` if the input cannot be sanitized into a valid identifier
/// (e.g. it contains only digits or special characters).
///
/// # Example
///
/// ```
/// use component_package_manager::manager::sanitize_to_wit_identifier;
///
/// assert_eq!(sanitize_to_wit_identifier("My_Component"), Some("my-component".to_string()));
/// assert_eq!(sanitize_to_wit_identifier("123fetch"), Some("fetch".to_string()));
/// assert_eq!(sanitize_to_wit_identifier("!!!"), None);
/// ```
// r[impl manager.name.sanitize.valid]
// r[impl manager.name.sanitize.uppercase]
// r[impl manager.name.sanitize.underscores]
// r[impl manager.name.sanitize.leading-digits]
#[must_use]
pub fn sanitize_to_wit_identifier(input: &str) -> Option<String> {
    // Lowercase and replace non-alphanumeric characters with hyphens.
    let sanitized: String = input
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();

    // Collapse consecutive hyphens, strip leading/trailing hyphens.
    let collapsed: String = sanitized
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");

    // Strip leading digits (WIT identifiers must start with [a-z]).
    let trimmed = collapsed.trim_start_matches(|c: char| c.is_ascii_digit());

    // Strip a possible leading hyphen left after digit removal.
    let trimmed = trimmed.strip_prefix('-').unwrap_or(trimmed);

    if trimmed.is_empty() {
        return None;
    }

    Some(trimmed.to_string())
}

/// Derive a human-friendly component name for use in `wasm.toml`.
///
/// Follows this priority chain:
/// 1. **WIT package name** — strip the `@version` suffix.
/// 2. **OCI `image.title`** — sanitized to a WIT-legal identifier.
/// 3. **Last segment of the repository path** — sanitized; used when no
///    collision with `existing_names`.
/// 4. **Full repository path** — with `/` replaced by `-`; used on collision.
///
/// # Example
///
/// ```
/// use std::collections::HashSet;
/// use component_package_manager::manager::derive_component_name;
///
/// let existing = HashSet::new();
/// let name = derive_component_name(
///     Some("wasi:http@0.2.10"),
///     None,
///     "webassembly/wasi-http",
///     &existing,
/// );
/// assert_eq!(name, "wasi:http");
/// ```
// r[impl manager.name.last-segment]
// r[impl manager.name.wit-package]
// r[impl manager.name.oci-title]
// r[impl manager.name.collision]
#[must_use]
pub fn derive_component_name<S: std::hash::BuildHasher>(
    package_name: Option<&str>,
    oci_title: Option<&str>,
    repository: &str,
    existing_names: &HashSet<String, S>,
) -> String {
    // 1. WIT package name (strip @version).
    if let Some(name) = package_name {
        return name.split('@').next().unwrap_or(name).to_string();
    }

    // 2. OCI image.title annotation.
    if let Some(title) = oci_title
        && let Some(sanitized) = sanitize_to_wit_identifier(title)
    {
        return sanitized;
    }

    // 3. Last segment of the repository path.
    let last_segment = repository.rsplit('/').next().unwrap_or(repository);
    if let Some(sanitized) = sanitize_to_wit_identifier(last_segment)
        && !existing_names.contains(&sanitized)
    {
        return sanitized;
    }

    // 4. Full repository path (on collision or if last segment fails).
    sanitize_to_wit_identifier(&repository.replace('/', "-"))
        .unwrap_or_else(|| repository.to_string())
}

/// Try to parse a tag as a semantic version.
///
/// Reverses the `+`->`_` OCI-tag mapping that [`crate::publish::oci_tag`]
/// applies on publish. OCI tags cannot contain `+`, so a SemVer with build
/// metadata such as `0.1.0+2022-11-15` is stored under the tag
/// `0.1.0_2022-11-15`. When we read tags back from a registry during indexing
/// we decode the first `_` to `+` so they round-trip and parse as SemVer,
/// instead of being discarded as non-semver (which skips the whole package).
pub(crate) fn parse_tag_as_semver(tag: &str) -> Option<semver::Version> {
    if let Ok(version) = semver::Version::parse(tag) {
        return Some(version);
    }
    // Reverse the `+`->`_` OCI-tag mapping (see `publish::oci_tag`). SemVer has
    // at most one `+` (all build metadata follows it and cannot itself contain
    // `_`), so `oci_tag` introduces at most one `_`; decode the first `_` back
    // to `+` and retry.
    if tag.contains('_') {
        return semver::Version::parse(&tag.replacen('_', "+", 1)).ok();
    }
    None
}

/// Pick the latest stable semver tag from a list of tags.
///
/// Filters out:
/// - Tags that are not valid semver versions
/// - Pre-release tags (e.g. `0.3.0-preview-2026-02-20`)
/// - The literal `latest` tag
/// - Hash-based tags (e.g. `sha256-abc123.sig`)
///
/// Returns the tag string for the highest remaining version, or `None` if
/// no stable semver tag is found.
///
/// # Example
///
/// ```
/// use component_package_manager::manager::pick_latest_stable_tag;
///
/// let tags = vec![
///     "0.2.0".into(),
///     "0.2.10".into(),
///     "0.3.0-preview-2026-02-20".into(),
///     "latest".into(),
///     "sha256-abc123.sig".into(),
/// ];
/// assert_eq!(pick_latest_stable_tag(&tags), Some("0.2.10".to_string()));
/// ```
#[must_use]
pub fn pick_latest_stable_tag(tags: &[String]) -> Option<String> {
    tags.iter()
        .filter(|t| *t != "latest" && !t.starts_with("sha256-"))
        .filter_map(|t| parse_tag_as_semver(t).map(|v| (t, v)))
        .filter(|(_, v)| v.pre.is_empty())
        .max_by(|(_, a), (_, b)| a.cmp(b))
        .map(|(t, _)| t.clone())
}

/// Filter tags for display in user-facing suggestions.
///
/// When `requested_version` is `None` (bare install), all pre-release tags
/// are excluded. When `requested_version` is `Some("0.3")`, pre-release
/// tags whose version starts with the same major.minor prefix (e.g.
/// `0.3.0-preview-2026-02-20`) are included.
///
/// In all cases, `latest`, hash-based tags, and non-semver tags are
/// excluded.
///
/// # Example
///
/// ```
/// use component_package_manager::manager::filter_tag_suggestions;
///
/// let tags = vec![
///     "0.2.0".into(),
///     "0.2.10".into(),
///     "0.3.0-preview-2026-02-20".into(),
///     "latest".into(),
///     "sha256-abc123.sig".into(),
/// ];
/// // No version prefix: skip pre-release
/// let suggestions = filter_tag_suggestions(&tags, None);
/// assert_eq!(suggestions, vec!["0.2.0", "0.2.10"]);
///
/// // With prefix "0.3": include matching pre-release
/// let suggestions = filter_tag_suggestions(&tags, Some("0.3"));
/// assert_eq!(suggestions, vec!["0.2.0", "0.2.10", "0.3.0-preview-2026-02-20"]);
/// ```
#[must_use]
pub fn filter_tag_suggestions(tags: &[String], requested_version: Option<&str>) -> Vec<String> {
    // Normalize the requested version by stripping an optional leading `v`.
    let normalized_req = requested_version.map(|v| v.strip_prefix('v').unwrap_or(v));

    tags.iter()
        .filter(|t| *t != "latest" && !t.starts_with("sha256-"))
        .filter(|t| {
            let Some(v) = parse_tag_as_semver(t) else {
                return false;
            };
            if v.pre.is_empty() {
                return true;
            }
            // Include pre-release tags only when the user's request shares
            // the same major.minor version. Parse the prefix to extract
            // numeric components for exact comparison.
            if let Some(prefix) = normalized_req {
                return prefix_matches_version(prefix, v.major, v.minor);
            }
            false
        })
        .cloned()
        .collect()
}

/// Check whether a user-supplied version prefix matches a given major.minor.
///
/// Parses the prefix to extract numeric major and (optional) minor
/// components, then compares them exactly against the provided values.
fn prefix_matches_version(prefix: &str, major: u64, minor: u64) -> bool {
    let mut parts = prefix.split('.');
    let Some(first) = parts.next().and_then(|s| s.parse::<u64>().ok()) else {
        return false;
    };
    if first != major {
        return false;
    }
    // If the prefix has a minor component, it must match exactly.
    // If not (e.g. just "0"), match any minor.
    match parts.next() {
        Some(s) => s.parse::<u64>().ok().is_some_and(|m| m == minor),
        None => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── vendor_filename ─────────────────────────────────────────────────

    // r[verify manager.vendor-filename.basic]
    #[test]
    fn vendor_filename_basic() {
        let name = vendor_filename("yoshuawuyts:acp", Some("3.0.0"));
        assert_eq!(name, "yoshuawuyts-acp-3.0.0.wasm");
    }

    // r[verify manager.vendor-filename.no-version]
    #[test]
    fn vendor_filename_no_version() {
        let name = vendor_filename("wasi:http", None);
        assert_eq!(name, "wasi-http.wasm");
    }

    // r[verify manager.vendor-filename.embedded-version]
    #[test]
    fn vendor_filename_embedded_version_wins() {
        // `wit_parser::PackageName` Display renders as `ns:name@version`.
        let name = vendor_filename("yoshuawuyts:acp@3.0.0", Some("ignored"));
        assert_eq!(name, "yoshuawuyts-acp-3.0.0.wasm");
    }

    #[test]
    fn vendor_filename_empty_version_falls_back() {
        // Empty version string is treated as "no version".
        let name = vendor_filename("wasi:http", Some(""));
        assert_eq!(name, "wasi-http.wasm");
    }

    #[test]
    fn vendor_filename_strips_slashes() {
        let name = vendor_filename("org:nested/component", Some("1.0.0"));
        assert_eq!(name, "org-nested-component-1.0.0.wasm");
    }

    // ── should_sync ─────────────────────────────────────────────────────

    // r[verify manager.sync.no-previous]
    #[test]
    fn should_sync_no_previous() {
        assert!(should_sync(None, 3600, 1000));
    }

    // r[verify manager.sync.stale]
    #[test]
    fn should_sync_stale() {
        assert!(should_sync(Some(1000), 3600, 5000));
    }

    // r[verify manager.sync.fresh]
    #[test]
    fn should_sync_fresh() {
        assert!(!should_sync(Some(1000), 3600, 2000));
    }

    #[test]
    fn should_sync_exact_boundary() {
        // Exactly at the interval boundary should trigger sync.
        assert!(should_sync(Some(0), 3600, 3600));
    }

    // ── sanitize_to_wit_identifier ──────────────────────────────────────

    // r[verify manager.name.sanitize.valid]
    #[test]
    fn sanitize_already_valid() {
        assert_eq!(
            sanitize_to_wit_identifier("fetch"),
            Some("fetch".to_string())
        );
    }

    // r[verify manager.name.sanitize.uppercase]
    #[test]
    fn sanitize_uppercase() {
        assert_eq!(
            sanitize_to_wit_identifier("Fetch"),
            Some("fetch".to_string())
        );
    }

    // r[verify manager.name.sanitize.underscores]
    #[test]
    fn sanitize_underscores() {
        assert_eq!(
            sanitize_to_wit_identifier("my_component"),
            Some("my-component".to_string())
        );
    }

    // r[verify manager.name.sanitize.leading-digits]
    #[test]
    fn sanitize_leading_digits() {
        assert_eq!(
            sanitize_to_wit_identifier("123fetch"),
            Some("fetch".to_string())
        );
    }

    #[test]
    fn sanitize_all_digits() {
        assert_eq!(sanitize_to_wit_identifier("12345"), None);
    }

    #[test]
    fn sanitize_empty_after_sanitization() {
        assert_eq!(sanitize_to_wit_identifier("!!!"), None);
    }

    #[test]
    fn sanitize_complex() {
        assert_eq!(
            sanitize_to_wit_identifier("My Cool_Fetch.Component"),
            Some("my-cool-fetch-component".to_string())
        );
    }

    // ── derive_component_name ───────────────────────────────────────────

    // r[verify manager.name.wit-package]
    #[test]
    fn derive_name_wit_package_name() {
        let existing = HashSet::new();
        let name = derive_component_name(
            Some("wasi:http@0.2.10"),
            None,
            "webassembly/wasi-http",
            &existing,
        );
        assert_eq!(name, "wasi:http");
    }

    // r[verify manager.name.oci-title]
    #[test]
    fn derive_name_oci_title() {
        let existing = HashSet::new();
        let name = derive_component_name(
            None,
            Some("My Fetch Component"),
            "yoshuawuyts/fetch",
            &existing,
        );
        assert_eq!(name, "my-fetch-component");
    }

    // r[verify manager.name.last-segment]
    #[test]
    fn derive_name_last_segment() {
        let existing = HashSet::new();
        let name = derive_component_name(None, None, "yoshuawuyts/fetch", &existing);
        assert_eq!(name, "fetch");
    }

    // r[verify manager.name.collision]
    #[test]
    fn derive_name_collision() {
        let mut existing = HashSet::new();
        existing.insert("fetch".to_string());
        let name = derive_component_name(None, None, "yoshuawuyts/fetch", &existing);
        assert_eq!(name, "yoshuawuyts-fetch");
    }

    #[test]
    fn derive_name_repo_with_underscores_dots() {
        let existing = HashSet::new();
        let name = derive_component_name(None, None, "my_org/my.component", &existing);
        assert_eq!(name, "my-component");
    }

    #[test]
    fn derive_name_repo_with_underscores_dots_collision() {
        let mut existing = HashSet::new();
        existing.insert("my-component".to_string());
        let name = derive_component_name(None, None, "my_org/my.component", &existing);
        assert_eq!(name, "my-org-my-component");
    }

    #[test]
    fn derive_name_oci_title_invalid_chars() {
        let existing = HashSet::new();
        let name = derive_component_name(None, Some("!!!"), "yoshuawuyts/fetch", &existing);
        // Title sanitizes to empty → falls through to last segment
        assert_eq!(name, "fetch");
    }

    #[test]
    fn derive_name_oci_title_sanitizes_to_empty() {
        let existing = HashSet::new();
        let name = derive_component_name(None, Some("12345"), "yoshuawuyts/fetch", &existing);
        // Title is all digits → sanitizes to None → falls through
        assert_eq!(name, "fetch");
    }

    // ── pick_latest_stable_tag ──────────────────────────────────────────

    #[test]
    fn pick_latest_stable_tag_basic() {
        let tags = vec![
            "0.2.0".into(),
            "0.2.10".into(),
            "0.3.0-preview-2026-02-20".into(),
            "latest".into(),
            "sha256-abc123.sig".into(),
        ];
        assert_eq!(pick_latest_stable_tag(&tags), Some("0.2.10".to_string()));
    }

    #[test]
    fn pick_latest_stable_tag_empty() {
        let tags: Vec<String> = vec![];
        assert_eq!(pick_latest_stable_tag(&tags), None);
    }

    #[test]
    fn pick_latest_stable_tag_only_prerelease() {
        let tags = vec![
            "0.3.0-preview-2026-02-20".into(),
            "latest".into(),
            "sha256-abc.sig".into(),
        ];
        assert_eq!(pick_latest_stable_tag(&tags), None);
    }

    #[test]
    fn pick_latest_stable_tag_multiple_stable() {
        let tags = vec![
            "1.0.0".into(),
            "1.1.0".into(),
            "2.0.0".into(),
            "0.9.0".into(),
        ];
        assert_eq!(pick_latest_stable_tag(&tags), Some("2.0.0".to_string()));
    }

    #[test]
    fn pick_latest_stable_tag_ignores_non_semver() {
        let tags = vec!["not-a-version".into(), "v1".into(), "1.0.0".into()];
        assert_eq!(pick_latest_stable_tag(&tags), Some("1.0.0".to_string()));
    }

    #[test]
    fn pick_latest_stable_tag_ignores_v_prefixed() {
        // `v`-prefixed tags are not valid SemVer and are not accepted.
        let tags = vec![
            "v0.2.0".into(),
            "v0.2.10".into(),
            "v0.3.0-preview-2026-02-20".into(),
            "latest".into(),
        ];
        assert_eq!(pick_latest_stable_tag(&tags), None);
    }

    #[test]
    fn pick_latest_stable_tag_mixed_v_and_bare() {
        // Only the bare SemVer tag is considered; `v`-prefixed tags are ignored.
        let tags = vec!["v1.0.0".into(), "2.0.0".into(), "v0.9.0".into()];
        assert_eq!(pick_latest_stable_tag(&tags), Some("2.0.0".to_string()));
    }

    // ── filter_tag_suggestions ──────────────────────────────────────────

    #[test]
    fn filter_tag_suggestions_no_prefix() {
        let tags = vec![
            "0.2.0".into(),
            "0.2.10".into(),
            "0.3.0-preview-2026-02-20".into(),
            "latest".into(),
            "sha256-abc123.sig".into(),
        ];
        let suggestions = filter_tag_suggestions(&tags, None);
        assert_eq!(suggestions, vec!["0.2.0", "0.2.10"]);
    }

    #[test]
    fn filter_tag_suggestions_with_matching_prefix() {
        let tags = vec![
            "0.2.0".into(),
            "0.2.10".into(),
            "0.3.0-preview-2026-02-20".into(),
            "latest".into(),
        ];
        let suggestions = filter_tag_suggestions(&tags, Some("0.3"));
        assert_eq!(
            suggestions,
            vec!["0.2.0", "0.2.10", "0.3.0-preview-2026-02-20"]
        );
    }

    #[test]
    fn filter_tag_suggestions_with_non_matching_prefix() {
        let tags = vec![
            "0.2.0".into(),
            "0.3.0-preview-2026-02-20".into(),
            "1.0.0-rc1".into(),
        ];
        let suggestions = filter_tag_suggestions(&tags, Some("0.2"));
        // Only stable tags and pre-release matching 0.2.* prefix
        assert_eq!(suggestions, vec!["0.2.0"]);
    }

    #[test]
    fn filter_tag_suggestions_empty() {
        let tags: Vec<String> = vec![];
        let suggestions = filter_tag_suggestions(&tags, None);
        assert!(suggestions.is_empty());
    }

    #[test]
    fn filter_tag_suggestions_all_prerelease_no_prefix() {
        let tags = vec!["0.3.0-preview-2026-02-20".into(), "1.0.0-alpha.1".into()];
        let suggestions = filter_tag_suggestions(&tags, None);
        assert!(suggestions.is_empty());
    }

    #[test]
    fn filter_tag_suggestions_does_not_match_different_minor() {
        // "0.3" should NOT match a tag with minor=30 (e.g. "0.30.0-beta")
        let tags = vec!["0.30.0-beta".into(), "0.3.0-preview".into(), "1.0.0".into()];
        let suggestions = filter_tag_suggestions(&tags, Some("0.3"));
        assert_eq!(suggestions, vec!["0.3.0-preview", "1.0.0"]);
    }

    #[test]
    fn filter_tag_suggestions_major_only_prefix() {
        // "1" should match any pre-release tag with major=1
        let tags = vec![
            "1.0.0-rc1".into(),
            "1.5.0-beta".into(),
            "2.0.0-alpha".into(),
        ];
        let suggestions = filter_tag_suggestions(&tags, Some("1"));
        assert_eq!(suggestions, vec!["1.0.0-rc1", "1.5.0-beta"]);
    }

    #[test]
    fn filter_tag_suggestions_excludes_v_prefixed_tags() {
        // `v`-prefixed tags are not valid SemVer and are excluded.
        let tags = vec![
            "v0.2.0".into(),
            "v0.2.10".into(),
            "v0.3.0-preview".into(),
            "latest".into(),
        ];
        let suggestions = filter_tag_suggestions(&tags, None);
        assert!(suggestions.is_empty());
    }

    #[test]
    fn filter_tag_suggestions_v_prefixed_request() {
        let tags = vec!["0.2.0".into(), "0.3.0-preview".into(), "1.0.0-rc1".into()];
        // "v0.3" request should match 0.3.* pre-release tags
        let suggestions = filter_tag_suggestions(&tags, Some("v0.3"));
        assert_eq!(suggestions, vec!["0.2.0", "0.3.0-preview"]);
    }

    // ── parse_tag_as_semver ─────────────────────────────────────────────

    #[test]
    fn parse_tag_as_semver_decodes_underscore_build_metadata() {
        // OCI tags cannot contain `+`, so `publish::oci_tag` maps SemVer build
        // metadata `0.1.0+2022-11-15` onto `0.1.0_2022-11-15`. The read path
        // must reverse this so the tag round-trips as SemVer.
        let cases = [
            ("0.1.0_2022-11-15", (0, 1, 0), "2022-11-15"),
            ("0.2.0_stripe-2022-11-15", (0, 2, 0), "stripe-2022-11-15"),
            ("0.1.0_3.7.1-pre.0", (0, 1, 0), "3.7.1-pre.0"),
        ];
        for (tag, (major, minor, patch), build) in cases {
            let v = parse_tag_as_semver(tag).expect("underscore tag should parse");
            assert_eq!((v.major, v.minor, v.patch), (major, minor, patch));
            assert!(v.pre.is_empty(), "{tag} should have no pre-release");
            assert!(!v.build.is_empty(), "{tag} should have build metadata");
            assert_eq!(v.build.as_str(), build);
        }
    }

    #[test]
    fn parse_tag_as_semver_plain_and_rejects_v_prefix() {
        let plain = parse_tag_as_semver("1.2.3").expect("plain should parse");
        assert_eq!((plain.major, plain.minor, plain.patch), (1, 2, 3));
        assert!(plain.build.is_empty());

        // `v`-prefixed tags are not valid SemVer and are rejected.
        assert!(parse_tag_as_semver("v1.2.3").is_none());
    }

    #[test]
    fn parse_tag_as_semver_rejects_non_semver() {
        assert!(parse_tag_as_semver("latest").is_none());
        assert!(parse_tag_as_semver("sha256-abc").is_none());
        assert!(parse_tag_as_semver("not-a-version").is_none());
    }

    #[test]
    fn pick_latest_stable_tag_underscore_build_metadata() {
        let tags = vec!["0.1.0_2022-11-15".into(), "0.2.0_stripe-2022-11-15".into()];
        // Build metadata is ignored for precedence, and the raw underscore tag
        // is returned unchanged so it can be used to pull the artifact.
        assert_eq!(
            pick_latest_stable_tag(&tags),
            Some("0.2.0_stripe-2022-11-15".to_string())
        );
    }
}
