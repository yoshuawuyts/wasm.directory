//! Shared WIT code rendering helpers.
//!
//! Provides functions for rendering WIT type references as HTML spans
//! with proper linking and syntax coloring, used by both the interface
//! and item detail pages.

use crate::wit_doc::{HandleKind, TypeRef};

/// Render a `TypeRef` as an inline HTML span with links and colors.
pub(crate) fn render_type_ref(ty: &TypeRef) -> html::inline_text::Span {
    let mut span = html::inline_text::Span::builder();
    match ty {
        TypeRef::Primitive { name } => {
            span.class("text-ink-500").text(name.clone());
        }
        TypeRef::Named {
            name,
            url: Some(url),
            type_kind,
        } => {
            let color = match type_kind {
                Some(crate::wit_doc::WitTypeKind::Resource) => "text-wit-resource hover:underline",
                Some(crate::wit_doc::WitTypeKind::Record) => "text-wit-struct hover:underline",
                Some(crate::wit_doc::WitTypeKind::Enum) => "text-wit-enum hover:underline",
                _ => "text-accent hover:underline",
            };
            span.anchor(|a| a.href(url.clone()).class(color).text(name.clone()));
        }
        TypeRef::Named {
            name, url: None, ..
        } => {
            span.text(name.clone());
        }
        TypeRef::List { ty } => {
            span.text("list\u{200b}<".to_owned())
                .push(render_type_ref(ty))
                .text(">".to_owned());
        }
        TypeRef::Option { ty } => {
            span.text("option\u{200b}<".to_owned())
                .push(render_type_ref(ty))
                .text(">".to_owned());
        }
        TypeRef::Result { ok, err } => {
            span.text("result\u{200b}<".to_owned());
            if let Some(ok) = ok {
                span.push(render_type_ref(ok));
            } else {
                span.text("_".to_owned());
            }
            span.text(", ".to_owned());
            if let Some(err) = err {
                span.push(render_type_ref(err));
            } else {
                span.text("_".to_owned());
            }
            span.text(">".to_owned());
        }
        TypeRef::Tuple { types } => {
            span.text("tuple\u{200b}<".to_owned());
            for (i, t) in types.iter().enumerate() {
                if i > 0 {
                    span.text(", ".to_owned());
                }
                span.push(render_type_ref(t));
            }
            span.text(">".to_owned());
        }
        TypeRef::Handle {
            handle_kind,
            resource_name,
            resource_url,
        } => match handle_kind {
            HandleKind::Own => {
                if let Some(url) = resource_url {
                    span.anchor(|a| {
                        a.href(url.clone())
                            .class("text-accent hover:underline")
                            .text(resource_name.clone())
                    });
                } else {
                    span.text(resource_name.clone());
                }
            }
            HandleKind::Borrow => {
                span.text("borrow\u{200b}<".to_owned());
                if let Some(url) = resource_url {
                    span.anchor(|a| {
                        a.href(url.clone())
                            .class("text-accent hover:underline")
                            .text(resource_name.clone())
                    });
                } else {
                    span.text(resource_name.clone());
                }
                span.text(">".to_owned());
            }
        },
        TypeRef::Future { ty } => match ty {
            Some(t) => {
                span.text("future\u{200b}<".to_owned())
                    .push(render_type_ref(t))
                    .text(">".to_owned());
            }
            None => {
                span.text("future".to_owned());
            }
        },
        TypeRef::Stream { ty } => match ty {
            Some(t) => {
                span.text("stream\u{200b}<".to_owned())
                    .push(render_type_ref(t))
                    .text(">".to_owned());
            }
            None => {
                span.text("stream".to_owned());
            }
        },
    }
    span.build()
}

/// CSS class for the standard WIT code block container.
pub(crate) const CODE_BLOCK_CLASS: &str = crate::components::ds::code::CODE_BLOCK_CLASS;

/// Render a type definition inline inside a `<code>` block.
pub(crate) fn render_type_in_code(
    c: &mut html::inline_text::builders::CodeBuilder,
    ty: &crate::wit_doc::TypeDoc,
    indent: &str,
) {
    use crate::wit_doc::TypeKind;

    match &ty.kind {
        TypeKind::Record { fields } => {
            c.text(indent.to_owned())
                .span(|s| s.class("text-ink-500").text("record "))
                .span(|s| s.class("text-wit-struct font-medium").text(ty.name.clone()))
                .text(" {\n".to_owned());
            for f in fields {
                c.text(format!("{indent}  {}: ", f.name))
                    .push(render_type_ref(&f.ty))
                    .text(",\n".to_owned());
            }
            c.text(format!("{indent}}}"));
        }
        TypeKind::Variant { cases } => {
            c.text(indent.to_owned())
                .span(|s| s.class("text-ink-500").text("variant "))
                .span(|s| s.class("text-wit-struct font-medium").text(ty.name.clone()))
                .text(" {\n".to_owned());
            for case in cases {
                c.text(format!("{indent}  {}", case.name));
                if let Some(t) = &case.ty {
                    c.text("(".to_owned())
                        .push(render_type_ref(t))
                        .text(")".to_owned());
                }
                c.text(",\n".to_owned());
            }
            c.text(format!("{indent}}}"));
        }
        TypeKind::Enum { cases } => {
            c.text(indent.to_owned())
                .span(|s| s.class("text-ink-500").text("enum "))
                .span(|s| s.class("text-wit-enum font-medium").text(ty.name.clone()))
                .text(" {\n".to_owned());
            for case in cases {
                c.text(format!("{indent}  {},\n", case.name));
            }
            c.text(format!("{indent}}}"));
        }
        TypeKind::Flags { flags } => {
            c.text(indent.to_owned())
                .span(|s| s.class("text-ink-500").text("flags "))
                .span(|s| s.class("text-wit-enum font-medium").text(ty.name.clone()))
                .text(" {\n".to_owned());
            for f in flags {
                c.text(format!("{indent}  {},\n", f.name));
            }
            c.text(format!("{indent}}}"));
        }
        TypeKind::Resource { .. } => {
            c.text(indent.to_owned())
                .span(|s| s.class("text-ink-500").text("resource "))
                .span(|s| {
                    s.class("text-wit-resource font-medium")
                        .text(ty.name.clone())
                })
                .text(";".to_owned());
        }
        TypeKind::Alias(type_ref) => {
            c.text(indent.to_owned())
                .span(|s| s.class("text-ink-500").text("type "))
                .span(|s| s.class("text-accent font-medium").text(ty.name.clone()))
                .text(" = ".to_owned())
                .push(render_type_ref(type_ref))
                .text(";".to_owned());
        }
    }
}

/// Render a function signature inline inside a `<code>` block.
pub(crate) fn render_func_in_code(
    c: &mut html::inline_text::builders::CodeBuilder,
    func: &crate::wit_doc::FunctionDoc,
    indent: &str,
) {
    c.text(indent.to_owned())
        .span(|s| s.class("text-wit-func font-medium").text(func.name.clone()))
        .text(": ".to_owned());
    if func.is_async {
        c.span(|s| s.class("text-ink-500").text("async func"));
    } else {
        c.span(|s| s.class("text-ink-500").text("func"));
    }
    c.text("(".to_owned());

    let visible: Vec<_> = func.params.iter().filter(|p| p.name != "self").collect();
    for (i, p) in visible.iter().enumerate() {
        if i > 0 {
            c.text(", ".to_owned());
        }
        c.text(format!("{}: ", p.name)).push(render_type_ref(&p.ty));
    }
    c.text(")".to_owned());

    if let Some(ret) = &func.result {
        c.text(" -> ".to_owned()).push(render_type_ref(ret));
    }
    c.text(";".to_owned());
}
