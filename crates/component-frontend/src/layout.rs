//! Base HTML document layout.
//!
//! Provides the shared page shell — `<html>`, `<head>`, and `<body>` wrapper —
//! used by all pages.

// r[impl frontend.rendering.html-crate]
// r[impl frontend.styling.tailwind]
// r[impl frontend.styling.light-theme]
// r[impl frontend.styling.accent-color]
// r[impl frontend.styling.responsive]

use crate::footer;

/// Accent color used throughout the UI.
pub(crate) const ACCENT_COLOR: &str = "#18181B";

/// Render a complete HTML document with the given title and body content.
///
/// Includes the shared navigation bar, Tailwind CSS via CDN, custom accent
/// color CSS variables, and footer.
#[allow(dead_code)]
#[must_use]
pub(crate) fn document(title: &str, body_content: &str) -> String {
    document_inner(title, body_content, "", MAIN_CLASS_CENTERED, true)
}

/// Render a complete HTML document with nav bar, title, and body content.
#[must_use]
pub(crate) fn document_with_nav(title: &str, body_content: &str) -> String {
    use crate::components::ds::navbar::{self, NavLink};
    const LINKS: &[NavLink] = &[
        NavLink {
            label: "Docs",
            href: "/docs",
        },
        NavLink {
            label: "Downloads",
            href: "/downloads",
        },
    ];
    let nav = navbar::render_bar_grid(&[], LINKS);
    document_inner(title, body_content, &nav, MAIN_CLASS_CENTERED, true)
}

/// Render a document whose `<body>` is the top-level grid.
///
/// Unlike the classic `flex flex-col` body, this does not wrap the content
/// in a `<main>` and does not apply the default flex body class — the
/// caller provides the body class (expected to be the grid definition) and
/// the body children (e.g. `<header>`, `<aside>`, `<main>`, `<aside>`).
#[must_use]
pub(crate) fn document_grid(title: &str, body_class: &str, body_content: &str) -> String {
    document_inner_grid(title, body_class, body_content)
}

/// Render a standalone design system page (wider container, no nav/footer).
#[must_use]
pub(crate) fn document_design_system(title: &str, body_content: &str) -> String {
    document_inner(title, body_content, "", MAIN_CLASS_CENTERED, false)
}

/// Render the landing-page document — full-width main, sticky navbar, and
/// the rich site footer. The body is expected to provide its own
/// `max-w-[1280px]` containers per section.
#[must_use]
pub(crate) fn document_landing(title: &str, body_content: &str) -> String {
    use crate::components::ds::navbar::{self, NavLink};
    const LINKS: &[NavLink] = &[
        NavLink {
            label: "Packages",
            href: "/all",
        },
        NavLink {
            label: "Docs",
            href: "/docs",
        },
        NavLink {
            label: "Spec",
            href: "/docs",
        },
    ];
    let nav = navbar::render_bar_grid(&[], LINKS);
    document_inner(title, body_content, &nav, MAIN_CLASS_FULL, true)
}

/// Capped main column for non-detail pages — sits in the first grid track
/// (which already caps at 1280px) and supplies its own inline padding.
const MAIN_CLASS_CENTERED: &str = "w-full px-4 md:px-8 pb-12";
/// Full-bleed main for landing pages whose sections cap themselves.
const MAIN_CLASS_FULL: &str = "col-span-full w-full";

/// Default `<body>` class for the simple grid layout (non-detail pages).
///
/// Single column capped at 1280px with a trailing `1fr` whitespace track
/// (matching the holy-grail reference). Header and footer span all tracks
/// via `col-span-full`.
pub(crate) const BODY_CLASS_SIMPLE_GRID: &str = "bg-canvas text-ink-900 min-h-dvh leading-relaxed font-sans antialiased grid grid-rows-[auto_1fr_auto] grid-cols-[minmax(0,1280px)_1fr] [&>*]:min-w-0";

/// `<body>` class for detail pages (header / sidebar / main / toc / footer).
///
/// Mobile-first holy-grail grid (see `references/holy-grail.html`):
/// - mobile: single capped column + 1fr whitespace.
/// - md (≥768): adds 200px sidebar track on the left.
/// - lg (≥1024): adds 200px toc track on the right.
pub(crate) const BODY_CLASS_GRID: &str = "bg-canvas text-ink-900 min-h-dvh leading-relaxed font-sans antialiased grid grid-rows-[auto_1fr_auto] grid-cols-[minmax(0,1280px)_1fr] md:grid-cols-[280px_minmax(0,1000px)_1fr] lg:grid-cols-[280px_minmax(0,780px)_220px_1fr] [&>*]:min-w-0";

/// Inner document renderer for the simple-grid body — header, main, footer
/// emitted as direct grid children of `.page-grid-simple`.
fn document_inner(
    title: &str,
    body_content: &str,
    nav: &str,
    main_class: &str,
    show_footer: bool,
) -> String {
    let footer_html = if show_footer {
        footer::render()
    } else {
        String::new()
    };
    let body_children = format!(
        r#"{nav}
  <main id="content" class="{main_class}">
    {body_content}
  </main>
  {footer_html}"#,
    );
    render_document(title, BODY_CLASS_SIMPLE_GRID, &body_children)
}

/// Inner document renderer for the grid-body layout used by detail pages.
///
/// The caller supplies the full set of body children (`<header>`, `<aside>`,
/// `<main>`, `<aside>`) as a pre-rendered string.
fn document_inner_grid(title: &str, body_class: &str, body_content: &str) -> String {
    render_document(title, body_class, body_content)
}

/// Shared HTML document template — emits the full `<!DOCTYPE html>` …
/// `</html>` shell with the given body class and body children.
fn render_document(title: &str, body_class: &str, body_children: &str) -> String {
    let escaped_title = escape_html_text(title);

    format!(
        r#"<!DOCTYPE html>
<html lang="en" style="view-transition-name:root">
<head>
  <meta charset="utf-8">
  <meta name="color-scheme" content="light dark">
  <style>html{{background:#F4F4F5;scrollbar-gutter:stable}}@media(prefers-color-scheme:dark){{html:not([data-theme=light]){{background:#1C1C20}}}}html[data-theme=dark]{{background:#1C1C20}}html[data-theme=light]{{background:#F4F4F5}}</style>
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <meta name="description" content="Browse and discover WebAssembly components and WIT interfaces published to OCI registries.">
  <title>{escaped_title} — component registry</title>
  <script src="https://cdn.tailwindcss.com"></script>
  <script>
    /* Early theme init — prevent flash of wrong theme */
    (function() {{
      var t = localStorage.getItem('ds-theme');
      if (t === 'dark' || t === 'light') {{
        document.documentElement.setAttribute('data-theme', t);
        document.documentElement.style.background = t === 'dark' ? '#1C1C20' : '#F4F4F5';
      }} else if (window.matchMedia('(prefers-color-scheme: dark)').matches) {{
        document.documentElement.style.background = '#1C1C20';
      }}
    }})();
  </script>
  <script>
    tailwind.config = {{
      theme: {{
        extend: {{
          colors: {{
            /* — New design system tokens — */
            canvas: 'var(--c-canvas)',
            surface: {{
              DEFAULT: 'var(--c-surface)',
              muted:   'var(--c-surface-muted)',
            }},
            surfaceMuted: 'var(--c-surface-muted)',
            ink: {{
              900: 'var(--c-ink-900)',
              700: 'var(--c-ink-700)',
              500: 'var(--c-ink-500)',
              400: 'var(--c-ink-400)',
              300: 'var(--c-ink-300)',
            }},
            line: 'var(--c-line)',
            lineSoft: 'var(--c-line-soft)',
            rule: 'var(--c-rule)',
            positive: 'var(--c-positive)',
            negative: 'var(--c-negative)',
            accent: 'var(--c-accent)',
            cat: {{
              blue: 'var(--c-cat-blue)',       blueInk: 'var(--c-cat-blue-ink)',
              pink: 'var(--c-cat-pink)',       pinkInk: 'var(--c-cat-pink-ink)',
              green: 'var(--c-cat-green)',     greenInk: 'var(--c-cat-green-ink)',
              peach: 'var(--c-cat-peach)',     peachInk: 'var(--c-cat-peach-ink)',
              lilac: 'var(--c-cat-lilac)',     lilacInk: 'var(--c-cat-lilac-ink)',
              cream: 'var(--c-cat-cream)',     creamInk: 'var(--c-cat-cream-ink)',
              teal: 'var(--c-cat-teal)',       tealInk: 'var(--c-cat-teal-ink)',
              rust: 'var(--c-cat-rust)',       rustInk: 'var(--c-cat-rust-ink)',
              plum: 'var(--c-cat-plum)',       plumInk: 'var(--c-cat-plum-ink)',
              slate: 'var(--c-cat-slate)',     slateInk: 'var(--c-cat-slate-ink)',
            }},
            /* WIT semantic colors */
            wit: {{
              struct:   'var(--color-wit-struct)',
              enum:     'var(--color-wit-enum)',
              resource: 'var(--color-wit-resource)',
              func:     'var(--color-wit-func)',
              world:    'var(--color-wit-world)',
              iface:    'var(--color-wit-iface)',
              import:   'var(--color-wit-import)',
              module:   'var(--color-wit-module)',
            }},
          }},
          fontFamily: {{
            sans: ['-apple-system', 'BlinkMacSystemFont', 'system-ui', '"Segoe UI"', '"Helvetica Neue"', 'Helvetica', 'Arial', 'sans-serif'],
            mono: ['ui-monospace', 'SFMono-Regular', 'Menlo', 'Monaco', 'Consolas', 'monospace'],
          }},
          letterSpacing: {{
            display: '-0.06em',
          }},
          fontSize: {{
            sm: ['0.875rem', {{ lineHeight: '1.375rem' }}],
            lg: ['1.125rem', {{ lineHeight: '1.625rem' }}],
          }},
          boxShadow: {{
            tooltip: 'var(--shadow-tooltip)',
            card: 'var(--shadow-card)',
          }},
          borderRadius: {{
            DEFAULT: '3px',
            sm: '2px',
            md: '4px',
            lg: '5px',
            pill: '9999px',
          }},
          transitionTimingFunction: {{
            standard: 'cubic-bezier(0.2, 0, 0, 1)',
            entrance: 'cubic-bezier(0, 0, 0, 1)',
            exit: 'cubic-bezier(0.4, 0, 1, 1)',
            spring: 'cubic-bezier(0.34, 1.56, 0.64, 1)',
          }},
          transitionDuration: {{
            fast: '120ms',
            base: '180ms',
            slow: '260ms',
            page: '360ms',
          }},
        }}
      }}
    }}
  </script>
  <style>
    /* ── Design system tokens ─────────────────────────────── */
    :root {{
      color-scheme: light dark;

      /* Layout chrome */
      --navbar-h: 3rem;  /* must match navbar h-12 */
      --navbar-offset: calc(var(--navbar-h) + 1px); /* clears the navbar's 1px hairline */

      /* Light mode — calm paper, near-black ink, pastel categoricals */
      --c-canvas:        #F4F4F5;
      --c-surface:       #FAFAFA;
      --c-surface-muted: #EDEDEF;
      --c-ink-900:       {ACCENT_COLOR};
      --c-ink-700:       #3F3F46;
      --c-ink-500:       #71717A;
      --c-ink-400:       #A1A1AA;
      --c-ink-300:       #D4D4D8;
      --c-line:          #D4D4D8;
      --c-line-soft:     #E4E4E7;
      --c-positive:      #1F8A4C;
      --c-negative:      #9B4F5E;
      --c-accent:        {ACCENT_COLOR};

      --c-cat-blue:      #D6E4FF;  --c-cat-blue-ink:   #3D5A99;
      --c-cat-pink:      #FBD9DF;  --c-cat-pink-ink:   #9B4F5E;
      --c-cat-green:     #D2ECD8;  --c-cat-green-ink:  #3F7A52;
      --c-cat-peach:     #F8E2C2;  --c-cat-peach-ink:  #8E6529;
      --c-cat-lilac:     #E4DAF1;  --c-cat-lilac-ink:  #6B528F;
      --c-cat-cream:     #F4ECC2;  --c-cat-cream-ink:  #7A6A2A;
      --c-cat-teal:      #BFE3EE;  --c-cat-teal-ink:   #1F6F87;
      --c-cat-rust:      #F4D2C0;  --c-cat-rust-ink:   #9F5536;
      --c-cat-plum:      #E8C5E8;  --c-cat-plum-ink:   #7E2E7E;
      --c-cat-slate:     #DADCE0;  --c-cat-slate-ink:  #535A66;

      --c-rule:          var(--c-ink-900);
      --c-swatch-border: var(--c-line-soft);
      --c-scrollbar:     #D4D4D8;

      --shadow-tooltip:  0 8px 24px -8px rgba(20,22,28,0.35);
      --shadow-card:     0 1px 0 0 rgba(20,22,28,0.04);

      /* WIT syntax coloring */
      --color-wit-struct:   #4338ca;
      --color-wit-enum:     #0d7377;
      --color-wit-resource: #b45309;
      --color-wit-func:     #15803d;
      --color-wit-world:    #9333ea;
      --color-wit-iface:    #0369a1;
      --color-wit-import:   #b91c1c;
      --color-wit-module:   #be185d;

    }}

    @media (prefers-color-scheme: dark) {{
      :root:not([data-theme="light"]) {{
        --c-canvas:        #1C1C20;
        --c-surface:       #26262B;
        --c-surface-muted: #2F2F35;
        --c-ink-900:       #ECECEE;
        --c-ink-700:       #B5B5BB;
        --c-ink-500:       #8B8B92;
        --c-ink-400:       #76767D;
        --c-ink-300:       #4A4A50;
        --c-line:          #3A3A40;
        --c-line-soft:     #323238;
        --c-positive:      #5EC787;
        --c-negative:      #EE7B8E;
        --c-accent:        #8FB1F5;

        --c-cat-blue:      #B8D0FF;  --c-cat-blue-ink:   #1F3F8C;
        --c-cat-pink:      #FFB8B0;  --c-cat-pink-ink:   #9E2823;
        --c-cat-green:     #B5E8C0;  --c-cat-green-ink:  #1F6738;
        --c-cat-peach:     #FBD3A0;  --c-cat-peach-ink:  #7A4E10;
        --c-cat-lilac:     #C6B1F0;  --c-cat-lilac-ink:  #422684;
        --c-cat-cream:     #F5E696;  --c-cat-cream-ink:  #6B5610;
        --c-cat-teal:      #A6DDF0;  --c-cat-teal-ink:   #0F5C7A;
        --c-cat-rust:      #F5BFA0;  --c-cat-rust-ink:   #87401C;
        --c-cat-plum:      #DDB2EF;  --c-cat-plum-ink:   #571485;
        --c-cat-slate:     #C6CDD8;  --c-cat-slate-ink:  #424B5C;

        --c-rule:          #6B6B72;
        --c-swatch-border: #3A3A40;
        --c-scrollbar:     #4A4A50;

        --shadow-tooltip:  0 10px 28px -10px rgba(0,0,0,0.7);
        --shadow-card:     inset 0 1px 0 0 rgba(255,255,255,0.06), 0 1px 0 0 rgba(0,0,0,0.5), 0 8px 16px -12px rgba(0,0,0,0.6);

        /* WIT dark variants — brighter to read against dark canvas */
        --color-wit-struct:   #818cf8;
        --color-wit-enum:     #2dd4bf;
        --color-wit-resource: #fbbf24;
        --color-wit-func:     #4ade80;
        --color-wit-world:    #c084fc;
        --color-wit-iface:    #38bdf8;
        --color-wit-import:   #f87171;
        --color-wit-module:   #f472b6;
      }}
    }}

    :root[data-theme="dark"] {{
      --c-canvas:        #1C1C20;
      --c-surface:       #26262B;
      --c-surface-muted: #2F2F35;
      --c-ink-900:       #ECECEE;
      --c-ink-700:       #B5B5BB;
      --c-ink-500:       #8B8B92;
      --c-ink-400:       #76767D;
      --c-ink-300:       #4A4A50;
      --c-line:          #3A3A40;
      --c-line-soft:     #323238;
      --c-positive:      #5EC787;
      --c-negative:      #EE7B8E;
      --c-accent:        #8FB1F5;

      --c-cat-blue:      #B8D0FF;  --c-cat-blue-ink:   #1F3F8C;
      --c-cat-pink:      #FFB8B0;  --c-cat-pink-ink:   #9E2823;
      --c-cat-green:     #B5E8C0;  --c-cat-green-ink:  #1F6738;
      --c-cat-peach:     #FBD3A0;  --c-cat-peach-ink:  #7A4E10;
      --c-cat-lilac:     #C6B1F0;  --c-cat-lilac-ink:  #422684;
      --c-cat-cream:     #F5E696;  --c-cat-cream-ink:  #6B5610;
      --c-cat-teal:      #A6DDF0;  --c-cat-teal-ink:   #0F5C7A;
      --c-cat-rust:      #F5BFA0;  --c-cat-rust-ink:   #87401C;
      --c-cat-plum:      #DDB2EF;  --c-cat-plum-ink:   #571485;
      --c-cat-slate:     #C6CDD8;  --c-cat-slate-ink:  #424B5C;

      --c-rule:          #6B6B72;
      --c-swatch-border: #3A3A40;
      --c-scrollbar:     #4A4A50;

      --shadow-tooltip:  0 10px 28px -10px rgba(0,0,0,0.7);
      --shadow-card:     inset 0 1px 0 0 rgba(255,255,255,0.06), 0 1px 0 0 rgba(0,0,0,0.5), 0 8px 16px -12px rgba(0,0,0,0.6);

      --color-wit-struct:   #818cf8;
      --color-wit-enum:     #2dd4bf;
      --color-wit-resource: #fbbf24;
      --color-wit-func:     #4ade80;
      --color-wit-world:    #c084fc;
      --color-wit-iface:    #38bdf8;
      --color-wit-import:   #f87171;
      --color-wit-module:   #f472b6;
    }}

    html, body {{
      background-color: var(--c-canvas);
      color: var(--c-ink-900);
      -webkit-font-smoothing: antialiased;
      -moz-osx-font-smoothing: grayscale;
    }}
    /* Consistent focus ring for keyboard navigation */
    :focus-visible {{
      outline: 2px solid var(--c-accent);
      outline-offset: 2px;
    }}
    :focus:not(:focus-visible) {{
      outline: none;
    }}
    ::selection {{
      background: color-mix(in oklab, var(--c-accent) 35%, transparent);
      color: var(--c-ink-900);
    }}
    @view-transition {{
      navigation: auto;
    }}
    ::view-transition-old(root) {{
      animation: none;
    }}
    ::view-transition-new(root) {{
      animation: none;
    }}
    @media (prefers-reduced-motion: reduce) {{
      ::view-transition-old(root),
      ::view-transition-new(root) {{
        animation: none;
      }}
    }}
    /* Card hover — pop out with scale, shadow, and strong border */
    .card-lift {{
      transition: transform 120ms cubic-bezier(0.2, 0, 0, 1), box-shadow 120ms cubic-bezier(0.2, 0, 0, 1);
      transform-origin: center center;
    }}
    /* Prose styling for rendered markdown documentation */
    .prose-doc h1 {{
      font-size: 28px;
      line-height: 1.15;
      font-weight: 600;
      letter-spacing: -0.01em;
      margin-bottom: 1rem;
    }}
    .prose-doc h2 {{
      font-size: 22px;
      font-weight: 600;
      letter-spacing: -0.01em;
      margin-top: 2rem;
      margin-bottom: 0.75rem;
    }}
    .prose-doc h3 {{
      font-size: 18px;
      font-weight: 600;
      letter-spacing: -0.01em;
      margin-top: 1.5rem;
      margin-bottom: 0.5rem;
    }}
    .prose-doc h4 {{
      font-size: 16px;
      font-weight: 600;
      margin-top: 1.25rem;
      margin-bottom: 0.5rem;
    }}
    .prose-doc p {{
      margin-bottom: 0.75em;
    }}
    .prose-doc p:last-child {{
      margin-bottom: 0;
    }}
    .prose-doc code {{
      background: var(--c-surface-muted);
      padding: 0.1em 0.3em;
      font-size: 0.9em;
    }}
    .prose-doc a {{
      color: var(--c-accent);
      text-decoration: underline;
      text-underline-offset: 2px;
    }}
    .prose-doc a:hover {{
      opacity: 0.8;
    }}
    .prose-doc ul, .prose-doc ol {{
      margin: 0.5em 0;
      padding-left: 1.5em;
    }}
    .prose-doc li {{
      margin-bottom: 0.25em;
    }}
    .prose-doc pre {{
      background: var(--c-surface-muted);
      padding: 0.75em 1em;
      overflow-x: auto;
      margin: 0.75em 0;
      font-size: 0.875em;
    }}
    .card-lift:hover {{
      transform: scale(1.03);
      box-shadow: var(--shadow-card);
      z-index: 1;
      position: relative;
      outline: 2px solid var(--c-ink-900);
      outline-offset: -2px;
    }}
    @media (prefers-reduced-motion: reduce) {{
      .card-lift {{ transition: none; }}
      .card-lift:hover {{ transform: none; box-shadow: none; }}
    }}
    /* Card kind variants — thin left border for categorization */
    .card-interface {{
      border-left: 2px solid var(--color-wit-iface);
    }}
    .card-component {{
      border-left: 2px solid var(--c-accent);
    }}
    /* Copy hint */
    .copy-hint {{
      cursor: pointer;
      position: relative;
    }}
    .copy-hint::after {{
      content: 'click to copy';
      position: absolute;
      right: -0.25rem;
      top: 50%;
      transform: translateX(100%) translateY(-50%);
      font-size: 0.65rem;
      color: var(--c-ink-400);
      opacity: 0;
      transition: opacity 0.15s;
      white-space: nowrap;
      pointer-events: none;
    }}
    .copy-hint:hover::after {{
      opacity: 1;
    }}
    .copy-hint.copied::after {{
      content: 'copied!';
      color: var(--c-accent);
      opacity: 1;
    }}
    @media (prefers-reduced-motion: reduce) {{
      .copy-hint::after {{ transition: none; }}
    }}
    /* Keyboard shortcut badge — inside search input, Linear-style */
    .search-kbd {{
      position: absolute;
      right: 0.5rem;
      top: 50%;
      transform: translateY(-50%);
      display: inline-flex;
      align-items: center;
      justify-content: center;
      width: 1.5rem;
      height: 1.5rem;
      border: 2px solid var(--c-line);
      border-radius: 0;
      font-size: 0.8125rem;
      font-family: inherit;
      color: var(--c-ink-500);
      background: var(--c-surface-muted);
      line-height: 1;
      pointer-events: none;
      transition: opacity 0.1s;
    }}
    .search-form:focus-within .search-kbd {{
      opacity: 0;
      pointer-events: none;
    }}
    /* Search carousel placeholder */
    .search-carousel {{
      position: absolute;
      left: 1rem;
      top: 50%;
      transform: translateY(-50%);
      font-size: 1rem;
      color: var(--c-ink-400);
      pointer-events: none;
      white-space: nowrap;
      overflow: hidden;
      transition: opacity 0.3s cubic-bezier(0.25, 1, 0.5, 1);
    }}
    .carousel-word {{
      display: inline;
    }}
    @media (prefers-reduced-motion: reduce) {{
      .carousel-word {{
        transition: none;
      }}
    }}
    /* Tab buttons — pill style, managed via Tailwind classes.
       The .tab-btn class is only used as a JS selector. */
    .tab-btn {{
      cursor: pointer;
      transition: background-color 0.15s, color 0.15s;
    }}
    ::-webkit-scrollbar {{
      width: 10px;
      height: 10px;
    }}
    ::-webkit-scrollbar-thumb {{
      background: var(--c-scrollbar);
      border-radius: 999px;
    }}
    @media (prefers-reduced-motion: reduce) {{
      .tab-btn {{ transition: none; }}
    }}

    /* ── Design system component styles ─────────────────── */
    input:focus-visible, select:focus-visible, textarea:focus-visible {{ outline: none; }}
    .hairline {{ border-color: var(--c-line-soft); }}

    /* Body grid is defined entirely in Tailwind utility classes on
       `<body>` (see BODY_CLASS_GRID / BODY_CLASS_SIMPLE_GRID). */

    .rule {{ border-color: var(--c-rule) !important; border-top-width: 1.5px !important; }}
    .swatch {{ height: 88px; border-radius: 5px; border: 1px solid var(--c-swatch-border); }}
    .mono {{ font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace; }}
    .bar {{ height: 28px; border-radius: 4px; display: inline-flex; align-items: center; padding: 0 12px; font-size: 12px; font-weight: 500; white-space: nowrap; }}
    .bar-sm {{ height: 20px; border-radius: 3px; display: inline-flex; align-items: center; padding: 0 6px; font-size: 11px; font-weight: 500; white-space: nowrap; }}
    /* Motion preview helpers */
    .ease-curve {{ height: 56px; width: 100%; display: block; }}
    .ease-curve path.track {{ stroke: var(--c-line-soft); fill: none; stroke-width: 1; }}
    .ease-curve path.curve {{ stroke: var(--c-ink-900); fill: none; stroke-width: 1.5; stroke-linecap: round; }}
    .motion-target {{ height: 32px; width: 32px; border-radius: 5px; background: var(--c-ink-900); will-change: transform, opacity; }}
    .motion-track:hover .motion-target {{ transform: translateX(184px); }}
    .motion-target.t-fast {{ transition: transform 120ms cubic-bezier(0.2, 0, 0, 1); }}
    .motion-target.t-base {{ transition: transform 180ms cubic-bezier(0.2, 0, 0, 1); }}
    .motion-target.t-slow {{ transition: transform 260ms cubic-bezier(0.2, 0, 0, 1); }}
    .motion-target.t-spring {{ transition: transform 360ms cubic-bezier(0.34, 1.56, 0.64, 1); }}
    @media (prefers-reduced-motion: reduce) {{ .motion-target {{ transition: none !important; }} }}
    /* Search modal */
    .search-modal {{ position: fixed; inset: 0; z-index: 50; display: flex; align-items: flex-start; justify-content: center; padding-top: 4px; }}
    .search-modal.hidden {{ display: none; }}
    .search-scrim {{ position: absolute; inset: 0; background: rgba(15, 15, 17, 0.4); backdrop-filter: blur(2px); }}
    .search-dialog {{ position: relative; width: 100%; max-width: 600px; margin: 0 16px; background: var(--c-surface); border: 1px solid var(--c-line); border-radius: 12px; box-shadow: 0 16px 48px rgba(0,0,0,.2); overflow: hidden; }}
    .search-input-row {{ display: flex; align-items: center; gap: 10px; padding: 0 16px; height: 48px; border-bottom: 1px solid var(--c-line-soft); }}
    .search-hint {{ padding: 10px 16px; font-size: 12px; color: var(--c-ink-500); }}
    /* Sigil */
    .sigil {{ display: inline-flex; align-items: center; justify-content: center; height: 18px; width: 18px; border-radius: 3px; font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace; font-size: 10px; font-weight: 600; line-height: 1; flex-shrink: 0; text-transform: uppercase; }}
    /* Tree-link */
    .tree-link {{ display: flex; align-items: flex-start; gap: 8px; padding: 3px 8px 3px 0; border-radius: 4px; font-size: 13px; color: var(--c-ink-700); text-decoration: none; line-height: 1.4; min-width: 0; }}
    .tree-link .mono {{ font-size: 12.5px; min-width: 0; overflow-wrap: break-word; word-break: break-all; }}
    .tree-link:hover {{ background: var(--c-surface-muted); color: var(--c-ink-900); }}
    .tree-link.active {{ background: var(--c-surface-muted); color: var(--c-ink-900); font-weight: 500; }}
    .tree-link .chev {{ width: 10px; height: 10px; flex-shrink: 0; color: var(--c-ink-400); transition: transform 120ms cubic-bezier(.2, 0, 0, 1); margin-top: 4px; }}
    .tree-link::before {{ content: ""; width: 10px; flex-shrink: 0; }}
    .tree-link .project-icon {{ display: inline-flex; align-items: center; height: 18px; flex-shrink: 0; }}
    summary.tree-link::before {{ display: none; }}
    details[open]>summary .chev {{ transform: rotate(90deg); }}
    details>summary {{ list-style: none; cursor: pointer; }}
    details>summary::-webkit-details-marker {{ display: none; }}
    .tree-children {{ margin-left: 5px; padding-left: 17px; border-left: 1px solid var(--c-line-soft); margin-top: 1px; }}
    /* TOC link */
    .toc-link {{ display: block; padding: 3px 10px; font-size: 12px; color: var(--c-ink-500); border-left: 1.5px solid transparent; line-height: 1.5; text-decoration: none; }}
    .toc-link:hover {{ color: var(--c-ink-900); border-left-color: var(--c-line); }}
    .toc-link.active {{ color: var(--c-ink-900); border-left-color: var(--c-ink-900); }}
    .toc-link.indent {{ padding-left: 22px; }}
    /* Detail page: sidebar spans content + footer rows; footer skips sidebar column */
    .detail-footer {{ grid-column: 1 / -1; }}
    @media (min-width: 768px) {{
      .detail-footer {{ grid-column: 2 / -1; }}
      .detail-sidebar {{ grid-row: 2 / 4; }}
    }}
    /* Item list */
    .item-list {{ margin: 0; background: var(--c-surface); border-radius: 8px; overflow: hidden; }}
    .item-row {{ display: flex; gap: 12px; align-items: baseline; padding: 12px 14px; border-top: 1px solid var(--c-surface-muted); color: inherit; text-decoration: none; }}
    .item-row:first-child {{ border-top: none; }}
    .item-row:hover {{ background: var(--c-surface-muted); }}
    .item-row.deprecated .name, .item-row.deprecated .desc {{ color: var(--c-ink-400); }}
    .item-row.deprecated .name {{ text-decoration: line-through; text-decoration-thickness: 1px; }}
    .item-row.deprecated .sigil {{ opacity: .5; }}
    .item-row > div {{ flex: 1; min-width: 0; }}
    .item-row .name {{ font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace; font-size: 13.5px; color: var(--c-ink-900); font-weight: 500; text-decoration: none; }}
    .item-row:hover .name {{ color: var(--c-ink-900); }}
    .item-row .desc {{ color: var(--c-ink-700); font-size: 13px; line-height: 1.55; margin-top: 2px; }}
    .item-row .meta {{ font-size: 11px; color: var(--c-ink-500); font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace; flex-shrink: 0; }}
    /* Item details */
    .id-header {{ display: flex; align-items: center; flex-wrap: wrap; gap: 12px; padding: 10px 12px; border-radius: 5px; background: var(--c-canvas); border: 1px solid var(--c-line-soft); }}
    .id-kind {{ display: inline-flex; align-items: center; justify-content: center; height: 22px; min-width: 64px; padding: 0 8px; border-radius: 4px; font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace; font-size: 11px; font-weight: 600; letter-spacing: 0.04em; }}
    .id-kind-unary {{ background: var(--c-cat-blue); color: var(--c-cat-blue-ink); }}
    .id-kind-server {{ background: var(--c-cat-peach); color: var(--c-cat-peach-ink); }}
    .id-kind-client {{ background: var(--c-cat-lilac); color: var(--c-cat-lilac-ink); }}
    .id-kind-bidi {{ background: var(--c-cat-pink); color: var(--c-cat-pink-ink); }}
    .id-path {{ font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace; font-size: 13px; }}
    .id-path .pkg {{ color: var(--c-ink-400); }}
    .id-path .svc {{ color: var(--c-ink-900); }}
    .id-path .sl {{ color: var(--c-ink-400); }}
    .id-path .meth {{ color: var(--c-cat-plum-ink); font-weight: 600; }}
    .id-path .seg {{ color: var(--c-ink-900); }}
    .id-path .par {{ color: var(--c-cat-plum-ink); }}
    .id-method {{ display: inline-flex; align-items: center; justify-content: center; height: 22px; min-width: 64px; padding: 0 8px; border-radius: 4px; font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace; font-size: 11px; font-weight: 600; letter-spacing: 0.04em; }}
    .id-method-get {{ background: var(--c-cat-blue); color: var(--c-cat-blue-ink); }}
    .id-method-post {{ background: var(--c-cat-green); color: var(--c-cat-green-ink); }}
    .id-method-put {{ background: var(--c-cat-peach); color: var(--c-cat-peach-ink); }}
    .id-method-patch {{ background: var(--c-cat-lilac); color: var(--c-cat-lilac-ink); }}
    .id-method-delete {{ background: var(--c-cat-pink); color: var(--c-cat-pink-ink); }}
    .id-auth-tag {{ display: inline-flex; align-items: center; gap: 6px; height: 24px; padding: 0 8px; border-radius: 9999px; background: var(--c-cat-cream); color: var(--c-cat-cream-ink); font-size: 11px; font-weight: 500; margin-left: auto; }}
    .id-auth-tag::before {{ content: ''; display: inline-block; height: 6px; width: 6px; border-radius: 999px; background: var(--c-cat-cream-ink); }}
    .id-http-status {{ display: inline-flex; align-items: center; justify-content: center; height: 22px; min-width: 44px; padding: 0 8px; border-radius: 4px; font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace; font-size: 11.5px; font-weight: 600; letter-spacing: 0.02em; flex: none; }}
    .id-http-status-2xx {{ background: var(--c-cat-green); color: var(--c-cat-green-ink); }}
    .id-http-status-3xx {{ background: var(--c-cat-blue); color: var(--c-cat-blue-ink); }}
    .id-http-status-4xx {{ background: var(--c-cat-peach); color: var(--c-cat-peach-ink); }}
    .id-http-status-5xx {{ background: var(--c-cat-pink); color: var(--c-cat-pink-ink); }}
    /* Code block */
    pre.id-code {{ background: var(--c-surface); border: 1px solid var(--c-line-soft); border-radius: 5px; padding: 14px 16px; font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace; font-size: 12.5px; line-height: 1.55; overflow-x: auto; color: var(--c-ink-900); margin: 0; }}
    pre.id-code .k {{ color: var(--color-wit-struct); }}
    pre.id-code .s {{ color: var(--color-wit-resource); }}
    pre.id-code .n {{ color: var(--color-wit-func); }}
    pre.id-code .c {{ color: var(--c-ink-500); font-style: italic; }}
    pre.id-code .p {{ color: var(--c-ink-400); }}
    pre.id-code .ty {{ color: var(--color-wit-world); }}
    pre.id-code .fn {{ color: var(--color-wit-iface); }}
    pre.id-code .at {{ color: var(--color-wit-iface); }}
    pre.id-code .v {{ color: var(--c-ink-900); font-weight: 600; }}
    pre.id-code .f {{ color: var(--color-wit-struct); }}
    pre.id-code .h {{ color: var(--color-wit-module); font-weight: 600; }}
    /* Dark-mode code overrides */
    [data-theme="dark"] pre.id-code .k {{ color: #6699cc; }}
    [data-theme="dark"] pre.id-code .f {{ color: #6699cc; }}
    [data-theme="dark"] pre.id-code .s {{ color: #99cc99; }}
    [data-theme="dark"] pre.id-code .n {{ color: #f99157; }}
    [data-theme="dark"] pre.id-code .ty {{ color: #ffcc66; }}
    [data-theme="dark"] pre.id-code .fn {{ color: #66cccc; }}
    [data-theme="dark"] pre.id-code .at {{ color: #66cccc; }}
    [data-theme="dark"] pre.id-code .h {{ color: #cc99cc; }}
    [data-theme="dark"] pre.id-code .c {{ color: #999999; }}
    [data-theme="dark"] pre.id-code .p {{ color: #888888; }}
    [data-theme="dark"] pre.id-code .v {{ color: #cccccc; }}
    @media (prefers-color-scheme: dark) {{
      :root:not([data-theme="light"]) pre.id-code .k {{ color: #6699cc; }}
      :root:not([data-theme="light"]) pre.id-code .f {{ color: #6699cc; }}
      :root:not([data-theme="light"]) pre.id-code .s {{ color: #99cc99; }}
      :root:not([data-theme="light"]) pre.id-code .n {{ color: #f99157; }}
      :root:not([data-theme="light"]) pre.id-code .ty {{ color: #ffcc66; }}
      :root:not([data-theme="light"]) pre.id-code .fn {{ color: #66cccc; }}
      :root:not([data-theme="light"]) pre.id-code .at {{ color: #66cccc; }}
      :root:not([data-theme="light"]) pre.id-code .h {{ color: #cc99cc; }}
      :root:not([data-theme="light"]) pre.id-code .c {{ color: #999999; }}
      :root:not([data-theme="light"]) pre.id-code .p {{ color: #888888; }}
      :root:not([data-theme="light"]) pre.id-code .v {{ color: #cccccc; }}
    }}
    /* Item title */
    .id-title-head {{ display: flex; align-items: baseline; justify-content: space-between; gap: 16px; flex-wrap: wrap; margin-bottom: 8px; }}
    .id-title-head h2 {{ font-size: 24px; font-weight: 600; letter-spacing: -0.02em; display: flex; align-items: baseline; gap: 6px; margin: 0; }}
    .id-title-head .id-anchor {{ color: var(--c-ink-400); text-decoration: none; font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace; font-size: 14px; margin-left: 6px; opacity: 0; transition: opacity .12s ease; }}
    .id-title-head h2:hover .id-anchor, .id-title-head .id-anchor:hover {{ opacity: 1; color: var(--c-ink-900); }}
    .id-title-head .id-aux {{ display: flex; align-items: center; gap: 6px; color: var(--c-ink-500); }}
    .id-since-tag {{ display: inline-flex; align-items: center; font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace; font-size: 10.5px; color: var(--c-ink-500); padding: 1px 6px; border: 1px solid var(--c-line-soft); border-radius: 3px; }}
    .id-src-link {{ font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace; font-size: 11px; color: var(--c-ink-500); text-decoration: none; padding: 2px 6px; border-radius: 3px; }}
    .id-src-link:hover {{ color: var(--c-ink-900); background: var(--c-surface-muted); }}
    .id-page-tagline {{ color: var(--c-ink-700); font-size: 14px; line-height: 1.6; margin: 0; max-width: 72ch; }}
    /* Status dots */
    .id-status-dot {{ display: inline-block; height: 8px; width: 8px; border-radius: 999px; }}
    .id-status-ok {{ background: var(--c-cat-green-ink); }}
    .id-status-client {{ background: var(--c-cat-peach-ink); }}
    .id-status-server {{ background: var(--c-cat-pink-ink); }}
    .id-status-cancelled {{ background: var(--c-cat-slate-ink); }}
    /* Language tabs */
    .id-lang-tabs {{ display: flex; align-items: center; gap: 0; border-bottom: 1px solid var(--c-line-soft); margin-bottom: -1px; }}
    .id-lang-tab {{ display: inline-flex; align-items: center; gap: 6px; height: 30px; padding: 0 10px; font-size: 12px; color: var(--c-ink-500); border: 1px solid transparent; border-bottom: none; border-top-left-radius: 4px; border-top-right-radius: 4px; position: relative; top: 1px; cursor: default; transition: color 120ms ease, background 120ms ease, border-color 120ms ease; }}
    .id-lang-tab + .id-lang-tab {{ margin-left: -1px; }}
    .id-lang-tab.is-clickable {{ cursor: pointer; user-select: none; }}
    .id-lang-tab.is-clickable:not(.is-active):hover {{ color: var(--c-ink-900); background: var(--c-surface-muted); border-color: var(--c-line-soft); }}
    .id-lang-tab.is-clickable:not(.is-active):hover .dot {{ background: var(--c-ink-500); }}
    .id-lang-tab.is-clickable:focus-visible {{ outline: 2px solid var(--c-ink-900); outline-offset: -2px; }}
    .id-lang-tab.is-active {{ color: var(--c-ink-900); background: var(--c-surface); border-color: var(--c-line-soft); }}
    .id-lang-tab.is-soon {{ color: var(--c-ink-400); }}
    .id-lang-tab .dot {{ display: inline-block; height: 6px; width: 6px; border-radius: 999px; background: var(--c-ink-300); }}
    .id-lang-tab.is-active .dot {{ background: var(--c-positive); }}
    .id-lang-panel pre.id-code {{ border-top-left-radius: 0; }}
    /* Animations */
    @keyframes ds-spin {{ to {{ transform: rotate(360deg); }} }}
    .ds-spinner {{ animation: ds-spin 0.8s linear infinite; }}
    @keyframes ds-pulse {{ 0%, 100% {{ opacity: 1; }} 50% {{ opacity: .5; }} }}
    .ds-skel {{ animation: ds-pulse 1.4s ease-in-out infinite; }}
  </style>
</head>
<body class="{body_class}">
  {body_children}
  {search_modal}
  <script>
    /* Search command palette */
    (function() {{
      var modal = document.getElementById('search-modal');
      if (!modal) return;
      var input = document.getElementById('search-modal-input');
      var scrim = modal.querySelector('.search-scrim');
      var closeHint = document.getElementById('search-close-hint');
      function open() {{
        modal.classList.remove('hidden');
        if (input) input.focus();
      }}
      function close() {{
        modal.classList.add('hidden');
        if (input) input.value = '';
      }}
      document.querySelectorAll('.search-trigger').forEach(function(btn) {{
        btn.addEventListener('click', open);
      }});
      if (scrim) scrim.addEventListener('click', close);
      if (closeHint) closeHint.addEventListener('click', close);
      document.addEventListener('keydown', function(e) {{
        if (modal.classList.contains('hidden')) {{
          var el = document.activeElement;
          var tag = el && el.tagName;
          var inField = tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT' || (el && el.isContentEditable);
          if (e.key === '/' && !e.ctrlKey && !e.metaKey && !e.altKey && !inField) {{
            e.preventDefault(); open();
          }}
        }} else {{
          if (e.key === 'Escape' || e.key === '/') {{ e.preventDefault(); close(); }}
        }}
      }});
    }})();
    // Click-to-copy for install hint
    document.addEventListener('click', function(e) {{
      var el = e.target.closest('.copy-hint');
      if (!el) return;
      var text = el.textContent || '';
      if (navigator.clipboard) {{
        navigator.clipboard.writeText(text).then(function() {{
          el.classList.add('copied');
          setTimeout(function() {{ el.classList.remove('copied'); }}, 1200);
        }});
      }}
    }});
    // Tab switching
    document.addEventListener('click', function(e) {{
      var btn = e.target.closest('.tab-btn');
      if (!btn) return;
      var group = btn.closest('.tab-group');
      if (!group) return;
      var tab = btn.getAttribute('data-tab');
      var activeClass = 'bg-ink-900 text-canvas font-medium';
      var inactiveClass = 'bg-surfaceMuted text-ink-700 hover:bg-ink-300';
      // Update tab buttons
      group.querySelectorAll('.tab-btn').forEach(function(b) {{
        var isActive = b === btn;
        b.setAttribute('aria-selected', isActive ? 'true' : 'false');
        activeClass.split(' ').forEach(function(c) {{
          if (isActive) b.classList.add(c); else b.classList.remove(c);
        }});
        inactiveClass.split(' ').forEach(function(c) {{
          if (isActive) b.classList.remove(c); else b.classList.add(c);
        }});
      }});
      // Show/hide panels
      group.querySelectorAll('.tab-panel').forEach(function(p) {{
        if (p.id === 'panel-' + tab) {{
          p.style.display = '';
        }} else {{
          p.style.display = 'none';
        }}
      }});
    }});
    // Search placeholder carousel
    (function() {{
      var words = [
        'components\u2026',
        'interfaces\u2026',
        'libraries\u2026',
        'plugins\u2026',
        'servers\u2026',
        'tools\u2026',
        'apps\u2026',
        'extensions\u2026',
        'handlers\u2026',
        'services\u2026',
        'applets\u2026',
        'clients\u2026',
        'addons\u2026',
        'modules\u2026',
        'packages\u2026',
        'widgets\u2026',
        'expansions\u2026',
        'augmentations\u2026',
        'supplements\u2026',
        'accessories\u2026',
        'middleware\u2026',
        'hooks\u2026',
        'mods\u2026',
        'bundles\u2026',
        'toolkits\u2026',
        'SDKs\u2026',
        'adapters\u2026',
        'drivers\u2026',
        'providers\u2026',
        'connectors\u2026',
        'shims\u2026',
        'polyfills\u2026',
      ];
      var el = document.getElementById('carousel-word');
      var overlay = document.getElementById('search-carousel');
      var input = document.getElementById('search-input');
      if (!el || !overlay || !input) return;
      var idx = 0;
      var reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
      function updateVisibility() {{
        var hasValue = input.value.length > 0;
        overlay.style.opacity = hasValue ? '0' : '';
      }}
      input.addEventListener('input', updateVisibility);
      input.addEventListener('focus', updateVisibility);
      input.addEventListener('blur', updateVisibility);
      updateVisibility();

      var currentWord = words[idx];
      el.textContent = currentWord;
      var typing = false;

      function jitter() {{
        return 50 + Math.random() * 90;
      }}

      function deleteWord(cb) {{
        var text = el.textContent;
        if (text.length === 0) {{ cb(); return; }}
        typing = true;
        var first = true;
        function step() {{
          text = text.slice(0, -1);
          el.textContent = text;
          if (text.length > 0) {{
            if (first) {{
              first = false;
              setTimeout(step, 300);
            }} else {{
              setTimeout(step, 20 + Math.random() * 25);
            }}
          }} else {{
            typing = false;
            cb();
          }}
        }}
        setTimeout(step, 20);
      }}

      function typeWord(word, cb) {{
        var i = 0;
        typing = true;
        function step() {{
          i++;
          el.textContent = word.slice(0, i);
          if (i < word.length) {{
            setTimeout(step, jitter());
          }} else {{
            typing = false;
            if (cb) cb();
          }}
        }}
        setTimeout(step, jitter());
      }}

      function cycle() {{
        if (input.value || typing) return;
        deleteWord(function() {{
          setTimeout(function() {{
            var next = idx;
            while (next === idx) next = Math.floor(Math.random() * words.length);
            idx = next;
            typeWord(words[idx]);
          }}, reducedMotion ? 0 : 200);
        }});
      }}

      setInterval(cycle, 5000);
    }})();
  </script>
  <script>
    /* Theme dropdown */
    (function() {{
      var trigger = document.getElementById('theme-trigger');
      var menu = document.getElementById('theme-menu');
      if (!trigger || !menu) return;
      var root = document.documentElement;
      var mq = window.matchMedia('(prefers-color-scheme: dark)');
      var stored = localStorage.getItem('ds-theme');
      var current = (stored === 'dark' || stored === 'light') ? stored : 'auto';

      function updateIcon(mode) {{
        document.querySelectorAll('.theme-icon').forEach(function(el) {{ el.style.display = 'none'; }});
        document.querySelectorAll('.theme-icon-' + mode).forEach(function(el) {{ el.style.display = ''; }});
      }}
      function apply(mode) {{
        current = mode;
        updateIcon(mode);
        if (mode === 'auto') {{
          root.removeAttribute('data-theme');
          root.style.background = mq.matches ? '#1C1C20' : '#F4F4F5';
          localStorage.removeItem('ds-theme');
        }} else {{
          root.setAttribute('data-theme', mode);
          root.style.background = mode === 'dark' ? '#1C1C20' : '#F4F4F5';
          localStorage.setItem('ds-theme', mode);
        }}
      }}
      updateIcon(current);
      trigger.addEventListener('click', function(e) {{
        e.stopPropagation();
        var open = !menu.classList.contains('hidden');
        menu.classList.toggle('hidden');
        trigger.setAttribute('aria-expanded', !open);
      }});
      menu.querySelectorAll('.theme-option').forEach(function(btn) {{
        btn.addEventListener('click', function() {{
          apply(this.getAttribute('data-theme-value'));
          menu.classList.add('hidden');
          trigger.setAttribute('aria-expanded', 'false');
        }});
      }});
      document.addEventListener('click', function() {{
        menu.classList.add('hidden');
        trigger.setAttribute('aria-expanded', 'false');
      }});
      mq.addEventListener('change', function() {{
        if (current === 'auto') {{
          root.style.background = mq.matches ? '#1C1C20' : '#F4F4F5';
        }}
      }});
    }})();
  </script>
</body>
</html>"#,
        escaped_title = escaped_title,
        body_class = body_class,
        body_children = body_children,
        search_modal = crate::components::ds::navbar::render_search_modal(),
    )
}

#[must_use]
fn escape_html_text(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#x27;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    // r[verify frontend.rendering.html-crate]
    // r[verify frontend.styling.tailwind]
    // r[verify frontend.styling.light-theme]
    // r[verify frontend.styling.accent-color]
    // r[verify frontend.styling.responsive]
    #[test]
    fn document_includes_expected_rendering_and_styling_primitives() {
        let html = document("Home", "<p>Body</p>");
        assert!(html.contains("<html lang=\"en\""));
        assert!(html.contains("https://cdn.tailwindcss.com"));
        assert!(html.contains(ACCENT_COLOR));
        assert!(html.contains("<meta name=\"viewport\""));
        assert!(html.contains("bg-canvas text-ink-900"));
        assert!(html.contains("html, body"));
        assert!(html.contains("background-color: var(--c-canvas);"));
        assert!(html.contains("color: var(--c-ink-900);"));
        // Dark mode infrastructure
        assert!(html.contains("prefers-color-scheme: dark"));
        assert!(html.contains("data-theme"));
    }
}
