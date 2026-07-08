//! Permission types for controlling Wasm Component sandbox capabilities.
//!
//! [`RunPermissions`] defines which host capabilities a Wasm Component may
//! access at runtime. All fields are optional to support layered merging:
//! global config → per-component config → manifest → CLI flags.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Sandbox permissions for executing a Wasm Component.
///
/// Every field is `Option` so that multiple layers of configuration can be
/// merged together — only the fields that are explicitly set in a given layer
/// participate in the merge. Call [`RunPermissions::resolve`] to collapse
/// the options into concrete [`ResolvedPermissions`] with sensible defaults.
///
/// # Defaults (when no layer sets a value)
///
/// | Field             | Default |
/// |-------------------|---------|
/// | `inherit_env`     | `false` |
/// | `allow_env`       | `[]`    |
/// | `allow_dirs`      | `[]`    |
/// | `inherit_stdio`   | `true`  |
/// | `inherit_network` | `false` |
///
/// # Example
///
/// ```rust
/// use wasm_manifest::RunPermissions;
///
/// let toml_str = r#"
/// inherit-env = true
/// allow-dirs = ["/data", "./output"]
/// "#;
///
/// let perms: RunPermissions = toml::from_str(toml_str).unwrap();
/// assert_eq!(perms.inherit_env, Some(true));
/// assert!(perms.inherit_stdio.is_none());
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
#[must_use]
pub struct RunPermissions {
    /// Inherit **all** host environment variables.
    #[serde(rename = "inherit-env", skip_serializing_if = "Option::is_none")]
    pub inherit_env: Option<bool>,

    /// Allowlist of individual environment variables to forward.
    #[serde(rename = "allow-env", skip_serializing_if = "Option::is_none")]
    pub allow_env: Option<Vec<String>>,

    /// Host directories to pre-open for the guest.
    #[serde(rename = "allow-dirs", skip_serializing_if = "Option::is_none")]
    pub allow_dirs: Option<Vec<PathBuf>>,

    /// Inherit stdin / stdout / stderr from the host process.
    #[serde(rename = "inherit-stdio", skip_serializing_if = "Option::is_none")]
    pub inherit_stdio: Option<bool>,

    /// Allow the guest to access the network.
    #[serde(rename = "inherit-network", skip_serializing_if = "Option::is_none")]
    pub inherit_network: Option<bool>,
}

impl RunPermissions {
    /// Merge `overrides` on top of `self`.
    ///
    /// For every field, a `Some` value in `overrides` replaces the
    /// corresponding value in `self`; `None` in `overrides` preserves the
    /// existing value.
    ///
    /// # Example
    ///
    /// ```rust
    /// use wasm_manifest::RunPermissions;
    ///
    /// let base = RunPermissions {
    ///     inherit_env: Some(false),
    ///     inherit_stdio: Some(true),
    ///     ..Default::default()
    /// };
    /// let overrides = RunPermissions {
    ///     inherit_env: Some(true),
    ///     ..Default::default()
    /// };
    /// let merged = base.merge(overrides);
    /// assert_eq!(merged.inherit_env, Some(true));
    /// assert_eq!(merged.inherit_stdio, Some(true)); // preserved from base
    /// ```
    pub fn merge(self, overrides: Self) -> Self {
        Self {
            inherit_env: overrides.inherit_env.or(self.inherit_env),
            allow_env: overrides.allow_env.or(self.allow_env),
            allow_dirs: overrides.allow_dirs.or(self.allow_dirs),
            inherit_stdio: overrides.inherit_stdio.or(self.inherit_stdio),
            inherit_network: overrides.inherit_network.or(self.inherit_network),
        }
    }

    /// Collapse optional fields into concrete values using built-in defaults.
    ///
    /// # Example
    ///
    /// ```rust
    /// use wasm_manifest::RunPermissions;
    ///
    /// let perms = RunPermissions {
    ///     inherit_env: Some(true),
    ///     ..Default::default()
    /// };
    /// let resolved = perms.resolve();
    /// assert!(resolved.inherit_env);
    /// assert!(resolved.inherit_stdio); // default is true
    /// assert!(!resolved.inherit_network); // default is false
    /// ```
    pub fn resolve(self) -> ResolvedPermissions {
        ResolvedPermissions {
            inherit_env: self.inherit_env.unwrap_or(false),
            allow_env: self.allow_env.unwrap_or_default(),
            allow_dirs: self.allow_dirs.unwrap_or_default(),
            inherit_stdio: self.inherit_stdio.unwrap_or(true),
            inherit_network: self.inherit_network.unwrap_or(false),
        }
    }
}

/// Fully resolved permissions with no optional fields.
///
/// Produced by [`RunPermissions::resolve`].
///
/// # Example
///
/// ```rust
/// use wasm_manifest::RunPermissions;
///
/// let resolved = RunPermissions::default().resolve();
/// assert!(!resolved.inherit_env);
/// assert!(resolved.allow_env.is_empty());
/// assert!(resolved.allow_dirs.is_empty());
/// assert!(resolved.inherit_stdio);
/// assert!(!resolved.inherit_network);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub struct ResolvedPermissions {
    /// Inherit all host environment variables.
    pub inherit_env: bool,
    /// Explicit env-var allowlist.
    pub allow_env: Vec<String>,
    /// Pre-opened host directories.
    pub allow_dirs: Vec<PathBuf>,
    /// Inherit stdin/stdout/stderr.
    pub inherit_stdio: bool,
    /// Allow network access.
    pub inherit_network: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    // r[verify permissions.defaults]
    #[test]
    fn defaults_resolve_correctly() {
        let resolved = RunPermissions::default().resolve();
        assert!(!resolved.inherit_env);
        assert!(resolved.allow_env.is_empty());
        assert!(resolved.allow_dirs.is_empty());
        assert!(resolved.inherit_stdio);
        assert!(!resolved.inherit_network);
    }

    // r[verify permissions.merge]
    #[test]
    fn merge_overrides_some_fields() {
        let base = RunPermissions {
            inherit_env: Some(false),
            inherit_stdio: Some(true),
            ..Default::default()
        };
        let overrides = RunPermissions {
            inherit_env: Some(true),
            allow_dirs: Some(vec![PathBuf::from("/data")]),
            ..Default::default()
        };
        let merged = base.merge(overrides);
        assert_eq!(merged.inherit_env, Some(true));
        assert_eq!(merged.inherit_stdio, Some(true));
        assert_eq!(merged.allow_dirs, Some(vec![PathBuf::from("/data")]));
        assert!(merged.allow_env.is_none());
    }

    // r[verify permissions.merge-preserve]
    #[test]
    fn merge_preserves_base_when_override_is_none() {
        let base = RunPermissions {
            inherit_network: Some(true),
            ..Default::default()
        };
        let merged = base.merge(RunPermissions::default());
        assert_eq!(merged.inherit_network, Some(true));
    }

    // r[verify permissions.serde]
    #[test]
    fn serde_roundtrip() {
        let perms = RunPermissions {
            inherit_env: Some(true),
            allow_env: Some(vec!["HOME".into(), "PATH".into()]),
            allow_dirs: Some(vec![PathBuf::from("/data")]),
            inherit_stdio: Some(true),
            inherit_network: Some(false),
        };
        let toml_str = toml::to_string(&perms).expect("serialize");
        let parsed: RunPermissions = toml::from_str(&toml_str).expect("deserialize");
        assert_eq!(perms, parsed);
    }

    // r[verify permissions.toml]
    #[test]
    fn deserialize_from_toml_fragment() {
        let toml_str = r#"
inherit-env = true
allow-dirs = ["/data", "./output"]
"#;
        let perms: RunPermissions = toml::from_str(toml_str).expect("parse");
        assert_eq!(perms.inherit_env, Some(true));
        assert_eq!(
            perms.allow_dirs,
            Some(vec![PathBuf::from("/data"), PathBuf::from("./output")])
        );
        assert!(perms.inherit_stdio.is_none());
    }
}
