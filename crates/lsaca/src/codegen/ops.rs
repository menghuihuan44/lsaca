//! Value operations shared by the helper emitter and the expression generator:
//! copying, releasing, comparing and debug formatting values according to the
//! ORC ownership model.

use crate::codegen::types::TypeNames;
use crate::codegen::util;
use crate::types::Ty;

/// True when values of this type own heap resources (and therefore need
/// copy/release operations).
pub fn is_managed(ty: &Ty) -> bool {
    !ty.is_bitwise_copy()
}

/// True when the type is the runtime provided frozen instantiation
/// LsVec_String, LsOpt_String or LsRes_String_String.
pub fn is_frozen_shared(ty: &Ty) -> bool {
    match ty {
        Ty::Vec(elem) => **elem == Ty::String,
        Ty::Option(elem) => **elem == Ty::String,
        Ty::Result(ok, err) => **ok == Ty::String && **err == Ty::String,
        _ => false,
    }
}

pub fn copy_fn_name(names: &TypeNames, ty: &Ty) -> String {
    format!("lsaca_copy_{}", names.mangle(ty))
}

pub fn release_fn_name(names: &TypeNames, ty: &Ty) -> String {
    format!("lsaca_release_{}", names.mangle(ty))
}

pub fn debug_fn_name(names: &TypeNames, ty: &Ty) -> String {
    format!("lsaca_debug_{}", names.mangle(ty))
}

pub fn eq_fn_name(names: &TypeNames, ty: &Ty) -> String {
    format!("lsaca_eq_{}", names.mangle(ty))
}

/// An owned copy of a borrowed value expression of the given type.
pub fn copy_expr(names: &TypeNames, ty: &Ty, expr: &str) -> String {
    if !is_managed(ty) {
        return expr.to_string();
    }
    if ty.is_string() {
        return format!("((LsString*)ls_retain({}))", expr);
    }
    if ty.is_rc() {
        return format!("(({})ls_retain({}))", names.ctype(ty), expr);
    }
    format!("{}({})", copy_fn_name(names, ty), expr)
}

/// A statement releasing the value held by an expression, if the type owns
/// resources. The expression is evaluated exactly once by the released call.
pub fn release_stmt(names: &TypeNames, ty: &Ty, expr: &str) -> Option<String> {
    if !is_managed(ty) {
        return None;
    }
    if ty.is_string() {
        return Some(format!("ls_release({});", expr));
    }
    if ty.is_rc() {
        return Some(format!("ls_release({});", expr));
    }
    Some(format!("{}({});", release_fn_name(names, ty), expr))
}

/// A C boolean expression comparing two values.
pub fn eq_expr(names: &TypeNames, ty: &Ty, a: &str, b: &str) -> String {
    if ty.is_unit() {
        String::from("true")
    } else if ty.is_string() {
        format!("ls_str_eq(({}), ({}))", a, b)
    } else if ty.is_bool() || ty.is_integer() || ty.is_float() || ty.is_rc() {
        // Scalars compare directly; Rc handles compare by identity (their C
        // representation is the cell pointer).
        format!("(({}) == ({}))", a, b)
    } else if let Ty::Box(inner) = ty {
        // Boxes compare by payload value, like the value they box.
        eq_expr(
            names,
            inner,
            &format!("({})->payload", a),
            &format!("({})->payload", b),
        )
    } else if ty.is_fn() {
        format!(
            "(({}.env == {}.env) && ({} != NULL) && ({}.fn_ptr == {}.fn_ptr))",
            a, b, a, a, b
        )
    } else {
        format!("{}(({}), ({}))", eq_fn_name(names, ty), a, b)
    }
}

/// The C macro/function used to append a value of this scalar kind to a
/// formatting accumulator. Returns None when the type is a compound type that
/// needs its generated debug helper.
pub fn debug_scalar_kind(ty: &Ty) -> Option<DebugScalar> {
    match ty {
        Ty::Unit => Some(DebugScalar::Unit),
        Ty::Bool => Some(DebugScalar::Bool),
        Ty::Int(int_ty) => {
            if int_ty.is_signed() {
                Some(DebugScalar::I64)
            } else {
                Some(DebugScalar::U64)
            }
        }
        Ty::Float(float_ty) => {
            if float_ty.name() == "f32" {
                Some(DebugScalar::F32)
            } else {
                Some(DebugScalar::F64)
            }
        }
        Ty::String => Some(DebugScalar::String),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugScalar {
    Unit,
    Bool,
    I64,
    U64,
    F32,
    F64,
    String,
}

/// A statement appending the debug representation of a scalar (or the macro
/// name for a scalar kind) to the formatting accumulator. Compound types are
/// handled by their generated debug helper instead.
pub fn debug_append_statement(
    names: &TypeNames,
    ty: &Ty,
    acc: &str,
    value: &str,
) -> Option<String> {
    if ty.is_unit() {
        return Some(format!(
            "{} = ls_format_push_cstr({}, {});",
            acc,
            acc,
            util::c_string_literal("()")
        ));
    }
    match debug_scalar_kind(ty) {
        Some(DebugScalar::Bool) => Some(format!(
            "{} = ls_fmt_bool({}, (bool)({}));",
            acc, acc, value
        )),
        Some(DebugScalar::I64) => Some(format!(
            "{} = ls_fmt_i64({}, (int64_t)({}));",
            acc, acc, value
        )),
        Some(DebugScalar::U64) => Some(format!(
            "{} = ls_fmt_u64({}, (uint64_t)({}));",
            acc, acc, value
        )),
        Some(DebugScalar::F32) => Some(format!(
            "{} = ls_fmt_f32_debug({}, (float)({}));",
            acc, acc, value
        )),
        Some(DebugScalar::F64) => Some(format!(
            "{} = ls_fmt_f64_debug({}, (double)({}));",
            acc, acc, value
        )),
        Some(DebugScalar::String) => Some(format!(
            "{} = ls_fmt_debug_string({}, ({}));",
            acc, acc, value
        )),
        Some(DebugScalar::Unit) => None,
        None => Some(format!(
            "{} = {}({}, ({}));",
            acc,
            debug_fn_name(names, ty),
            acc,
            value
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_value_operations() {
        let names = TypeNames::default();
        assert_eq!(copy_expr(&names, &Ty::i64(), "v0"), "v0");
        assert_eq!(
            copy_expr(&names, &Ty::string(), "v0"),
            "((LsString*)ls_retain(v0))"
        );
        assert_eq!(
            release_stmt(&names, &Ty::string(), "v0"),
            Some(String::from("ls_release(v0);"))
        );
        assert_eq!(release_stmt(&names, &Ty::i64(), "v0"), None);
        assert_eq!(
            copy_expr(&names, &Ty::vec(Ty::i64()), "v0"),
            "lsaca_copy_Vec_i64(v0)"
        );
        assert_eq!(
            copy_expr(&names, &Ty::rc(Ty::i64()), "v0"),
            "((LsRc_i64*)ls_retain(v0))"
        );
        assert_eq!(
            release_stmt(&names, &Ty::rc(Ty::i64()), "v0"),
            Some(String::from("ls_release(v0);"))
        );
        assert_eq!(
            eq_expr(&names, &Ty::rc(Ty::i64()), "a", "b"),
            "((a) == (b))"
        );
    }
}
