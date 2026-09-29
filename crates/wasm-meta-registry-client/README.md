# wasm-meta-registry-client

HTTP client for fetching package metadata from a
[component-meta-registry](../component-meta-registry) instance.

## Features

- Shared `KnownPackage` type matching the meta-registry `/v1/packages` API
- Optional `dependents` and `latest_release_at` listing metadata, included in
  package responses without additional requests. Missing fields from older
  servers remain `None`; known zero dependents are `Some(0)`. Release time is
  the newest semver publication, not the latest repository scan.
- `RegistryClient` with ETag-based conditional fetches and exponential-backoff
  retries (behind the `client` feature, enabled by default)

## Rust source compatibility

The optional listing fields are backward-compatible in JSON, not in Rust
struct literals. Rust callers constructing the re-exported `KnownPackage` must
provide `dependents` and `latest_release_at` (use `None` when unavailable).
This intentional pre-1.0 source break requires a new minor release, not a patch;
see the [type's compatibility notes](../wasm-meta-registry-types/README.md#rust-source-compatibility)
for migration details.

## Usage

```rust,no_run
use wasm_meta_registry_client::{KnownPackage, RegistryClient, FetchResult};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = RegistryClient::new("http://localhost:8081");
    match client.fetch_packages(None, 100).await? {
        FetchResult::NotModified => println!("up to date"),
        FetchResult::Updated { packages, .. } => {
            for pkg in &packages {
                println!("{}", pkg.reference());
            }
        }
    }
    Ok(())
}
```

## License

Licensed under Apache License, Version 2.0, with LLVM Exceptions.
