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

## Package listings

Search results, namespace pages, and the all-packages page share a two-row item:
`namespace/package-name` and description above kind, version, direct dependent
count, and relative last-update time. Identities and metadata wrap on narrow
screens, and version tags are never truncated. Descriptions are visually
truncated to one line with an ellipsis; the full summary remains accessible and
the source description is available on hover.

The homepage's New releases, New packages, and Popular packages cards link
directly to the displayed version at `/{namespace}/{name}/{version}`, without
`registry` or `repository` query parameters. New releases keep their individual
release versions rather than redirecting to the latest release. Packages without
a version retain the latest-version link; packages without a WIT identity remain
unlinked.

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

Shell examples retain `--proto '=https'` so redirects cannot downgrade script
retrieval to HTTP; the old TLS-version flag is omitted.

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

## Relationship pages

Package pages group **Dependents**, **Imported by**, and **Exported by** links
separately from the left sidebar's item tree, without a visible section heading.
Each applicable link has the same small, decorative arrow directly after its
label, using a 4px inline gap, to indicate navigation to a search page in
the current tab. The full row remains clickable. The group retains an accessible
Relationships name. Dependents always targets the package; import/export links
appear for interface packages and individual interfaces, where they are scoped
to that interface. Links remain available when there are no matches.

On mobile, the navbar menu button opens that same sidebar and relationship
group in the style guide's C06 drawer, preserving its C01 navigation and
expanded groups. Relationship links are not duplicated in the page content.
The drawer closes with its close button, Escape, the scrim, or a switch to
desktop width. Keyboard focus stays within the open drawer and returns to its
menu button on close.

The dedicated result pages use fixed queries, separate from ordinary text search:

| Route | Results |
| --- | --- |
| `/search/dependents?package=wasi%3Aio` | Direct and transitive dependent packages |
| `/search/imported-by?package=wasi%3Aio` | Worlds importing any interface from the package |
| `/search/exported-by?package=wasi%3Aio&interface=streams` | Worlds exporting the named interface |

Both world queries accept an optional `interface` parameter. Package identities
are version-independent `namespace:name` values. The old versioned
`/{namespace}/{name}/{version}/dependents` route redirects to the new page.

Results search all indexed releases and show each package or world once, at its
newest **matching** release rather than an unrelated newer release. Result links
carry the OCI registry and repository to disambiguate mirrors. Worlds embedded
in compiled components link to the owning component page using the API's
`is_synthetic` flag, even when package kind is unknown. Authored worlds named
`root` retain their own world-detail link. Relationship rows keep matching-release
metadata separate from ordinary listings' latest-release ages and direct counts.

Once a repository is resolved, generated package-local links retain that source
through the sidebar, breadcrumbs, version selector, WIT references, interfaces,
types, functions, worlds, and child modules/components. Latest-version and legacy
dependency redirects preserve it too. Missing sources, mismatched WIT identities,
and unavailable tags never fall back to another mirror. Links to other packages
and version-independent relationship searches remain unscoped.

All three pages accept `offset` and `limit` (default 100, capped at 100).
Pagination follows the API's deduplicated result page, independently of optional
display totals. Empty results, out-of-range pages, invalid queries, and registry
failures have distinct responses; upstream failures are not cached as empty
results.

Relationships reflect indexed WIT declarations, not dependency-range solving.
Only Dependents follows transitive relationships. Import/export searches use
each world's own declarations, and completeness depends on what the index has
extracted.

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
