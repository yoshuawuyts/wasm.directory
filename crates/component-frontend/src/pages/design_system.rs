//! Design system reference page — `/design-system`.
//!
//! A living style guide that showcases every token, component, and pattern
//! from the design system. Sections are numbered to match `design-system.html`.

use crate::components::ds;
use crate::layout;

const RULE: &str = r#"<div class="border-t rule"></div>"#;
const RULE_MT: &str = r#"<div class="border-t rule mt-16"></div>"#;

/// TOC entries: (href, label).
const TOC_ENTRIES: &[(&str, &str)] = &[
    ("#colors", "01 \u{2014} Color"),
    ("#typography", "02 \u{2014} Typography"),
    ("#spacing", "03 \u{2014} Spacing & Radius"),
    ("#elevation", "04 \u{2014} Elevation"),
    ("#buttons", "05 \u{2014} Buttons"),
    ("#tabs", "06 \u{2014} Tabs & Pills"),
    ("#nav", "07 \u{2014} Navigation"),
    ("#code", "08 \u{2014} Code Samples"),
    ("#bars", "09 \u{2014} Labels"),
    ("#tooltip", "10 \u{2014} Tooltip"),
    ("#table", "11 \u{2014} Table"),
    ("#icons", "12 \u{2014} Icons"),
    ("#fields", "13 \u{2014} Form Fields"),
    (
        "#toggles",
        "14 \u{2014} Checkbox \u{00b7} Radio \u{00b7} Switch",
    ),
    ("#badges", "15 \u{2014} Badges"),
    ("#dropdown", "16 \u{2014} Dropdown"),
    ("#modal", "17 \u{2014} Modal"),
    ("#breadcrumb", "18 \u{2014} Breadcrumb & Pagination"),
    ("#progress", "19 \u{2014} Progress & Spinner"),
    ("#empty", "20 \u{2014} Empty State"),
    ("#grid", "21 \u{2014} Grid"),
    ("#regions", "22 \u{2014} Regions"),
    ("#motion", "23 \u{2014} Motion"),
    ("#details", "24 \u{2014} Details"),
    ("#sigils", "25 \u{2014} Sigils"),
];

/// TOC entries for composed components.
const TOC_COMPONENT_ENTRIES: &[(&str, &str)] = &[
    ("#c-sidebar", "C01 \u{2014} Nested Sidebar"),
    ("#c-toc", "C02 \u{2014} On This Page"),
    ("#c-page-header", "C03 \u{2014} Page Header"),
    ("#c-item-list", "C04 \u{2014} Item List"),
    ("#c-item-details", "C05 \u{2014} Item Details"),
    ("#c-navbar", "C06 \u{2014} Navbar"),
    ("#c-hero", "C07 \u{2014} Hero"),
    ("#c-install-card", "C08 \u{2014} Install Card"),
    ("#c-metrics-strip", "C09 \u{2014} Metrics Strip"),
    ("#c-link-list", "C10 \u{2014} Link List"),
    ("#c-principles-grid", "C11 \u{2014} Principles Grid"),
    ("#c-cta-strip", "C12 \u{2014} CTA Strip"),
    ("#c-footer", "C13 \u{2014} Footer"),
];

/// Render the design system reference page.
#[must_use]
pub(crate) fn render() -> String {
    let mut html = String::with_capacity(128 * 1024);

    // Page header + TOC
    html.push_str(&ds::header::render("v1.0",
            "Foundations \u{00b7} Components \u{00b7} Patterns",
            "Design System",
            "A quiet, data-forward visual language built around soft rules, neutral ink, and a categorical pastel palette. Optimized for dense dashboards and analytical interfaces.",));
    html.push_str(RULE);
    html.push_str(&ds::toc::render(TOC_ENTRIES, TOC_COMPONENT_ENTRIES));
    html.push_str(RULE);

    // Foundations (01–24)
    html.push_str(&ds::color::render(
        "colors",
        "01",
        "Color",
        "Neutral surfaces and ink form the structural base. Pastel categoricals encode chart series with paired ink tones for legibility.",
        &[
        ds::color::SwatchGroup {
        title: "Surfaces",
        grid_class: "grid grid-cols-2 md:grid-cols-3 gap-4",
        swatches: ds::color::SURFACES
        },
        ds::color::SwatchGroup {
        title: "Ink",
        grid_class: "grid grid-cols-2 md:grid-cols-5 gap-4",
        swatches: ds::color::INK
        },
        ds::color::SwatchGroup {
        title: "Lines",
        grid_class: "grid grid-cols-2 md:grid-cols-3 gap-4",
        swatches: ds::color::LINES
        },
        ds::color::SwatchGroup {
        title: "Semantic",
        grid_class: "grid grid-cols-2 md:grid-cols-3 gap-4",
        swatches: ds::color::SEMANTIC
        },
        ds::color::SwatchGroup {
        title: "Categorical",
        grid_class: "grid grid-cols-2 md:grid-cols-5 gap-4",
        swatches: ds::color::CATEGORICAL
        },
        ],
    ));
    html.push_str(RULE_MT);
    html.push_str(&ds::typography::render(
        "typography",
        "02",
        "Typography",
        "System UI stack for native rendering across platforms. Tight tracking on display sizes; relaxed for body.",
        ds::typography::SAMPLES,
    ));
    html.push_str(RULE_MT);
    html.push_str(&ds::spacing::render(
        "spacing",
        "03",
        "Spacing & Radius",
        "4px base scale. Radii stay small for a precise, instrumental feel; pills used for selection chips only.",
        ds::spacing::SPACING,
        ds::spacing::RADII,
    ));
    html.push_str(RULE_MT);
    html.push_str(&ds::elevation::render(
        "elevation",
        "04",
        "Elevation",
        "Soft rules do most of the work. Shadow is reserved for floating overlays.",
    ));
    html.push_str(RULE_MT);
    html.push_str(&ds::buttons::render(
        "buttons",
        "05",
        "Buttons",
        "Two variants: a soft gray fill or a 1.5px ink outline. The system reserves solid ink for typography only \u{2014} buttons are never pure black. Two heights: 32px (compact toolbars) and 36px (mobile / primary CTAs).",
    ));
    html.push_str(RULE_MT);
    html.push_str(&ds::tabs::render(
        "tabs",
        "06",
        "Tabs & Pills",
        "Segmented controls for binary scoping; underline tabs for sub-views; panel tabs for grouping a primary action with related content; pills for filterable chips.",
    ));
    html.push_str(RULE_MT);
    html.push_str(&ds::navigation::render(
        "nav",
        "07",
        "Navigation",
        "Sidebar list. Active item uses a muted surface fill with full ink weight. Groups separated by a soft rule.",
        ds::navigation::GROUP_1,
        ds::navigation::GROUP_2,
    ));
    html.push_str(RULE_MT);
    html.push_str(&ds::code::render(
        "code",
        "08",
        "Code Samples",
        "One panel \u{2014} <code class=\"mono text-[12px]\">pre.id-code</code> \u{2014} sitting on <code class=\"mono text-[12px]\">--c-surface</code>, with token colours pulled from the theme-aware <code class=\"mono text-[12px]\">--color-wit-*</code> palette so chroma stays balanced on both light and dark pages. Three forms: a plain block, a tabbed multi-language block, and a paired request / response grid.",
        ds::code::TOKENS,
        ds::code::ANATOMY_ITEMS,
    ));
    html.push_str(RULE_MT);
    html.push_str(&ds::labels::render(
        "bars",
        "09",
        "Labels",
        "28px tall, 6px radius, label inset 12px. Pastel fill with paired ink for text \u{2014} 4.5:1 contrast minimum.",
        ds::labels::BARS,
    ));
    html.push_str(RULE_MT);
    html.push_str(&ds::tooltip::render(
        "tooltip",
        "10",
        "Tooltip",
        "Inverted surface with backdrop blur. Caption label above, key/value rows with right-aligned medium values.",
    ));
    html.push_str(RULE_MT);
    html.push_str(&ds::table::render(
        "table",
        "11",
        "Table",
        r##"Two patterns cover everything: a <strong>definition</strong> table (no <code class="mono text-[12px]">&lt;thead&gt;</code>, identifier on the left, meaning on the right) and a <strong>tabular</strong> table (labeled columns, <code class="mono text-[12px]">tabular-nums</code> for figures). 13px body, 1.5px soft row separators (<code class="mono text-[12px]">border-lineSoft</code>), <code class="mono text-[12px]">py-3</code> rows. When the leading column is a category, use the <a href="#c-item-details" class="text-ink-700 underline decoration-line decoration-1 underline-offset-[3px] hover:text-ink-900">.id-http-status</a> pill family."##,
        ds::table::TAB_ENTRIES,
    ));
    html.push_str(RULE_MT);
    html.push_str(&ds::icons::render(
        "icons",
        "12",
        "Icons",
        r#"<a href="https://lucide.dev" class="text-ink-700 underline decoration-line decoration-1 underline-offset-[3px] hover:text-ink-900">Lucide</a> outline icons, drawn at <code class="mono text-[12px]">stroke-width="1.75"</code> with <code class="mono text-[12px]">stroke-linecap="round"</code> and <code class="mono text-[12px]">stroke-linejoin="round"</code>. Sizes: <strong>14px</strong> inside dense controls (tree links, kbd hints, tabs), <strong>16px</strong> in toolbars and buttons, <strong>18px</strong> on mobile and in empty states. Always <code class="mono text-[12px]">currentColor</code> so they pick up the surrounding ink scale; never coloured directly."#,
        ds::icons::INLINE_ICONS,
        ds::icons::GRID_ICONS,
    ));
    html.push_str(RULE_MT);
    html.push_str(&ds::fields::render(
        "fields",
        "13",
        "Form Fields",
        "Inputs sit on a surface with a 1px line border. Focus darkens the border to ink \u{2014} no thickening, no glow. Two sizes: <strong>md</strong> (default) for primary forms, <strong>sm</strong> for dense contexts like sidebars, metadata strips, and toolbars.",
        ds::fields::STATES,
        ds::fields::COMMANDS,
    ));
    html.push_str(RULE_MT);
    html.push_str(&ds::toggles::render(
        "toggles",
        "14",
        "Checkbox \u{00b7} Radio \u{00b7} Switch",
        "All controls render in ink-900 when active. 16px hit area minimum on each control; full-row click target via wrapping label.",
    ));
    html.push_str(RULE_MT);
    html.push_str(&ds::badges::render(
        "badges",
        "15",
        "Badges",
        "Compact pill labels. Use categorical pairs for status; ink for counts and metadata.",
        ds::badges::STATUSES,
    ));
    html.push_str(RULE_MT);
    html.push_str(&ds::dropdown::render(
        "dropdown",
        "16",
        "Dropdown",
        "Floating menu on white. 1px gray border + tooltip-grade shadow. Section dividers separate logical groups.",
    ));
    html.push_str(RULE_MT);
    html.push_str(&ds::modal::render(
        "modal",
        "17",
        "Modal",
        "Centered dialog over a 50% ink scrim. 8px radius, 1px gray border, 24px padding. Header / body / footer rhythm.",
    ));
    html.push_str(RULE_MT);
    html.push_str(&ds::breadcrumb::render(
        "breadcrumb",
        "19",
        "Breadcrumb &<br />Pagination",
        "Navigation context. Breadcrumb uses chevron separators and dims all but the current item. Pagination is square-buttoned for compact toolbars.",
    ));
    html.push_str(RULE_MT);
    html.push_str(&ds::progress::render(
        "progress",
        "19",
        "Progress & Spinner",
        "Determinate progress as a 6px ink track. Indeterminate as a 16px spinner (CSS animation). Skeleton shimmer for placeholder content.",
    ));
    html.push_str(RULE_MT);
    html.push_str(&ds::empty::render(
        "empty",
        "20",
        "Empty State",
        "Centered illustration glyph, title, body, and primary CTA. Used for empty tables, search misses, and first-run views.",
    ));
    html.push_str(RULE_MT);
    html.push_str(&ds::grid::render(
        "grid",
        "21",
        "Grid",
        r#"Pages live in a <code class="mono text-[12px]">max-w-[1440px]</code> container with <code class="mono text-[12px]">px-4 md:px-6</code> gutters. Inside, a small set of column shapes covers every layout: <strong>three-column</strong> (sidebar · reading · on-this-page) for documentation; <strong>two-column</strong> for narrative pages and this style guide; <strong>single column</strong> bounded by a reading measure for prose. Reading text is always capped at <code class="mono text-[12px]">max-w-[72ch]</code> regardless of the column it sits in."#,
        ds::grid::RULES,
    ));
    html.push_str(RULE_MT);
    html.push_str(&ds::regions::render(
        "regions",
        "22",
        "Regions",
        "Pages are composed of stacked <em>regions</em>. The primary region uses the canvas surface; secondary regions (supporting data, references, appendices) switch to the white surface. The surface swap signals \u{201c}this is additional content\u{201d} \u{2014} no rules or borders are drawn between regions.",
        ds::regions::RULES,
    ));
    html.push_str(RULE_MT);
    html.push_str(&ds::motion::render(
        "motion",
        "23",
        "Motion",
        r#"Motion is functional: it explains state changes, never decorates them. Most transitions sit between 120–260ms on the <code class="px-1 py-0.5 rounded-sm bg-surfaceMuted text-ink-900 mono text-[0.875em]">standard</code> curve. Anything longer needs a reason."#,
        ds::motion::CURVES,
        ds::motion::DURATIONS,
        ds::motion::PREVIEWS,
        ds::motion::RULES,
    ));
    html.push_str(RULE_MT);
    html.push_str(&ds::details::render(
        "details",
        "24",
        "Details",
        "Compact key/value lists for sidebars and inspector panels. Three variants: stacked for spacious layouts, inline for narrow rails, and sectioned when groups need separation.",
        ds::details::STACKED,
        ds::details::INLINE,
        ds::details::SECTIONED_A,
        ds::details::SECTIONED_B,
        ds::details::CARD_DETAILS,
        ds::details::SIDEBAR_PRIMARY,
        ds::details::SIDEBAR_SECONDARY,
    ));
    html.push_str(RULE_MT);
    html.push_str(&ds::sigil::render(
        "sigils",
        "25",
        "Sigils",
        "18\u{00d7}18px rounded squares with a single monospace letter, used to classify items by kind in sidebars, item lists, and detail pages. Each sigil pairs a categorical background with its ink counterpart for 4.5:1 contrast.",
        ds::sigil::ALL,
    ));

    // Part Two \u{2014} Components
    html.push_str(&ds::part_two::render("Part Two",
            "Components",
            "Composed patterns built from the foundations above. Each component documents its anchor markup and the variants it supports.",));
    html.push_str(&ds::sidebar::render(
        "c-sidebar",
        "C01",
        "Nested Sidebar",
        r#"Hierarchical navigation for reference docs. Top-level entries collapse with native <code class="mono text-[12px]">&lt;details&gt;</code>; sigils classify each row by kind (command, group, flag, env, etc.)."#,
        ds::sidebar::SIGIL_LEGEND,
        ds::sidebar::ANATOMY_ITEMS,
    ));
    html.push_str(RULE_MT);
    html.push_str(&ds::on_this_page::render(
        "c-toc",
        "C02",
        "On This Page",
        "Right-rail table of contents for long reference pages. A 1.5px left border lights up on hover and active state \u{2014} the only visual cue, no background fills.",
        ds::on_this_page::TOC_LINKS,
        ds::on_this_page::ANATOMY_ITEMS,
    ));
    html.push_str(RULE_MT);
    html.push_str(&ds::page_header::render(
        "c-page-header",
        "C03",
        "Page Header",
        "Top-of-page identification block: a kicker, a large title, an optional tagline, and an optional metadata strip. Used to anchor reference and documentation pages.",
        ds::page_header::ANATOMY_ITEMS,
    ));
    html.push_str(RULE_MT);
    html.push_str(&ds::item_list::render(
        "c-item-list",
        "C04",
        "Item List",
        "Compact index of a group\u{2019}s children \u{2014} subcommands, endpoints, schemas. Each row is a sigil, a name + one-line description, and trailing meta. Rows separate with hairline rules, no card chrome.",
        ds::item_list::CMD_ROWS,
        ds::item_list::ENDPOINT_ROWS,
        ds::item_list::ANATOMY_ITEMS,
    ));
    html.push_str(RULE_MT);
    html.push_str(&ds::item_details::render(
        "c-item-details",
        "C05",
        "Item Details",
        r##"Reference page for a single endpoint, RPC, schema, or command. A method/kind pill anchors the symbol below the title; a one-sentence tagline explains it; an optional structured request-body table, a responses list, and paired example panels stack below in fixed order. Used as the destination from <a href="#c-item-list" class="text-ink-700 underline decoration-line decoration-1 underline-offset-[3px] hover:text-ink-900">Item List</a> rows."##,
        ds::item_details::ANATOMY_ITEMS,
    ));
    html.push_str(RULE_MT);
    html.push_str(&ds::navbar::render(
        "c-navbar",
        "C06",
        "Navbar",
        "Sticky page chrome: brand mark, command palette trigger, primary nav, theme toggle. Sits above all content with a translucent <code class=\"mono text-[12px]\">bg-canvas/90</code> + <code class=\"mono text-[12px]\">backdrop-blur</code> so scrolling content reads through without losing legibility.",
        ds::navbar::DRAWER_LINKS,
        ds::navbar::ANATOMY_ITEMS,
    ));

    // Landing-page composed components (C07–C13)
    html.push_str(RULE_MT);
    html.push_str(&render_landing_components());

    layout::document_design_system("Design System", &html)
}

/// Render the landing-page composed components (C07–C13). Each section uses
/// the two-column [`ds::section`] helper and showcases one component with
/// representative content.
fn render_landing_components() -> String {
    use crate::components::ds::{
        cta_strip::{self, CtaStrip},
        footer::{self, Footer, FooterColumn, FooterLink},
        hero::{self, Hero, HeroCta, HeroCtaStyle},
        install_card::{self, InstallCard},
        link_list::{self, LeftStyle, LinkRow, RightStyle},
        metrics_strip::{self, Metric},
        principles_grid::{self, Principle},
    };

    let mut html = String::with_capacity(32 * 1024);

    // C07 — Hero
    let hero_demo = hero::render(&Hero {
        kicker: &["v1.0", "Stable"],
        title: "The package manager for components.",
        lede: "Resolve, vendor, and compose WebAssembly components from any registry.",
        ctas: &[
            HeroCta {
                label: "Get started",
                href: "#",
                style: HeroCtaStyle::Primary,
            },
            HeroCta {
                label: "Browse",
                href: "#",
                style: HeroCtaStyle::Secondary,
            },
        ],
        right: r#"<div class="rounded-lg border border-line bg-surface shadow-card h-40 grid place-items-center text-[12px] text-ink-500 mono">install card slot</div>"#,
    });
    html.push_str(&ds::section(
        "c-hero",
        "C07",
        "Hero",
        "Two-column landing-page intro: small mono kicker, large balanced headline, lede, primary + secondary CTAs and an optional ghost link. The right column is a free-form slot.",
        &hero_demo,
    ));

    // C08 — Install Card
    let install_demo = install_card::render(&InstallCard {
        platforms: &["macOS", "Linux", "Windows"],
        filename: "install.sh",
        snippet_html: &format!(
            "{}\n{}",
            install_card::prompt("curl -sSf https://wasm.dev/install.sh | sh"),
            install_card::positive("  ✓ Installed wasm v0.4.0"),
        ),
        sha: "9e4a…c0f1",
        copy_command: "curl -sSf https://wasm.dev/install.sh | sh",
    });
    html.push_str(RULE_MT);
    html.push_str(&ds::section(
        "c-install-card",
        "C08",
        "Install Card",
        "Surface card showing a multi-platform install snippet. Tab strip selects the platform; the body is a styled <code class=\"mono text-[12px]\">&lt;pre&gt;</code> using design-system code spans; the footer carries a SHA and a copy button.",
        &install_demo,
    ));

    // C09 — Metrics Strip
    let metrics_demo = metrics_strip::render(&[
        Metric {
            label: "Packages",
            value: "73",
            delta: Some("+2 this week"),
            verified: false,
        },
        Metric {
            label: "Authors",
            value: "13",
            delta: Some("+1"),
            verified: false,
        },
        Metric {
            label: "Versions",
            value: "124",
            delta: Some("+4"),
            verified: false,
        },
        Metric {
            label: "Integrity",
            value: "100%",
            delta: Some("verified"),
            verified: true,
        },
    ]);
    html.push_str(RULE_MT);
    html.push_str(&ds::section(
        "c-metrics-strip",
        "C09",
        "Metrics Strip",
        "Four-up divided stat row used to summarise the registry at a glance. Mono kicker, tabular value, optional delta with an optional verification dot.",
        &metrics_demo,
    ));

    // C10 — Link List
    let featured_demo = link_list::render(
        "Featured",
        &[
            LinkRow {
                left: "wasi:http",
                right: "WASI standard for HTTP",
                href: "#",
            },
            LinkRow {
                left: "wasi:cli",
                right: "Command-line entry points",
                href: "#",
            },
        ],
        &LeftStyle::Mono,
        &RightStyle::Description,
    );
    let categories_demo = link_list::render(
        "Categories",
        &[
            LinkRow {
                left: "HTTP & networking",
                right: "1 248",
                href: "#",
            },
            LinkRow {
                left: "Storage",
                right: "512",
                href: "#",
            },
        ],
        &LeftStyle::Plain,
        &RightStyle::Count,
    );
    let link_list_demo = format!(
        r#"<div class="grid sm:grid-cols-2 gap-x-12 gap-y-10">{featured_demo}{categories_demo}</div>"#
    );
    html.push_str(RULE_MT);
    html.push_str(&ds::section(
        "c-link-list",
        "C10",
        "Link List",
        "Two-line directory list with a heavy top rule and hairline row separators. Two variants: mono name + muted description, or plain name + tabular count.",
        &link_list_demo,
    ));

    // C11 — Principles Grid
    let principles_demo = principles_grid::render(
        "Why",
        "Built for components.",
        "A package manager designed around the WebAssembly Component Model.",
        &[
            Principle {
                bg_class: "bg-cat-blue",
                fg_class: "text-cat-blueInk",
                icon_svg: r#"<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/></svg>"#,
                title: "Reproducible",
                body: "Every dependency is locked by content hash.",
            },
            Principle {
                bg_class: "bg-cat-green",
                fg_class: "text-cat-greenInk",
                icon_svg: r#"<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><path d="M3 7l9 6 9-6"/><path d="M3 7v10l9 6 9-6V7"/><path d="M3 7l9-4 9 4"/></svg>"#,
                title: "Federated",
                body: "Pull from any OCI-compatible registry.",
            },
        ],
    );
    html.push_str(RULE_MT);
    html.push_str(&ds::section(
        "c-principles-grid",
        "C11",
        "Principles Grid",
        "2×N grid of value-proposition tiles. Each tile has a coloured square sigil (categorical palette) plus a short title and body.",
        &principles_demo,
    ));

    // C12 — CTA Strip
    let cta_demo = cta_strip::render(&CtaStrip {
        kicker: "For maintainers",
        title: "Publish your component.",
        body_html: "Add your namespace and run <code class=\"px-1 py-0.5 rounded-sm bg-surfaceMuted text-ink-900 mono text-[0.875em]\">component publish</code>.",
        primary_label: "Publishing guide",
        primary_href: "#",
        secondary_label: "Read the spec",
        secondary_href: "#",
    });
    html.push_str(RULE_MT);
    html.push_str(&ds::section(
        "c-cta-strip",
        "C12",
        "CTA Strip",
        "Pre-footer band with a top hairline. Kicker + headline + body on the left, primary + secondary CTAs on the right.",
        &cta_demo,
    ));

    // C13 — Footer
    let footer_demo = footer::render(&Footer {
        brand: "component",
        lede: "A package manager and registry for WebAssembly components.",
        status: "All systems operational",
        columns: &[
            FooterColumn {
                kicker: "Browse",
                links: &[
                    FooterLink {
                        label: "Packages",
                        href: "#",
                    },
                    FooterLink {
                        label: "Authors",
                        href: "#",
                    },
                ],
            },
            FooterColumn {
                kicker: "Develop",
                links: &[
                    FooterLink {
                        label: "Docs",
                        href: "#",
                    },
                    FooterLink {
                        label: "Spec",
                        href: "#",
                    },
                ],
            },
            FooterColumn {
                kicker: "Community",
                links: &[FooterLink {
                    label: "GitHub",
                    href: "#",
                }],
            },
        ],
    });
    html.push_str(RULE_MT);
    html.push_str(&ds::section(
        "c-footer",
        "C13",
        "Footer",
        "Site footer: brand block (name + lede + status pill) and four link columns, with a hairline-separated bottom row carrying the mono copyright and legal links.",
        &footer_demo,
    ));

    html
}
