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
component-meta-registry registry/ --sync-interval 3600 --bind 0.0.0.0:8080
```

## API Endpoints

- `GET /v1/health` — Health check
- `GET /v1/stats` — Package, namespace, and version counts for the whole index
- `GET /v1/namespaces?offset={n}&limit={n}` — Namespaces in alphabetical order with package counts
- `GET /v1/namespaces/{namespace}/packages?offset={n}&limit={n}` — Packages in one exact namespace
- `GET /v1/search?q={query}&offset={n}&limit={n}` — Search packages
- `GET /v1/search/by-import?interface={package}&offset={n}&limit={n}` — Existing package-level import search
- `GET /v1/search/by-export?interface={package}&offset={n}&limit={n}` — Existing package-level export search
- `GET /v1/relationships/dependents?package={namespace:name}&offset={n}&limit={n}` — Direct and transitive dependent packages
- `GET /v1/relationships/imported-by?package={namespace:name}&interface={member}&offset={n}&limit={n}` — Worlds importing the package or exact interface
- `GET /v1/relationships/exported-by?package={namespace:name}&interface={member}&offset={n}&limit={n}` — Worlds exporting the package or exact interface
- `GET /v1/packages?offset={n}&limit={n}` — List all packages
- `GET /v1/packages/{registry}/{repository}` — Get a specific package

### Namespace discovery

Namespace endpoints return `RegistryPage<T>` with `results`, `total`, `offset`,
`limit`, and `has_next`. Namespace entries have `name` and `packages` fields;
namespace package entries use the existing `KnownPackage` shape.
The default limit is 20, capped at 100; zero selects the default.

Directory membership comes from every `[namespace]` declaration loaded from the
registry directory, including namespace-only files and packages awaiting indexing.
Unregistered WIT namespaces and repository-owner fallbacks do not create
directory entries. The server retains these registrations at startup and passes
them to `router_with_namespaces`; embedded servers should pass `Config::namespaces`
to that constructor as well. The storage-only `router` has no registrations.
Configuration edits take effect on server restart, as with configured packages.

The `packages` count describes indexed repositories with semver releases, not
configured package entries. Each repository counts once, regardless of how many
release tags it has. Package assignment prefers the registered WIT namespace,
falling back to the first repository path segment when no WIT mapping exists.
Registered namespaces with no indexed releases have zero counts and valid empty
package listings; database errors still fail the request. Membership, sorting,
and grouping happen before pagination. Namespace package matching is exact,
not a capped substring search. `/v1/stats` retains its existing indexed-release
semantics, so its namespace count can differ from the directory's registration total.

### Relationship discovery

Relationship targets are version-independent WIT identities, such as
`package=wasi%3Aio`. The `interface` parameter is optional for world queries:
omitting it matches any interface in that package; `interface=streams` matches
that exact member. Dependents does not accept `interface`. Missing or malformed
identities and invalid pagination parameters return `400` with a JSON `error`.
Namespace, package, and interface names each follow the WIT identifier grammar
`[a-z][a-z0-9]*(-[a-z][a-z0-9]*)*`.

Dependents follows the indexed WIT package dependency declarations in reverse,
directly and transitively, ignoring dependency versions and unresolved foreign
keys. Cycles terminate and the target itself is excluded. Declarations matching
the source package's own identity are ignored as self-edges, even when that
identity is reached transitively. For unregistered components this check uses
the extracted package name. World queries instead match the world's own indexed
import/export declarations, with no transitive traversal. These are exact-name
queries, not substring searches. They describe the existing index, whose extractor
currently records world-derived package dependencies, not every possible
source-level dependency.

Each package, or owning-package/world pair, appears once at its highest matching
semantic version. A newer release that dropped the relationship does not replace
an older matching release. Only tags attached to the matching manifest qualify;
non-semver tags (including signatures and `latest`) are excluded. The `version`
field preserves the actual OCI tag, including `_`-encoded build metadata. Use it
for versioned links rather than the embedded package's latest tag.

Results deduplicate mirrored WIT identities and preserve the owning package's
kind. Registered WIT identities take precedence over synthetic names extracted
from compiled components; components without a registered identity remain
distinct by OCI identity. A compiled component's synthetic world belongs on its
owning version page, while a WIT package's world has a versioned world page.
Use the world's `is_synthetic` flag for this distinction, not the optional
package kind or the world name alone.

The three endpoints return `RelationshipPage<T>`:

- `results`: dependent entries contain `package` (the existing `KnownPackage`
  shape) and `version`; world entries also contain `name`, `is_synthetic`, and optional
  `description`.
- `total`: optional count of eligible, deduplicated matches for this query, not
  the whole registry.
- `offset` and `limit`: effective pagination values. Offset defaults to `0`;
  limit defaults to `20`, treats `0` as the default, and is capped at `100`.
- `has_next`: whether another page exists, independent of total availability.

Release filtering and deduplication happen before pagination, in stable package
identity/world-name order. A well-formed target need not itself be indexed to
match declared references. No matches returns an empty page (with `total: 0`);
database failures return an error, not a successful empty result. These endpoints
do not change the shapes or matching behavior of `/v1/search` or the existing
package-level import/export searches.

Candidates stream in bytewise identity order. Release selection retains only
the requested page and the current identity's best matching release; full
package/world metadata is loaded only for that page. Repositories and worlds
are loaded in ID batches; tags, descriptions, and listing
metadata are hydrated with set-based queries and reused by worlds sharing a
repository. Exact totals and canonical SemVer filtering still require scanning
all lightweight matching candidates.
The page size bounds retained release-selection memory, not total database
scan or transfer work.

Run the focused store and HTTP relationship tests with:

```sh
env -u COMPONENT_DATABASE_URL cargo test -p wasm-package-manager -p component-meta-registry --lib relationship
```

These fixtures use isolated SQLite databases. The HTTP fixtures require
`COMPONENT_DATABASE_URL` to be unset so they cannot migrate or modify a configured
shared database.

## License

Licensed under Apache License, Version 2.0, with LLVM Exceptions.
