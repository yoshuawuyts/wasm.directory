//! Build a `clap::Command` from a [`LibrarySurface`] and parse user
//! input back into an [`Invocation`].

use std::collections::BTreeSet;

use clap::{Arg, ArgAction, ArgMatches, Command, value_parser};
use wasmtime::component::Val;

use crate::wit::{FuncDecl, FuncPath, LibraryItem, LibrarySurface, ParamDecl, WitTy};

/// A fully-parsed user invocation, ready to hand off to wasmtime.
#[derive(Debug)]
#[must_use]
pub struct Invocation {
    /// Logical path to the function: an optional interface
    /// (`Some("namespace:pkg/iface@version")`) plus the function
    /// name.
    pub path: FuncPath,
    /// Arguments to pass to wasmtime, in WIT-declaration order.
    pub args: Vec<Val>,
    /// Result types expected by the WIT signature. The runtime
    /// returns one [`Val`] per entry; the wire-up uses this to
    /// validate the result count and to drive type-aware
    /// rendering decisions.
    pub expected_results: Vec<WitTy>,
}

/// Errors raised when translating a [`LibrarySurface`] into a
/// [`clap::Command`] or when parsing user input back into an
/// [`Invocation`].
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CliError {
    /// A type at this point in the surface cannot be expressed as a
    /// CLI argument (resource handles or unsupported compounds).
    #[error("unsupported argument type for `{param}`: {reason}")]
    UnsupportedArg {
        /// Name of the parameter whose type we couldn't express.
        param: String,
        /// Human-readable explanation of why.
        reason: String,
    },
    /// Two record fields in the same function would map to the same
    /// `--flag` name even after prefixing.
    #[error("argument flag `--{flag}` collides between two parameters")]
    FlagCollision {
        /// The colliding flag name (without leading dashes).
        flag: String,
    },
    /// User supplied a value that doesn't parse as the expected type.
    #[error("invalid value for `{param}`: {reason}")]
    InvalidValue {
        /// Name of the parameter whose value didn't parse.
        param: String,
        /// Human-readable parse-error message.
        reason: String,
    },
    /// User asked for a function that the surface doesn't expose.
    #[error("no such export: {path}")]
    UnknownFunc {
        /// The export path the user requested.
        path: String,
    },
}

/// Build a top-level `clap::Command` representing every export of
/// `surface` as a sub-command tree.
// r[impl run.library-help]
// r[impl run.library-dispatch]
pub fn build_clap(surface: &LibrarySurface, program_name: &str) -> Result<Command, CliError> {
    let mut root = Command::new(program_name.to_string())
        .about("Dynamically dispatch to a library-style component.")
        .subcommand_required(true)
        .arg_required_else_help(true);

    for item in &surface.items {
        match item {
            LibraryItem::Func(f) => {
                root = root.subcommand(build_func_command(f)?);
            }
            LibraryItem::Interface {
                name, doc, funcs, ..
            } => {
                let mut iface_cmd = Command::new(name.clone())
                    .subcommand_required(true)
                    .arg_required_else_help(true);
                if let Some(doc) = doc {
                    iface_cmd = iface_cmd.about(doc.trim().to_string());
                }
                for f in funcs {
                    iface_cmd = iface_cmd.subcommand(build_func_command(f)?);
                }
                root = root.subcommand(iface_cmd);
            }
        }
    }
    Ok(root)
}

/// Build a single function as a `clap::Command`.
fn build_func_command(func: &FuncDecl) -> Result<Command, CliError> {
    let mut cmd = Command::new(func.name.clone());
    if let Some(doc) = &func.doc {
        cmd = cmd.about(doc.trim().to_string());
    }
    let mut seen_flags: BTreeSet<String> = BTreeSet::new();
    let multi_record = func
        .params
        .iter()
        .filter(|p| matches!(p.ty, WitTy::Record(_)))
        .count()
        > 1;
    let last_idx = func.params.len().saturating_sub(1);

    for (i, param) in func.params.iter().enumerate() {
        let last = i == last_idx;
        cmd = add_param_args(cmd, param, multi_record, last, &mut seen_flags)?;
    }
    Ok(cmd)
}

/// Append argument(s) for a single parameter to `cmd`.
fn add_param_args(
    mut cmd: Command,
    param: &ParamDecl,
    multi_record: bool,
    last: bool,
    seen: &mut BTreeSet<String>,
) -> Result<Command, CliError> {
    match &param.ty {
        // Optional wraps the underlying rule but flips required→false.
        // Only primitive-like inner types are supported on the CLI;
        // `option<record>`, `option<list<_>>`, etc. would require a
        // different surface and are explicitly rejected here.
        WitTy::Option(inner) => match inner.as_ref() {
            WitTy::Bool
            | WitTy::S8
            | WitTy::S16
            | WitTy::S32
            | WitTy::S64
            | WitTy::U8
            | WitTy::U16
            | WitTy::U32
            | WitTy::U64
            | WitTy::F32
            | WitTy::F64
            | WitTy::Char
            | WitTy::String
            | WitTy::Variant(_)
            | WitTy::Enum(_) => {
                let inner_param = ParamDecl {
                    name: param.name.clone(),
                    ty: (**inner).clone(),
                };
                let arg = positional_for_primitive(&inner_param);
                cmd = cmd.arg(arg.required(false));
                Ok(cmd)
            }
            other => Err(CliError::UnsupportedArg {
                param: param.name.clone(),
                reason: format!(
                    "option<{}> parameters are not supported as CLI input",
                    debug_kind(other)
                ),
            }),
        },
        WitTy::Record(fields) => {
            for (fname, fty) in fields {
                let flag = if multi_record {
                    format!("{}-{}", param.name, fname)
                } else {
                    fname.clone()
                };
                if !seen.insert(flag.clone()) {
                    return Err(CliError::FlagCollision { flag });
                }
                let mut arg = Arg::new(flag.clone())
                    .long(flag)
                    .required(true)
                    .help(format!("field `{fname}` of `{}`", param.name));
                // list<T> fields are repeatable flags: --flag v1 --flag v2
                if let WitTy::List(inner) = fty {
                    arg = arg.action(ArgAction::Append).num_args(1).required(false);
                    cmd = cmd.arg(attach_value_parser(arg, inner));
                } else {
                    arg = arg.num_args(1);
                    cmd = cmd.arg(attach_value_parser(arg, fty));
                }
            }
            Ok(cmd)
        }
        WitTy::List(inner) => {
            let name = param.name.clone();
            let mut arg = Arg::new(name.clone()).help(format!("list parameter `{}`", param.name));
            if last {
                arg = arg.num_args(0..);
            } else {
                arg = arg
                    .long(name)
                    .action(ArgAction::Append)
                    .num_args(1)
                    .required(false);
            }
            cmd = cmd.arg(attach_value_parser(arg, inner));
            Ok(cmd)
        }
        WitTy::Bool
        | WitTy::S8
        | WitTy::S16
        | WitTy::S32
        | WitTy::S64
        | WitTy::U8
        | WitTy::U16
        | WitTy::U32
        | WitTy::U64
        | WitTy::F32
        | WitTy::F64
        | WitTy::Char
        | WitTy::String
        | WitTy::Variant(_)
        | WitTy::Enum(_) => {
            let arg = positional_for_primitive(param);
            cmd = cmd.arg(arg);
            Ok(cmd)
        }
        WitTy::Result { .. } | WitTy::Tuple(_) | WitTy::Flags(_) => Err(CliError::UnsupportedArg {
            param: param.name.clone(),
            reason: format!(
                "{} parameters are not supported as CLI input",
                debug_kind(&param.ty)
            ),
        }),
    }
}

/// Build a positional `Arg` for a primitive / string / variant /
/// enum parameter.
fn positional_for_primitive(param: &ParamDecl) -> Arg {
    let arg = Arg::new(param.name.clone())
        .required(true)
        .num_args(1)
        .help(format!("parameter `{}`", param.name));
    attach_value_parser(arg, &param.ty)
}

/// Attach a `value_parser` for a [`WitTy`]. We only validate basic
/// number ranges and case allowlists here; conversion to `Val`
/// happens in [`parse_invocation`].
// r[impl run.library-args]
fn attach_value_parser(arg: Arg, ty: &WitTy) -> Arg {
    match ty {
        WitTy::Bool => arg.value_parser(value_parser!(bool)),
        WitTy::S8 => arg.value_parser(value_parser!(i8)),
        WitTy::S16 => arg.value_parser(value_parser!(i16)),
        WitTy::S32 => arg.value_parser(value_parser!(i32)),
        WitTy::S64 => arg.value_parser(value_parser!(i64)),
        WitTy::U8 => arg.value_parser(value_parser!(u8)),
        WitTy::U16 => arg.value_parser(value_parser!(u16)),
        WitTy::U32 => arg.value_parser(value_parser!(u32)),
        WitTy::U64 => arg.value_parser(value_parser!(u64)),
        WitTy::F32 => arg.value_parser(parse_f32),
        WitTy::F64 => arg.value_parser(parse_f64),
        WitTy::Char => arg.value_parser(parse_char),
        WitTy::Enum(cases) => arg.value_parser(cases.clone()),
        WitTy::Variant(cases) => {
            // Variant cases on the CLI are written as `name` or
            // `name=payload`. The allowlist applies to the bare
            // case name; we surface allowed cases in the help text.
            let names: Vec<String> = cases.iter().map(|(n, _)| n.clone()).collect();
            arg.value_parser(VariantCaseParser { names })
        }
        // For string/list/option/record/result/tuple/flags clap
        // accepts the raw token; we parse it into a `Val` later
        // in `parse_invocation`.
        WitTy::String
        | WitTy::List(_)
        | WitTy::Option(_)
        | WitTy::Record(_)
        | WitTy::Result { .. }
        | WitTy::Tuple(_)
        | WitTy::Flags(_) => arg,
    }
}

/// Custom value-parser for `f32`. Clap's `value_parser!` macro
/// doesn't have a built-in shorthand for floats.
fn parse_f32(s: &str) -> Result<f32, String> {
    s.parse::<f32>().map_err(|e| e.to_string())
}

/// Custom value-parser for `f64`.
fn parse_f64(s: &str) -> Result<f64, String> {
    s.parse::<f64>().map_err(|e| e.to_string())
}

/// Custom value-parser for `char`: the input must be exactly one
/// Unicode codepoint.
fn parse_char(s: &str) -> Result<char, String> {
    let mut chars = s.chars();
    let c = chars
        .next()
        .ok_or_else(|| "empty value for `char`".to_string())?;
    if chars.next().is_some() {
        return Err(format!("char must be exactly one codepoint, got {s:?}"));
    }
    Ok(c)
}

/// Clap value-parser for `variant` arguments. Accepts either a bare
/// case name or `name=payload`; validates only the bare-name part
/// against the allowlist of case names.
#[derive(Clone)]
struct VariantCaseParser {
    names: Vec<String>,
}

impl clap::builder::TypedValueParser for VariantCaseParser {
    type Value = String;

    fn parse_ref(
        &self,
        cmd: &Command,
        arg: Option<&Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, clap::Error> {
        let value = value
            .to_str()
            .ok_or_else(|| clap::Error::new(clap::error::ErrorKind::InvalidUtf8))?
            .to_string();
        let case_name = value.split('=').next().unwrap_or(&value);
        if !self.names.iter().any(|n| n == case_name) {
            let mut err = clap::Error::new(clap::error::ErrorKind::InvalidValue).with_cmd(cmd);
            if let Some(arg) = arg {
                err.insert(
                    clap::error::ContextKind::InvalidArg,
                    clap::error::ContextValue::String(arg.to_string()),
                );
            }
            err.insert(
                clap::error::ContextKind::InvalidValue,
                clap::error::ContextValue::String(case_name.to_string()),
            );
            err.insert(
                clap::error::ContextKind::ValidValue,
                clap::error::ContextValue::Strings(self.names.clone()),
            );
            return Err(err);
        }
        Ok(value)
    }

    fn possible_values(
        &self,
    ) -> Option<Box<dyn Iterator<Item = clap::builder::PossibleValue> + '_>> {
        Some(Box::new(
            self.names
                .iter()
                .map(|n| clap::builder::PossibleValue::new(n.clone())),
        ))
    }
}

fn debug_kind(ty: &WitTy) -> &'static str {
    match ty {
        WitTy::Bool => "bool",
        WitTy::S8 | WitTy::S16 | WitTy::S32 | WitTy::S64 => "signed integer",
        WitTy::U8 | WitTy::U16 | WitTy::U32 | WitTy::U64 => "unsigned integer",
        WitTy::F32 | WitTy::F64 => "float",
        WitTy::Char => "char",
        WitTy::String => "string",
        WitTy::List(_) => "list",
        WitTy::Option(_) => "option",
        WitTy::Result { .. } => "result",
        WitTy::Record(_) => "record",
        WitTy::Variant(_) => "variant",
        WitTy::Enum(_) => "enum",
        WitTy::Flags(_) => "flags",
        WitTy::Tuple(_) => "tuple",
    }
}

/// Parse a top-level [`ArgMatches`] back into an [`Invocation`] for
/// `surface`.
// r[impl run.library-args]
pub fn parse_invocation(
    matches: &ArgMatches,
    surface: &LibrarySurface,
) -> Result<Invocation, CliError> {
    let (sub_name, sub_matches) = matches.subcommand().ok_or_else(|| CliError::UnknownFunc {
        path: "<none>".to_string(),
    })?;

    // Top-level: free function, or interface name.
    for item in &surface.items {
        match item {
            LibraryItem::Func(f) if f.name == sub_name => {
                let args = collect_args(sub_matches, f)?;
                return Ok(Invocation {
                    path: FuncPath {
                        interface: None,
                        func: f.name.clone(),
                    },
                    args,
                    expected_results: f.results.iter().map(|r| r.ty.clone()).collect(),
                });
            }
            LibraryItem::Interface {
                name,
                export_name,
                funcs,
                ..
            } if name == sub_name => {
                let (fname, fmatches) = sub_matches
                    .subcommand()
                    .ok_or_else(|| CliError::UnknownFunc { path: name.clone() })?;
                let f = funcs.iter().find(|f| f.name == fname).ok_or_else(|| {
                    CliError::UnknownFunc {
                        path: format!("{name}::{fname}"),
                    }
                })?;
                let args = collect_args(fmatches, f)?;
                return Ok(Invocation {
                    path: FuncPath {
                        interface: Some(export_name.clone()),
                        func: f.name.clone(),
                    },
                    args,
                    expected_results: f.results.iter().map(|r| r.ty.clone()).collect(),
                });
            }
            _ => {}
        }
    }
    Err(CliError::UnknownFunc {
        path: sub_name.to_string(),
    })
}

/// Collect every argument for `func` from `matches` and convert each
/// to a [`Val`] in WIT declaration order.
fn collect_args(matches: &ArgMatches, func: &FuncDecl) -> Result<Vec<Val>, CliError> {
    let multi_record = func
        .params
        .iter()
        .filter(|p| matches!(p.ty, WitTy::Record(_)))
        .count()
        > 1;
    let mut out = Vec::with_capacity(func.params.len());
    for param in &func.params {
        out.push(collect_one(matches, param, multi_record)?);
    }
    Ok(out)
}

/// Collect a single parameter from `matches`.
fn collect_one(
    matches: &ArgMatches,
    param: &ParamDecl,
    multi_record: bool,
) -> Result<Val, CliError> {
    match &param.ty {
        WitTy::Option(inner) => {
            // Try to read a positional value; if absent, return None.
            let id = param.name.as_str();
            if matches.contains_id(id) {
                let inner_param = ParamDecl {
                    name: param.name.clone(),
                    ty: (**inner).clone(),
                };
                let v = collect_one(matches, &inner_param, multi_record)?;
                Ok(Val::Option(Some(Box::new(v))))
            } else {
                Ok(Val::Option(None))
            }
        }
        WitTy::Record(fields) => {
            // CRITICAL: emit fields in WIT-declaration order.
            // r[impl run.library-args]
            let mut pairs = Vec::with_capacity(fields.len());
            for (fname, fty) in fields {
                let flag = if multi_record {
                    format!("{}-{}", param.name, fname)
                } else {
                    fname.clone()
                };
                let v = collect_typed(matches, &flag, fty)?;
                pairs.push((fname.clone(), v));
            }
            Ok(Val::Record(pairs))
        }
        WitTy::List(inner) => {
            let id = param.name.as_str();
            let elems = collect_typed_many(matches, id, inner)?;
            Ok(Val::List(elems))
        }
        WitTy::Variant(cases) => {
            let raw: &String =
                matches
                    .get_one::<String>(&param.name)
                    .ok_or_else(|| CliError::InvalidValue {
                        param: param.name.clone(),
                        reason: "missing variant value".to_string(),
                    })?;
            let (case_name, payload_str) = match raw.split_once('=') {
                Some((n, p)) => (n, Some(p)),
                None => (raw.as_str(), None),
            };
            let case = cases.iter().find(|(n, _)| n == case_name).ok_or_else(|| {
                CliError::InvalidValue {
                    param: param.name.clone(),
                    reason: format!("unknown variant case `{case_name}`"),
                }
            })?;
            let payload = match (&case.1, payload_str) {
                (None, None) => None,
                (Some(payload_ty), Some(p)) => {
                    Some(Box::new(primitive_from_str(payload_ty, p, &param.name)?))
                }
                (None, Some(_)) => {
                    return Err(CliError::InvalidValue {
                        param: param.name.clone(),
                        reason: format!("case `{case_name}` takes no payload"),
                    });
                }
                (Some(_), None) => {
                    return Err(CliError::InvalidValue {
                        param: param.name.clone(),
                        reason: format!("case `{case_name}` requires a payload"),
                    });
                }
            };
            Ok(Val::Variant(case_name.to_string(), payload))
        }
        WitTy::Enum(_) => {
            let raw: &String =
                matches
                    .get_one::<String>(&param.name)
                    .ok_or_else(|| CliError::InvalidValue {
                        param: param.name.clone(),
                        reason: "missing enum value".to_string(),
                    })?;
            Ok(Val::Enum(raw.clone()))
        }
        // Primitives — clap has already coerced the value where it can.
        WitTy::Bool => Ok(Val::Bool(
            *matches.get_one::<bool>(&param.name).unwrap_or(&false),
        )),
        WitTy::S8 => Ok(Val::S8(*matches.get_one::<i8>(&param.name).unwrap_or(&0))),
        WitTy::S16 => Ok(Val::S16(*matches.get_one::<i16>(&param.name).unwrap_or(&0))),
        WitTy::S32 => Ok(Val::S32(*matches.get_one::<i32>(&param.name).unwrap_or(&0))),
        WitTy::S64 => Ok(Val::S64(*matches.get_one::<i64>(&param.name).unwrap_or(&0))),
        WitTy::U8 => Ok(Val::U8(*matches.get_one::<u8>(&param.name).unwrap_or(&0))),
        WitTy::U16 => Ok(Val::U16(*matches.get_one::<u16>(&param.name).unwrap_or(&0))),
        WitTy::U32 => Ok(Val::U32(*matches.get_one::<u32>(&param.name).unwrap_or(&0))),
        WitTy::U64 => Ok(Val::U64(*matches.get_one::<u64>(&param.name).unwrap_or(&0))),
        WitTy::F32 => Ok(Val::Float32(
            *matches.get_one::<f32>(&param.name).unwrap_or(&0.0),
        )),
        WitTy::F64 => Ok(Val::Float64(
            *matches.get_one::<f64>(&param.name).unwrap_or(&0.0),
        )),
        WitTy::Char => Ok(Val::Char(
            *matches.get_one::<char>(&param.name).unwrap_or(&'\0'),
        )),
        WitTy::String => Ok(Val::String(
            matches
                .get_one::<String>(&param.name)
                .cloned()
                .unwrap_or_default(),
        )),
        WitTy::Result { .. } | WitTy::Tuple(_) | WitTy::Flags(_) => Err(CliError::UnsupportedArg {
            param: param.name.clone(),
            reason: "compound type not supported as CLI input".to_string(),
        }),
    }
}

/// Read a single typed value from `matches` for a record field /
/// list element, downcasting through the type clap stored under the
/// hood (chosen by `attach_value_parser`).
fn collect_typed(matches: &ArgMatches, id: &str, ty: &WitTy) -> Result<Val, CliError> {
    let missing = || CliError::InvalidValue {
        param: id.to_string(),
        reason: "missing required value".to_string(),
    };
    match ty {
        WitTy::Bool => matches
            .get_one::<bool>(id)
            .map(|v| Val::Bool(*v))
            .ok_or_else(missing),
        WitTy::S8 => matches
            .get_one::<i8>(id)
            .map(|v| Val::S8(*v))
            .ok_or_else(missing),
        WitTy::S16 => matches
            .get_one::<i16>(id)
            .map(|v| Val::S16(*v))
            .ok_or_else(missing),
        WitTy::S32 => matches
            .get_one::<i32>(id)
            .map(|v| Val::S32(*v))
            .ok_or_else(missing),
        WitTy::S64 => matches
            .get_one::<i64>(id)
            .map(|v| Val::S64(*v))
            .ok_or_else(missing),
        WitTy::U8 => matches
            .get_one::<u8>(id)
            .map(|v| Val::U8(*v))
            .ok_or_else(missing),
        WitTy::U16 => matches
            .get_one::<u16>(id)
            .map(|v| Val::U16(*v))
            .ok_or_else(missing),
        WitTy::U32 => matches
            .get_one::<u32>(id)
            .map(|v| Val::U32(*v))
            .ok_or_else(missing),
        WitTy::U64 => matches
            .get_one::<u64>(id)
            .map(|v| Val::U64(*v))
            .ok_or_else(missing),
        WitTy::F32 => matches
            .get_one::<f32>(id)
            .map(|v| Val::Float32(*v))
            .ok_or_else(missing),
        WitTy::F64 => matches
            .get_one::<f64>(id)
            .map(|v| Val::Float64(*v))
            .ok_or_else(missing),
        WitTy::Char => matches
            .get_one::<char>(id)
            .map(|v| Val::Char(*v))
            .ok_or_else(missing),
        // string/enum stored as String — wrap directly.
        WitTy::String | WitTy::Enum(_) => {
            let raw: &String = matches.get_one::<String>(id).ok_or_else(missing)?;
            primitive_from_str(ty, raw, id)
        }
        // list<T> stored as repeated values — collect all occurrences.
        WitTy::List(inner) => {
            let elems = collect_typed_many(matches, id, inner)?;
            Ok(Val::List(elems))
        }
        other => Err(CliError::UnsupportedArg {
            param: id.to_string(),
            reason: format!("cannot collect {}", debug_kind(other)),
        }),
    }
}

/// Read repeated typed values for a `list<T>` parameter.
fn collect_typed_many(matches: &ArgMatches, id: &str, ty: &WitTy) -> Result<Vec<Val>, CliError> {
    macro_rules! many {
        ($t:ty, $ctor:ident) => {{
            matches
                .get_many::<$t>(id)
                .map(|it| it.copied().map(Val::$ctor).collect::<Vec<_>>())
                .unwrap_or_default()
        }};
    }
    Ok(match ty {
        WitTy::Bool => many!(bool, Bool),
        WitTy::S8 => many!(i8, S8),
        WitTy::S16 => many!(i16, S16),
        WitTy::S32 => many!(i32, S32),
        WitTy::S64 => many!(i64, S64),
        WitTy::U8 => many!(u8, U8),
        WitTy::U16 => many!(u16, U16),
        WitTy::U32 => many!(u32, U32),
        WitTy::U64 => many!(u64, U64),
        WitTy::F32 => many!(f32, Float32),
        WitTy::F64 => many!(f64, Float64),
        WitTy::Char => many!(char, Char),
        WitTy::String | WitTy::Enum(_) => {
            let raws: Vec<String> = matches
                .get_many::<String>(id)
                .map(|it| it.cloned().collect())
                .unwrap_or_default();
            let mut out = Vec::with_capacity(raws.len());
            for raw in &raws {
                out.push(primitive_from_str(ty, raw, id)?);
            }
            out
        }
        other => {
            return Err(CliError::UnsupportedArg {
                param: id.to_string(),
                reason: format!(
                    "nested {} list not supported as CLI input",
                    debug_kind(other)
                ),
            });
        }
    })
}

/// Parse a raw CLI string into a [`Val`] for a primitive type.
fn primitive_from_str(ty: &WitTy, s: &str, param: &str) -> Result<Val, CliError> {
    let invalid = |reason: String| CliError::InvalidValue {
        param: param.to_string(),
        reason,
    };
    Ok(match ty {
        WitTy::Bool => Val::Bool(
            s.parse()
                .map_err(|e: std::str::ParseBoolError| invalid(e.to_string()))?,
        ),
        WitTy::S8 => Val::S8(
            s.parse()
                .map_err(|e: std::num::ParseIntError| invalid(e.to_string()))?,
        ),
        WitTy::S16 => Val::S16(
            s.parse()
                .map_err(|e: std::num::ParseIntError| invalid(e.to_string()))?,
        ),
        WitTy::S32 => Val::S32(
            s.parse()
                .map_err(|e: std::num::ParseIntError| invalid(e.to_string()))?,
        ),
        WitTy::S64 => Val::S64(
            s.parse()
                .map_err(|e: std::num::ParseIntError| invalid(e.to_string()))?,
        ),
        WitTy::U8 => Val::U8(
            s.parse()
                .map_err(|e: std::num::ParseIntError| invalid(e.to_string()))?,
        ),
        WitTy::U16 => Val::U16(
            s.parse()
                .map_err(|e: std::num::ParseIntError| invalid(e.to_string()))?,
        ),
        WitTy::U32 => Val::U32(
            s.parse()
                .map_err(|e: std::num::ParseIntError| invalid(e.to_string()))?,
        ),
        WitTy::U64 => Val::U64(
            s.parse()
                .map_err(|e: std::num::ParseIntError| invalid(e.to_string()))?,
        ),
        WitTy::F32 => Val::Float32(
            s.parse()
                .map_err(|e: std::num::ParseFloatError| invalid(e.to_string()))?,
        ),
        WitTy::F64 => Val::Float64(
            s.parse()
                .map_err(|e: std::num::ParseFloatError| invalid(e.to_string()))?,
        ),
        WitTy::Char => {
            let mut chars = s.chars();
            let c = chars
                .next()
                .ok_or_else(|| invalid("empty char".to_string()))?;
            if chars.next().is_some() {
                return Err(invalid(format!(
                    "char must be exactly one codepoint, got '{s}'"
                )));
            }
            Val::Char(c)
        }
        WitTy::String => Val::String(s.to_string()),
        WitTy::Enum(_) => Val::Enum(s.to_string()),
        other => {
            return Err(CliError::UnsupportedArg {
                param: param.to_string(),
                reason: format!("cannot parse `{}` from a single string", debug_kind(other)),
            });
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn func(name: &str, params: Vec<(&str, WitTy)>) -> FuncDecl {
        FuncDecl {
            name: name.to_string(),
            doc: None,
            params: params
                .into_iter()
                .map(|(n, ty)| ParamDecl {
                    name: n.to_string(),
                    ty,
                })
                .collect(),
            results: Vec::new(),
        }
    }

    fn surface(items: Vec<LibraryItem>) -> LibrarySurface {
        LibrarySurface { items }
    }

    fn parse(s: &LibrarySurface, argv: &[&str]) -> Result<Invocation, String> {
        let cmd = build_clap(s, "test").map_err(|e| e.to_string())?;
        let matches = cmd
            .try_get_matches_from(std::iter::once("test").chain(argv.iter().copied()))
            .map_err(|e| e.to_string())?;
        parse_invocation(&matches, s).map_err(|e| e.to_string())
    }

    // r[verify run.library-args]
    #[test]
    fn round_trip_string_arg() {
        let s = surface(vec![LibraryItem::Func(func(
            "to-word",
            vec![("markdown", WitTy::String)],
        ))]);
        let inv = parse(&s, &["to-word", "# hi"]).unwrap();
        assert_eq!(inv.path.interface, None);
        assert_eq!(inv.path.func, "to-word");
        assert_eq!(inv.args.len(), 1);
        assert!(matches!(&inv.args[0], Val::String(s) if s == "# hi"));
    }

    // r[verify run.library-args]
    #[test]
    fn interface_dispatch() {
        let s = surface(vec![LibraryItem::Interface {
            name: "math".to_string(),
            export_name: "test:kitchen-sink/math".to_string(),
            doc: None,
            funcs: vec![func("add", vec![("a", WitTy::S32), ("b", WitTy::S32)])],
        }]);
        let inv = parse(&s, &["math", "add", "1", "2"]).unwrap();
        assert_eq!(
            inv.path.interface.as_deref(),
            Some("test:kitchen-sink/math")
        );
        assert_eq!(inv.path.func, "add");
        assert!(matches!(inv.args[0], Val::S32(1)));
        assert!(matches!(inv.args[1], Val::S32(2)));
    }

    // r[verify run.library-args]
    #[test]
    fn record_field_order_preserved() {
        // Declared order: name, age. CLI flag order: --age first.
        let person_ty = WitTy::Record(vec![
            ("name".to_string(), WitTy::String),
            ("age".to_string(), WitTy::U32),
        ]);
        let s = surface(vec![LibraryItem::Func(func(
            "greet",
            vec![("person", person_ty)],
        ))]);
        let inv = parse(&s, &["greet", "--age", "37", "--name", "Ada"]).unwrap();
        let Val::Record(pairs) = &inv.args[0] else {
            panic!("expected record");
        };
        // wasmtime requires WIT-declaration order at call time.
        assert_eq!(pairs[0].0, "name");
        assert_eq!(pairs[1].0, "age");
    }

    // r[verify run.library-args]
    #[test]
    fn list_positional_when_last() {
        let s = surface(vec![LibraryItem::Interface {
            name: "math".to_string(),
            export_name: "math".to_string(),
            doc: None,
            funcs: vec![func("sum", vec![("xs", WitTy::List(Box::new(WitTy::S32)))])],
        }]);
        let inv = parse(&s, &["math", "sum", "1", "2", "3"]).unwrap();
        let Val::List(vals) = &inv.args[0] else {
            panic!("expected list");
        };
        assert_eq!(vals.len(), 3);
    }

    // r[verify run.library-args]
    #[test]
    fn variant_with_payload() {
        let pick_ty = WitTy::Variant(vec![
            ("red".to_string(), None),
            ("green".to_string(), None),
            ("blue".to_string(), Some(Box::new(WitTy::String))),
        ]);
        let s = surface(vec![LibraryItem::Func(func("pick", vec![("c", pick_ty)]))]);
        let inv = parse(&s, &["pick", "blue=indigo"]).unwrap();
        match &inv.args[0] {
            Val::Variant(case, Some(payload)) => {
                assert_eq!(case, "blue");
                assert!(matches!(&**payload, Val::String(s) if s == "indigo"));
            }
            other => panic!("expected variant blue(...), got {other:?}"),
        }
    }

    // r[verify run.library-help]
    #[test]
    fn missing_arg_is_clap_usage_error() {
        let s = surface(vec![LibraryItem::Func(func(
            "to-word",
            vec![("markdown", WitTy::String)],
        ))]);
        let res = parse(&s, &["to-word"]);
        let err = res.expect_err("missing arg should fail");
        assert!(
            err.contains("required") || err.contains("USAGE") || err.contains("Usage"),
            "expected clap usage error, got: {err}"
        );
    }

    // r[verify run.library-args]
    #[test]
    fn bad_variant_case_caught_by_clap() {
        let pick_ty = WitTy::Variant(vec![
            ("red".to_string(), None),
            ("green".to_string(), None),
            ("blue".to_string(), Some(Box::new(WitTy::String))),
        ]);
        let s = surface(vec![LibraryItem::Func(func("pick", vec![("c", pick_ty)]))]);
        let err = parse(&s, &["pick", "yellow"]).expect_err("unknown case must fail");
        assert!(
            err.contains("yellow") || err.contains("invalid value"),
            "expected clap rejection of unknown variant case, got: {err}"
        );
        assert!(
            err.contains("red") || err.contains("blue"),
            "expected allowed cases listed in error, got: {err}"
        );
    }

    // r[verify run.library-args]
    #[test]
    fn bad_float_caught_by_clap() {
        let s = surface(vec![LibraryItem::Func(func(
            "set",
            vec![("x", WitTy::F64)],
        ))]);
        let err = parse(&s, &["set", "not-a-number"]).expect_err("bad float must fail");
        assert!(
            err.contains("invalid value") || err.contains("invalid float"),
            "expected clap float error, got: {err}"
        );
    }

    // r[verify run.library-args]
    #[test]
    fn bad_char_caught_by_clap() {
        let s = surface(vec![LibraryItem::Func(func(
            "at",
            vec![("c", WitTy::Char)],
        ))]);
        let err = parse(&s, &["at", "abc"]).expect_err("multi-char must fail");
        assert!(
            err.contains("char") || err.contains("codepoint"),
            "got: {err}"
        );
    }

    // r[verify run.library-args]
    #[test]
    fn multi_record_field_prefixing() {
        // Two record params force the `--<param>-<field>` prefix.
        let rec_a = WitTy::Record(vec![("x".to_string(), WitTy::U32)]);
        let rec_b = WitTy::Record(vec![("x".to_string(), WitTy::U32)]);
        let s = surface(vec![LibraryItem::Func(func(
            "merge",
            vec![("a", rec_a), ("b", rec_b)],
        ))]);
        let inv = parse(&s, &["merge", "--a-x", "1", "--b-x", "2"]).expect("parse");
        let Val::Record(ar) = &inv.args[0] else {
            panic!("expected record");
        };
        assert!(matches!(ar[0].1, Val::U32(1)));
        let Val::Record(br) = &inv.args[1] else {
            panic!("expected record");
        };
        assert!(matches!(br[0].1, Val::U32(2)));
    }

    // r[verify run.library-args]
    #[test]
    fn record_with_list_field() {
        let rec_ty = WitTy::Record(vec![
            ("name".to_string(), WitTy::String),
            (
                "group-columns".to_string(),
                WitTy::List(Box::new(WitTy::String)),
            ),
        ]);
        let s = surface(vec![LibraryItem::Func(func(
            "transform",
            vec![("config", rec_ty)],
        ))]);
        let inv = parse(
            &s,
            &[
                "transform",
                "--name",
                "test",
                "--group-columns",
                "col1",
                "--group-columns",
                "col2",
            ],
        )
        .unwrap();
        let Val::Record(pairs) = &inv.args[0] else {
            panic!("expected record");
        };
        assert_eq!(pairs[0].0, "name");
        assert!(matches!(&pairs[0].1, Val::String(s) if s == "test"));
        assert_eq!(pairs[1].0, "group-columns");
        let Val::List(elems) = &pairs[1].1 else {
            panic!("expected list");
        };
        assert_eq!(elems.len(), 2);
        assert!(matches!(&elems[0], Val::String(s) if s == "col1"));
        assert!(matches!(&elems[1], Val::String(s) if s == "col2"));
    }

    // r[verify run.library-args]
    #[test]
    fn record_with_empty_list_field() {
        let rec_ty = WitTy::Record(vec![
            ("name".to_string(), WitTy::String),
            ("tags".to_string(), WitTy::List(Box::new(WitTy::U32))),
        ]);
        let s = surface(vec![LibraryItem::Func(func(
            "create",
            vec![("item", rec_ty)],
        ))]);
        let inv = parse(&s, &["create", "--name", "hello"]).unwrap();
        let Val::Record(pairs) = &inv.args[0] else {
            panic!("expected record");
        };
        assert_eq!(pairs[1].0, "tags");
        let Val::List(elems) = &pairs[1].1 else {
            panic!("expected list");
        };
        assert!(elems.is_empty());
    }

    // r[verify run.library-args]
    #[test]
    fn multi_record_collision_errors() {
        // Param `a-b` field `c` and param `a` field `b-c` both
        // produce `--a-b-c` after prefixing → must be rejected at
        // CLI-build time.
        let rec_outer = WitTy::Record(vec![("b-c".to_string(), WitTy::U32)]);
        let rec_inner = WitTy::Record(vec![("c".to_string(), WitTy::U32)]);
        let s = surface(vec![LibraryItem::Func(func(
            "collide",
            vec![("a", rec_outer), ("a-b", rec_inner)],
        ))]);
        let err = build_clap(&s, "test").expect_err("must detect collision");
        assert!(matches!(err, CliError::FlagCollision { ref flag } if flag == "a-b-c"));
    }
}
