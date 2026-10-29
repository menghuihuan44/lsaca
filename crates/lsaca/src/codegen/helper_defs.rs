//! Definitions of the generated per-type helpers. The prototypes and the
//! environment support live in the parent module.

use crate::codegen::c_writer::CWriter;
use crate::codegen::helpers::{compound_types, managed_types};
use crate::codegen::ops;
use crate::codegen::types::{
    EMPTY_STRUCT_FIELD, FN_ENV_FIELD, OPTION_SOME_MEMBER, RESULT_ERR_MEMBER, RESULT_OK_MEMBER,
    TypeNames, TypeRegistry, UNION_FIELD, field_cname, positional_field, variant_member,
};
use crate::codegen::util;
use crate::hir;
use crate::types::Ty;

pub fn emit_all(w: &mut CWriter, program: &hir::Program, registry: &TypeRegistry) {
    let names = registry.names().clone();
    for ty in managed_types(program, registry) {
        if ty.is_hash_map() {
            // The map family is emitted below, after every helper its key and
            // value callbacks can call.
            continue;
        }
        emit_copy(w, program, &names, &ty);
        emit_release(w, program, &names, &ty);
    }
    for ty in compound_types(program, registry) {
        if ty.is_hash_map() {
            continue;
        }
        if let Ty::Vec(element) = &ty
            && !ops::is_frozen_shared(&ty)
        {
            emit_push_slow(w, &names, &ty, element);
        }
    }
    for ty in compound_types(program, registry) {
        if ty.is_hash_map() {
            continue;
        }
        emit_debug(w, program, &names, &ty);
        emit_eq(w, program, &names, &ty);
    }
    for ty in compound_types(program, registry) {
        if ty.is_hash_map() || matches!(ty, Ty::Rc(_) | Ty::Box(_)) {
            continue;
        }
        emit_trace(w, program, &names, &ty);
    }
    for ty in managed_types(program, registry) {
        match ty {
            Ty::Rc(inner) => emit_rc_helpers(w, program, &names, &inner),
            Ty::Box(inner) => emit_box_helpers(w, program, &names, &inner),
            _ => {}
        }
    }
    // The map family is emitted last: its callbacks call the copy, release,
    // debug and equality helpers of the key and value types emitted above.
    for ty in compound_types(program, registry) {
        if let Ty::HashMap(key, value) = &ty {
            emit_map_helpers(w, &names, &ty, key, value);
        }
    }
}

/// Out-of-line growing append used by `Vec::push`. Taking and returning the
/// vector by value keeps the vector's address from escaping in the emitted fast
/// path, so the compiler can keep `len` and `cap` in registers inside push
/// loops instead of reloading them from the stack on every iteration.
fn emit_push_slow(w: &mut CWriter, names: &TypeNames, ty: &Ty, element: &Ty) {
    let c_ty = names.ctype(ty);
    let element_ty = names.ctype(element);
    w.line(format!(
        "{} {}({} v, {} value) {{",
        c_ty,
        ops::push_slow_fn_name(names, ty),
        c_ty,
        element_ty
    ));
    w.indent();
    w.line(format!(
        "ls_vec_insert_bytes((void**)&v.data, &v.len, &v.cap, v.len, (const void*)&value, sizeof({}));",
        element_ty
    ));
    w.line("return v;");
    w.dedent();
    w.line("}");
    w.blank();
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
            if !program.is_copy(elem) {
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
                emit_assign_copy(w, program, names, elem, &target, &value);
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
                        emit_assign_copy(w, program, names, field_ty, &target, &value);
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
                        emit_assign_copy(w, program, names, field_ty, &target, &value);
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
        Ty::Rc(_) => {
            w.line(format!("return ({})ls_retain(v);", c_ty));
        }
        Ty::Box(inner) => {
            // Copying a box deep-copies the payload: boxes own their value.
            w.line(format!(
                "return lsaca_box_new_{}({});",
                names.mangle(inner),
                ops::copy_expr(names, inner, "v->payload")
            ));
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

fn emit_assign_copy(
    w: &mut CWriter,
    program: &hir::Program,
    names: &TypeNames,
    ty: &Ty,
    target: &str,
    value: &str,
) {
    if program.is_copy(ty) {
        w.line(format!("{} = {};", target, value));
    } else {
        w.line(format!(
            "{} = {};",
            target,
            ops::copy_expr(names, ty, value)
        ));
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
            if !program.is_copy(elem) {
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
            emit_release_value(
                w,
                program,
                names,
                elem,
                &format!("v.u.{}", OPTION_SOME_MEMBER),
            );
            w.dedent();
            w.line("}");
        }
        Ty::Result(ok, err) => {
            w.line("if (v.tag == 0) {");
            w.indent();
            emit_release_value(w, program, names, ok, &format!("v.u.{}", RESULT_OK_MEMBER));
            w.dedent();
            w.line("} else {");
            w.indent();
            emit_release_value(
                w,
                program,
                names,
                err,
                &format!("v.u.{}", RESULT_ERR_MEMBER),
            );
            w.dedent();
            w.line("}");
        }
        Ty::Tuple(elems) => {
            for (index, elem) in elems.iter().enumerate() {
                emit_release_value(
                    w,
                    program,
                    names,
                    elem,
                    &format!("v.{}", positional_field(index)),
                );
            }
            w.line("(void)v;");
        }
        Ty::Struct(index) => {
            // The user Drop hook runs before the fields are released, exactly
            // like a language-level drop at scope exit.
            if let Some(hook) = program.drop_hook(ty) {
                w.line(format!("{}(&v);", program.fns[hook].c_name));
            }
            if let Some((_, fields)) = struct_item(program, *index) {
                for (field_name, field_ty) in &fields {
                    emit_release_value(
                        w,
                        program,
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
                // The user Drop hook runs before the active payload variant is
                // released.
                if let Some(hook) = program.drop_hook(ty) {
                    w.line(format!("{}(&v);", program.fns[hook].c_name));
                }
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
                            program,
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
        Ty::Rc(_) | Ty::Box(_) => {
            w.line("ls_release(v);");
        }
        _ => {
            w.line("(void)v;");
        }
    }
    w.dedent();
    w.line("}");
    w.blank();
}

fn emit_release_value(
    w: &mut CWriter,
    program: &hir::Program,
    names: &TypeNames,
    ty: &Ty,
    value: &str,
) {
    if program.is_copy(ty) {
        return;
    }
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
        Ty::Rc(inner) => {
            push_cstr(w, "Rc(");
            emit_debug_append(w, names, inner, "v->payload");
            push_cstr(w, ")");
        }
        Ty::Box(inner) => {
            // Like Rust, formatting a box formats the value inside it.
            emit_debug_append(w, names, inner, "v->payload");
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
        Ty::Rc(_) => {
            w.line("return a == b;");
        }
        Ty::Box(inner) => {
            // Boxes compare by payload value, like the value they box.
            w.line(format!(
                "return {};",
                ops::eq_expr(names, inner, "a->payload", "b->payload")
            ));
        }
        _ => {
            w.line("(void)a;");
            w.line("(void)b;");
            w.line("return true;");
        }
    }
}

// ---------------------------------------------------------------------------
// tracing helpers for the cycle collector
// ---------------------------------------------------------------------------

/// True when values of this type can hold a reference to a managed allocation.
fn traceable(ty: &Ty) -> bool {
    match ty {
        Ty::Rc(_) | Ty::Box(_) | Ty::Fn(_, _) => true,
        // A reference borrows a place owned by someone else: it never owns an
        // allocation of its own, so the cycle collector must not follow it.
        Ty::Ref { .. } => false,
        Ty::Unit | Ty::Bool | Ty::Int(_) | Ty::Float(_) | Ty::String => false,
        _ => true,
    }
}

/// Emit the tracing of one value slot: visit every managed handle it contains
/// so the cycle collector can follow the ownership graph.
pub(crate) fn emit_trace_slot(w: &mut CWriter, names: &TypeNames, ty: &Ty, slot: &str) {
    match ty {
        Ty::Rc(_) | Ty::Box(_) | Ty::HashMap(_, _) => {
            w.line(format!("if ({} != NULL) {{", slot));
            w.indent();
            w.line(format!("visit((void*)({}));", slot));
            w.dedent();
            w.line("}");
        }
        Ty::Fn(_, _) => {
            w.line(format!("if ({}.{} != NULL) {{", slot, FN_ENV_FIELD));
            w.indent();
            w.line(format!("visit({}.{});", slot, FN_ENV_FIELD));
            w.dedent();
            w.line("}");
        }
        // Borrowed references do not participate in cycle tracing.
        Ty::Ref { .. } => {}
        Ty::Unit | Ty::Bool | Ty::Int(_) | Ty::Float(_) | Ty::String => {}
        _ => {
            w.line(format!(
                "lsaca_trace_{}(&{}, visit);",
                names.mangle(ty),
                slot
            ));
        }
    }
}

/// Emit the tracing function of one compound type. The collector calls it for
/// every managed allocation to discover the handles it owns.
fn emit_trace(w: &mut CWriter, program: &hir::Program, names: &TypeNames, ty: &Ty) {
    let c_ty = names.ctype(ty);
    let suffix = names.mangle(ty);
    w.line(format!(
        "void lsaca_trace_{}(const {}* value, LsTraceVisit visit) {{",
        suffix, c_ty
    ));
    w.indent();
    w.line("(void)value;");
    match ty {
        Ty::Vec(elem) => {
            if traceable(elem) {
                w.line("size_t i;");
                w.line("for (i = 0; i < value->len; i += 1) {");
                w.indent();
                emit_trace_slot(w, names, elem, "value->data[i]");
                w.dedent();
                w.line("}");
            }
        }
        Ty::Option(elem) => {
            if traceable(elem) {
                w.line("if (value->tag == 1) {");
                w.indent();
                emit_trace_slot(
                    w,
                    names,
                    elem,
                    &format!("value->{}.{}", UNION_FIELD, OPTION_SOME_MEMBER),
                );
                w.dedent();
                w.line("}");
            }
        }
        Ty::Result(ok, err) => {
            if traceable(ok) || traceable(err) {
                w.line("if (value->tag == 0) {");
                w.indent();
                emit_trace_slot(
                    w,
                    names,
                    ok,
                    &format!("value->{}.{}", UNION_FIELD, RESULT_OK_MEMBER),
                );
                w.dedent();
                w.line("} else {");
                w.indent();
                emit_trace_slot(
                    w,
                    names,
                    err,
                    &format!("value->{}.{}", UNION_FIELD, RESULT_ERR_MEMBER),
                );
                w.dedent();
                w.line("}");
            }
        }
        Ty::Tuple(elems) => {
            for (index, elem) in elems.iter().enumerate() {
                if traceable(elem) {
                    emit_trace_slot(
                        w,
                        names,
                        elem,
                        &format!("value->{}", positional_field(index)),
                    );
                }
            }
        }
        Ty::Struct(index) => {
            if let Some((_, fields)) = struct_item(program, *index) {
                for (name, field_ty) in &fields {
                    if traceable(field_ty) {
                        emit_trace_slot(
                            w,
                            names,
                            field_ty,
                            &format!("value->{}", field_cname(name)),
                        );
                    }
                }
            }
        }
        Ty::Enum(index) => {
            if let Some(item) = enum_item(program, *index) {
                let variants: Vec<_> = item
                    .variants
                    .iter()
                    .filter(|variant| variant.fields.iter().any(traceable))
                    .collect();
                if !variants.is_empty() {
                    w.line("switch (value->tag) {");
                    w.indent();
                    for variant in variants {
                        w.line(format!("case {}: {{", variant.tag));
                        w.indent();
                        let member = variant_member(&variant.name);
                        for (field_index, field_ty) in variant.fields.iter().enumerate() {
                            if traceable(field_ty) {
                                emit_trace_slot(
                                    w,
                                    names,
                                    field_ty,
                                    &format!(
                                        "value->{}.{}.{}",
                                        UNION_FIELD,
                                        member,
                                        positional_field(field_index)
                                    ),
                                );
                            }
                        }
                        w.line("break;");
                        w.dedent();
                        w.line("}");
                    }
                    w.dedent();
                    w.line("}");
                }
            }
        }
        Ty::Fn(_, _) => {
            w.line(format!("if (value->{} != NULL) {{", FN_ENV_FIELD));
            w.indent();
            w.line(format!("visit(value->{});", FN_ENV_FIELD));
            w.dedent();
            w.line("}");
        }
        _ => {}
    }
    w.line("(void)visit;");
    w.dedent();
    w.line("}");
    w.blank();
}

/// Emit the helpers of one Rc cell type: allocation, payload access, release and
/// tracing. The cell itself is a managed allocation registered with the cycle
/// collector, so a cycle through its payload is collectable.
fn emit_rc_helpers(w: &mut CWriter, program: &hir::Program, names: &TypeNames, inner: &Ty) {
    let suffix = names.mangle(inner);
    let cell = names.tag(&Ty::rc(inner.clone()));
    let cell_ptr = format!("{}*", cell);
    let inner_ty = names.ctype(inner);

    w.line(format!(
        "{} lsaca_rc_new_{}({} value) {{",
        cell_ptr, suffix, inner_ty
    ));
    w.indent();
    w.line(format!(
        "{} self = ({})ls_alloc_managed(sizeof({}), lsaca_rc_drop_{}, lsaca_rc_trace_{});",
        cell_ptr, cell_ptr, cell, suffix, suffix
    ));
    w.line("self->payload = value;");
    w.line("return self;");
    w.dedent();
    w.line("}");
    w.blank();

    w.line(format!(
        "{} lsaca_rc_get_{}(const {}* self) {{",
        inner_ty, suffix, cell
    ));
    w.indent();
    w.line(format!(
        "return {};",
        ops::copy_expr(names, inner, "self->payload")
    ));
    w.dedent();
    w.line("}");
    w.blank();

    w.line(format!(
        "void lsaca_rc_set_{}({} self, {} value) {{",
        suffix, cell_ptr, inner_ty
    ));
    w.indent();
    emit_release_value(w, program, names, inner, "self->payload");
    w.line("self->payload = value;");
    w.dedent();
    w.line("}");
    w.blank();

    w.line(format!("void lsaca_rc_drop_{}(void* payload) {{", suffix));
    w.indent();
    w.line(format!("{} self = ({})payload;", cell_ptr, cell_ptr));
    emit_release_value(w, program, names, inner, "self->payload");
    w.line("(void)self;");
    w.dedent();
    w.line("}");
    w.blank();

    w.line(format!(
        "void lsaca_rc_trace_{}(const void* payload, LsTraceVisit visit) {{",
        suffix
    ));
    w.indent();
    w.line(format!("const {}* self = (const {}*)payload;", cell, cell));
    w.line("(void)self;");
    emit_trace_slot(w, names, inner, "self->payload");
    w.line("(void)visit;");
    w.dedent();
    w.line("}");
    w.blank();
}

/// Emit the helpers of one Box cell type: allocation, payload access, deep
/// copy and tracing. Unlike an Rc cell, a box is uniquely owned, so copying a
/// box allocates a fresh cell and deep-copies the payload.
fn emit_box_helpers(w: &mut CWriter, program: &hir::Program, names: &TypeNames, inner: &Ty) {
    let suffix = names.mangle(inner);
    let cell = names.tag(&Ty::box_(inner.clone()));
    let cell_ptr = format!("{}*", cell);
    let inner_ty = names.ctype(inner);

    w.line(format!(
        "{} lsaca_box_new_{}({} value) {{",
        cell_ptr, suffix, inner_ty
    ));
    w.indent();
    w.line(format!(
        "{} self = ({})ls_alloc_managed(sizeof({}), lsaca_box_drop_{}, lsaca_box_trace_{});",
        cell_ptr, cell_ptr, cell, suffix, suffix
    ));
    w.line("self->payload = value;");
    w.line("return self;");
    w.dedent();
    w.line("}");
    w.blank();

    w.line(format!(
        "{} lsaca_box_get_{}(const {}* self) {{",
        inner_ty, suffix, cell
    ));
    w.indent();
    w.line(format!(
        "return {};",
        ops::copy_expr(names, inner, "self->payload")
    ));
    w.dedent();
    w.line("}");
    w.blank();

    w.line(format!(
        "void lsaca_box_set_{}({} self, {} value) {{",
        suffix, cell_ptr, inner_ty
    ));
    w.indent();
    emit_release_value(w, program, names, inner, "self->payload");
    w.line("self->payload = value;");
    w.dedent();
    w.line("}");
    w.blank();

    w.line(format!("void lsaca_box_drop_{}(void* payload) {{", suffix));
    w.indent();
    w.line(format!("{} self = ({})payload;", cell_ptr, cell_ptr));
    emit_release_value(w, program, names, inner, "self->payload");
    w.line("(void)self;");
    w.dedent();
    w.line("}");
    w.blank();

    w.line(format!(
        "void lsaca_box_trace_{}(const void* payload, LsTraceVisit visit) {{",
        suffix
    ));
    w.indent();
    w.line(format!("const {}* self = (const {}*)payload;", cell, cell));
    w.line("(void)self;");
    emit_trace_slot(w, names, inner, "self->payload");
    w.line("(void)visit;");
    w.dedent();
    w.line("}");
    w.blank();
}

// ---------------------------------------------------------------------------
// HashMap helpers
// ---------------------------------------------------------------------------

/// Emit the four key callbacks a map table needs: copy, drop, hash and
/// equality. Keys are restricted to scalars and strings, so the callbacks are
/// typed operations on the slot bytes.
fn emit_map_key_callbacks(w: &mut CWriter, names: &TypeNames, key: &Ty, suffix: &str) {
    let key_ty = names.ctype(key);

    w.line(format!(
        "static void lsaca_map_copy_key_{}(void* dst, const void* src) {{",
        suffix
    ));
    w.indent();
    if key.is_string() {
        w.line("*(LsString**)dst = ((LsString*)ls_retain((void*)(*(LsString* const*)src)));");
    } else if key.is_integer() || key.is_bool() {
        w.line(format!("*({}*)dst = *(const {}*)src;", key_ty, key_ty));
    } else {
        let value = ops::copy_expr(names, key, &format!("*(const {}*)src", key_ty));
        w.line(format!("*({}*)dst = {};", key_ty, value));
    }
    w.dedent();
    w.line("}");
    w.blank();

    w.line(format!(
        "static void lsaca_map_drop_key_{}(void* slot) {{",
        suffix
    ));
    w.indent();
    if key.is_string() {
        w.line("ls_release((void*)(*(LsString**)slot));");
    } else if key.is_integer() || key.is_bool() {
        w.line("(void)slot;");
    } else if let Some(stmt) = ops::release_stmt(names, key, &format!("*({}*)slot", key_ty)) {
        w.line(stmt);
    } else {
        w.line("(void)slot;");
    }
    w.dedent();
    w.line("}");
    w.blank();

    w.line(format!(
        "static uint64_t lsaca_map_hash_key_{}(const void* slot) {{",
        suffix
    ));
    w.indent();
    match key {
        Ty::Int(int) if int.is_signed() => w.line("return ls_hash_i64(slot);"),
        Ty::Int(_) => w.line("return ls_hash_u64(slot);"),
        Ty::Bool => w.line("return ls_hash_bool(slot);"),
        Ty::String => w.line("return ls_hash_string_slot(slot);"),
        _ => w.line(format!("return ls_hash_bytes(slot, sizeof({}));", key_ty)),
    }
    w.dedent();
    w.line("}");
    w.blank();

    w.line(format!(
        "static bool lsaca_map_eq_key_{}(const void* a, const void* b) {{",
        suffix
    ));
    w.indent();
    match key {
        Ty::Int(int) if int.is_signed() => w.line("return ls_eq_i64(a, b);"),
        Ty::Int(_) => w.line("return ls_eq_u64(a, b);"),
        Ty::Bool => w.line("return ls_eq_bool(a, b);"),
        Ty::String => w.line("return ls_eq_string_slot(a, b);"),
        _ => {
            let equal = ops::eq_expr(
                names,
                key,
                &format!("*(const {}*)a", key_ty),
                &format!("*(const {}*)b", key_ty),
            );
            w.line(format!("return {};", equal));
        }
    }
    w.dedent();
    w.line("}");
    w.blank();
}

/// Emit the two value callbacks a map table needs: copy and drop. Values may be
/// any type, so these delegate to the generated copy and release helpers.
fn emit_map_value_callbacks(w: &mut CWriter, names: &TypeNames, value: &Ty, suffix: &str) {
    let value_ty = names.ctype(value);

    w.line(format!(
        "static void lsaca_map_copy_value_{}(void* dst, const void* src) {{",
        suffix
    ));
    w.indent();
    let source = if value.is_string() {
        String::from("(*(LsString* const*)src)")
    } else {
        format!("*(const {}*)src", value_ty)
    };
    let copied = ops::copy_expr(names, value, &source);
    w.line(format!("*({}*)dst = {};", value_ty, copied));
    w.dedent();
    w.line("}");
    w.blank();

    w.line(format!(
        "static void lsaca_map_drop_value_{}(void* slot) {{",
        suffix
    ));
    w.indent();
    let slot_value = if value.is_string() {
        String::from("(*(LsString**)slot)")
    } else {
        format!("*({}*)slot", value_ty)
    };
    if let Some(stmt) = ops::release_stmt(names, value, &slot_value) {
        w.line(stmt);
    } else {
        w.line("(void)slot;");
    }
    w.dedent();
    w.line("}");
    w.blank();
}

/// Emit the complete helper family of one HashMap instantiation. The wrapper is
/// a managed allocation holding the byte table, so the cycle collector can
/// reclaim maps that take part in a cycle. Key and value types are handled
/// through the callbacks emitted above.
fn emit_map_helpers(w: &mut CWriter, names: &TypeNames, ty: &Ty, key: &Ty, value: &Ty) {
    let Some(suffix) = names.map_suffix(ty) else {
        return;
    };
    let cell = names.tag(ty);
    let cell_ptr = format!("{}*", cell);
    let key_ty = names.ctype(key);
    let value_ty = names.ctype(value);
    let key_vec_ty = names.ctype(&Ty::vec(key.clone()));
    let value_vec_ty = names.ctype(&Ty::vec(value.clone()));
    let copy_key = format!("lsaca_map_copy_key_{}", suffix);
    let drop_key = format!("lsaca_map_drop_key_{}", suffix);
    let hash_key = format!("lsaca_map_hash_key_{}", suffix);
    let eq_key = format!("lsaca_map_eq_key_{}", suffix);
    let copy_value = format!("lsaca_map_copy_value_{}", suffix);
    let drop_value = format!("lsaca_map_drop_value_{}", suffix);

    emit_map_key_callbacks(w, names, key, &suffix);
    emit_map_value_callbacks(w, names, value, &suffix);

    // Managed wrapper allocation: the table itself only allocates on insert.
    w.line(format!("{} lsaca_map_new_{}(void) {{", cell_ptr, suffix));
    w.indent();
    w.line(format!(
        "{} self = ({})ls_alloc_managed(sizeof({}), lsaca_map_drop_{}, lsaca_map_trace_{});",
        cell_ptr, cell_ptr, cell, suffix, suffix
    ));
    w.line(format!(
        "ls_map_init(&self->table, sizeof({}), sizeof({}), {}, {}, {}, {}, {}, {});",
        key_ty, value_ty, hash_key, eq_key, copy_key, drop_key, copy_value, drop_value
    ));
    w.line("return self;");
    w.dedent();
    w.line("}");
    w.blank();

    // Release all entries, then the slot arrays. The arrays are NULL until the
    // first insert, and free(NULL) is well defined.
    w.line(format!("void lsaca_map_drop_{}(void* payload) {{", suffix));
    w.indent();
    w.line(format!("{} self = ({})payload;", cell_ptr, cell_ptr));
    w.line("ls_map_clear(&self->table);");
    w.line("free(self->table.keys);");
    w.line("free(self->table.values);");
    w.line("free(self->table.states);");
    w.dedent();
    w.line("}");
    w.blank();

    // Keys are scalars or strings and can never hold a cycle; only values are
    // traced.
    w.line(format!(
        "void lsaca_map_trace_{}(const void* payload, LsTraceVisit visit) {{",
        suffix
    ));
    w.indent();
    w.line(format!("const {}* self = (const {}*)payload;", cell, cell));
    if traceable(value) {
        w.line("LsMapIter iter;");
        w.line("const void* key_slot;");
        w.line("const void* value_slot;");
        w.line("(void)self;");
        w.line("ls_map_iter_init(&iter, &self->table);");
        w.line("while (ls_map_iter_next(&iter, &key_slot, &value_slot)) {");
        w.indent();
        emit_trace_slot(w, names, value, &format!("(*({}*)value_slot)", value_ty));
        w.dedent();
        w.line("}");
    } else {
        w.line("(void)self;");
    }
    w.line("(void)visit;");
    w.dedent();
    w.line("}");
    w.blank();

    // Deep copy: the table insert adopts the key and value blobs, so the local
    // copies are not released afterwards.
    w.line(format!(
        "{} {}(const {} other) {{",
        cell_ptr,
        ops::copy_fn_name(names, ty),
        cell_ptr
    ));
    w.indent();
    w.line(format!("{} out = lsaca_map_new_{}();", cell_ptr, suffix));
    w.line("LsMapIter iter;");
    w.line("const void* key_slot;");
    w.line("const void* value_slot;");
    w.line("ls_map_iter_init(&iter, &other->table);");
    w.line("while (ls_map_iter_next(&iter, &key_slot, &value_slot)) {");
    w.indent();
    w.line(format!("{} key;", key_ty));
    w.line(format!("{} value;", value_ty));
    w.line(format!("{}(&key, key_slot);", copy_key));
    w.line(format!("{}(&value, value_slot);", copy_value));
    w.line("(void)ls_map_insert(&out->table, &key, &value, NULL);");
    w.dedent();
    w.line("}");
    w.line("return out;");
    w.dedent();
    w.line("}");
    w.blank();

    w.line(format!(
        "bool lsaca_map_insert_{}({} self, {} key, {} value, {}* out_old) {{",
        suffix, cell_ptr, key_ty, value_ty, value_ty
    ));
    w.indent();
    w.line("return ls_map_insert(&self->table, (const void*)&key, (const void*)&value, (void*)out_old);");
    w.dedent();
    w.line("}");
    w.blank();

    w.line(format!(
        "bool lsaca_map_get_{}({} self, {} key, {}* out) {{",
        suffix, cell_ptr, key_ty, value_ty
    ));
    w.indent();
    w.line("void* slot = NULL;");
    w.line("if (!ls_map_get_ptr(&self->table, (const void*)&key, &slot)) {");
    w.indent();
    w.line("return false;");
    w.dedent();
    w.line("}");
    let fetched = ops::copy_expr(names, value, &format!("*(const {}*)slot", value_ty));
    w.line(format!("*out = {};", fetched));
    w.line("return true;");
    w.dedent();
    w.line("}");
    w.blank();

    w.line(format!(
        "bool lsaca_map_get_ptr_{}({} self, {} key, {}** out) {{",
        suffix, cell_ptr, key_ty, value_ty
    ));
    w.indent();
    w.line("return ls_map_get_ptr(&self->table, (const void*)&key, (void**)out);");
    w.dedent();
    w.line("}");
    w.blank();

    w.line(format!(
        "bool lsaca_map_contains_{}({} self, {} key) {{",
        suffix, cell_ptr, key_ty
    ));
    w.indent();
    w.line("void* slot = NULL;");
    w.line("return ls_map_get_ptr(&self->table, (const void*)&key, &slot);");
    w.dedent();
    w.line("}");
    w.blank();

    w.line(format!(
        "bool lsaca_map_remove_{}({} self, {} key, {}* out) {{",
        suffix, cell_ptr, key_ty, value_ty
    ));
    w.indent();
    w.line("return ls_map_remove(&self->table, (const void*)&key, (void*)out);");
    w.dedent();
    w.line("}");
    w.blank();

    w.line(format!(
        "int64_t lsaca_map_len_{}({} self) {{",
        suffix, cell_ptr
    ));
    w.indent();
    w.line("return (int64_t)ls_map_len(&self->table);");
    w.dedent();
    w.line("}");
    w.blank();

    w.line(format!(
        "bool lsaca_map_is_empty_{}({} self) {{",
        suffix, cell_ptr
    ));
    w.indent();
    w.line("return ls_map_is_empty(&self->table);");
    w.dedent();
    w.line("}");
    w.blank();

    w.line(format!(
        "void lsaca_map_clear_{}({} self) {{",
        suffix, cell_ptr
    ));
    w.indent();
    w.line("ls_map_clear(&self->table);");
    w.dedent();
    w.line("}");
    w.blank();

    w.line(format!(
        "{}* lsaca_map_keys_{}({} self) {{",
        key_vec_ty, suffix, cell_ptr
    ));
    w.indent();
    w.line(format!(
        "{}* out = ({}*)ls_alloc(sizeof({}));",
        key_vec_ty, key_vec_ty, key_vec_ty
    ));
    w.line("LsMapIter iter;");
    w.line("const void* key_slot;");
    w.line("const void* value_slot;");
    w.line("size_t index = 0;");
    w.line("out->len = ls_map_len(&self->table);");
    w.line("out->cap = out->len;");
    w.line(format!(
        "out->data = ({}*)ls_vec_alloc(out->len, sizeof({}));",
        key_ty, key_ty
    ));
    w.line("ls_map_iter_init(&iter, &self->table);");
    w.line("while (ls_map_iter_next(&iter, &key_slot, &value_slot)) {");
    w.indent();
    w.line(format!("{}(&out->data[index], key_slot);", copy_key));
    w.line("index += 1;");
    w.dedent();
    w.line("}");
    w.line("return out;");
    w.dedent();
    w.line("}");
    w.blank();

    w.line(format!(
        "{}* lsaca_map_values_{}({} self) {{",
        value_vec_ty, suffix, cell_ptr
    ));
    w.indent();
    w.line(format!(
        "{}* out = ({}*)ls_alloc(sizeof({}));",
        value_vec_ty, value_vec_ty, value_vec_ty
    ));
    w.line("LsMapIter iter;");
    w.line("const void* key_slot;");
    w.line("const void* value_slot;");
    w.line("size_t index = 0;");
    w.line("out->len = ls_map_len(&self->table);");
    w.line("out->cap = out->len;");
    w.line(format!(
        "out->data = ({}*)ls_vec_alloc(out->len, sizeof({}));",
        value_ty, value_ty
    ));
    w.line("ls_map_iter_init(&iter, &self->table);");
    w.line("while (ls_map_iter_next(&iter, &key_slot, &value_slot)) {");
    w.indent();
    w.line(format!("{}(&out->data[index], value_slot);", copy_value));
    w.line("index += 1;");
    w.dedent();
    w.line("}");
    w.line("return out;");
    w.dedent();
    w.line("}");
    w.blank();

    // The debug helper appends the rendered map through its out parameter, in
    // the statement form used by the other generated debug helpers (DESIGN
    // 5.5). The out parameter points at the accumulator handle.
    w.line(format!(
        "void lsaca_map_debug_{}(const {}* self, LsString** out) {{",
        suffix, cell
    ));
    w.indent();
    w.line("LsMapIter iter;");
    w.line("const void* key_slot;");
    w.line("const void* value_slot;");
    w.line("size_t index = 0;");
    w.line(format!(
        "*out = ls_format_push_cstr(*out, {});",
        util::c_string_literal("{")
    ));
    w.line("ls_map_iter_init(&iter, &self->table);");
    w.line("while (ls_map_iter_next(&iter, &key_slot, &value_slot)) {");
    w.indent();
    w.line("if (index > 0) {");
    w.indent();
    w.line(format!(
        "*out = ls_format_push_cstr(*out, {});",
        util::c_string_literal(", ")
    ));
    w.dedent();
    w.line("}");
    let key_expr = if key.is_string() {
        String::from("(*(LsString* const*)key_slot)")
    } else {
        format!("(*(const {}*)key_slot)", key_ty)
    };
    if let Some(stmt) = ops::debug_append_statement(names, key, "*out", &key_expr) {
        w.line(stmt);
    }
    w.line(format!(
        "*out = ls_format_push_cstr(*out, {});",
        util::c_string_literal(": ")
    ));
    let value_expr = if value.is_string() {
        String::from("(*(LsString* const*)value_slot)")
    } else {
        format!("(*(const {}*)value_slot)", value_ty)
    };
    if let Some(stmt) = ops::debug_append_statement(names, value, "*out", &value_expr) {
        w.line(stmt);
    }
    w.line("index += 1;");
    w.dedent();
    w.line("}");
    w.line(format!(
        "*out = ls_format_push_cstr(*out, {});",
        util::c_string_literal("}")
    ));
    w.dedent();
    w.line("}");
    w.blank();

    w.line(format!(
        "bool lsaca_map_eq_{}(const {}* a, const {}* b) {{",
        suffix, cell, cell
    ));
    w.indent();
    w.line("LsMapIter iter;");
    w.line("const void* key_slot;");
    w.line("const void* value_slot;");
    w.line("void* other_slot = NULL;");
    w.line("if (a == b) {");
    w.indent();
    w.line("return true;");
    w.dedent();
    w.line("}");
    w.line("if (ls_map_len(&a->table) != ls_map_len(&b->table)) {");
    w.indent();
    w.line("return false;");
    w.dedent();
    w.line("}");
    w.line("ls_map_iter_init(&iter, &a->table);");
    w.line("while (ls_map_iter_next(&iter, &key_slot, &value_slot)) {");
    w.indent();
    w.line("if (!ls_map_get_ptr(&b->table, key_slot, &other_slot)) {");
    w.indent();
    w.line("return false;");
    w.dedent();
    w.line("}");
    let equal = ops::eq_expr(
        names,
        value,
        &format!("(*(const {}*)value_slot)", value_ty),
        &format!("(*(const {}*)other_slot)", value_ty),
    );
    w.line(format!("if (!({})) {{", equal));
    w.indent();
    w.line("return false;");
    w.dedent();
    w.line("}");
    w.dedent();
    w.line("}");
    w.line("return true;");
    w.dedent();
    w.line("}");
    w.blank();
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
