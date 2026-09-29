//! Reusable UI components encoding the design system.
//!
//! Each submodule provides builder functions that return `html` crate types
//! with design-system Tailwind classes baked in. Pages call these instead of
//! writing raw class strings.

pub(crate) mod ds;
pub(crate) mod mobile_sidebar;
pub(crate) mod page_shell;
pub(crate) mod page_sidebar;
pub(crate) mod relationship_links;
pub(crate) mod wit_render;
