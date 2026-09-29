use std::fmt;

/// A version-independent package or interface identity for relationship queries.
///
/// ```
/// use wasm_meta_registry_types::RelationshipTarget;
///
/// let target = RelationshipTarget::new("wasi:io", Some("streams"))?;
/// assert_eq!(target.package(), "wasi:io");
/// assert_eq!(target.interface(), Some("streams"));
/// # Ok::<(), wasm_meta_registry_types::RelationshipTargetError>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationshipTarget {
    package: String,
    interface: Option<String>,
}

impl RelationshipTarget {
    /// Validate a `namespace:name` identity and optional interface name.
    ///
    /// Each name follows `[a-z][a-z0-9]*(-[a-z][a-z0-9]*)*`.
    /// Versions and search operators are not accepted.
    pub fn new(package: &str, interface: Option<&str>) -> Result<Self, RelationshipTargetError> {
        let Some((namespace, name)) = package.split_once(':') else {
            return Err(RelationshipTargetError::Package);
        };
        if !is_identifier(namespace) || !is_identifier(name) {
            return Err(RelationshipTargetError::Package);
        }
        if interface.is_some_and(|name| !is_identifier(name)) {
            return Err(RelationshipTargetError::Interface);
        }
        Ok(Self {
            package: package.to_owned(),
            interface: interface.map(str::to_owned),
        })
    }

    /// The version-independent `namespace:name` identity.
    #[must_use]
    pub fn package(&self) -> &str {
        &self.package
    }

    /// The exact interface name, or `None` for package-wide matching.
    #[must_use]
    pub fn interface(&self) -> Option<&str> {
        self.interface.as_deref()
    }
}

fn is_identifier(value: &str) -> bool {
    value.split('-').all(|segment| {
        let mut bytes = segment.bytes();
        bytes.next().is_some_and(|byte| byte.is_ascii_lowercase())
            && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
    })
}

/// Why a relationship query's target could not be parsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationshipTargetError {
    /// The package is not a version-independent `namespace:name`.
    Package,
    /// The optional interface is not an individual interface name.
    Interface,
}

impl fmt::Display for RelationshipTargetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Package => "Expected a package name such as wasi:io, without a version.",
            Self::Interface => "Expected an interface name such as streams, without a version.",
        })
    }
}

impl std::error::Error for RelationshipTargetError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_package_and_interface_scopes() {
        for interface in [None, Some("streams"), Some("outgoing-handler")] {
            let target = RelationshipTarget::new("wasi:io", interface).expect("valid target");
            assert_eq!(target.package(), "wasi:io");
            assert_eq!(target.interface(), interface);
        }
    }

    #[test]
    fn rejects_malformed_and_versioned_targets() {
        for package in [
            "",
            "wasi",
            ":io",
            "wasi:",
            "wasi:io:streams",
            "wasi:io/streams",
            "wasi:io@0.2.0",
            "wasi:io&limit=1",
            "wasi:io\n",
            "../:io",
        ] {
            assert_eq!(
                RelationshipTarget::new(package, None),
                Err(RelationshipTargetError::Package),
                "{package:?}"
            );
        }
        for interface in [
            "",
            "streams@0.2.0",
            "wasi:io/streams",
            "../streams",
            "<script>",
        ] {
            assert_eq!(
                RelationshipTarget::new("wasi:io", Some(interface)),
                Err(RelationshipTargetError::Interface),
            );
        }
    }

    #[test]
    fn validates_every_identifier_segment_consistently() {
        for name in ["a", "foo2", "foo2-bar3", "a1-b2-c3"] {
            let package = format!("{name}:{name}");
            RelationshipTarget::new(&package, Some(name)).expect("valid WIT names");
        }
        for name in [
            "", "Foo", "fOo", "2foo", "foo--bar", "foo-2bar", "-foo", "foo-", "foo_bar", "fo\u{f3}",
        ] {
            for package in [format!("{name}:io"), format!("wasi:{name}")] {
                assert_eq!(
                    RelationshipTarget::new(&package, None),
                    Err(RelationshipTargetError::Package),
                    "{package:?}"
                );
            }
            assert_eq!(
                RelationshipTarget::new("wasi:io", Some(name)),
                Err(RelationshipTargetError::Interface),
                "{name:?}"
            );
        }
    }
}
