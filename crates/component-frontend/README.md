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
- **Styling**: Vendored Tailwind CSS browser runtime, served locally
- **Data**: Fetched from the `component-meta-registry` API via
  `wstd::http::Client`

## Tailwind

Every document loads the unchanged Tailwind **3.4.17** browser runtime from
`/assets/tailwind-3.4.17-176e894661aa.js`. The JavaScript is embedded in the
component with `include_bytes!`; neither builds nor page requests download it.
The route serves `text/javascript; charset=utf-8` with
`Cache-Control: public, max-age=31536000, immutable`. No static-file directory,
registry API access, Node installation, or additional build step is required.

This is the Play CDN runtime served locally, **not precompiled CSS**. It still
generates CSS in the browser, observes added/changed DOM classes, and uses the
existing inline `tailwind.config` in `src/layout.rs`. Its client-side compilation
cost and upstream warning that the Play CDN is not intended for production are
deliberately retained. The script must stay synchronous and precede the inline
configuration and theme initialization.

**Provenance:** the exact bytes were retrieved from
<https://cdn.tailwindcss.com/3.4.17>. Their SHA-256 is
`176e894661aa9cdc9a5cba6c720044cbbf7b8bd80d1c9a142a7c24b1b6c50d15`.
The [license and attributions](assets/tailwind-3.4.17-LICENSE.txt) include the
identified upstream licenses and notices, including Tailwind/Preflight MIT,
didYouMean Apache-2.0, and Can I Use CC BY 4.0 data attribution. The legacy CDN
does not expose a bundle-specific license manifest or source map: this reviewed
record is **not an exhaustive dependency inventory**. Bundled notices are also
retained in the unmodified JavaScript.

The license document is embedded too, served as `text/plain; charset=utf-8`
at `/assets/tailwind-3.4.17-LICENSE.txt` with a one-day cache lifetime, and linked
from the JavaScript response using `Link: ...; rel="license"`. The deployed
component therefore carries these notices without needing separate files.

From the repository root, verify the checked-in bytes with:

```sh
printf '%s  %s\n' \
  '176e894661aa9cdc9a5cba6c720044cbbf7b8bd80d1c9a142a7c24b1b6c50d15' \
  'crates/component-frontend/assets/tailwind-3.4.17-176e894661aa.js' \
  | shasum -a 256 --check
```

To update the runtime, deliberately select an explicit upstream version URL,
record and verify the new checksum, and review its license and compatibility
with the inline config. Keep the upstream bytes, warnings, and license notices
intact. Give any changed bytes a new version/checksum filename and URL, update
`src/tailwind.rs` and this provenance record together. Account for cached HTML
referencing old URLs before retiring superseded assets. Never replace bytes
behind an existing immutable URL. An asset-only
refresh requires pressing Enter in `cargo xtask serve` or rebuilding manually:
the development watcher watches `src/`, not `assets/`.

After an update, check `/docs`, `/design-system`, and a package page using local
test data in a fresh browser context with nonlocal requests blocked. Confirm
there are no attempted remote Tailwind requests, compare responsive grids and
arbitrary utilities, exercise tab/modal class changes and a newly added arbitrary
class, and check system/explicit themes and reduced motion. The existing runtime
warning is expected; JavaScript errors or missing styles are not. Run the
frontend tests and WASI build as well; string-only checks do not establish
rendering equivalence.

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
