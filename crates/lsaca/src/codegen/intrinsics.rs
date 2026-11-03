//! Inline expansion of intrinsic functions.
//!
//! An `extern fn` declaration in the standard library lowers to a HIR function
//! with `intrinsic = true`: it has an empty body and never emits a C
//! definition. Instead every call site is expanded by the emitter table in this
//! module. The table reuses the existing `HBuiltin*` emitters so an intrinsic
//! and the corresponding builtin method/constructor/function produce exactly the
//! same C. A name without an emitter is a compiler internal error: it must never
//! turn into a call of a function that does not exist.

use super::emit::Gen;
use super::util;
use crate::hir::{HBuiltinAssoc, HBuiltinFn, HBuiltinMethod, HExpr, HExprKind, HFn};
use crate::span::Span;
use crate::types::Ty;

impl<'a> Gen<'a> {
    /// Expand a call of the intrinsic function `item` into `target`.
    ///
    /// The receiver is only present for language constructs like
    /// `value.clone()`; intrinsics declared by the standard library are free
    /// functions.
    pub(crate) fn gen_intrinsic_call(
        &mut self,
        item: &HFn,
        receiver: Option<&HExpr>,
        args: &[HExpr],
        ty: &Ty,
        target: &str,
        span: Span,
    ) {
        if receiver.is_some() {
            util::internal_error(format!(
                "intrinsic function `{}` cannot have a receiver",
                item.name
            ));
        }
        // Generic intrinsic instances carry their type arguments in the
        // HIR name (for example `__vec_push<i64>`): the emitter tables match
        // the bare declaration name, so strip everything from the first
        // opening angle bracket.
        let full_name = item.name.as_str();
        let name = full_name.split('<').next().unwrap_or(full_name).trim();
        // Constructors are not part of the builtin enum tables: they build an
        // Option/Result value directly from their single argument.
        match name {
            "__opt_some" | "__res_ok" | "__res_err" => {
                let inner = require_argument(name, args);
                let kind = match name {
                    "__opt_some" => HExprKind::OptionSome(Box::new(inner.clone())),
                    "__res_ok" => HExprKind::ResultOk(Box::new(inner.clone())),
                    _ => HExprKind::ResultErr(Box::new(inner.clone())),
                };
                let expr = HExpr::new(ty.clone(), kind, span);
                self.gen_into(&expr, target);
                return;
            }
            "__opt_none" => {
                let zero = self.zero_expr(ty);
                self.line(format!("{} = {};", target, zero));
                return;
            }
            "__clone" => {
                let value = require_argument(name, args);
                self.gen_builtin_method(value, HBuiltinMethod::Clone, &[], ty, target, span);
                return;
            }
            "__drop" => util::internal_error("intrinsic `__drop` has no emitter in phase A"),
            _ => {}
        }
        // Slice and str intrinsics operate on the by-value fat views and the
        // pointer-and-length pairs, so they are emitted before the legacy
        // builtin tables. They consume fat str values only: a legacy
        // read-only String method with the same name still routes through the
        // builtin method table below.
        if self.try_gen_slice_intrinsic(name, args, ty, target, span) {
            return;
        }
        // Phase C maps every operating-system intrinsic onto an existing
        // runtime function; the table is disjoint from the builtin names.
        if self.try_gen_phase_c_intrinsic(name, args, ty, target, span) {
            return;
        }
        if let Some(method) = method_for(name) {
            let value = require_argument(name, args);
            self.gen_builtin_method(value, method, &args[1..], ty, target, span);
            return;
        }
        if let Some(assoc) = assoc_for(name) {
            self.gen_builtin_assoc(assoc, args, &[], ty, target, span);
            return;
        }
        if let Some(function) = builtin_fn_for(name) {
            self.gen_builtin_call(function, args, ty, target, span);
            return;
        }
        util::internal_error(format!("no codegen emitter for intrinsic `{}`", full_name));
    }
}

/// The single argument of a constructor-like intrinsic.
fn require_argument<'e>(name: &str, args: &'e [HExpr]) -> &'e HExpr {
    match args.first() {
        Some(argument) => argument,
        None => util::internal_error(format!(
            "intrinsic `{}` is called without an argument",
            name
        )),
    }
}

/// Map an intrinsic name to a builtin method emitter.
///
/// Both the declared name (`__vec_len`) and the bare name (`vec_len`) are
/// accepted so the table keeps working if the standard library changes the
/// declaration spelling.
fn method_for(name: &str) -> Option<HBuiltinMethod> {
    let name = name.strip_prefix("__").unwrap_or(name);
    let method = match name {
        "vec_len" => HBuiltinMethod::VecLen,
        "vec_is_empty" => HBuiltinMethod::VecIsEmpty,
        "vec_push" => HBuiltinMethod::VecPush,
        "vec_pop" => HBuiltinMethod::VecPop,
        "vec_get" => HBuiltinMethod::VecGet,
        "vec_insert" => HBuiltinMethod::VecInsert,
        "vec_remove" => HBuiltinMethod::VecRemove,
        "vec_clear" => HBuiltinMethod::VecClear,
        "vec_first" => HBuiltinMethod::VecFirst,
        "vec_last" => HBuiltinMethod::VecLast,
        "vec_extend" => HBuiltinMethod::VecExtend,
        "vec_reverse" => HBuiltinMethod::VecReverse,
        "str_len" => HBuiltinMethod::StrLen,
        "str_is_empty" => HBuiltinMethod::StrIsEmpty,
        "str_push_str" => HBuiltinMethod::StrPushStr,
        "str_push_char" => HBuiltinMethod::StrPush,
        "str_pop" => HBuiltinMethod::StrPop,
        "str_clear" => HBuiltinMethod::StrClear,
        "str_contains" => HBuiltinMethod::StrContains,
        "str_starts_with" => HBuiltinMethod::StrStartsWith,
        "str_ends_with" => HBuiltinMethod::StrEndsWith,
        "str_find" => HBuiltinMethod::StrFind,
        "str_replace" => HBuiltinMethod::StrReplace,
        "str_to_uppercase" => HBuiltinMethod::StrToUppercase,
        "str_to_lowercase" => HBuiltinMethod::StrToLowercase,
        "str_trim" => HBuiltinMethod::StrTrim,
        "str_split" => HBuiltinMethod::StrSplit,
        "str_parse_i64" => HBuiltinMethod::StrParseI64,
        "str_parse_f64" => HBuiltinMethod::StrParseF64,
        "str_substring" => HBuiltinMethod::StrSubstring,
        "str_repeat" => HBuiltinMethod::StrRepeat,
        "str_char_count" => HBuiltinMethod::StrCharCount,
        "str_chars" => HBuiltinMethod::StrChars,
        "str_char_at" => HBuiltinMethod::StrCharAt,
        "str_is_char_boundary" => HBuiltinMethod::StrIsCharBoundary,
        "opt_is_some" => HBuiltinMethod::OptIsSome,
        "opt_is_none" => HBuiltinMethod::OptIsNone,
        "opt_unwrap" => HBuiltinMethod::OptUnwrap,
        "opt_unwrap_or" => HBuiltinMethod::OptUnwrapOr,
        "opt_expect" => HBuiltinMethod::OptExpect,
        "res_is_ok" => HBuiltinMethod::ResIsOk,
        "res_is_err" => HBuiltinMethod::ResIsErr,
        "res_unwrap" => HBuiltinMethod::ResUnwrap,
        "res_unwrap_or" => HBuiltinMethod::ResUnwrapOr,
        "res_expect" => HBuiltinMethod::ResExpect,
        "res_ok_value" => HBuiltinMethod::ResOk,
        "res_err_value" => HBuiltinMethod::ResErr,
        "rc_get" => HBuiltinMethod::RcGet,
        "rc_set" => HBuiltinMethod::RcSet,
        "rc_strong_count" => HBuiltinMethod::RcStrongCount,
        "box_get" => HBuiltinMethod::BoxGet,
        "box_set" => HBuiltinMethod::BoxSet,
        "map_insert" => HBuiltinMethod::MapInsert,
        "map_get" => HBuiltinMethod::MapGet,
        "map_get_with" => HBuiltinMethod::MapGetWith,
        "map_contains_key" => HBuiltinMethod::MapContainsKey,
        "map_remove" => HBuiltinMethod::MapRemove,
        "map_len" => HBuiltinMethod::MapLen,
        "map_is_empty" => HBuiltinMethod::MapIsEmpty,
        "map_clear" => HBuiltinMethod::MapClear,
        "map_keys" => HBuiltinMethod::MapKeys,
        "map_values" => HBuiltinMethod::MapValues,
        _ => return None,
    };
    Some(method)
}

/// Map an intrinsic name to a builtin constructor emitter.
fn assoc_for(name: &str) -> Option<HBuiltinAssoc> {
    let name = name.strip_prefix("__").unwrap_or(name);
    let assoc = match name {
        "vec_new" => HBuiltinAssoc::VecNew,
        "vec_with_capacity" => HBuiltinAssoc::VecWithCapacity,
        "str_new" => HBuiltinAssoc::StringNew,
        "str_from" => HBuiltinAssoc::StringFrom,
        "str_from_char" => HBuiltinAssoc::StringFromChar,
        "rc_new" => HBuiltinAssoc::RcNew,
        "box_new" => HBuiltinAssoc::BoxNew,
        "map_new" => HBuiltinAssoc::HashMapNew,
        _ => return None,
    };
    Some(assoc)
}

/// Map an intrinsic name to a free builtin function emitter.
fn builtin_fn_for(name: &str) -> Option<HBuiltinFn> {
    let name = name.strip_prefix("__").unwrap_or(name);
    let function = match name {
        "sqrt" => HBuiltinFn::Sqrt,
        "abs" => HBuiltinFn::Abs,
        "min" => HBuiltinFn::Min,
        "max" => HBuiltinFn::Max,
        "pow" => HBuiltinFn::Pow,
        "floor" => HBuiltinFn::Floor,
        "ceil" => HBuiltinFn::Ceil,
        "round" => HBuiltinFn::Round,
        "sin" => HBuiltinFn::Sin,
        "cos" => HBuiltinFn::Cos,
        "tan" => HBuiltinFn::Tan,
        "ln" => HBuiltinFn::Ln,
        "log10" => HBuiltinFn::Log10,
        "exp" => HBuiltinFn::Exp,
        "read_line" => HBuiltinFn::ReadLine,
        "args" => HBuiltinFn::Args,
        "exit" => HBuiltinFn::Exit,
        "read_file" => HBuiltinFn::ReadFile,
        "write_file" => HBuiltinFn::WriteFile,
        _ => return None,
    };
    Some(function)
}
