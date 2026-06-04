//! Extract a [`LibrarySurface`] from a Wasm component's WIT.
//!
//! The surface is a flat IR over the supported subset of WIT types
//! that `component run` can map onto a `clap` CLI. Resources are
//! rejected because they cannot be sensibly represented on the
//! command line.

use wit_parser::decoding::{DecodedWasm, decode};
use wit_parser::{Resolve, Type, TypeDefKind, WorldItem, WorldKey};

/// Logical path to a single exported function on a component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FuncPath {
    /// `Some(name)` when the function lives inside a nested
    /// interface export; `None` for free world-level exports.
    pub interface: Option<String>,
    /// The function's name as declared in the WIT.
    pub func: String,
}

/// Local IR mirroring the supported subset of WIT types.
///
/// `WitTy::Record` and `WitTy::Variant` preserve WIT declaration
/// order, which is mandatory: wasmtime's runtime checks record fields
/// by position and name (see
/// `wasmtime/src/runtime/component/values.rs`), so we have to emit
/// them in the order they were declared.
// r[impl run.library-args]
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum WitTy {
    /// `bool`
    Bool,
    /// `s8`
    S8,
    /// `s16`
    S16,
    /// `s32`
    S32,
    /// `s64`
    S64,
    /// `u8`
    U8,
    /// `u16`
    U16,
    /// `u32`
    U32,
    /// `u64`
    U64,
    /// `f32`
    F32,
    /// `f64`
    F64,
    /// `char`
    Char,
    /// `string`
    String,
    /// `list<T>`
    List(Box<WitTy>),
    /// `option<T>`
    Option(Box<WitTy>),
    /// `result<T, E>` (either side may be absent).
    Result {
        /// The success-payload type, or `None` for `result<_, E>`.
        ok: Option<Box<WitTy>>,
        /// The error-payload type, or `None` for `result<T, _>`.
        err: Option<Box<WitTy>>,
    },
    /// `record { name: type, ... }` — fields preserved in WIT
    /// declaration order.
    Record(Vec<(String, WitTy)>),
    /// `variant { case, case(payload), ... }`.
    Variant(Vec<(String, Option<Box<WitTy>>)>),
    /// `enum { case-a, case-b, ... }`.
    Enum(Vec<String>),
    /// `flags { flag-a, flag-b, ... }`.
    Flags(Vec<String>),
    /// `tuple<T1, T2, ...>`.
    Tuple(Vec<WitTy>),
}

/// A single function parameter.
#[derive(Debug, Clone)]
pub struct ParamDecl {
    /// Parameter name as declared in the WIT.
    pub name: String,
    /// Parameter type.
    pub ty: WitTy,
}

/// A single function result. Currently unnamed.
#[derive(Debug, Clone)]
pub struct ResultDecl {
    /// Type of the result. Used by the wire-up to validate the
    /// number of returned values matches the declared signature
    /// and to drive future type-aware error messages.
    pub ty: WitTy,
}

/// A single exported function.
#[derive(Debug, Clone)]
pub struct FuncDecl {
    /// Function name as declared in the WIT.
    pub name: String,
    /// Doc-comment, used as the clap `about` text.
    pub doc: Option<String>,
    /// Parameters in declaration order.
    pub params: Vec<ParamDecl>,
    /// Function results, used to populate
    /// [`crate::Invocation::expected_results`] for runtime sanity
    /// checks.
    pub results: Vec<ResultDecl>,
}

/// A top-level item in the library surface.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum LibraryItem {
    /// Free function exported at the world level.
    Func(FuncDecl),
    /// An exported interface containing one or more functions.
    Interface {
        /// Short, user-facing name (e.g. `math`).
        name: String,
        /// Fully-qualified export name used by wasmtime
        /// (`namespace:pkg/iface@version`). May equal `name` when the
        /// interface was declared inline at the world level.
        export_name: String,
        /// Doc-comment declared on the interface, if any.
        doc: Option<String>,
        /// Functions exported by the interface, in WIT order.
        funcs: Vec<FuncDecl>,
    },
}

/// The full set of dynamically-dispatchable exports of a component.
#[derive(Debug, Clone)]
#[must_use]
pub struct LibrarySurface {
    /// Top-level items (functions and interfaces).
    pub items: Vec<LibraryItem>,
}

/// Errors raised when we cannot extract a usable surface.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum LibraryExtractError {
    /// The component bytes could not be decoded as a WIT-bearing
    /// component.
    #[error("failed to decode component WIT: {0}")]
    Decode(String),
    /// The component is a WIT package, not a compiled component.
    #[error("input is a WIT package, not a compiled component")]
    NotAComponent,
    // r[impl run.library-resources-rejected]
    /// The component exports a resource type, which cannot be
    /// expressed as a CLI argument.
    #[error("resource type `{name}` is not supported by `component run`")]
    Resource {
        /// Name of the resource type (or interface) that triggered
        /// the rejection.
        name: String,
    },
    /// A WIT type kind we don't support yet (futures, streams,
    /// error-context, owned/borrowed handles).
    #[error("unsupported WIT type kind: {kind}")]
    UnsupportedKind {
        /// Human-readable label for the unsupported kind
        /// (`"future"`, `"stream"`, `"map"`, etc.).
        kind: &'static str,
    },
    // r[impl run.library-resources-rejected]
    /// Every exported function was skipped because it uses an
    /// unsupported type (resource, stream, future, …), leaving no
    /// invocable surface to expose as a CLI.
    #[error("no invocable functions: all exports use unsupported types ({reasons})")]
    NoInvocableFunctions {
        /// Comma-separated reasons describing why each export was
        /// skipped (e.g. the resource/stream/future detail).
        reasons: String,
    },
}

impl LibraryExtractError {
    /// Whether this error means a single export should be skipped (it uses a
    /// type we cannot express as a CLI argument) rather than aborting the
    /// whole extraction. Decode/invariant failures are *not* skippable — they
    /// indicate a real problem and must propagate so extraction fails loudly.
    fn is_skippable(&self) -> bool {
        matches!(
            self,
            LibraryExtractError::Resource { .. } | LibraryExtractError::UnsupportedKind { .. }
        )
    }
}

/// Best-effort extraction for components that lack a `component-type` custom
/// section (e.g. components built with older `wit-bindgen` toolchains).
///
/// Streams the binary through [`wasmparser::Validator`] to obtain the
/// fully-typed [`wasmparser::types::Types`] table, then resolves each
/// root-level export via [`TypesRef::component_item_for_export`]. Free
/// functions become [`LibraryItem::Func`]; instance exports become
/// [`LibraryItem::Interface`] entries containing one [`FuncDecl`] per
/// exported function. The full WIT type vocabulary is supported
/// (records, variants, lists, options, results, tuples, enums, flags),
/// not just the primitive subset.
///
/// Individual functions that reference unsupported types (resources,
/// futures, streams, maps, fixed-length lists, error-context) are
/// silently dropped — that's consistent with the `Option<LibrarySurface>`
/// contract of the fallback path. When extraction fails entirely or
/// yields nothing, returns `None` so the original `wit_parser::decode`
/// error can propagate.
fn fallback_library_surface(bytes: &[u8]) -> Option<LibrarySurface> {
    use std::collections::HashMap;
    use wasmparser::component_types::{
        ComponentDefinedType, ComponentDefinedTypeId, ComponentEntityType, ComponentFuncType,
        ComponentFuncTypeId, ComponentInstanceType, ComponentInstanceTypeId, ComponentValType,
    };
    use wasmparser::types::TypesRef;
    use wasmparser::{
        Encoding, Parser, Payload, PrimitiveValType, ValidPayload, Validator, WasmFeatures,
    };

    fn prim_to_wit(p: PrimitiveValType) -> Option<WitTy> {
        #[allow(clippy::match_wildcard_for_single_variants)]
        match p {
            PrimitiveValType::Bool => Some(WitTy::Bool),
            PrimitiveValType::S8 => Some(WitTy::S8),
            PrimitiveValType::S16 => Some(WitTy::S16),
            PrimitiveValType::S32 => Some(WitTy::S32),
            PrimitiveValType::S64 => Some(WitTy::S64),
            PrimitiveValType::U8 => Some(WitTy::U8),
            PrimitiveValType::U16 => Some(WitTy::U16),
            PrimitiveValType::U32 => Some(WitTy::U32),
            PrimitiveValType::U64 => Some(WitTy::U64),
            PrimitiveValType::F32 => Some(WitTy::F32),
            PrimitiveValType::F64 => Some(WitTy::F64),
            PrimitiveValType::Char => Some(WitTy::Char),
            PrimitiveValType::String => Some(WitTy::String),
            // `error-context` (and any future kinds) cannot be CLI args.
            _ => None,
        }
    }

    fn cval_to_wit(r: &TypesRef<'_>, vt: &ComponentValType) -> Option<WitTy> {
        match vt {
            ComponentValType::Primitive(p) => prim_to_wit(*p),
            ComponentValType::Type(id) => defined_to_wit(r, defined_type(r, *id)),
        }
    }

    fn defined_to_wit(r: &TypesRef<'_>, d: &ComponentDefinedType) -> Option<WitTy> {
        match d {
            ComponentDefinedType::Primitive(p) => prim_to_wit(*p),
            ComponentDefinedType::Record(rec) => {
                let mut fields = Vec::with_capacity(rec.fields.len());
                for (fname, fty) in &rec.fields {
                    fields.push((fname.to_string(), cval_to_wit(r, fty)?));
                }
                Some(WitTy::Record(fields))
            }
            ComponentDefinedType::Variant(v) => {
                let mut cases = Vec::with_capacity(v.cases.len());
                for (cname, case) in &v.cases {
                    let payload = match &case.ty {
                        Some(t) => Some(Box::new(cval_to_wit(r, t)?)),
                        None => None,
                    };
                    cases.push((cname.to_string(), payload));
                }
                Some(WitTy::Variant(cases))
            }
            ComponentDefinedType::List(t) => Some(WitTy::List(Box::new(cval_to_wit(r, t)?))),
            ComponentDefinedType::Option(t) => Some(WitTy::Option(Box::new(cval_to_wit(r, t)?))),
            ComponentDefinedType::Tuple(t) => {
                let mut tys = Vec::with_capacity(t.types.len());
                for v in &t.types {
                    tys.push(cval_to_wit(r, v)?);
                }
                Some(WitTy::Tuple(tys))
            }
            ComponentDefinedType::Enum(s) => {
                Some(WitTy::Enum(s.iter().map(ToString::to_string).collect()))
            }
            ComponentDefinedType::Flags(s) => {
                Some(WitTy::Flags(s.iter().map(ToString::to_string).collect()))
            }
            ComponentDefinedType::Result { ok, err } => {
                let ok = match ok {
                    Some(t) => Some(Box::new(cval_to_wit(r, t)?)),
                    None => None,
                };
                let err = match err {
                    Some(t) => Some(Box::new(cval_to_wit(r, t)?)),
                    None => None,
                };
                Some(WitTy::Result { ok, err })
            }
            // Unsupported: owned/borrowed handles (resources), futures,
            // streams, maps, fixed-length lists. Skip the affected function.
            ComponentDefinedType::Own(_)
            | ComponentDefinedType::Borrow(_)
            | ComponentDefinedType::Future(_)
            | ComponentDefinedType::Stream(_)
            | ComponentDefinedType::Map(_, _)
            | ComponentDefinedType::FixedLengthList(_, _) => None,
        }
    }

    // Type ids surfaced by `wasmparser::Validator` always resolve, so the
    // underlying `Index` impl can't panic on us — these wrappers just
    // isolate the one place clippy would otherwise flag.
    #[allow(clippy::indexing_slicing)]
    fn func_type<'a>(r: &'a TypesRef<'a>, id: ComponentFuncTypeId) -> &'a ComponentFuncType {
        &r[id]
    }
    #[allow(clippy::indexing_slicing)]
    fn instance_type<'a>(
        r: &'a TypesRef<'a>,
        id: ComponentInstanceTypeId,
    ) -> &'a ComponentInstanceType {
        &r[id]
    }
    #[allow(clippy::indexing_slicing)]
    fn defined_type<'a>(
        r: &'a TypesRef<'a>,
        id: ComponentDefinedTypeId,
    ) -> &'a ComponentDefinedType {
        &r[id]
    }

    fn build_func_decl(
        r: &TypesRef<'_>,
        name: &str,
        func_ty: &ComponentFuncType,
        doc: Option<String>,
    ) -> Option<FuncDecl> {
        let mut params = Vec::with_capacity(func_ty.params.len());
        for (pname, pty) in &func_ty.params {
            params.push(ParamDecl {
                name: pname.to_string(),
                ty: cval_to_wit(r, pty)?,
            });
        }
        let results = match &func_ty.result {
            Some(ty) => vec![ResultDecl {
                ty: cval_to_wit(r, ty)?,
            }],
            None => Vec::new(),
        };
        Some(FuncDecl {
            name: name.to_string(),
            doc,
            params,
            results,
        })
    }

    fn iface_short_name(export_name: &str) -> String {
        // "local:time-server/time"   → "time"
        // "wasi:io/streams@0.2.0"   → "streams"
        let after_slash = export_name.rsplit('/').next().unwrap_or(export_name);
        after_slash
            .split('@')
            .next()
            .unwrap_or(after_slash)
            .to_string()
    }

    // Parse `package-docs` JSON into `{ iface_short_name -> { func_name -> doc } }`.
    // The empty string `""` keys the top-level (free) functions. Unknown JSON shapes
    // are silently ignored — docs are a best-effort enhancement, never required.
    fn parse_package_docs(raw: Option<&[u8]>) -> HashMap<String, HashMap<String, String>> {
        let mut out: HashMap<String, HashMap<String, String>> = HashMap::new();
        let Some(bytes) = raw else { return out };
        let Ok(json) = serde_json::from_slice::<serde_json::Value>(bytes) else {
            return out;
        };
        if let Some(top_funcs) = json.get("funcs").and_then(|v| v.as_object()) {
            let entry = out.entry(String::new()).or_default();
            for (fname, fobj) in top_funcs {
                if let Some(doc) = fobj.get("docs").and_then(|d| d.as_str()) {
                    entry.insert(fname.clone(), doc.to_string());
                }
            }
        }
        if let Some(ifaces) = json.get("interfaces").and_then(|v| v.as_object()) {
            for (iname, iobj) in ifaces {
                let Some(funcs) = iobj.get("funcs").and_then(|v| v.as_object()) else {
                    continue;
                };
                let entry = out.entry(iname.clone()).or_default();
                for (fname, fobj) in funcs {
                    if let Some(doc) = fobj.get("docs").and_then(|d| d.as_str()) {
                        entry.insert(fname.clone(), doc.to_string());
                    }
                }
            }
        }
        out
    }

    // Stream the bytes through the validator, recording the root export
    // names + `package-docs` custom section. The validator returns the
    // fully-populated `Types` on its outermost `ValidPayload::End`.
    let mut validator = Validator::new_with_features(WasmFeatures::all());
    let mut depth: u32 = 0;
    let mut root_exports: Vec<String> = Vec::new();
    let mut package_docs: Option<Vec<u8>> = None;
    let mut types: Option<wasmparser::types::Types> = None;

    for payload in Parser::new(0).parse_all(bytes) {
        let Ok(payload) = payload else { return None };
        match &payload {
            Payload::Version { encoding, .. } => {
                if depth == 0 && *encoding != Encoding::Component {
                    return None;
                }
                depth += 1;
            }
            Payload::End(_) => {
                depth = depth.saturating_sub(1);
            }
            Payload::ComponentExportSection(reader) if depth == 1 => {
                for export in reader.clone() {
                    let Ok(export) = export else { return None };
                    root_exports.push(export.name.name.to_string());
                }
            }
            Payload::CustomSection(cs) if depth == 1 && cs.name() == "package-docs" => {
                package_docs = Some(cs.data().to_vec());
            }
            _ => {}
        }
        let Ok(valid) = validator.payload(&payload) else {
            return None;
        };
        if let ValidPayload::End(t) = valid {
            types = Some(t);
        }
    }

    let types = types?;
    let r = types.as_ref();
    let docs = parse_package_docs(package_docs.as_deref());

    let mut items = Vec::new();
    for export_name in &root_exports {
        let Some(item) = r.component_item_for_export(export_name) else {
            continue;
        };
        match item.ty {
            ComponentEntityType::Func(fid) => {
                let doc = docs.get("").and_then(|m| m.get(export_name)).cloned();
                if let Some(decl) = build_func_decl(&r, export_name, func_type(&r, fid), doc) {
                    items.push(LibraryItem::Func(decl));
                }
            }
            ComponentEntityType::Instance(iid) => {
                let short = iface_short_name(export_name);
                // Skip "exports" — componentize-py/js bootstrap interface.
                // Calling its `init` panics because the interpreter is already
                // running. Matches the primary path's skip logic.
                if short == "exports" {
                    continue;
                }
                let iface_ty = instance_type(&r, iid);
                let iface_docs = docs.get(&short);
                let mut funcs = Vec::new();
                for (fname, fitem) in &iface_ty.exports {
                    let ComponentEntityType::Func(fid) = fitem.ty else {
                        continue;
                    };
                    let fname_s = fname.as_str().to_string();
                    let doc = iface_docs.and_then(|m| m.get(&fname_s)).cloned();
                    if let Some(decl) = build_func_decl(&r, &fname_s, func_type(&r, fid), doc) {
                        funcs.push(decl);
                    }
                }
                if !funcs.is_empty() {
                    items.push(LibraryItem::Interface {
                        name: short,
                        export_name: export_name.clone(),
                        doc: None,
                        funcs,
                    });
                }
            }
            _ => {}
        }
    }

    if items.is_empty() {
        None
    } else {
        Some(LibrarySurface { items })
    }
}

/// Decode `bytes` and walk the world's exports into a
/// [`LibrarySurface`].
pub fn extract_library_surface(bytes: &[u8]) -> Result<LibrarySurface, LibraryExtractError> {
    let decoded = match decode(bytes) {
        Ok(d) => d,
        Err(e) => {
            // `wit_parser::decode` requires a `component-type` custom section that
            // older bindgen toolchains (e.g. wit-bindgen-c 0.37) do not emit.
            // Try a direct wasmparser walk as a last resort.
            if let Some(surface) = fallback_library_surface(bytes) {
                return Ok(surface);
            }
            return Err(LibraryExtractError::Decode(e.to_string()));
        }
    };
    let (resolve, world_id) = match decoded {
        DecodedWasm::Component(r, w) => (r, w),
        DecodedWasm::WitPackage(_, _) => return Err(LibraryExtractError::NotAComponent),
    };

    let world = resolve
        .worlds
        .get(world_id)
        .ok_or_else(|| LibraryExtractError::Decode("world id not in resolve".to_string()))?;

    let mut items = Vec::new();
    // Reasons why individual exports were skipped (unsupported types). Used to
    // build a helpful error if *every* export ends up skipped.
    let mut skipped: Vec<String> = Vec::new();
    for (key, item) in &world.exports {
        match item {
            WorldItem::Function(func) => {
                match func_to_decl(&resolve, &func.name, func) {
                    Ok(decl) => items.push(LibraryItem::Func(decl)),
                    // skip functions with unsupported types (streams, futures, resources, etc.)
                    Err(e) if e.is_skippable() => skipped.push(e.to_string()),
                    // decode/invariant errors are real bugs — fail loudly.
                    Err(e) => return Err(e),
                }
            }
            WorldItem::Interface { id, .. } => {
                let iface = resolve.interfaces.get(*id).ok_or_else(|| {
                    LibraryExtractError::Decode("interface id not in resolve".to_string())
                })?;
                let iface_name = world_key_label(&resolve, key, iface.name.as_deref());
                // Skip "exports" — this is a componentize-py/componentize-js internal
                // bootstrap interface (not part of the user-facing API). Calling its
                // `init` function causes the Python/JS interpreter to panic because it
                // is already running.
                if iface_name == "exports" {
                    continue;
                }
                let export_name = world_key_export_name(&resolve, key, iface);
                let mut funcs = Vec::with_capacity(iface.functions.len());
                for func in iface.functions.values() {
                    match func_to_decl(&resolve, &func.name, func) {
                        Ok(decl) => funcs.push(decl),
                        // skip functions with unsupported types (streams, futures, resources, etc.)
                        Err(e) if e.is_skippable() => skipped.push(e.to_string()),
                        // decode/invariant errors are real bugs — fail loudly.
                        Err(e) => return Err(e),
                    }
                }
                if !funcs.is_empty() {
                    items.push(LibraryItem::Interface {
                        name: iface_name,
                        export_name,
                        doc: iface.docs.contents.clone(),
                        funcs,
                    });
                }
            }
            WorldItem::Type { .. } => {
                // Type aliases at the world level are not invocable.
            }
        }
    }

    // r[impl run.library-resources-rejected]
    // If no invocable items survived, every export used an unsupported type.
    // Surface that as an error (with the underlying reasons) instead of
    // returning an empty surface, which would otherwise make `component run`
    // print help and exit 0.
    if items.is_empty() {
        let reasons = if skipped.is_empty() {
            "no exported functions".to_string()
        } else {
            dedup_preserving_order(skipped).join(", ")
        };
        return Err(LibraryExtractError::NoInvocableFunctions { reasons });
    }

    Ok(LibrarySurface { items })
}

/// Remove duplicate strings while preserving first-seen order.
fn dedup_preserving_order(items: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    items
        .into_iter()
        .filter(|item| seen.insert(item.clone()))
        .collect()
}
fn func_to_decl(
    resolve: &Resolve,
    name: &str,
    func: &wit_parser::Function,
) -> Result<FuncDecl, LibraryExtractError> {
    let mut params = Vec::with_capacity(func.params.len());
    for p in &func.params {
        params.push(ParamDecl {
            name: p.name.clone(),
            ty: type_to_wit_ty(resolve, &p.ty)?,
        });
    }
    let results = match &func.result {
        Some(ty) => vec![ResultDecl {
            ty: type_to_wit_ty(resolve, ty)?,
        }],
        None => Vec::new(),
    };
    Ok(FuncDecl {
        name: name.to_string(),
        doc: func.docs.contents.clone(),
        params,
        results,
    })
}

/// Convert a `wit_parser::Type` into a [`WitTy`].
fn type_to_wit_ty(resolve: &Resolve, ty: &Type) -> Result<WitTy, LibraryExtractError> {
    match ty {
        Type::Bool => Ok(WitTy::Bool),
        Type::S8 => Ok(WitTy::S8),
        Type::S16 => Ok(WitTy::S16),
        Type::S32 => Ok(WitTy::S32),
        Type::S64 => Ok(WitTy::S64),
        Type::U8 => Ok(WitTy::U8),
        Type::U16 => Ok(WitTy::U16),
        Type::U32 => Ok(WitTy::U32),
        Type::U64 => Ok(WitTy::U64),
        Type::F32 => Ok(WitTy::F32),
        Type::F64 => Ok(WitTy::F64),
        Type::Char => Ok(WitTy::Char),
        Type::String => Ok(WitTy::String),
        Type::ErrorContext => Err(LibraryExtractError::UnsupportedKind {
            kind: "error-context",
        }),
        Type::Id(id) => {
            let td = resolve
                .types
                .get(*id)
                .ok_or_else(|| LibraryExtractError::Decode("type id not in resolve".to_string()))?;
            type_def_to_wit_ty(resolve, td)
        }
    }
}

/// Convert a `wit_parser::TypeDef` into a [`WitTy`].
fn type_def_to_wit_ty(
    resolve: &Resolve,
    td: &wit_parser::TypeDef,
) -> Result<WitTy, LibraryExtractError> {
    let resource_name = || td.name.clone().unwrap_or_else(|| "<anonymous>".to_string());
    match &td.kind {
        TypeDefKind::List(inner) => Ok(WitTy::List(Box::new(type_to_wit_ty(resolve, inner)?))),
        TypeDefKind::Option(inner) => Ok(WitTy::Option(Box::new(type_to_wit_ty(resolve, inner)?))),
        TypeDefKind::Result(r) => {
            let ok = match &r.ok {
                Some(t) => Some(Box::new(type_to_wit_ty(resolve, t)?)),
                None => None,
            };
            let err = match &r.err {
                Some(t) => Some(Box::new(type_to_wit_ty(resolve, t)?)),
                None => None,
            };
            Ok(WitTy::Result { ok, err })
        }
        TypeDefKind::Record(rec) => {
            let mut fields = Vec::with_capacity(rec.fields.len());
            for f in &rec.fields {
                fields.push((f.name.clone(), type_to_wit_ty(resolve, &f.ty)?));
            }
            Ok(WitTy::Record(fields))
        }
        TypeDefKind::Variant(v) => {
            let mut cases = Vec::with_capacity(v.cases.len());
            for c in &v.cases {
                let payload = match &c.ty {
                    Some(t) => Some(Box::new(type_to_wit_ty(resolve, t)?)),
                    None => None,
                };
                cases.push((c.name.clone(), payload));
            }
            Ok(WitTy::Variant(cases))
        }
        TypeDefKind::Enum(e) => Ok(WitTy::Enum(
            e.cases.iter().map(|c| c.name.clone()).collect(),
        )),
        TypeDefKind::Flags(f) => Ok(WitTy::Flags(
            f.flags.iter().map(|fl| fl.name.clone()).collect(),
        )),
        TypeDefKind::Tuple(t) => {
            let mut tys = Vec::with_capacity(t.types.len());
            for inner in &t.types {
                tys.push(type_to_wit_ty(resolve, inner)?);
            }
            Ok(WitTy::Tuple(tys))
        }
        TypeDefKind::Type(inner) => type_to_wit_ty(resolve, inner),
        TypeDefKind::Resource | TypeDefKind::Handle(_) => Err(LibraryExtractError::Resource {
            name: resource_name(),
        }),
        TypeDefKind::Future(_) => Err(LibraryExtractError::UnsupportedKind { kind: "future" }),
        TypeDefKind::Stream(_) => Err(LibraryExtractError::UnsupportedKind { kind: "stream" }),
        TypeDefKind::Map(_, _) => Err(LibraryExtractError::UnsupportedKind { kind: "map" }),
        TypeDefKind::FixedLengthList(_, _) => Err(LibraryExtractError::UnsupportedKind {
            kind: "fixed-length-list",
        }),
        TypeDefKind::Unknown => Err(LibraryExtractError::UnsupportedKind { kind: "unknown" }),
    }
}

/// Best-effort name for an interface export, used as the clap
/// sub-command name.
fn world_key_label(resolve: &Resolve, key: &WorldKey, iface_name: Option<&str>) -> String {
    match key {
        WorldKey::Name(name) => name.clone(),
        WorldKey::Interface(id) => {
            if let Some(iface) = resolve.interfaces.get(*id)
                && let Some(name) = iface.name.as_deref()
            {
                return name.to_string();
            }
            iface_name.unwrap_or("interface").to_string()
        }
    }
}

/// Compute the fully-qualified export name wasmtime uses for an
/// interface export. For named world keys (declared inline) it is
/// just the bare name; for `WorldKey::Interface(id)` it's
/// `namespace:pkg/iface@version`.
fn world_key_export_name(
    resolve: &Resolve,
    key: &WorldKey,
    iface: &wit_parser::Interface,
) -> String {
    match key {
        WorldKey::Name(name) => name.clone(),
        WorldKey::Interface(_) => {
            let name = iface.name.as_deref().unwrap_or("interface");
            let Some(pkg_id) = iface.package else {
                return name.to_string();
            };
            let Some(pkg) = resolve.packages.get(pkg_id) else {
                return name.to_string();
            };
            let pname = &pkg.name;
            match &pname.version {
                Some(v) => format!("{}:{}/{name}@{v}", pname.namespace, pname.name),
                None => format!("{}:{}/{name}", pname.namespace, pname.name),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_path(name: &str) -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join(name)
    }

    fn read_fixture(name: &str) -> Vec<u8> {
        std::fs::read(fixture_path(name)).expect("read fixture")
    }

    // r[verify run.library-detection]
    #[test]
    fn extract_wordmark_surface() {
        let bytes = read_fixture("library_wordmark.wasm");
        let surface = extract_library_surface(&bytes).expect("extract");
        assert_eq!(surface.items.len(), 1);
        let LibraryItem::Func(decl) = &surface.items[0] else {
            panic!("expected free function, got {:?}", surface.items[0]);
        };
        assert_eq!(decl.name, "to-word");
        assert_eq!(decl.params.len(), 1);
        assert_eq!(decl.params[0].name, "markdown");
        assert!(matches!(decl.params[0].ty, WitTy::String));
        assert_eq!(decl.results.len(), 1);
        assert!(matches!(
            decl.results[0].ty,
            WitTy::Result {
                ok: Some(_),
                err: Some(_)
            }
        ));
    }

    // r[verify run.library-dispatch]
    #[test]
    fn extract_kitchen_sink_surface() {
        let bytes = read_fixture("library_kitchen_sink.wasm");
        let surface = extract_library_surface(&bytes).expect("extract");

        // Must contain at least one interface (math) plus the free
        // functions.
        let has_iface = surface
            .items
            .iter()
            .any(|i| matches!(i, LibraryItem::Interface { .. }));
        assert!(has_iface, "expected math interface in surface");

        let names: Vec<&str> = surface
            .items
            .iter()
            .map(|i| match i {
                LibraryItem::Func(f) => f.name.as_str(),
                LibraryItem::Interface { name, .. } => name.as_str(),
            })
            .collect();
        for expected in &["shout", "greet", "pick", "fail"] {
            assert!(
                names.iter().any(|n| *n == *expected),
                "missing export {expected}; got {names:?}"
            );
        }
    }

    // r[verify run.library-resources-rejected]
    #[test]
    fn extract_resources_fixture_is_rejected() {
        let bytes = read_fixture("library_resources.wasm");
        let err = extract_library_surface(&bytes).expect_err("must reject resource");
        // Every export uses a resource type, so all functions are skipped and
        // the surface ends up empty. We reject with `NoInvocableFunctions`,
        // whose reasons must name the unsupported `resource` type.
        match &err {
            LibraryExtractError::NoInvocableFunctions { reasons } => assert!(
                reasons.to_lowercase().contains("resource"),
                "expected reasons to mention resource, got {reasons:?}"
            ),
            other => panic!("expected NoInvocableFunctions error, got {other:?}"),
        }
    }

    // The validator-driven `fallback_library_surface` is normally not
    // invoked for fixtures with a `component-type` custom section.
    // Call it directly so we exercise the new code path and confirm it
    // recovers the full WIT type vocabulary (records, variants, lists,
    // options, results, tuples, enums, flags) — not just primitives.
    #[test]
    fn fallback_recovers_kitchen_sink_surface() {
        let bytes = read_fixture("library_kitchen_sink.wasm");
        let surface = fallback_library_surface(&bytes).expect("fallback must recover a surface");

        let names: Vec<&str> = surface
            .items
            .iter()
            .map(|i| match i {
                LibraryItem::Func(f) => f.name.as_str(),
                LibraryItem::Interface { name, .. } => name.as_str(),
            })
            .collect();
        for expected in &["shout", "greet", "pick", "fail"] {
            assert!(
                names.iter().any(|n| *n == *expected),
                "fallback missing export {expected}; got {names:?}"
            );
        }

        // At least one item must use a non-primitive type, proving we are
        // resolving through `ComponentDefinedType` and not just primitives.
        let has_complex = surface.items.iter().any(|i| match i {
            LibraryItem::Func(f) => {
                f.params.iter().any(|p| is_complex(&p.ty))
                    || f.results.iter().any(|r| is_complex(&r.ty))
            }
            LibraryItem::Interface { funcs, .. } => funcs.iter().any(|f| {
                f.params.iter().any(|p| is_complex(&p.ty))
                    || f.results.iter().any(|r| is_complex(&r.ty))
            }),
        });
        assert!(
            has_complex,
            "fallback only recovered primitive types; expected at least one record/variant/list/option/result/tuple/enum/flags"
        );
    }

    fn is_complex(ty: &WitTy) -> bool {
        matches!(
            ty,
            WitTy::Record(_)
                | WitTy::Variant(_)
                | WitTy::List(_)
                | WitTy::Option(_)
                | WitTy::Result { .. }
                | WitTy::Tuple(_)
                | WitTy::Enum(_)
                | WitTy::Flags(_)
        )
    }

    // The validator-driven fallback must reject WIT-package binaries the
    // same way the primary path's `NotAComponent` rejection does — i.e.
    // by returning `None` so the original decode error propagates to the
    // user. Use the resources fixture as a stand-in: it IS a component,
    // so a wit-package check isn't directly exercisable without crafting
    // bytes; instead just confirm that an obviously-not-a-component byte
    // string (an empty buffer) returns None rather than panicking.
    #[test]
    fn fallback_rejects_non_component_bytes() {
        // Empty buffer is not a valid wasm header at all.
        assert!(fallback_library_surface(&[]).is_none());
        // A bare wasm-module header (magic + version, no component bit) is
        // a core module, not a component — the fallback must bow out.
        let core_module: [u8; 8] = [0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
        assert!(fallback_library_surface(&core_module).is_none());
    }
}
