# Architecture

This document describes the high-level architecture of `component(1)`. If you want to
familiarize yourself with the codebase, you are in the right place.

## Overview

`component(1)` is a unified developer tool for WebAssembly. It can pull and install
Wasm Components and WIT interfaces from OCI registries, run components via
Wasmtime with sandboxed permissions, and manage local state through the CLI.

The project is a Cargo workspace with twelve crates:

```
crates/
├── component-cli                      # Binary — the `component(1)` command
├── component-cli-internal-run         # Library — internal Wasmtime component execution for the CLI
├── wit2cli                            # Library — translate a component's WIT exports into a clap sub-CLI
├── wasm-package-manager          # Library — OCI registry interaction, caching, metadata
├── wasm-package-manager-migration # Library — SeaORM migrations and entity definitions
├── wasm-manifest                 # Library — manifest and lockfile types
├── component-detector                 # Library — local .wasm file discovery
├── component-meta-registry            # Binary + library — HTTP metadata server for package search
├── wasm-meta-registry-client     # Library — HTTP client for a meta-registry instance
├── wasm-meta-registry-types      # Library — shared serde wire types for the meta-registry API
├── component-frontend                 # Library (cdylib) — server-side rendered web frontend (wasm32-wasip2 component)
└── xtask                              # Internal — build automation (fmt, clippy, test, SQL migrations)
```

## Crate Dependency Graph

```
component-cli ──────┬──► component-cli-internal-run ──► wasm-manifest
                    ├──► wit2cli
                    ├──► component-detector
                    ├──► wasm-manifest
                    ├──► wasm-meta-registry-types
                    └──► wasm-package-manager ──┬──► wasm-package-manager-migration
                                                     ├──► component-detector
                                                     ├──► wasm-manifest
                                                     ├──► wasm-meta-registry-client ──► wasm-meta-registry-types
                                                     └──► wasm-meta-registry-types

component-meta-registry ──┬──► wasm-package-manager
                          └──► wasm-meta-registry-types

component-frontend   (standalone wasm32-wasip2 component)
```

`component-cli` is the main entry point. It depends on `wasm-package-manager` for all
registry and storage operations, on `wasm-manifest` for reading project manifests and
lockfiles, on `component-detector` for finding local `.wasm` files, on
`component-cli-internal-run` for executing components via Wasmtime, on `wit2cli` for
exposing a component's WIT exports as CLI sub-commands, and on
`wasm-meta-registry-types` for deserializing search results.

`wasm-package-manager` is the core library. It depends on
`wasm-package-manager-migration` for the database schema,
`wasm-meta-registry-client` for syncing the local package index from a meta-registry,
and on `wasm-meta-registry-types`, `component-detector`, and `wasm-manifest`.

`component-meta-registry` is an independent server binary that also uses
`wasm-package-manager` to index OCI registries and expose a search API, sharing its
wire types via `wasm-meta-registry-types`.

`component-frontend` is a standalone server-side rendered web frontend compiled as a
`wasm32-wasip2` component; it is not depended on by any other crate.

`xtask` is a development-only crate and is not depended on by any other crate.

## component-cli

The `component(1)` binary lives in `crates/component-cli`. It uses [clap] for argument
parsing and dispatches to one of the following command modules:

| Command      | Module          | Purpose |
|------------- |---------------- |-------- |
| `run`        | `run/`          | Execute a Wasm Component via [wasmtime] with WASI sandboxing |
| `init`       | `init/`         | Scaffold a project with manifest, lockfile, and vendor dirs |
| `install`    | `install/`      | Pull packages and vendor them into `vendor/` |
| `compose`    | `compose/`      | Compose Wasm components from WAC scripts |
| `local`      | `local/`        | Detect `.wasm` files in the current project |
| `registry`   | `registry/`     | Manage cached packages (pull, tags, search, sync, delete, list, known, inspect) |
| `self`       | `self_/`        | Tool configuration, completions, man pages, state, logs, clean |

[clap]: https://docs.rs/clap
[wasmtime]: https://docs.rs/wasmtime

### Run Command and Permissions

`component run` executes a Wasm Component using Wasmtime's WASIp2 implementation.
Permissions are resolved through a four-layer merge:

1. **Global config** — `$XDG_CONFIG_HOME/wasm/config.toml` defaults
2. **Global components** — `$XDG_CONFIG_HOME/wasm/components.toml` per-component overrides
3. **Project manifest** — `wasm.toml` per-component permissions
4. **CLI flags** — command-line overrides (highest precedence)

The `RunPermissions` type is defined in `wasm-manifest` and controls environment
variables, directory access, stdio inheritance, and network access.

### Component Execution

The actual Wasmtime execution is delegated to `component-cli-internal-run`. The CLI
resolves permissions and arguments, then hands the component bytes off for invocation.

## component-cli-internal-run

An internal library in `crates/component-cli-internal-run` that executes WebAssembly
components via [wasmtime]. It is **not** intended for third-party use — it is an
implementation detail of `component-cli` and its API may change without notice. It exposes
three entry points:

- **`validate_component`** — checks that a byte slice is a Wasm Component (not a core
  module or WIT-only package).
- **`execute_cli_component`** — builds the Wasmtime runtime, wires WASI permissions,
  instantiates the component, and invokes `wasi:cli/run@0.2.0#run`.
- **`execute_library_function`** — invokes an arbitrary exported function on a
  "library-style" component using wasmtime's untyped `Func::call` API.

## wit2cli

A library in `crates/wit2cli` that translates a WebAssembly component's WIT exports into a
[clap] `Command`. Given a compiled component, it extracts a `LibrarySurface` describing
every exported function, builds a `clap::Command` mirroring the WIT shape, and converts
parsed `ArgMatches` into a `Vec<Val>` ready to hand off to wasmtime. The type-mapping rules
are documented end-to-end by the snapshot tests under `crates/wit2cli/tests/snapshots/`.
`component-cli` uses it to expose a component's exported functions as CLI sub-commands.

## wasm-package-manager

The core library lives in `crates/wasm-package-manager`. It handles all
interaction with OCI registries, local caching, and metadata extraction.

```
src/
├── lib.rs              # Public API re-exports, format_size()
├── config.rs           # Config loading (global + local merge), credential helpers
├── credential_helper.rs
├── progress.rs         # ProgressEvent enum for pull progress reporting
├── manager/
│   ├── mod.rs          # Manager — high-level API (pull, install, delete, search, sync)
│   └── logic.rs        # Pure functions (vendor_filename, should_sync, derive_component_name, etc.)
├── oci/
│   ├── client.rs       # OCI registry client (wraps oci-wasm + oci-client)
│   ├── models.rs       # OCI data types
│   ├── raw.rs          # RawImageEntry — internal image metadata with DB IDs
│   ├── image_entry.rs  # ImageEntry — public query result type
│   └── logic.rs        # Pure functions (filter_wasm_layers, classify_tag, compute_orphaned_layers)
├── types/
│   ├── detect.rs       # WIT package detection (is_wit_package)
│   ├── parser.rs       # WIT text parsing and metadata extraction
│   ├── raw.rs          # RawWitPackage — internal type with DB IDs
│   ├── wit_package.rs  # WitPackage — public query result type
│   └── worlds.rs       # World-level analysis
├── components/
│   └── models.rs       # Component data types
├── storage/
│   ├── mod.rs            # Store facade
│   ├── store.rs          # SeaORM operations + cacache layer caching
│   ├── db_config.rs      # COMPONENT_DATABASE_URL parsing & redaction
│   ├── config.rs         # StateInfo (cache dirs, database path, log dir)
│   ├── models/           # Migrations shim
│   └── known_package.rs  # KnownPackage — re-exported from wasm-meta-registry-client
```

### wasm-meta-registry-client

A standalone crate that provides the HTTP client for fetching package metadata
from a `component-meta-registry` instance. It contains:

- `KnownPackage` — the shared wire type returned by the `/v1/packages` endpoint.
- `RegistryClient` — HTTP client with ETag-based conditional fetches and
  exponential-backoff retries (behind the `client` feature).

### Manager

`Manager` is the main entry point. It composes a `Client` (OCI), a `Store`
(SeaORM + cacache), and a `Config`. Key operations:

- **`pull`** / **`pull_with_progress`** — fetch an OCI image, store layers in
  cacache, record metadata in the database, and extract WIT interface information.
- **`install`** / **`install_with_progress`** — pull then hard-link (vendor)
  layers into a project-local directory.
- **`delete`** — remove a cached package and its orphaned layers.
- **`search_packages`** / **`list_known_packages`** — query the local metadata
  database.
- **`sync_from_meta_registry`** — update the local package index from a
  meta-registry server.

### Storage

Storage is split into two systems:

- **Relational metadata** — stores all structured metadata (OCI manifests,
  tags, WIT interfaces, worlds, components). Managed via
  [SeaORM](https://www.sea-ql.org/SeaORM/) with backend-portable migrations.
  The default backend is SQLite (a file under the platform data directory);
  setting `COMPONENT_DATABASE_URL=postgres://...` switches to PostgreSQL.
- **cacache** — content-addressable blob store for OCI image layers.
  Deduplicates identical layers across packages. Vendoring uses hard links so
  disk usage is shared with the cache.

### Database Schema

The schema follows a three-layer design, defined under
`crates/wasm-package-manager-migration/`:

1. **OCI layer** — `oci_repository`, `oci_manifest`, `oci_tag`, `oci_layer`,
   `oci_referrer`, plus annotation tables. Models the OCI distribution spec.
2. **WIT layer** — `wit_package`, `wit_world`, `wit_world_import`,
   `wit_world_export`, `wit_package_dependency`. Models the WebAssembly
   Interface Type system. Foreign keys link imports/exports/dependencies to
   resolved packages (best-effort — NULL if the dependency is not yet
   cached).
3. **Wasm layer** — `wasm_component`, `component_target`. Links compiled
   components to the worlds they target.

To change the schema, hand-author a new migration module under
`crates/wasm-package-manager-migration/src/migrations/` and register
it in `Migrator::migrations()`. The same migration set drives both SQLite
and PostgreSQL; per-backend SQL fragments (e.g. trigger bodies) live in
`migrations/triggers.rs` and dispatch on `manager.get_database_backend()`.

## wasm-package-manager-migration

A library in `crates/wasm-package-manager-migration` that defines the database schema
for `wasm-package-manager`. It contains the SeaORM migration modules (applied in the
order registered in `Migrator::migrations()`) and the entity definitions used by the store.
Both SQLite and PostgreSQL are supported; per-backend SQL fragments (e.g. trigger bodies)
live in `migrations/triggers.rs` and dispatch on `SchemaManager::get_database_backend()`.
See [Database Schema](#database-schema) above for the schema layout.

## wasm-manifest

A small serialization library in `crates/wasm-manifest`. It defines the types
for reading and writing project manifests (`wasm.toml`) and lockfiles
(`wasm.lock.toml`).

Key types:

- **`Manifest`** — root type with a `dependencies: Dependencies` field.
- **`Dependencies`** — has `components` and `interfaces` maps of `String → Dependency`.
- **`Dependency`** — either a compact version string (`"1.0.0"`) or an
  explicit table with `registry`, `namespace`, `package`, `version`, and
  optional `permissions`. Bare versions use Cargo-style semver (`"1.0.0"` → `^1.0.0`).
- **`Lockfile`** — lists resolved packages with digests for reproducible builds.
- **`RunPermissions`** / **`ResolvedPermissions`** — sandbox controls for the
  `component run` command.

## component-detector

A small library in `crates/component-detector` that finds `.wasm` files in a
directory tree. It uses the [ignore] crate to respect `.gitignore` rules and
also scans well-known directories (`target/wasm32-*`, `pkg/`, `dist/`) that
may be git-ignored.

[ignore]: https://docs.rs/ignore

## component-meta-registry

An HTTP server in `crates/component-meta-registry` that indexes OCI registries and
exposes a search API. It consists of:

- **`config.rs`** — per-namespace TOML registry file parsing and configuration.
- **`indexer.rs`** — background thread that periodically syncs package metadata
  using `wasm-package-manager::Manager`.
- **`server.rs`** — [axum] HTTP router with search endpoints.

[axum]: https://docs.rs/axum

## wasm-meta-registry-types

A dependency-light library in `crates/wasm-meta-registry-types` that defines the
shared wire types serialized as JSON between the meta-registry server and its clients. It
has no HTTP, database, or runtime dependencies — only `serde` and `serde_json`. Both
`component-meta-registry` (server), `wasm-meta-registry-client`, and `component-cli`
depend on it so that the request/response shapes (such as `KnownPackage`) stay in sync.

## component-frontend

A standalone server-side rendered web frontend in `crates/component-frontend`, compiled as
a `wasm32-wasip2` component targeting `wasi:http`. It uses `wstd-axum` for routing and the
`html` crate for type-safe HTML generation. It is an independent component and is not
depended on by any other crate.

## xtask

Internal build automation in `crates/xtask`. The command `cargo xtask test` runs
the full CI suite:

1. `cargo nextest run` — test suite ([cargo-nextest] for parallel execution)
2. `cargo test --doc` — doc tests (not supported by nextest)
3. `cargo clippy` — lint check (with `-D warnings`)
4. `cargo fmt --check` — formatting check
5. `cargo xtask sql check` — apply migrations to in-memory SQLite (and to
   Postgres if `COMPONENT_DATABASE_URL` is set)
6. README freshness check — ensures `README.md` matches `component --help` output

[cargo-nextest]: https://nexte.st

SQL migrations are hand-authored as Rust modules under
`crates/wasm-package-manager-migration/`; see
[Database Schema](#database-schema) above.

## Project-Level Conventions

- **100% safe Rust** — `#![forbid(unsafe_code)]` is set workspace-wide.
- **Edition 2024** — all crates use the latest Rust edition.
- **Dual license** — MIT OR Apache-2.0.
- **XDG directories** — configuration, data, and state follow the XDG Base
  Directory specification.
- **`#[must_use]`** — applied to public functions that return values.
- **Public types** — public API types (`ImageEntry`, `KnownPackage`,
  `WitPackage`) omit database IDs and are separate from internal `Raw*` model
  types.
- **Pure logic** — side-effect-free functions are grouped in `logic.rs` files
  for easy testing.
