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

## Color theme

The shared sun/moon icon button switches between light and dark on every
activation and saves the selected scheme in `localStorage` under `ds-theme`.
The icon shows the current scheme; the accessible "Dark mode" button is pressed
when dark mode is active, and its tooltip describes the next action.

Without a valid saved choice, the frontend follows the system preference,
including live changes. Once selected, a scheme remains active across reloads
and system changes. The button does not reset to system mode. The initializer
runs in the document head to avoid a flash of the wrong theme.

The production navbar and design-system controls use the same renderer and
embedded scripts. Run the dependency-free script regressions with Node.js 18
or later, in addition to the repository's `cargo xtask test` checks:

```sh
node --test crates/component-frontend/src/theme/theme.test.cjs
```
