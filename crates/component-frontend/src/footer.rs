//! Site footer wrapper — delegates to the design-system footer component
//! with the default content used across all pages.

use crate::components::ds::footer::{Footer, FooterColumn, FooterLink};

const BROWSE: &[FooterLink] = &[
    FooterLink {
        label: "Packages",
        href: Some("/all"),
    },
    FooterLink {
        label: "Authors",
        href: None,
    },
    FooterLink {
        label: "Registries",
        href: None,
    },
    FooterLink {
        label: "Changelog",
        href: None,
    },
];

const DEVELOP: &[FooterLink] = &[
    FooterLink {
        label: "Documentation",
        href: None,
    },
    FooterLink {
        label: "CLI reference",
        href: None,
    },
    FooterLink {
        label: "Spec",
        href: None,
    },
];

const COMMUNITY: &[FooterLink] = &[
    FooterLink {
        label: "GitHub",
        href: Some("https://github.com/yoshuawuyts/component-cli"),
    },
    FooterLink {
        label: "Status",
        href: Some("/status"),
    },
];

const COLUMNS: &[FooterColumn] = &[
    FooterColumn {
        kicker: "Browse",
        links: BROWSE,
    },
    FooterColumn {
        kicker: "Develop",
        links: DEVELOP,
    },
    FooterColumn {
        kicker: "Community",
        links: COMMUNITY,
    },
];

/// Render the site footer.
#[must_use]
pub(crate) fn render() -> String {
    crate::components::ds::footer::render(&Footer {
        brand: "Wasm Directory",
        lede: "A meta-registry and package manager for WebAssembly components. Made by Yosh Wuyts and contributors. To be donated to the Bytecode Alliance.",
        status: "All systems operational",
        columns: COLUMNS,
    })
}
