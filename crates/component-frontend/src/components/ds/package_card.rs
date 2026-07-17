//! Package card component.
//!
//! Clickable card for package listings (home, namespace). Matches the
//! design system's Card elevation (section 04) and Details card (section 23).

#![allow(dead_code)]

use html::text_content::Division;
use wasm_meta_registry_client::KnownPackage;

/// Render a package card for grid listings.
pub(crate) fn render(pkg: &KnownPackage) -> Division {
    let description = pkg.description.as_deref().unwrap_or("No description");
    let version = crate::pick_redirect_version(&pkg.tags).unwrap_or_else(|| {
        pkg.tags
            .first()
            .cloned()
            .unwrap_or_else(|| "\u{2014}".to_owned())
    });

    let kind_badge = match pkg.kind {
        Some(wasm_meta_registry_client::PackageKind::Interface) => {
            Some(("bg-cat-blue text-cat-blueInk", "Types"))
        }
        Some(wasm_meta_registry_client::PackageKind::Component) => {
            Some(("bg-cat-peach text-cat-peachInk", "Component"))
        }
        _ => None,
    };

    if let (Some(ns), Some(name)) = (&pkg.wit_namespace, &pkg.wit_name) {
        Division::builder()
    .anchor(|a| {
        let card = a
            .href(format!("/{ns}/{name}"))
            .class("flex flex-col h-full bg-surface p-5 rounded-lg shadow-card card-lift");

        // Header: namespace + version
        card.span(|s| {
            s.class("flex justify-between items-start gap-2")
                .span(|left| {
                    left.class("text-[12px] text-ink-500 font-mono leading-tight truncate")
                        .text(ns.clone())
                })
                .span(|right| {
                    right
                        .class("text-[11px] text-ink-400 font-mono shrink-0")
                        .text(version.clone())
                })
        });

        // Name
        card.span(|s| {
            s.class("block text-[16px] font-semibold tracking-tight leading-snug mt-1 truncate")
                .text(name.clone())
        });

        // Description
        card.span(|s| {
            s.class("block text-[13px] text-ink-700 mt-3 overflow-hidden leading-relaxed")
                .style("display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; min-height: 2.5rem")
                .text(crate::markdown::render_inline(description))
        });

        // Kind badge (bottom)
        if let Some((badge_cls, badge_text)) = kind_badge {
            card.span(|s| {
                s.class("mt-3 flex")
                    .span(|badge| {
                        badge
                            .class(format!(
                                "inline-flex items-center px-2 h-5 rounded-pill text-[11px] font-medium {badge_cls}"
                            ))
                            .text(badge_text.to_owned())
                    })
            });
        }

        card
    })
    .build()
    } else {
        let display_name = pkg.repository.clone();
        let mut card = Division::builder();
        card.class("flex flex-col h-full bg-surface p-5 rounded-lg shadow-card card-lift");

        card.span(|s| {
            s.class("flex justify-between items-start gap-2")
                .span(|left| {
                    left.class("text-[16px] font-semibold tracking-tight leading-snug truncate")
                        .text(display_name)
                })
                .span(|right| {
                    right
                        .class("text-[11px] text-ink-400 font-mono shrink-0 mt-0.5")
                        .text(version.clone())
                })
        });

        card.span(|s| {
            s.class("block text-[13px] text-ink-700 mt-3 overflow-hidden leading-relaxed")
                .style("display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; min-height: 2.5rem")
                .text(crate::markdown::render_inline(description))
        });

        if let Some((badge_cls, badge_text)) = kind_badge {
            card.span(|s| {
                s.class("mt-3 flex")
                    .span(|badge| {
                        badge
                            .class(format!(
                                "inline-flex items-center px-2 h-5 rounded-pill text-[11px] font-medium {badge_cls}"
                            ))
                            .text(badge_text.to_owned())
                    })
            });
        }

        card.build()
    }
}

/// Grid class string for package card layouts.
pub(crate) fn grid(max_cols: u8) -> &'static str {
    match max_cols {
        3 => "grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4",
        _ => "grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-4",
    }
}
