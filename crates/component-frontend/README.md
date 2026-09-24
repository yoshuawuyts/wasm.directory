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

## Installers

The `/install/linux` and `/install/macos` routes serve the shared
`scripts/install.sh`; `/install/windows` serves `scripts/install.ps1`. The
canonical files are embedded into the component, so rebuild the frontend when
either script changes. The Docker build includes only these two files from
`scripts/`; no runtime script directory or registry API is needed.

Each URL serves the exact script as browser-readable plain text, with GET/HEAD
support and one-hour public caching. GET includes the script's `Content-Length`.
The shared HTTP adapter omits this optional header from HEAD responses: WASI
otherwise checks the empty transmitted body against the GET length and rejects
the response. HEAD keeps the remaining metadata and sends no body.

The homepage selects the command using browser platform information, and the
shell installer detects the OS and architecture locally. The routes do not choose
scripts based on User-Agent, and there is no generic `/install` endpoint.

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
