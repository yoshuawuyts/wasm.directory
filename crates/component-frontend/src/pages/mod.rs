//! Page rendering modules.

pub(crate) mod all;
pub(crate) mod child_component;
pub(crate) mod design_system;
pub(crate) mod detail;
pub(crate) mod docs;
pub(crate) mod downloads;
pub(crate) mod error;
pub(crate) mod home;
pub(crate) mod interface;
pub(crate) mod item;
pub(crate) mod namespace;
pub(crate) mod not_found;
pub(crate) mod package;
pub(crate) mod queue;
pub(crate) mod search;
pub(crate) mod world;

#[cfg(test)]
mod markdown_tests;
