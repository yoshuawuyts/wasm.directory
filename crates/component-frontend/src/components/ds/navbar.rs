//! C06 — Navbar.
//!
//! The production navbar is [`render_bar`]: a sticky translucent header
//! with home link, breadcrumbs, search trigger, nav links, and theme toggle.
//! The [`render`] function produces the design-system showcase page section.

use html::text_content::Division;

const SVG_HOME: &str = concat!(
    r#"<svg class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">"#,
    include_str!("../../../../../vendor/lucide/house.svg"),
    "</svg>"
);
const SVG_CHEV_RIGHT: &str = concat!(
    r#"<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="text-ink-300 flex-shrink-0" aria-hidden="true">"#,
    include_str!("../../../../../vendor/lucide/chevron-right.svg"),
    "</svg>"
);
const SVG_SEARCH_SM: &str = concat!(
    r#"<svg class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">"#,
    include_str!("../../../../../vendor/lucide/search.svg"),
    "</svg>"
);
const SVG_MOON_SM: &str = concat!(
    r#"<svg class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
    include_str!("../../../../../vendor/lucide/moon.svg"),
    "</svg>"
);
const SVG_HAMBURGER: &str = concat!(
    r#"<svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">"#,
    include_str!("../../../../../vendor/lucide/menu.svg"),
    "</svg>"
);
const SVG_SEARCH_LG: &str = concat!(
    r#"<svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">"#,
    include_str!("../../../../../vendor/lucide/search.svg"),
    "</svg>"
);
const SVG_CLOSE: &str = concat!(
    r#"<svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">"#,
    include_str!("../../../../../vendor/lucide/x.svg"),
    "</svg>"
);
const SVG_HOME_NAV: &str = concat!(
    r#"<svg class="h-3.5 w-3.5 text-ink-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">"#,
    include_str!("../../../../../vendor/lucide/house.svg"),
    "</svg>"
);
const SVG_DOCS: &str = concat!(
    r#"<svg class="h-3.5 w-3.5 text-ink-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">"#,
    include_str!("../../../../../vendor/lucide/book-open.svg"),
    "</svg>"
);
const SVG_TERMINAL_ICON: &str = concat!(
    r#"<svg class="h-3.5 w-3.5 text-ink-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">"#,
    include_str!("../../../../../vendor/lucide/square-terminal.svg"),
    "</svg>"
);
const SVG_CLOCK: &str = concat!(
    r#"<svg class="h-3.5 w-3.5 text-ink-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">"#,
    include_str!("../../../../../vendor/lucide/clock.svg"),
    "</svg>"
);
const SVG_MOON_NAV: &str = concat!(
    r#"<svg class="h-3.5 w-3.5 text-ink-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">"#,
    include_str!("../../../../../vendor/lucide/moon.svg"),
    "</svg>"
);
const SVG_BACK: &str = concat!(
    r#"<svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
    include_str!("../../../../../vendor/lucide/chevron-left.svg"),
    "</svg>"
);
const SVG_SEARCH_NAV: &str = concat!(
    r#"<svg class="h-3.5 w-3.5 text-ink-500 flex-shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">"#,
    include_str!("../../../../../vendor/lucide/search.svg"),
    "</svg>"
);

/// Nav link entry for the drawer menu.
pub(crate) struct DrawerLink {
    pub(crate) svg: &'static str,
    pub(crate) label: &'static str,
    pub(crate) active: bool,
}

// ---------------------------------------------------------------------------
// Production navbar
// ---------------------------------------------------------------------------

use super::breadcrumb::Crumb;

/// A top-level nav link shown in the right side of the bar.
pub(crate) struct NavLink {
    pub label: &'static str,
    pub href: &'static str,
}

/// Inline "alpha" badge rendered next to the brand mark in the navbar.
///
/// The Component Registry is in its alpha phase — not yet beta — so every
/// page advertises that fact in the header. Orange is used to draw the eye
/// without competing with the purple accent reserved for primary actions.
const ALPHA_BADGE: &str = r#"<span class="inline-flex items-center h-5 px-1.5 rounded text-[10px] font-semibold uppercase tracking-wider text-orange-700 bg-orange-100 border border-orange-200 whitespace-nowrap" title="This service is in alpha — expect breaking changes." aria-label="Alpha release">alpha</span>"#;

/// SVG icons for the theme dropdown (14px, currentColor).
const THEME_SUN: &str = r#"<svg class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><circle cx="12" cy="12" r="5"/><path d="M12 1v2M12 21v2M4.22 4.22l1.42 1.42M18.36 18.36l1.42 1.42M1 12h2M21 12h2M4.22 19.78l1.42-1.42M18.36 5.64l1.42-1.42"/></svg>"#;
const THEME_MOON: &str = concat!(
    r#"<svg class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
    include_str!("../../../../../vendor/lucide/moon.svg"),
    "</svg>"
);
const THEME_AUTO: &str = concat!(
    r#"<svg class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
    include_str!("../../../../../vendor/lucide/eclipse.svg"),
    "</svg>"
);

/// Render the theme dropdown (Auto / Light / Dark) with icons + labels.
pub(crate) fn theme_dropdown() -> String {
    format!(
        r#"<div class="relative" id="theme-dropdown">
<button type="button" id="theme-trigger" aria-label="Color theme" aria-haspopup="true" aria-expanded="false" class="inline-flex items-center justify-center h-7 w-7 rounded-md text-ink-500 hover:bg-surfaceMuted hover:text-ink-900 transition-colors">
<span class="theme-icon theme-icon-auto">{THEME_AUTO}</span>
<span class="theme-icon theme-icon-light" style="display:none">{THEME_SUN}</span>
<span class="theme-icon theme-icon-dark" style="display:none">{THEME_MOON}</span>
</button>
<div id="theme-menu" class="absolute right-0 mt-1.5 w-36 rounded-md bg-surface border border-line shadow-tooltip py-1 text-[13px] hidden z-50">
<button type="button" data-theme-value="auto" class="theme-option w-full text-left px-3 h-8 flex items-center gap-2.5 text-ink-700 hover:bg-surfaceMuted hover:text-ink-900 rounded-sm">{THEME_AUTO} Auto</button>
<button type="button" data-theme-value="light" class="theme-option w-full text-left px-3 h-8 flex items-center gap-2.5 text-ink-700 hover:bg-surfaceMuted hover:text-ink-900 rounded-sm">{THEME_SUN} Light</button>
<button type="button" data-theme-value="dark" class="theme-option w-full text-left px-3 h-8 flex items-center gap-2.5 text-ink-700 hover:bg-surfaceMuted hover:text-ink-900 rounded-sm">{THEME_MOON} Dark</button>
</div>
</div>"#
    )
}

#[allow(dead_code)]
/// Render the production navbar.
///
/// Produces a sticky translucent `<header>` with:
/// - Home icon + breadcrumb path (left, using the s18 breadcrumb component)
/// - Search trigger (center)
/// - Nav links + theme toggle (right)
pub(crate) fn render_bar(crumbs: &[Crumb], links: &[NavLink]) -> String {
    // Left: home icon + breadcrumb (delegates to s18_breadcrumb)
    let breadcrumb_html = super::breadcrumb::render_breadcrumb(crumbs);

    let left = Division::builder()
        .class("flex items-center gap-3 min-w-0")
        .anchor(|a| {
            a.href("/")
                .class("text-[13px] font-semibold text-ink-900 no-underline hover:text-ink-700 transition-colors whitespace-nowrap")
                .text("Component Registry")
        })
        .text(ALPHA_BADGE)
        .division(|d| d.class("w-px h-4 bg-line flex-shrink-0"))
        .text(breadcrumb_html)
        .build()
        .to_string();

    // Right: search + nav links + theme toggle
    let search = search_button(SVG_SEARCH_SM, "Type / to search", true);
    let mut right = Division::builder();
    right.class("flex items-center gap-2 text-[12px] text-ink-500");
    right.division(|d| d.class("hidden sm:block").text(search));
    for link in links {
        let href = link.href.to_owned();
        let label = link.label.to_owned();
        right.anchor(|a| {
            a.href(href)
                .class("inline-flex items-center h-7 px-2.5 rounded-md hover:bg-surfaceMuted hover:text-ink-900 hidden sm:inline-flex")
                .text(label)
        });
    }
    right.division(|d| d.class("hidden sm:block w-px h-4 bg-line mx-0.5"));
    right.text(theme_dropdown());
    let right = right.build().to_string();

    let bar = html::content::Header::builder()
        .class("sticky top-0 z-20 bg-canvas/90 backdrop-blur border-b hairline")
        .division(|inner| {
            inner
                .class("px-4 md:px-6 h-12 max-w-[1440px] flex items-center gap-8 lg:gap-10")
                .text(left)
                .division(|spacer| spacer.class("flex-1"))
                .text(right)
        })
        .build();

    bar.to_string()
}

/// Render the production navbar for grid-body pages.
///
/// Same content as [`render_bar`], but the outer `<header>` is a direct
/// grid child of the body grid and spans all columns (`col-span-full`) so
/// its translucent background bleeds edge-to-edge. The inner wrapper caps
/// at 1280px (left-aligned, no `mx-auto`) so the left and right clusters
/// align with the content tracks below.
#[must_use]
pub(crate) fn render_bar_grid(crumbs: &[Crumb], links: &[NavLink]) -> String {
    let breadcrumb_html = super::breadcrumb::render_breadcrumb(crumbs);

    let left = Division::builder()
        .class("flex items-center gap-3 min-w-0")
        .anchor(|a| {
            a.href("/")
                .class("text-[13px] font-semibold text-ink-900 no-underline hover:text-ink-700 transition-colors whitespace-nowrap")
                .text("Component Registry")
        })
        .text(ALPHA_BADGE)
        .division(|d| d.class("w-px h-4 bg-line flex-shrink-0"))
        .text(breadcrumb_html)
        .build()
        .to_string();

    let search = search_button(SVG_SEARCH_SM, "Type / to search", true);
    let mut right = Division::builder();
    right.class("flex items-center gap-2 min-w-0 text-[12px] text-ink-500");
    right.division(|d| d.class("hidden sm:block").text(search));
    for link in links {
        let href = link.href.to_owned();
        let label = link.label.to_owned();
        right.anchor(|a| {
            a.href(href)
                .class("inline-flex items-center h-7 px-2.5 rounded-md hover:bg-surfaceMuted hover:text-ink-900 hidden sm:inline-flex")
                .text(label)
        });
    }
    right.division(|d| d.class("hidden sm:block w-px h-4 bg-line mx-0.5"));
    right.text(theme_dropdown());
    let right = right.build().to_string();

    let bar = html::content::Header::builder()
        .class("col-span-full sticky top-0 z-20 bg-canvas/90 backdrop-blur border-b hairline")
        .division(|inner| {
            inner
                .class("w-full px-4 md:px-8 h-[var(--navbar-h)] flex items-center justify-between gap-4")
                .text(left)
                .text(right)
        })
        .build();

    bar.to_string()
}

// ---------------------------------------------------------------------------
// Design-system showcase (below)
// ---------------------------------------------------------------------------

pub(crate) const DRAWER_LINKS: &[DrawerLink] = &[
    DrawerLink {
        svg: SVG_HOME_NAV,
        label: "Home",
        active: false,
    },
    DrawerLink {
        svg: SVG_DOCS,
        label: "Guides",
        active: false,
    },
    DrawerLink {
        svg: SVG_TERMINAL_ICON,
        label: "Reference",
        active: true,
    },
    DrawerLink {
        svg: SVG_CLOCK,
        label: "Changelog",
        active: false,
    },
];

pub(crate) const ANATOMY_ITEMS: &[&str] = &[
    r#"<strong>Translucent surface</strong> — <code class="mono text-[12px]">sticky top-0 z-20 bg-canvas/90 backdrop-blur</code> with a <code class="mono text-[12px]">.hairline</code> bottom border. Content scrolls <em>under</em> the bar; the 90% canvas + blur keeps the chrome legible without erasing the page beneath. Never use a solid surface — that breaks the layered feel."#,
    r#"<strong>Height &amp; rhythm</strong> — fixed at <code class="mono text-[12px]">h-12</code> (48px). Inner gutters match the page container (<code class="mono text-[12px]">px-4 md:px-6</code>) so the brand mark aligns with the leftmost column of content below."#,
    r#"<strong>Brand cluster</strong> — 24×24 sigil + 13px mono name + optional 11px ink-500 mono context label that hides below <code class="mono text-[12px]">sm</code>. Wrapped in a single <code class="mono text-[12px]">&lt;a&gt;</code> back to the home page."#,
    r#"<strong>Command palette trigger</strong> — a button (not an input). Uses the form system’s compact recipe trimmed to bar height: <code class="mono text-[12px]">h-8 rounded-md border-line bg-surface</code>, placeholder colour <code class="mono text-[12px]">text-ink-500</code>, leading 14px magnifier (<code class="mono text-[12px]">ink-400</code>), trailing <code class="mono text-[12px]">⌘K</code> kbd hint matching Section 13’s prominent search variant. Clicking opens the palette modal — the input lives there, not in the bar."#,
    r#"<strong>Nav links</strong> — 12px ink-500 in <code class="mono text-[12px]">h-7 px-2 rounded-md</code> hit areas with <code class="mono text-[12px]">hover:bg-surfaceMuted hover:text-ink-900</code>. No underline, no separator dots — spacing carries the rhythm."#,
    r#"<strong>Theme toggle</strong> — same shape as the form system’s icon button (<code class="mono text-[12px]">h-7</code> bordered, surface bg). Always visible at every viewport so users can correct an unwanted theme without hunting."#,
    r#"<strong>Responsive collapse</strong> — drop the brand tagline below <code class="mono text-[12px]">sm</code>; trim primary nav at <code class="mono text-[12px]">md</code>; collapse the search button to a <code class="mono text-[12px]">h-8 w-8</code> icon button below <code class="mono text-[12px]">sm</code> and stash all links behind a hamburger."#,
];

/// Build a breadcrumb: Home icon -> "wasi" -> chevron -> "http".
fn breadcrumb(home_svg: &str, chev_svg: &str) -> String {
    let home_svg = home_svg.to_owned();
    let chev_svg = chev_svg.to_owned();
    Division::builder()
        .class("flex items-center gap-2 min-w-0")
        .text(format!(
            r##"<a href="#" aria-label="Home" class="inline-flex items-center justify-center h-6 w-6 rounded-md text-ink-700 no-underline hover:text-ink-900 hover:bg-surfaceMuted transition-colors">{home_svg}</a>"##
        ))
        .navigation(|nav| {
            nav.aria_label("Breadcrumb")
                .class("flex items-center gap-1.5 mono text-[13px] text-ink-500 min-w-0")
                .anchor(|a| {
                    a.href("#")
                        .class("no-underline hover:text-ink-900 truncate")
                        .text("wasi")
                })
                .text(chev_svg)
                .span(|s| s.class("text-ink-900 font-medium truncate").text("http"))
        })
        .build()
        .to_string()
}

/// Build the search command palette button.
fn search_button(svg: &str, placeholder: &str, show_hint: bool) -> String {
    let svg = svg.to_owned();
    let placeholder = placeholder.to_owned();
    let mut btn = html::forms::Button::builder();
    btn.type_("button");
    btn.class("search-trigger flex-1 max-w-[280px] h-8 px-2.5 rounded-md border border-line bg-surface text-[13px] text-ink-500 flex items-center gap-2 hover:bg-surfaceMuted hover:text-ink-700 transition-colors");
    btn.text(svg);
    btn.span(|s| s.class("truncate").text(placeholder));
    if show_hint {
        btn.span(|s| {
            s.class(
                "ml-auto mono text-[11px] text-ink-400 border border-line rounded px-1.5 leading-5",
            )
            .text("/")
        });
    }
    btn.build().to_string()
}

/// Render the search command palette modal.
///
/// Hidden by default. Opened via JS when the search button is clicked or
/// `/` is pressed. Overlays on top of the navbar with a centered input
/// and results panel, dark scrim behind.
pub(crate) fn render_search_modal() -> String {
    format!(
        r#"<div id="search-modal" class="search-modal hidden">
<div class="search-scrim"></div>
<div class="search-dialog">
<form action="/search" method="get" class="search-input-row">
{SVG_SEARCH_LG}
<input id="search-modal-input" type="search" name="q" placeholder="Search packages…" autocomplete="off" class="flex-1 bg-transparent text-[15px] text-ink-900 placeholder:text-ink-400 outline-none" />
<kbd class="mono text-[11px] text-ink-400 border border-line rounded px-1.5 py-0.5 cursor-pointer" id="search-close-hint">/</kbd>
</form>
<div class="search-hint">Type a query and press <kbd class="mono">Enter</kbd> to search.</div>
</div>
</div>"#
    )
}

/// Build placeholder content lines.
fn content_lines(padding: &str, widths: &[&str]) -> String {
    let padding = padding.to_owned();
    let mut div = Division::builder();
    div.class(format!("{padding} space-y-2.5"));
    for w in widths {
        let w = (*w).to_owned();
        div.division(|d| d.class(format!("h-2 rounded bg-surfaceMuted w-[{w}]")));
    }
    div.build().to_string()
}

/// Build the desktop navbar demo.
fn desktop() -> String {
    Division::builder()
        .division(|d| {
            d.class("text-[12px] text-ink-500 mb-3")
                .text("Desktop · ≥ 1024px")
        })
        .division(|card| {
            card.class("rounded-lg border border-line overflow-hidden bg-canvas")
                .division(|rel| {
                    rel.class("relative")
                        .division(|sticky| {
                            sticky
                                .class("sticky top-0 z-10 bg-canvas/90 backdrop-blur border-b hairline")
                                .division(|bar| {
                                    bar.class("px-4 md:px-6 h-12 grid grid-cols-[minmax(0,1fr)_minmax(0,2fr)_auto] items-center gap-4")
                                        .text(breadcrumb(SVG_HOME, SVG_CHEV_RIGHT))
                                        .text(search_button(SVG_SEARCH_SM, "Search commands, flags, env vars…", true))
                                        .division(|right| {
                                            right.class("flex items-center gap-1 text-[12px] text-ink-500")
                                                .anchor(|a| a.href("#").class("inline-flex items-center h-7 px-2 rounded-md hover:bg-surfaceMuted hover:text-ink-900").text("Guides"))
                                                .anchor(|a| a.href("#").class("inline-flex items-center h-7 px-2 rounded-md hover:bg-surfaceMuted hover:text-ink-900").text("Reference"))
                                                .anchor(|a| a.href("#").class("inline-flex items-center h-7 px-2 rounded-md hover:bg-surfaceMuted hover:text-ink-900").text("Changelog"))
                                                .button(|b| {
                                                    b.type_("button")
                                                        .aria_label("Toggle color theme")
                                                        .class("inline-flex items-center justify-center h-7 w-7 rounded-md border border-line bg-surface text-ink-700 hover:bg-surfaceMuted hover:text-ink-900 transition-colors")
                                                        .text(SVG_MOON_SM)
                                                })
                                        })
                                })
                        })
                        .text(content_lines("px-6 py-6", &["88%", "72%", "80%", "64%", "78%"]))
                })
        })
        .build()
        .to_string()
}

/// Build the tablet navbar demo.
fn tablet() -> String {
    Division::builder()
        .division(|d| {
            d.class("text-[12px] text-ink-500 mb-3")
                .text("Tablet · 640–1023px")
        })
        .division(|card| {
            card.class("rounded-lg border border-line overflow-hidden bg-canvas max-w-[680px]")
                .division(|rel| {
                    rel.class("relative")
                        .division(|sticky| {
                            sticky
                                .class("sticky top-0 z-10 bg-canvas/90 backdrop-blur border-b hairline")
                                .division(|bar| {
                                    bar.class("px-4 h-12 grid grid-cols-[minmax(0,1fr)_minmax(0,2fr)_auto] items-center gap-3")
                                        .text(breadcrumb(SVG_HOME, SVG_CHEV_RIGHT))
                                        .text(search_button(SVG_SEARCH_SM, "Search…", true))
                                        .division(|right| {
                                            right.class("flex items-center gap-1 text-[12px] text-ink-500")
                                                .anchor(|a| a.href("#").class("inline-flex items-center h-7 px-2 rounded-md hover:bg-surfaceMuted hover:text-ink-900").text("Reference"))
                                                .button(|b| {
                                                    b.type_("button")
                                                        .aria_label("Toggle color theme")
                                                        .class("inline-flex items-center justify-center h-7 w-7 rounded-md border border-line bg-surface text-ink-700 hover:bg-surfaceMuted hover:text-ink-900")
                                                        .text(SVG_MOON_SM)
                                                })
                                        })
                                })
                        })
                        .text(content_lines("px-4 py-5", &["80%", "68%", "74%", "60%"]))
                })
        })
        .paragraph(|p| {
            p.class("mt-3 text-[12px] text-ink-500")
                .text("Drops the secondary tagline and trims primary nav to the most-used link. Search collapses placeholder copy but keeps full width and the ")
                .code(|c| c.class("mono text-[12px]").text("⌘K"))
                .text(" hint.")
        })
        .build()
        .to_string()
}

/// Mobile state 1: collapsed.
fn mobile_collapsed() -> String {
    Division::builder()
        .class("w-[280px]")
        .division(|card| {
            card.class("rounded-lg border border-line overflow-hidden bg-canvas")
                .division(|rel| {
                    rel.class("relative")
                        .division(|sticky| {
                            sticky
                                .class("sticky top-0 z-10 bg-canvas/90 backdrop-blur border-b hairline")
                                .division(|bar| {
                                    bar.class("px-3 h-12 flex items-center gap-2")
                                        .button(|b| {
                                            b.type_("button")
                                                .aria_label("Open menu")
                                                .class("inline-flex items-center justify-center h-8 w-8 -ml-1 rounded-md text-ink-700 hover:bg-surfaceMuted hover:text-ink-900 flex-shrink-0")
                                                .text(SVG_HAMBURGER)
                                        })
                                        .navigation(|nav| {
                                            nav.aria_label("Breadcrumb")
                                                .class("flex items-center gap-1 mono text-[13px] text-ink-500 min-w-0")
                                                .anchor(|a| a.href("#").class("no-underline hover:text-ink-900 truncate").text("wasi"))
                                                .text(SVG_CHEV_RIGHT)
                                                .span(|s| s.class("text-ink-900 font-medium truncate").text("http"))
                                        })
                                        .button(|b| {
                                            b.type_("button")
                                                .aria_label("Search")
                                                .class("ml-auto inline-flex items-center justify-center h-8 w-8 -mr-1 rounded-md text-ink-700 hover:bg-surfaceMuted hover:text-ink-900 flex-shrink-0")
                                                .text(SVG_SEARCH_LG)
                                        })
                                })
                        })
                        .text(content_lines("px-3 py-4", &["88%", "70%", "82%"]))
                })
        })
        .paragraph(|p| {
            p.class("mt-3 text-[11px] mono uppercase tracking-wider text-ink-500")
                .text("Default")
        })
        .paragraph(|p| {
            p.class("mt-1 text-[12px] text-ink-500")
                .text("Hamburger left, breadcrumb middle, search right. Three things, no chrome.")
        })
        .build()
        .to_string()
}

/// Mobile state 2: drawer open.
fn mobile_drawer(links: &[DrawerLink]) -> String {
    let mut drawer_nav = html::content::Navigation::builder();
    drawer_nav.class("p-3 space-y-0.5 text-[13px]");
    for link in links {
        let svg = link.svg.to_owned();
        let label = link.label.to_owned();
        let class = if link.active {
            "flex items-center gap-2.5 h-8 px-2 rounded-md bg-surfaceMuted text-ink-900 no-underline font-medium"
        } else {
            "flex items-center gap-2.5 h-8 px-2 rounded-md text-ink-700 hover:bg-surfaceMuted hover:text-ink-900 no-underline"
        };
        let class = class.to_owned();
        drawer_nav.anchor(|a| a.href("#").class(class).text(svg).text(label));
    }
    drawer_nav.division(|d| d.class("border-t hairline my-2"));
    drawer_nav.button(|b| {
        b.type_("button")
            .class("w-full flex items-center gap-2.5 h-8 px-2 rounded-md text-ink-700 hover:bg-surfaceMuted hover:text-ink-900")
            .text(SVG_MOON_NAV)
            .text("Theme")
            .span(|s| {
                s.class("ml-auto text-[11px] text-ink-500 mono")
                    .text("auto")
            })
    });
    let drawer_nav = drawer_nav.build().to_string();

    Division::builder()
        .class("w-[280px]")
        .division(|card| {
            card.class("rounded-lg border border-line overflow-hidden bg-canvas relative")
                // Bar with close button
                .division(|bar_wrap| {
                    bar_wrap
                        .class("sticky top-0 z-30 bg-canvas/90 backdrop-blur border-b hairline")
                        .division(|bar| {
                            bar.class("px-3 h-12 flex items-center gap-2")
                                .button(|b| {
                                    b.type_("button")
                                        .aria_label("Close menu")
                                        .class("inline-flex items-center justify-center h-8 w-8 -ml-1 rounded-md text-ink-900 bg-surfaceMuted flex-shrink-0")
                                        .text(SVG_CLOSE)
                                })
                                .navigation(|nav| {
                                    nav.aria_label("Breadcrumb")
                                        .class("flex items-center gap-1 mono text-[13px] text-ink-500 min-w-0")
                                        .anchor(|a| a.href("#").class("no-underline hover:text-ink-900 truncate").text("wasi"))
                                        .text(SVG_CHEV_RIGHT)
                                        .span(|s| s.class("text-ink-900 font-medium truncate").text("http"))
                                })
                                .button(|b| {
                                    b.type_("button")
                                        .aria_label("Search")
                                        .disabled(true)
                                        .class("ml-auto inline-flex items-center justify-center h-8 w-8 -mr-1 rounded-md text-ink-400 flex-shrink-0")
                                        .text(SVG_SEARCH_LG)
                                })
                        })
                })
                // Drawer panel
                .division(|panel| {
                    panel
                        .class("relative")
                        // Scrim
                        .division(|scrim| {
                            scrim
                                .class("absolute inset-0 bg-ink-900/30")
                                .aria_hidden(true)
                        })
                        // Sliding panel
                        .division(|slide| {
                            slide
                                .class("relative w-[230px] bg-canvas border-r hairline shadow-card")
                                .text(drawer_nav)
                        })
                })
        })
        .paragraph(|p| {
            p.class("mt-3 text-[11px] mono uppercase tracking-wider text-ink-500")
                .text("Drawer open")
        })
        .paragraph(|p| {
            p.class("mt-1 text-[12px] text-ink-500")
                .text("Tap hamburger → 230px panel slides in from the left, scrim dims the page. Home, primary nav, and theme toggle live here. Hamburger flips to an ")
                .code(|c| c.class("mono text-[12px]").text("×"))
                .text(".")
        })
        .build()
        .to_string()
}

/// Mobile state 3: search active.
fn mobile_search() -> String {
    Division::builder()
        .class("w-[280px]")
        .division(|card| {
            card.class("rounded-lg border border-line overflow-hidden bg-canvas")
                .division(|rel| {
                    rel.class("relative")
                        // Search bar
                        .division(|sticky| {
                            sticky
                                .class("sticky top-0 z-10 bg-canvas/90 backdrop-blur border-b hairline")
                                .division(|bar| {
                                    bar.class("px-3 h-12 flex items-center gap-2")
                                        .button(|b| {
                                            b.type_("button")
                                                .aria_label("Cancel search")
                                                .class("inline-flex items-center justify-center h-8 w-8 -ml-1 rounded-md text-ink-700 hover:bg-surfaceMuted hover:text-ink-900 flex-shrink-0")
                                                .text(SVG_BACK)
                                        })
                                        .division(|input| {
                                            input.class("flex-1 h-8 px-2.5 rounded-md border border-ink-900 bg-canvas text-[13px] text-ink-900 flex items-center gap-2 min-w-0")
                                                .text(SVG_SEARCH_NAV)
                                                .span(|s| {
                                                    // Raw HTML: cursor-blink <span> is a zero-width
                                                    // element that can't be expressed as a Span child.
                                                    s.class("mono truncate")
                                                        .text(r#"wasi:htt<span class="inline-block w-[1px] h-3.5 bg-ink-900 align-middle ml-px"></span>"#)
                                                })
                                        })
                                })
                        })
                        // Result list
                        .division(|results| {
                            results.class("py-1.5 text-[13px]")
                                .division(|hdr| {
                                    hdr.class("px-2 pt-1 pb-1.5 mono uppercase tracking-wider text-[10px] text-ink-500")
                                        .text("Packages")
                                })
                                // Raw HTML: sigil <span>s use inline style= for category colors.
                                // Span::style() creates a <style> child, not an inline style attribute.
                                .text(r##"<a href="#" class="flex items-center gap-2.5 px-3 py-1.5 hover:bg-surfaceMuted no-underline"><span class="sigil" style="background:var(--c-cat-lilac);color:var(--c-cat-lilac-ink);">G</span><span class="mono text-ink-900">wasi:<span class="font-semibold">http</span></span></a>"##)
                                .text(r##"<a href="#" class="flex items-center gap-2.5 px-3 py-1.5 hover:bg-surfaceMuted no-underline"><span class="sigil" style="background:var(--c-cat-lilac);color:var(--c-cat-lilac-ink);">G</span><span class="mono text-ink-700">wasi:<span class="font-semibold">http</span>-types</span></a>"##)
                                .division(|hdr| {
                                    hdr.class("px-2 pt-2 pb-1.5 mono uppercase tracking-wider text-[10px] text-ink-500")
                                        .text("Commands")
                                })
                                .text(r##"<a href="#" class="flex items-center gap-2.5 px-3 py-1.5 hover:bg-surfaceMuted no-underline"><span class="sigil" style="background:var(--c-cat-green);color:var(--c-cat-green-ink);">c</span><span class="mono text-ink-700">component <span class="font-semibold">http</span> serve</span></a>"##)
                        })
                })
        })
        .paragraph(|p| {
            p.class("mt-3 text-[11px] mono uppercase tracking-wider text-ink-500")
                .text("Search active")
        })
        .paragraph(|p| {
            p.class("mt-1 text-[12px] text-ink-500")
                .text("Tap search → input expands across the bar, hamburger swaps to a back chevron, breadcrumb hides. Results render directly under the bar in the existing surface.")
        })
        .build()
        .to_string()
}

/// Anatomy / rules section.
fn anatomy(items: &[&str]) -> String {
    let mut ul = html::text_content::UnorderedList::builder();
    ul.class(
        "text-[13px] text-ink-700 leading-relaxed space-y-1.5 pl-5 list-disc marker:text-ink-400",
    );
    for item in items {
        let item = (*item).to_owned();
        ul.list_item(|li| li.paragraph(|p| p.text(item)));
    }
    Division::builder()
        .division(|d| d.class("text-[12px] text-ink-500 mb-3").text("Anatomy"))
        .text(ul.build().to_string())
        .build()
        .to_string()
}

/// Render this section.
pub(crate) fn render(
    section_id: &str,
    num: &str,
    title: &str,
    desc: &str,
    drawer_links: &[DrawerLink],
    anatomy_items: &[&str],
) -> String {
    let content = Division::builder()
        .class("space-y-12")
        .text(desktop())
        .text(tablet())
        .division(|d| {
            d.division(|lbl| {
                lbl.class("text-[12px] text-ink-500 mb-3")
                    .text("Mobile · &lt; 640px · three states")
            })
            .division(|wrap| {
                wrap.class("flex flex-wrap gap-6 items-start")
                    .text(mobile_collapsed())
                    .text(mobile_drawer(drawer_links))
                    .text(mobile_search())
            })
        })
        .text(anatomy(anatomy_items))
        .build()
        .to_string();

    super::section(section_id, num, title, desc, &content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot() {
        insta::assert_snapshot!(crate::components::ds::pretty_html(&render(
            "c-navbar",
            "C06",
            "Navbar",
            "Sticky page chrome: brand mark, command palette trigger, primary nav, theme toggle. Sits above all content with a translucent <code class=\"mono text-[12px]\">bg-canvas/90</code> + <code class=\"mono text-[12px]\">backdrop-blur</code> so scrolling content reads through without losing legibility.",
            DRAWER_LINKS,
            ANATOMY_ITEMS,
        )));
    }

    #[test]
    fn render_bar_grid_includes_alpha_badge() {
        let html = render_bar_grid(&[], &[]);
        assert!(
            html.contains(">alpha</span>"),
            "expected alpha badge text in navbar: {html}",
        );
        assert!(
            html.contains("text-orange-700"),
            "expected orange styling on alpha badge: {html}",
        );
    }

    #[test]
    fn render_bar_includes_alpha_badge() {
        let html = render_bar(&[], &[]);
        assert!(
            html.contains(">alpha</span>"),
            "expected alpha badge text in navbar: {html}",
        );
    }
}
