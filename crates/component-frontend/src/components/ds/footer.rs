//! Site footer — brand block + link columns + bottom legal row.

use html::content::Footer as FooterEl;

/// A single link in a footer column.
pub(crate) struct FooterLink {
    /// Visible link text.
    pub label: &'static str,
    /// Destination URL. `None` marks an in-progress placeholder that is
    /// rendered as a non-navigable, visibly muted label instead of a link.
    pub href: Option<&'static str>,
}

/// A column of footer links.
pub(crate) struct FooterColumn {
    pub kicker: &'static str,
    pub links: &'static [FooterLink],
}

/// Configuration for [`render`].
pub(crate) struct Footer<'a> {
    /// Brand name shown at the top-left.
    pub brand: &'a str,
    /// Short lede paragraph under the brand name.
    pub lede: &'a str,
    /// System status text (e.g. `"All systems operational"`).
    pub status: &'a str,
    /// Right-hand link columns.
    pub columns: &'a [FooterColumn],
}

/// Render the footer.
#[must_use]
pub(crate) fn render(footer: &Footer<'_>) -> String {
    let brand = footer.brand.to_owned();
    let lede = footer.lede.to_owned();
    let status = footer.status.to_owned();

    FooterEl::builder()
        .class("col-span-full")
        .division(|grid| {
            let grid = grid
                .class("max-w-[1280px] mx-auto w-full px-4 md:px-8 py-10 grid grid-cols-2 md:grid-cols-[2fr_1fr_1fr_1fr] gap-x-8 gap-y-6 text-[13px]")
                .division(|brand_col| {
                    brand_col
                        .class("col-span-2 md:col-span-1 flex flex-col")
                        .division(|d| {
                            d.class("text-[15px] font-semibold tracking-tight")
                                .text(brand)
                        })
                        .paragraph(|p| {
                            p.class("mt-3 max-w-xs text-ink-500 leading-relaxed").text(lede)
                        })
                        .division(|d| {
                            d.class("mt-auto pt-6 text-[12px] text-ink-500 mono inline-flex items-center gap-2")
                                .span(|s| s.class("h-1.5 w-1.5 rounded-full bg-positive"))
                                .text(status)
                        })
                });
            for col in footer.columns {
                push_column(grid, col);
            }
            grid
        })
        .build()
        .to_string()
}

fn push_column(parent: &mut html::text_content::builders::DivisionBuilder, col: &FooterColumn) {
    let kicker = col.kicker.to_owned();
    parent.division(|d| {
        let d = d.division(|k| {
            k.class("text-[12px] mono uppercase tracking-wider text-ink-500")
                .text(kicker)
        });
        d.unordered_list(|ul| {
            let ul = ul.class("mt-3 space-y-2 text-ink-700");
            for link in col.links {
                push_link(ul, link);
            }
            ul
        })
    });
}

/// Append a single footer link. A link with `href: None` renders as a
/// non-navigable, muted span so it stays visible while signalling that the
/// destination is still in progress.
fn push_link(ul: &mut html::text_content::builders::UnorderedListBuilder, link: &FooterLink) {
    let label = link.label.to_owned();
    match link.href {
        Some(href) => {
            let href = href.to_owned();
            ul.list_item(|li| {
                li.anchor(|a| {
                    a.href(href)
                        .class("hover:text-ink-900 no-underline")
                        .text(label)
                })
            });
        }
        None => {
            ul.list_item(|li| {
                li.span(|s| {
                    s.aria_disabled(true)
                        .class("text-ink-400 cursor-not-allowed")
                        .text(label)
                })
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot() {
        const BROWSE: &[FooterLink] = &[
            FooterLink {
                label: "Packages",
                href: Some("/packages"),
            },
            FooterLink {
                label: "Categories",
                href: Some("/categories"),
            },
        ];
        const COMMUNITY: &[FooterLink] = &[
            FooterLink {
                label: "GitHub",
                href: Some("https://github.com/yoshuawuyts/component-cli"),
            },
            FooterLink {
                label: "Spec",
                href: None,
            },
        ];
        const LEGAL: &[FooterLink] = &[
            FooterLink {
                label: "Privacy",
                href: Some("/privacy"),
            },
            FooterLink {
                label: "Terms",
                href: Some("/terms"),
            },
        ];
        let html = render(&Footer {
            brand: "component",
            lede: "The package manager for WebAssembly Components.",
            status: "All systems operational",
            columns: &[
                FooterColumn {
                    kicker: "Browse",
                    links: BROWSE,
                },
                FooterColumn {
                    kicker: "Community",
                    links: COMMUNITY,
                },
                FooterColumn {
                    kicker: "Legal",
                    links: LEGAL,
                },
            ],
        });
        insta::assert_snapshot!(crate::components::ds::pretty_html(&html));
    }
}
