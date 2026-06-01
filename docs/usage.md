# Usage Guide

This guide covers basic usage patterns for `wasm(1)`, a unified developer tool for WebAssembly.

## Installation

### Shell (Linux / macOS)

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/yoshuawuyts/component-cli/releases/latest/download/install.sh | sh
```

### PowerShell (Windows)

```powershell
irm https://github.com/yoshuawuyts/component-cli/releases/latest/download/install.ps1 | iex
```

### From crates.io

```bash
cargo install wasm
```

### As a Library

```bash
cargo add wasm
```

## Command Overview

`wasm` provides several command categories:

- **Package Management**: Pull, push, and list packages
- **Local Discovery**: Detect and manage local Wasm files
- **Inspection**: Examine Wasm component structure
- **Self Management**: Configure and manage the tool itself

## Package Management

### Pulling Packages

Download a package from a registry:

```bash
# Pull from GitHub Container Registry
component package pull ghcr.io/example/my-component:latest

# Pull from Docker Hub
component package pull myuser/my-component:v1.0.0

# Pull from a custom registry
component package pull registry.example.com/org/component:tag
```

The package is stored locally in content-addressable storage and can be listed with `component package list`.

### Publishing a Package

`component publish` reads a `[package]` section from `wasm.toml` and
uploads either the compiled component (`kind = "component"`) or a
freshly-built WIT package (`kind = "interface"`). The target reference
**must** be spelled out in full in the manifest — there is no implicit
default and no shorthand — so publishing is fully reproducible from
`wasm.toml` alone.

```toml
# wasm.toml
[package]
# Package identity in `namespace:name` form. Used as the OCI artifact
# title and (for interfaces) stamped onto WIT package decls.
name = "yoshuawuyts:fetch"
# Semver version. The single source of truth: WIT files must not
# declare their own `@version` — the publisher stamps this onto every
# top-level `package` decl during build. Becomes the OCI tag.
version = "0.1.0"
# OCI registry base (host + optional path), without a repository or tag.
# The published reference is `<registry>/<repository>:<version>`.
registry = "ghcr.io/yoshuawuyts"
# Catalog path within the registry (the namespace/package location).
repository = "yoshuawuyts/fetch"
# What kind of artifact this manifest publishes: "component" or "interface".
kind = "component"
# Path to the compiled component, relative to the manifest directory.
# Defaults to `build/<name-after-colon>.wasm` if omitted.
file = "build/fetch.wasm"
# Free-form metadata, mapped to `org.opencontainers.image.*` annotations.
description = "A tiny fetch helper"
license = "Apache-2.0"
authors = ["Yosh <yosh@example.com>"]
```

For an interface package, point `wit` at the WIT directory:

```toml
[package]
name = "wasi:logging"
version = "1.0.0"
registry = "ghcr.io/wasi"
repository = "wasi/logging"
kind = "interface"
# Path to the WIT directory, relative to the manifest. Defaults to "wit".
wit = "wit"
```

The manifest's `version` is the **single source of truth** — WIT files
must not declare their own `@version`; the publisher will stamp the
manifest version onto every top-level `package` decl during build.

Inspect what would be published without pushing:

```bash
component publish --dry-run
```

Publish for real:

```bash
component publish                       # uses [package].registry + repository
component publish --file build/x.wasm   # override the artifact path
```

The artifact is uploaded as a single OCI layer
(`application/vnd.wasm.config.v0+json` config, `application/wasm`
layer) with `org.opencontainers.image.{title,version,created,description,source,url,documentation,licenses,authors}`
annotations populated from the `[package]` section.

**Note**: You must be authenticated to push packages. See [Authentication](authentication.md) for details.


### Listing Packages

View all locally stored packages:

```bash
component package list
```

This shows:
- Registry and repository
- Tags
- Digests
- Pull timestamps
- Storage size

## Local Wasm File Discovery

### Listing Local Files

Detect Wasm files in the current directory:

```bash
component local list
```

This recursively scans for `.wasm` files and displays:
- File paths
- File sizes
- Component type (if applicable)

The detector respects `.gitignore` rules and standard ignore patterns.

## Inspecting Wasm Components

### Basic Inspection

Examine a Wasm component file:

```bash
component inspect file.wasm
```

This displays:
- Component structure
- Imports and exports
- Metadata
- Dependencies

### Detailed Information

For more detailed information, the inspect command shows:
- Component type information
- Interface definitions
- World descriptions
- Custom sections

## Self Management

### Viewing State

Check storage location and usage:

```bash
component self state
```

Displays:
- Executable location
- Data directory paths
- Storage sizes
- Migration status

### Cleaning Storage

Clean up unused content and optimize storage:

```bash
component self clean
```

This operation:
- Removes orphaned content
- Vacuums the database
- Reclaims disk space

## Common Workflows

### Exploring a Registry

1. Search for packages (coming soon)
2. Pull interesting packages to inspect them
3. Examine with `component inspect`

### Publishing a Package

1. Build your Wasm component
2. Authenticate with your registry (see [Authentication](authentication.md))
3. Push with `component package push registry.example.com/myorg/component:v1.0.0`

### Managing Local Development

1. Use `component local list` to discover Wasm files in your project
2. Inspect components with `component inspect`
3. Test components locally before publishing

### Cleaning Up After Development

1. Run `component self state` to check storage usage
2. Remove unused packages manually or with future commands
3. Run `component self clean` to reclaim space

## Running Library-style Components

`component run` can execute three kinds of WebAssembly components:

- **HTTP components** (export `wasi:http/incoming-handler`) are
  served on a local TCP port — use `--listen` to set the address.
- **CLI components** (export `wasi:cli/run`) are executed as
  programs; trailing arguments after `<INPUT>` become the guest's
  `argv`.
- **Library-style components** — anything that exports plain
  functions or interfaces but does not target either of the worlds
  above. The component's WIT exports are translated into a `clap`
  sub-CLI on the fly.

The WIT → CLI mapping is implemented by the
[`wit2cli`](../crates/wit2cli) crate. Its
[`tests/snapshots/`](../crates/wit2cli/tests/snapshots) directory
contains the canonical, end-user-facing spec for how each WIT type
translates into a CLI argument — one snapshot per shipped fixture.

### A worked example

Given a component with this WIT:

```wit
package yoshuawuyts:wordmark;

world wordmark {
    /// Convert a markdown document to a Word (.docx) document.
    export to-word: func(markdown: string) -> result<list<u8>, string>;
}
```

You can invoke it directly:

```bash
component run yoshuawuyts:wordmark to-word "# hello" > file.docx
```

This dispatches to the `to-word` export, passes `"# hello"` as the
single string parameter, and writes the resulting bytes verbatim to
stdout (or, with redirection, to `file.docx`).

### Argument mapping

| WIT type | CLI shape |
|----------|-----------|
| Primitives, `string`, `char`, `enum`, `variant` | Positional argument |
| `record` | Group of `--field-name VALUE` flags. With multiple record params, fields are prefixed: `--<param>-<field>` |
| `list<T>` | Repeated `--name V --name W`, or positional variadic when last |
| `option<T>` | Same as `T`, but optional |
| `variant V(payload)` | `name=value` for cases with a payload, `name` otherwise |

Resources and futures/streams are not supported.

### Output rules

- `list<u8>` results are written to stdout as raw bytes (this is
  what makes `> file.docx` work).
- `string` results are written verbatim with no trailing newline.
- Numeric / boolean / `char` results are rendered with `Display` and
  a trailing newline.
- Records, variants, enums, flags, tuples, and lists of non-`u8`
  are rendered as JSON.
- A `result::Err(e)` causes `component run` to print `e` to stderr
  and exit with code 1.

### Host flags vs. guest arguments

All host-side flags (`--global`, `--env`, `--dir`, `--inherit-env`,
`--inherit-network`, `--no-stdio`, `--listen`) must come **before**
the `<INPUT>` argument; everything after `<INPUT>` is forwarded to
the guest:

```bash
component run --inherit-env yoshuawuyts:wordmark to-word "# hi"
```

## Package Reference Format

Packages are referenced using OCI-style references:

```
[registry/]repository[:tag|@digest]
```

### Examples

```bash
# Full reference with registry and tag
ghcr.io/owner/repo:latest

# With digest instead of tag
ghcr.io/owner/repo@sha256:abcd1234...

# Custom registry with port (untested)
localhost:5000/myrepo:dev
```

### Registry Resolution

- Common registries: `ghcr.io`, `docker.io`, `mcr.microsoft.com`, `quay.io`
- Private registries require full domain specification

## Command-Line Help

Each command and subcommand has built-in help:

```bash
# Top-level help
component --help

# Subcommand help
component package --help
component package pull --help

# Self commands
component self --help
```

## Tips and Tricks

### Shell Completions

Generate shell completions for your preferred shell (user-local paths shown):

```bash
# Bash
component self completions bash > ~/.local/share/bash-completion/completions/component

# Zsh
component self completions zsh > ~/.zfunc/_component

# Fish
component self completions fish > ~/.config/fish/completions/component.fish
```

### Man Pages

Generate man pages for offline documentation. A user-local path is shown below;
for system-wide installation, use `sudo` and `/usr/local/share/man/man1/component.1`.

```bash
mkdir -p ~/.local/share/man/man1
component self man-pages > ~/.local/share/man/man1/component.1
man wasm
```

### Color Support

The CLI supports colored output via the `--color` flag:

```bash
component --color auto ...     # automatic color (default)
component --color always ...   # always use color
component --color never ...    # never use color
```

Color output can also be controlled via environment variables:

- `NO_COLOR=1` — disables color output
- `CLICOLOR=0` — disables color output
- `CLICOLOR_FORCE=1` — forces color output even when not in a terminal

### Quick Package Inspection

Combine commands to quickly pull and inspect:

```bash
component package pull ghcr.io/example/component:latest
component inspect ~/.local/share/wasm/store/content/<digest>
```

### Finding Package Content

After pulling a package, use `component package list` to find its digest, then access content in the store directory.

### Using with CI/CD

In CI/CD pipelines:

1. Authenticate using `docker login` or similar
2. Use `component package pull` to retrieve dependencies
3. Use `component package push` to publish artifacts
4. Use `component self clean` to manage storage between builds

## Troubleshooting

### Package Not Found

If pulling fails with "not found":
- Verify the package reference is correct
- Check authentication (see [Authentication](authentication.md))
- Ensure the package exists and is accessible

### Storage Issues

If you encounter storage errors:
- Run `component self state` to check space
- Run `component self clean` to free up space
- Check filesystem permissions on `~/.local/share/wasm`

### Network Errors

For network-related failures:
- Check internet connectivity
- Verify registry is accessible
- Check firewall and proxy settings

## Further Reading

- [Authentication](authentication.md) - Set up registry access
- [Configuration](configuration.md) - Understand storage and settings
- [API Documentation](https://docs.rs/wasm) - Library usage

## Component Composition

### Workspace Layout

Running `component init` creates a workspace that includes composition directories:

```text
my-workspace/
├── types/         # WIT interface definition files (.wit)
├── seams/         # WAC composition scripts (.wac)
├── build/         # Composed output artifacts
├── vendor/
│   ├── wasm/      # Vendored component binaries
│   └── wit/       # Vendored WIT interfaces
├── wasm.toml
└── wasm.lock.toml
```

### WAC Scripts

[WAC (WebAssembly Composition)](https://github.com/bytecodealliance/wac) is a
declarative language for composing Wasm components. Place `.wac` files in the
`seams/` directory to define how components are wired together.

### `component compose`

Compose Wasm components from WAC scripts:

```bash
# Compose a named WAC file (looks for seams/my-composition.wac)
component compose my-composition

# Compose all WAC files in seams/
component compose

# Use dynamic linking (import dependencies instead of embedding)
component compose my-composition --linker=dynamic

# Specify output directory
component compose my-composition -o output/
```

### Package Resolution

When resolving packages referenced in WAC files, the resolver checks:

1. **Manifest entries** — components and interfaces in `wasm.toml` mapped
   to vendored files in `vendor/wasm/` and `vendor/wit/`.
2. **Local directories** — `.wasm` and `.wit` files in `types/`.

## Getting Help

- GitHub Issues: [https://github.com/yoshuawuyts/wasm/issues](https://github.com/yoshuawuyts/wasm/issues)
- Command help: `wasm --help`
- This documentation: `/docs` directory
