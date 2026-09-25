# component-frontend

A server-side rendered web frontend for the WebAssembly package registry,
compiled as a `wasm32-wasip2` component targeting `wasi:http`.

## Building

```sh
cargo build -p component-frontend --target wasm32-wasip2
```

To set a custom API base URL (default: `http://localhost:8081`):

```sh
API_BASE_URL=https://registry.example.com cargo build -p component-frontend --target wasm32-wasip2
```

## Running

```sh
wasmtime serve -Scli target/wasm32-wasip2/debug/component_frontend.wasm
```

Then visit <http://localhost:8080> in your browser.

## Architecture

- **Framework**: [wstd-axum](https://github.com/bytecodealliance/wstd) — Axum
  on WASI
- **HTML**: Generated server-side with the [`html`](https://docs.rs/html) crate
- **Styling**: Tailwind CSS (CDN for development)
- **Data**: Fetched from the `component-meta-registry` API via
  `wstd::http::Client`

## Package listings

Search results, namespace pages, and the all-packages page share a two-row item:
`namespace/package-name` and description above kind, version, direct dependent
count, and relative last-update time. Identities and metadata wrap on narrow
screens, and version tags are never truncated. Descriptions are visually
truncated to one line with an ellipsis; the full summary remains accessible and
the source description is available on hover. Landing-page highlight cards are
unchanged.

Kind uses the style guide's compact inline label, in lowercase, before the
version. Rendered versions have one lowercase `v` prefix; stored tags and routes
are unchanged.
GitHub's package-dependents icon precedes the count. Hovering the icon or count
explains that this counts other indexed repositories directly depending on the
package, with each repository counted once.
A clock precedes the concise release age (for example, `3 days`). Screen-reader
text preserves the full meanings (`12 dependents`, `Updated 3 days ago`).
Unavailable dependent counts display `0`, with the fallback identified in the
tooltip and screen-reader text; the API value stays unknown. Missing or invalid
dates omit the clock and date entirely, without a placeholder.

Dependent counts measure distinct other indexed repositories declaring the WIT
package as a dependency, not transitive dependents. Last updated is the newest
semver release's publication time, falling back to when that release was first
indexed when publication metadata is unavailable; it is not the last scan time.
The exact date is available on hover. An unavailable date is never replaced with
the last scan time.

The [package-dependents SVG](../../vendor/octicons/README.md) is vendored with its
MIT license and rendered using the current text color; no icon package is needed.

## Favicon

Every page uses the shared document head's `/favicon.svg`, with `/favicon.ico`
as a fallback. Both assets are embedded in the component and served locally,
without accessing the registry API or requiring a static-file directory.

The favicon is adapted from the purple icon in
[WebAssembly/web-assembly-logo](https://github.com/WebAssembly/web-assembly-logo/blob/main/dist/icon/web-assembly-icon.svg)
by Carlos Baraza, published under
[CC0 1.0 Universal](https://github.com/WebAssembly/web-assembly-logo/blob/main/LICENSE).
It keeps the original silhouette and purple (`#654ff0`), with opaque white
lettering for readability on both light and dark browser chrome.

After editing the SVG, regenerate the 16px, 32px, and 48px ICO images with
[ImageMagick](https://imagemagick.org/), from the repository root:

```sh
magick -background none -density 384 \
  crates/component-frontend/assets/favicon.svg \
  -define icon:auto-resize=48,32,16 \
  crates/component-frontend/assets/favicon.ico
```
