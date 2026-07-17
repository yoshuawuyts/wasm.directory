//! Unified WIT item rendering.
//!
//! A single abstraction every page uses to render rows for "items" — worlds,
//! interfaces, functions, types, modules, child components. Consolidates what
//! used to be three parallel rendering subsystems (`item_list::DynItemRow`
//! built inline in each page, plus `section_group::item_row` for component
//! fallbacks) so the same logical item kind looks identical across every page.
//!
//! Adding a new kind is now one variant + one match arm in [`WitItem::sigil`].

use html::inline_text::Anchor;
use html::text_content::Division;

use super::item_list::{self, DynItemRow};
use super::sigil::{self as s, Sigil};

/// The kind of WIT item being rendered. Drives sigil selection.
#[derive(Debug, Clone, Copy)]
pub(crate) enum WitItemKind {
    /// A WIT world.
    World,
    /// A WIT interface (named or imported/exported).
    Interface,
    /// A freestanding function or method.
    Function,
    /// A type definition. The inner tag picks the type sigil.
    Type(TypeTag),
    /// A core wasm module child of a component.
    Module,
    /// A nested component child of a component.
    Component,
}

/// Display-only mirror of [`crate::wit_doc::types::TypeKind`] used purely to
/// pick a sigil, so [`WitItem`] doesn't drag the parser type through pages
/// that don't need it.
#[derive(Debug, Clone, Copy)]
pub(crate) enum TypeTag {
    /// Generic / unknown type — uses the catch-all `T` sigil.
    Generic,
    /// `record` — lilac "R".
    Record,
    /// `variant` — teal "V".
    Variant,
    /// `enum` — teal "E".
    Enum,
    /// `flags` — lilac "F".
    Flags,
    /// `resource` — peach "R".
    Resource,
}

impl TypeTag {
    /// Map a parser [`crate::wit_doc::types::TypeKind`] to its display tag.
    pub(crate) fn from_kind(kind: &crate::wit_doc::TypeKind) -> Self {
        use crate::wit_doc::TypeKind;
        match kind {
            TypeKind::Record { .. } => Self::Record,
            TypeKind::Variant { .. } => Self::Variant,
            TypeKind::Enum { .. } => Self::Enum,
            TypeKind::Flags { .. } => Self::Flags,
            TypeKind::Resource { .. } => Self::Resource,
            TypeKind::Alias(_) => Self::Generic,
        }
    }
}

/// A single renderable item row. Built once by each page from whatever data
/// source it has (rich `WitDocument`, `WitInterfaceRef`, `ComponentSummary`),
/// then handed to [`render_item_section`] for uniform rendering.
pub(crate) struct WitItem {
    /// What kind of item this is (drives the sigil).
    pub kind: WitItemKind,
    /// Display name shown in the row.
    pub name: String,
    /// Link target for the row.
    pub href: String,
    /// Optional first-sentence description shown under the name.
    pub docs: Option<String>,
    /// Optional inline version badge shown next to the name.
    pub version: String,
    /// Optional right-aligned meta string (e.g. stability).
    pub meta: String,
    /// Title / alt-text for the meta badge.
    pub meta_title: String,
    /// Whether to render the row as deprecated (struck-through).
    pub deprecated: bool,
    /// Optional HTML id for anchor targeting.
    pub id: Option<String>,
}

impl WitItem {
    /// The sigil for this item's kind.
    fn sigil(&self) -> &'static Sigil {
        match self.kind {
            WitItemKind::World => &s::WORLD,
            WitItemKind::Interface => &s::IFACE,
            WitItemKind::Function => &s::FUNC,
            WitItemKind::Type(tag) => match tag {
                TypeTag::Generic => &s::TYPE,
                TypeTag::Record => &s::RECORD,
                TypeTag::Variant => &s::VARIANT,
                TypeTag::Enum => &s::ENUM,
                TypeTag::Flags => &s::FLAGS,
                TypeTag::Resource => &s::RESOURCE,
            },
            WitItemKind::Module => &s::MODULE,
            WitItemKind::Component => &s::COMPONENT,
        }
    }

    /// Convert to the underlying `DynItemRow` consumed by `item_list`.
    fn to_dyn_row(&self) -> DynItemRow {
        let sigil = self.sigil();
        DynItemRow {
            sigil_bg: sigil.bg.to_owned(),
            sigil_color: sigil.color.to_owned(),
            sigil_text: sigil.text.to_owned(),
            name: self.name.clone(),
            href: self.href.clone(),
            desc: self.docs.clone().unwrap_or_default(),
            version: self.version.clone(),
            meta: self.meta.clone(),
            meta_title: self.meta_title.clone(),
            deprecated: self.deprecated,
            id: self.id.clone(),
        }
    }
}

/// Render a single item as an `<a class="item-row">`.
#[allow(dead_code)]
pub(crate) fn render_item_row(item: &WitItem) -> Anchor {
    item_list::render_dyn_item_row(&item.to_dyn_row())
}

/// Render a titled section containing a list of items.
///
/// Empty input returns an empty `<div>` so callers can unconditionally push
/// the result into a parent and the section just disappears.
pub(crate) fn render_item_section(title: &str, items: &[WitItem]) -> Division {
    if items.is_empty() {
        return Division::builder().build();
    }
    let rows: Vec<DynItemRow> = items.iter().map(WitItem::to_dyn_row).collect();
    item_list::render_dyn_item_list(title, &rows)
}

/// Convert a [`wasm_meta_registry_client::WitInterfaceRef`] into a
/// [`WitItem::Interface`].
///
/// The display name uses the world-page format `"package/interface"` (no
/// version suffix) so component imports/exports look identical to the
/// rich-WIT world page.
pub(crate) fn iface_ref_to_item(iface: &wasm_meta_registry_client::WitInterfaceRef) -> WitItem {
    // When the interface lives in the parent component's own package, render
    // it as native: drop the package prefix so it reads as a first-class
    // member of this component.
    let name = if iface.is_native {
        iface
            .interface
            .clone()
            .unwrap_or_else(|| iface.package.clone())
    } else {
        let mut name = iface.package.clone();
        if let Some(iface_name) = &iface.interface {
            name.push('/');
            name.push_str(iface_name);
        }
        name
    };
    let version = iface.version.clone().unwrap_or_default();
    WitItem {
        kind: WitItemKind::Interface,
        name,
        href: build_iface_href(iface).unwrap_or_default(),
        docs: iface.docs.clone(),
        version,
        meta: String::new(),
        meta_title: String::new(),
        deprecated: false,
        id: None,
    }
}

/// Build the URL for a WIT interface reference, mirroring the legacy
/// `package_shell::build_iface_href` logic.
fn build_iface_href(iface: &wasm_meta_registry_client::WitInterfaceRef) -> Option<String> {
    let (ns, name) = iface.package.split_once(':')?;
    Some(match (&iface.interface, &iface.version) {
        (Some(iface_name), Some(v)) => format!("/{ns}/{name}/{v}/interface/{iface_name}"),
        (None, Some(v)) => format!("/{ns}/{name}/{v}"),
        (Some(iface_name), None) => format!("/{ns}/{name}/interface/{iface_name}"),
        (None, None) => format!("/{ns}/{name}"),
    })
}
