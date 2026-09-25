# component-meta-registry

An HTTP server that indexes OCI registries for WebAssembly package metadata and
exposes a search API.

## Overview

`component-meta-registry` reads a directory of per-namespace TOML registry files,
periodically syncs manifest and config metadata via `wasm-package-manager`, and
serves search results over HTTP. The `wasm` CLI can query this API for remote
package discovery — users then install packages from the actual OCI registries.

## Registry format

Create a `registry/` directory with one TOML file per WIT namespace:

```
registry/
  ba.toml
  wasi.toml
  microsoft.toml
```

Each file defines a `[namespace]` table and zero or more `[[component]]` and
`[[interface]]` entries:

```toml
# wasi.toml
[namespace]
name = "wasi"
registry = "ghcr.io/webassembly"

[[interface]]
name = "io"
repository = "wasi/io"

[[interface]]
name = "clocks"
repository = "wasi/clocks"
```

```toml
# ba.toml
[namespace]
name = "ba"
registry = "ghcr.io/bytecodealliance"

[[component]]
name = "sample-wasi-http-rust"
repository = "sample-wasi-http-rust/sample-wasi-http-rust"
```

- **`[namespace]`** — maps a WIT namespace to an OCI registry base path
- **`[[component]]`** — a runnable Wasm component
- **`[[interface]]`** — a WIT interface type package
- **`name`** — the package name under the namespace (e.g., `wasi:io`)
- **`repository`** — the OCI repository path, relative to the namespace's `registry`

The filename (without `.toml`) must match the `namespace.name` field inside.

## Usage

```sh
component-meta-registry registry/ --bind 0.0.0.0:8080
```

Routine discovery defaults to **3,600 seconds (1 hour)** after the last
completed catalog pass, including across restarts. Override it explicitly with
`--sync-interval <seconds>`; this is an elapsed interval, not a midnight job.

Newly loaded, approved sources do not wait for that routine pass. The indexer lists
their complete tag history and enqueues every supported semantic-version release
in ascending order, skipping versions already queued or cached. Discovery
completion means those versions were scheduled, not that ingestion succeeded:
pending, active, and failed releases remain visible in the fetch queue. Interrupted
discovery resumes with deduplication, while failed scans retry with persisted
backoff from one minute up to one hour. Exhausted fetch retries remain failed;
bootstrap does not silently reset them or re-fetch immutable indexed versions.
Successful empty tag lists, including a registry's JSON `tags: null`, count as
complete discovery without making a package ready. Failed, malformed, or
wrong-repository responses remain errors; incomplete pagination never completes
discovery.

The source list is loaded from `registry/` at startup, not watched remotely.
Container images bake in that directory, so learning a merged registry entry
still requires loading an updated image/configuration. Once learned, its full
initial indexing is independent of routine discovery. No new public source
admission endpoint is involved.

Worker queue pickup and lease checks still wake at least once a minute when
idle; active ingestion, retries, and initial history continue between sweeps.
Metadata publish-time backfill retries independently at most hourly (or at the
shorter explicitly configured discovery interval). Accepted targeted version
notifications keep their priority and separate one-hour freshness cooldown.
Without a notification, a new release of an initialized package can wait up to
the discovery interval, plus any upstream failure or processing backlog.

On upgrading an existing database, each configured source gets one bounded
tag-list reconciliation because older source rows do not prove complete history
discovery. Already cached/queued versions are not downloaded again. The persisted
routine watermark is retained; explicit interval overrides remain intact until
the operator changes them.

## API Endpoints

- `GET /v1/health` — Health check
- `GET /v1/stats` — Package, namespace, and version counts for the whole index
- `GET /v1/search?q={query}&offset={n}&limit={n}` — Search packages
- `GET /v1/packages?offset={n}&limit={n}` — List all packages
- `GET /v1/packages/{registry}/{repository}` — Get a specific package

## License

Licensed under Apache License, Version 2.0, with LLVM Exceptions.
