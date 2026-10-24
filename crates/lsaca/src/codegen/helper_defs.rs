//! Definitions of the generated per-type helpers. The prototypes and the
//! environment support live in the parent module.

use crate::codegen::c_writer::CWriter;
use crate::codegen::helpers::{compound_types, managed_types};
use crate::codegen::ops::{self, is_managed};
use crate::codegen::types::{
    EMPTY_STRUCT_FIELD, OPTION_SOME_MEMBER, RESULT_ERR_MEMBER, RESULT_OK_MEMBER, TypeNames,
    TypeRegistry, UNION_FIELD, field_cname, positional_field, variant_member,
};
use crate::codegen::util;
use crate::hir;
use crate::types::Ty;

pub fn emit_all(w: &mut CWriter, program: &hir::Program, registry: &TypeRegistry) {
    let names = registry.names().clone();
    for ty in managed_types(program, registry) {
        emit_copy(w, program, &names, &ty);
        emit_release(w, program, &names, &ty);
    }
    for ty in compound_types(program, registry) {
        emit_debug(w, program, &names, &ty);
        emit_eq(w, program, &names, &ty);
    }
}

// ---------------------------------------------------------------------------
// copy
// ---------------------------------------------------------------------------

fn emit_copy(w: &mut CWriter, program: &hir::Program, names: &TypeNames, ty: &Ty) {
    let c_ty = names.ctype(ty);
    let suffix = names.mangle(ty);
    w.line(format!("{} lsaca_copy_{}({} v) {{", c_ty, suffix, c_ty));
    w.indent();
    match ty {
        Ty::Vec(elem) => {
            let elem_ty = names.ctype(elem);
            w.line(format!("{} out;", c_ty));
            if is_managed(elem) {
                w.line("size_t i;");
                w.line("out.len = v.len;");
                w.line("out.cap = v.len;");
                w.line(format!(
                    "out.data = ({}*)ls_vec_alloc(v.len, sizeof({}));",
                    elem_ty, elem_ty
                ));
                w.line("for (i = 0; i < v.len; i += 1) {");
                w.indent();
                w.line(format!(
                    "out.data[i] = {};",
                    ops::copy_expr(names, elem, "v.data[i]")
                ));
                w.dedent();
                w.line("}");
            } else {
                w.line("out.len = v.len;");
                w.line("out.cap = v.len;");
                w.line(format!(
                    "out.data = ({}*)ls_vec_clone_bytes((const void*)v.data, v.len, sizeof({}));",
                    elem_ty, elem_ty
                ));
            }
            w.line("return out;");
        }
        Ty::Option(elem) => {
            w.line(format!("{} out = ({}){{0}};", c_ty, c_ty));
            w.line("out.tag = v.tag;");
            w.line("if (v.tag == 1) {");
            w.indent();
            w.line(format!(
                "out.u.{m} = {copy};",
                m = OPTION_SOME_MEMBER,
                copy = ops::copy_expr(
                    names,
                    elem,
                    &format!("v.{}.{}", UNION_FIELD, OPTION_SOME_MEMBER)
                )
            ));
            w.dedent();
            w.line("}");
            w.line("return out;");
        }
        Ty::Result(ok, err) => {
            w.line(format!("{} out = ({}){{0}};", c_ty, c_ty));
            w.line("out.tag = v.tag;");
            w.line("if (v.tag == 0) {");
            w.indent();
            w.line(format!(
                "out.u.{m} = {copy};",
                m = RESULT_OK_MEMBER,
                copy = ops::copy_expr(
                    names,
                    ok,
                    &format!("v.{}.{}", UNION_FIELD, RESULT_OK_MEMBER)
                )
            ));
            w.dedent();
            w.line("} else {");
            w.indent();
            w.line(format!(
                "out.u.{m} = {copy};",
                m = RESULT_ERR_MEMBER,
                copy = ops::copy_expr(
                    names,
                    err,
                    &format!("v.{}.{}", UNION_FIELD, RESULT_ERR_MEMBER)
                )
            ));
            w.dedent();
            w.line("}");
            w.line("return out;");
        }
        Ty::Tuple(elems) => {
            w.line(format!("{} out;", c_ty));
            for (index, elem) in elems.iter().enumerate() {
                let target = format!("out.{}", positional_field(index));
                let value = format!("v.{}", positional_field(index));
                emit_assign_copy(w, names, elem, &target, &value);
            }
            w.line("return out;");
        }
        Ty::Struct(index) => {
            w.line(format!("{} out;", c_ty));
            match struct_item(program, *index) {
                Some((_, fields)) if !fields.is_empty() => {
                    for (field_name, field_ty) in &fields {
                        let target = format!("out.{}", field_cname(field_name));
                        let value = format!("v.{}", field_cname(field_name));
                        emit_assign_copy(w, names, field_ty, &target, &value);
                    }
                }
                _ => {
                    w.line(format!("out.{} = 0;", EMPTY_STRUCT_FIELD));
                }
            }
            w.line("return out;");
        }
        Ty::Enum(index) => {
            w.line(format!("{} out = ({}){{0}};", c_ty, c_ty));
            w.line("out.tag = v.tag;");
            if let Some(item) = enum_item(program, *index) {
                w.line("switch (v.tag) {");
                w.indent();
                for variant in &item.variants {
                    if variant.fields.is_empty() {
                        continue;
                    }
                    let member = variant_member(&variant.name);
                    w.line(format!("case {}: {{", variant.tag));
                    w.indent();
                    for (field_index, field_ty) in variant.fields.iter().enumerate() {
                        let target = format!(
                            "out.{}.{}.{}",
                            UNION_FIELD,
                            member,
                            positional_field(field_index)
                        );
                        let value = format!(
                            "v.{}.{}.{}",
                            UNION_FIELD,
                            member,
                            positional_field(field_index)
                        );
                        emit_assign_copy(w, names, field_ty, &target, &value);
                    }
                    w.line("break;");
                    w.dedent();
                    w.line("}");
                }
                w.dedent();
                w.line("}");
            } else {
                w.line("(void)v;");
            }
            w.line("return out;");
        }
        Ty::Fn(_, _) => {
            w.line(format!("{} out;", c_ty));
            w.line(format!(
                "out.{} = lsaca_env_retain(v.{});",
                crate::codegen::types::FN_ENV_FIELD,
                crate::codegen::types::FN_ENV_FIELD
            ));
            w.line(format!(
                "out.{} = v.{};",
                crate::codegen::types::FN_PTR_FIELD,
                crate::codegen::types::FN_PTR_FIELD
            ));
            w.line("return out;");
        }
        _ => {
            w.line("(void)v;");
            w.line(format!("{} out = ({}){{0}};", c_ty, c_ty));
            w.line("return out;");
        }
    }
    w.dedent();
    w.line("}");
    w.blank();
}

fn emit_assign_copy(w: &mut CWriter, names: &TypeNames, ty: &Ty, target: &str, value: &str) {
    if is_managed(ty) {
        w.line(format!(
            "{} = {};",
            target,
            ops::copy_expr(names, ty, value)
        ));
    } else {
        w.line(format!("{} = {};", target, value));
    }
}

// ---------------------------------------------------------------------------
// release
// ---------------------------------------------------------------------------

fn emit_release(w: &mut CWriter, program: &hir::Program, names: &TypeNames, ty: &Ty) {
    let c_ty = names.ctype(ty);
    let suffix = names.mangle(ty);
    w.line(format!("void lsaca_release_{}({} v) {{", suffix, c_ty));
    w.indent();
    match ty {
        Ty::Vec(elem) => {
            if is_managed(elem) {
                w.line("size_t i;");
                w.line("for (i = 0; i < v.len; i += 1) {");
                w.indent();
                if let Some(stmt) = ops::release_stmt(names, elem, "v.data[i]") {
                    w.line(stmt);
                }
                w.dedent();
                w.line("}");
            }
            w.line("ls_vec_free_bytes((void*)v.data);");
        }
        Ty::Option(elem) => {
            w.line("if (v.tag == 1) {");
            w.indent();
            emit_release_value(w, names, elem, &format!("v.u.{}", OPTION_SOME_MEMBER));
            w.dedent();
            w.line("}");
        }
        Ty::Result(ok, err) => {
            w.line("if (v.tag == 0) {");
            w.indent();
            emit_release_value(w, names, ok, &format!("v.u.{}", RESULT_OK_MEMBER));
            w.dedent();
            w.line("} else {");
            w.indent();
            emit_release_value(w, names, err, &format!("v.u.{}", RESULT_ERR_MEMBER));
            w.dedent();
            w.line("}");
        }
        Ty::Tuple(elems) => {
            for (index, elem) in elems.iter().enumerate() {
                emit_release_value(w, names, elem, &format!("v.{}", positional_field(index)));
            }
            w.line("(void)v;");
        }
        Ty::Struct(index) => {
            if let Some((_, fields)) = struct_item(program, *index) {
                for (field_name, field_ty) in &fields {
                    emit_release_value(
                        w,
                        names,
                        field_ty,
                        &format!("v.{}", field_cname(field_name)),
                    );
                }
            }
            w.line("(void)v;");
        }
        Ty::Enum(index) => match enum_item(program, *index) {
            Some(item) => {
                w.line("switch (v.tag) {");
                w.indent();
                for variant in &item.variants {
                    if variant.fields.is_empty() {
                        continue;
                    }
                    let member = variant_member(&variant.name);
                    w.line(format!("case {}: {{", variant.tag));
                    w.indent();
                    for (field_index, field_ty) in variant.fields.iter().enumerate() {
                        emit_release_value(
                            w,
                            names,
                            field_ty,
                            &format!(
                                "v.{}.{}.{}",
                                UNION_FIELD,
                                member,
                                positional_field(field_index)
                            ),
                        );
                    }
                    w.line("break;");
                    w.dedent();
                    w.line("}");
                }
                w.dedent();
                w.line("}");
                w.line("(void)v;");
            }
            None => {
                w.line("(void)v;");
            }
        },
        Ty::Fn(_, _) => {
            w.line(format!(
                "lsaca_env_release(v.{});",
                crate::codegen::types::FN_ENV_FIELD
            ));
        }
        _ => {
            w.line("(void)v;");
        }
    }
    w.dedent();
    w.line("}");
    w.blank();
}

fn emit_release_value(w: &mut CWriter, names: &TypeNames, ty: &Ty, value: &str) {
    if let Some(stmt) = ops::release_stmt(names, ty, value) {
        w.line(stmt);
    }
}

// ---------------------------------------------------------------------------
// debug
// ---------------------------------------------------------------------------

fn emit_debug(w: &mut CWriter, program: &hir::Program, names: &TypeNames, ty: &Ty) {
    let c_ty = names.ctype(ty);
    let suffix = names.mangle(ty);
    w.line(format!(
        "LsString* lsaca_debug_{}(LsString* acc, {} v) {{",
        suffix, c_ty
    ));
    w.indent();
    emit_debug_body(w, program, names, ty);
    w.line("return acc;");
    w.dedent();
    w.line("}");
    w.blank();
}

fn push_cstr(w: &mut CWriter, text: &str) {
    w.line(format!(
        "acc = ls_format_push_cstr(acc, {});",
        util::c_string_literal(text)
    ));
}

fn emit_debug_append(w: &mut CWriter, names: &TypeNames, ty: &Ty, value: &str) {
    if let Some(stmt) = ops::debug_append_statement(names, ty, "acc", value) {
        w.line(stmt);
    }
}

fn emit_debug_body(w: &mut CWriter, program: &hir::Program, names: &TypeNames, ty: &Ty) {
    match ty {
        Ty::Vec(elem) => {
            w.line("size_t i;");
            push_cstr(w, "[");
            w.line("for (i = 0; i < v.len; i += 1) {");
            w.indent();
            w.line("if (i > 0) {");
            w.indent();
            w.line("acc = ls_format_push_cstr(acc, \", \");");
            w.dedent();
            w.line("}");
            emit_debug_append(w, names, elem, "v.data[i]");
            w.dedent();
            w.line("}");
            push_cstr(w, "]");
        }
        Ty::Option(elem) => {
            w.line("if (v.tag == 1) {");
            w.indent();
            push_cstr(w, "Some(");
            emit_debug_append(w, names, elem, &format!("v.u.{}", OPTION_SOME_MEMBER));
            push_cstr(w, ")");
            w.dedent();
            w.line("} else {");
            w.indent();
            push_cstr(w, "None");
            w.dedent();
            w.line("}");
        }
        Ty::Result(ok, err) => {
            w.line("if (v.tag == 0) {");
            w.indent();
            push_cstr(w, "Ok(");
            emit_debug_append(w, names, ok, &format!("v.u.{}", RESULT_OK_MEMBER));
            push_cstr(w, ")");
            w.dedent();
            w.line("} else {");
            w.indent();
            push_cstr(w, "Err(");
            emit_debug_append(w, names, err, &format!("v.u.{}", RESULT_ERR_MEMBER));
            push_cstr(w, ")");
            w.dedent();
            w.line("}");
        }
        Ty::Tuple(elems) => {
            push_cstr(w, "(");
            for (index, elem) in elems.iter().enumerate() {
                if index > 0 {
                    w.line("acc = ls_format_push_cstr(acc, \", \");");
                }
                emit_debug_append(w, names, elem, &format!("v.{}", positional_field(index)));
            }
            if elems.len() == 1 {
                w.line("acc = ls_format_push_cstr(acc, \",\");");
            }
            push_cstr(w, ")");
        }
        Ty::Struct(index) => match struct_item(program, *index) {
            Some((name, fields)) if !fields.is_empty() => {
                push_cstr(w, &name);
                push_cstr(w, " { ");
                for (field_index, (field_name, field_ty)) in fields.iter().enumerate() {
                    if field_index > 0 {
                        w.line("acc = ls_format_push_cstr(acc, \", \");");
                    }
                    w.line(format!(
                        "acc = ls_format_push_cstr(acc, {});",
                        util::c_string_literal(&format!("{}: ", field_name))
                    ));
                    emit_debug_append(
                        w,
                        names,
                        field_ty,
                        &format!("v.{}", field_cname(field_name)),
                    );
                }
                push_cstr(w, " }");
            }
            Some((name, _)) => push_cstr(w, &name),
            None => push_cstr(w, "<struct>"),
        },
        Ty::Enum(index) => match enum_item(program, *index) {
            Some(item) => {
                w.line("switch (v.tag) {");
                w.indent();
                for variant in &item.variants {
                    w.line(format!("case {}: {{", variant.tag));
                    w.indent();
                    push_cstr(w, &variant.name);
                    if !variant.fields.is_empty() {
                        push_cstr(w, "(");
                        let member = variant_member(&variant.name);
                        for (field_index, field_ty) in variant.fields.iter().enumerate() {
                            if field_index > 0 {
                                w.line("acc = ls_format_push_cstr(acc, \", \");");
                            }
                            emit_debug_append(
                                w,
                                names,
                                field_ty,
                                &format!(
                                    "v.{}.{}.{}",
                                    UNION_FIELD,
                                    member,
                                    positional_field(field_index)
                                ),
                            );
                        }
                        push_cstr(w, ")");
                    }
                    w.line("break;");
                    w.dedent();
                    w.line("}");
                }
                w.dedent();
                w.line("}");
            }
            None => push_cstr(w, "<enum>"),
        },
        Ty::Fn(_, _) => {
            push_cstr(w, "<function>");
            w.line("(void)v;");
        }
        _ => {
            w.line("(void)v;");
        }
    }
}

// ---------------------------------------------------------------------------
// equality
// ---------------------------------------------------------------------------

fn emit_eq(w: &mut CWriter, program: &hir::Program, names: &TypeNames, ty: &Ty) {
    let c_ty = names.ctype(ty);
    let suffix = names.mangle(ty);
    w.line(format!(
        "bool lsaca_eq_{}({} a, {} b) {{",
        suffix, c_ty, c_ty
    ));
    w.indent();
    emit_eq_body(w, program, names, ty);
    w.dedent();
    w.line("}");
    w.blank();
}

fn emit_eq_check(w: &mut CWriter, names: &TypeNames, ty: &Ty, a: &str, b: &str) {
    w.line(format!("if (!{}) {{", ops::eq_expr(names, ty, a, b)));
    w.indent();
    w.line("return false;");
    w.dedent();
    w.line("}");
}

fn emit_eq_body(w: &mut CWriter, program: &hir::Program, names: &TypeNames, ty: &Ty) {
    match ty {
        Ty::Vec(elem) => {
            w.line("size_t i;");
            w.line("if (a.len != b.len) {");
            w.indent();
            w.line("return false;");
            w.dedent();
            w.line("}");
            w.line("for (i = 0; i < a.len; i += 1) {");
            w.indent();
            emit_eq_check(w, names, elem, "a.data[i]", "b.data[i]");
            w.dedent();
            w.line("}");
            w.line("return true;");
        }
        Ty::Option(elem) => {
            w.line("if (a.tag != b.tag) {");
            w.indent();
            w.line("return false;");
            w.dedent();
            w.line("}");
            w.line("if (a.tag == 1) {");
            w.indent();
            emit_eq_check(
                w,
                names,
                elem,
                &format!("a.u.{}", OPTION_SOME_MEMBER),
                &format!("b.u.{}", OPTION_SOME_MEMBER),
            );
            w.dedent();
            w.line("}");
            w.line("return true;");
        }
        Ty::Result(ok, err) => {
            w.line("if (a.tag != b.tag) {");
            w.indent();
            w.line("return false;");
            w.dedent();
            w.line("}");
            w.line("if (a.tag == 0) {");
            w.indent();
            emit_eq_check(
                w,
                names,
                ok,
                &format!("a.u.{}", RESULT_OK_MEMBER),
                &format!("b.u.{}", RESULT_OK_MEMBER),
            );
            w.dedent();
            w.line("} else {");
            w.indent();
            emit_eq_check(
                w,
                names,
                err,
                &format!("a.u.{}", RESULT_ERR_MEMBER),
                &format!("b.u.{}", RESULT_ERR_MEMBER),
            );
            w.dedent();
            w.line("}");
            w.line("return true;");
        }
        Ty::Tuple(elems) => {
            for (index, elem) in elems.iter().enumerate() {
                let a = format!("a.{}", positional_field(index));
                let b = format!("b.{}", positional_field(index));
                emit_eq_check(w, names, elem, &a, &b);
            }
            w.line("return true;");
        }
        Ty::Struct(index) => {
            if let Some((_, fields)) = struct_item(program, *index) {
                for (field_name, field_ty) in &fields {
                    let a = format!("a.{}", field_cname(field_name));
                    let b = format!("b.{}", field_cname(field_name));
                    emit_eq_check(w, names, field_ty, &a, &b);
                }
            }
            w.line("return true;");
        }
        Ty::Enum(index) => {
            w.line("if (a.tag != b.tag) {");
            w.indent();
            w.line("return false;");
            w.dedent();
            w.line("}");
            if let Some(item) = enum_item(program, *index) {
                w.line("switch (a.tag) {");
                w.indent();
                for variant in &item.variants {
                    if variant.fields.is_empty() {
                        continue;
                    }
                    let member = variant_member(&variant.name);
                    w.line(format!("case {}: {{", variant.tag));
                    w.indent();
                    for (field_index, field_ty) in variant.fields.iter().enumerate() {
                        let a = format!(
                            "a.{}.{}.{}",
                            UNION_FIELD,
                            member,
                            positional_field(field_index)
                        );
                        let b = format!(
                            "b.{}.{}.{}",
                            UNION_FIELD,
                            member,
                            positional_field(field_index)
                        );
                        emit_eq_check(w, names, field_ty, &a, &b);
                    }
                    w.line("break;");
                    w.dedent();
                    w.line("}");
                }
                w.dedent();
                w.line("}");
            }
            w.line("return true;");
        }
        Ty::Fn(_, _) => {
            w.line(format!(
                "return (a.{} == b.{}) && (a.{} != NULL) && (a.{} == b.{});",
                crate::codegen::types::FN_ENV_FIELD,
                crate::codegen::types::FN_ENV_FIELD,
                crate::codegen::types::FN_ENV_FIELD,
                crate::codegen::types::FN_PTR_FIELD,
                crate::codegen::types::FN_PTR_FIELD
            ));
        }
        _ => {
            w.line("(void)a;");
            w.line("(void)b;");
            w.line("return true;");
        }
    }
}

fn struct_item(program: &hir::Program, index: usize) -> Option<(String, Vec<(String, Ty)>)> {
    let item = program.structs.get(index)?;
    let fields = item
        .fields
        .iter()
        .map(|field| (field.name.clone(), field.ty.clone()))
        .collect();
    Some((item.name.clone(), fields))
}

fn enum_item(program: &hir::Program, index: usize) -> Option<&hir::HEnum> {
    program.enums.get(index)
}
