use chrono::{DateTime, Utc};
use wasm_package_manager_migration::entities::wit_package;

/// A public view of a WIT package, without internal database IDs.
///
/// This type is freely constructable and is the primary public API type
/// for representing WIT packages. Internal code uses [`RawWitPackage`]
/// with database IDs; this type strips those away.
///
/// # Example
///
/// ```
/// use wasm_package_manager::types::WitPackage;
///
/// let pkg = WitPackage {
///     package_name: "wasi:http".to_string(),
///     version: Some("0.2.10".to_string()),
///     description: Some("HTTP types and handler".to_string()),
///     wit_text: None,
///     created_at: "2025-01-01T00:00:00Z".to_string(),
/// };
/// assert_eq!(pkg.package_name, "wasi:http");
/// ```
#[derive(Debug, Clone)]
pub struct WitPackage {
    /// The WIT package name (e.g. "wasi:http").
    pub package_name: String,
    /// Semver version string, if known.
    pub version: Option<String>,
    /// Human-readable description of the type.
    pub description: Option<String>,
    /// Full WIT text representation, when available.
    pub wit_text: Option<String>,
    /// When this row was created.
    pub created_at: String,
}

impl From<wit_package::Model> for WitPackage {
    fn from(wt: wit_package::Model) -> Self {
        Self {
            package_name: wt.package_name,
            version: wt.version,
            description: wt.description,
            wit_text: wt.wit_text,
            created_at: format_ts(wt.created_at),
        }
    }
}

fn format_ts(ts: DateTime<Utc>) -> String {
    ts.to_rfc3339()
}
