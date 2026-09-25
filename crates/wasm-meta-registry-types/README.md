# wasm-meta-registry-types

Shared wire types for the
[component-meta-registry](../component-meta-registry) API.

This crate contains only data types with `serde` derive implementations — no
HTTP client, no server code, no database access. It is the single source of
truth for the JSON shapes exchanged between the meta-registry server and its
clients.

## Usage

```rust
use wasm_meta_registry_types::{KnownPackage, PackageDependencyRef};

let pkg = KnownPackage {
    registry: "ghcr.io".into(),
    repository: "user/my-component".into(),
    kind: Some(wasm_meta_registry_types::PackageKind::Component),
    description: Some("A useful component".into()),
    tags: vec!["v1.0.0".into()],
    signature_tags: vec![],
    attestation_tags: vec![],
    last_seen_at: "2025-01-01T00:00:00Z".into(),
    created_at: "2024-06-15T12:00:00Z".into(),
    wit_namespace: None,
    wit_name: None,
    dependents: None,
    latest_release_at: Some("2024-12-01T00:00:00Z".into()),
    dependencies: vec![],
};

assert_eq!(pkg.reference(), "ghcr.io/user/my-component");
```

`KnownPackage` includes optional listing metadata:

- `dependents`: distinct other indexed repositories declaring the WIT package
  as a dependency, matching the popularity ranking. Repeated edges and versions
  count once per repository. Mirrors share the identity-level count; a repository
  can count as a dependent of another tagged mirror, but not of itself alone.
  Zero is explicit; an unknown WIT identity leaves the field absent.
- `latest_release_at`: the newest publication time among this repository's valid
  semver tags, including its debut (not necessarily its highest version).
  A valid OCI creation annotation takes precedence over config creation time,
  with earliest tag/manifest indexing as fallback. Future dates are capped at
  that indexing time. Repository scan timestamps do not determine release age.

Both fields are omitted when unavailable and default to `None` when reading
older responses.
