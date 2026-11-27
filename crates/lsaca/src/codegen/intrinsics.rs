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
use super::types::positional_field;
use super::util;
use crate::hir::{HBuiltinAssoc, HBuiltinFn, HBuiltinMethod, HExpr, HExprKind, HFn};
use crate::span::Span;
use crate::types::{IntTy, Ty};

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
        // The borrowed collection accessors (HashMap::get/get_mut, Vec::get_mut,
        // slice::get_mut) return an Option of a borrow into the collection. They
        // are emitted before the legacy tables because those only carry the
        // owned, copying variants.
        if self.try_gen_borrowed_accessor(name, args, ty, target) {
            return;
        }
        // Slice and str intrinsics operate on the by-value fat views and the
        // pointer-and-length pairs, so they are emitted before the legacy
        // builtin tables. They consume fat str values only: a legacy
        // read-only String method with the same name still routes through the
        // builtin method table below.
        if self.try_gen_slice_intrinsic(name, args, ty, target, span) {
            return;
        }
        // v2.4 system surface: UDP and process spawning extend the frozen
        // Phase C runtime ABI. They live here because phase_c.rs is frozen.
        if self.try_gen_system_intrinsic(name, args, ty, target, span) {
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

    /// v2.4: emit the UDP and process intrinsic calls added on top of the
    /// frozen Phase C table. Returns false for every other name so the Phase C
    /// and builtin tables below still see it.
    fn try_gen_system_intrinsic(
        &mut self,
        name: &str,
        args: &[HExpr],
        ty: &Ty,
        target: &str,
        _span: Span,
    ) -> bool {
        let bare = name.strip_prefix("__").unwrap_or(name);
        match bare {
            "net_udp_bind" => {
                require_i64(name, ty);
                let address = system_text(self, name, args, 0);
                self.line(format!("{} = ls_net_udp_bind({});", target, address));
            }
            "net_udp_send_to" => {
                require_i64(name, ty);
                let handle = system_int(self, name, args, 0);
                let (data, len) = system_buffer(self, name, args, 1);
                let address = system_text(self, name, args, 2);
                self.line(format!(
                    "{} = ls_net_udp_send_to({}, (const void*)({}), {}, {});",
                    target, handle, data, len, address
                ));
            }
            "net_udp_recv_from" => {
                require_tuple(name, ty, &[Ty::i64(), Ty::String]);
                let handle = system_int(self, name, args, 0);
                let (data, len) = system_buffer(self, name, args, 1);
                let peer = self.new_plain_of("LsString*", "udp_peer");
                let count = self.new_plain_of("int64_t", "udp_count");
                self.line(format!(
                    "{} = ls_net_udp_recv_from({}, (void*)({}), {}, &{});",
                    count, handle, data, len, peer
                ));
                let zero = self.raw_zero_expr(ty);
                self.line(format!("{} = {};", target, zero));
                self.line(format!(
                    "{}.{} = (int64_t)({});",
                    target,
                    positional_field(0),
                    count
                ));
                self.line(format!("{}.{} = {};", target, positional_field(1), peer));
            }
            "proc_status" => {
                require_tuple(name, ty, &[Ty::i64(), Ty::i64()]);
                let argv = system_vec_string(self, name, args, 0);
                let directory = system_text(self, name, args, 1);
                let status = self.new_plain_of("int64_t", "proc_status");
                let exit_code = self.new_plain_of("int64_t", "proc_exit");
                self.line(format!(
                    "{} = ls_proc_status({}, {}, &{});",
                    status, argv, directory, exit_code
                ));
                let zero = self.raw_zero_expr(ty);
                self.line(format!("{} = {};", target, zero));
                self.line(format!(
                    "{}.{} = (int64_t)({});",
                    target,
                    positional_field(0),
                    status
                ));
                self.line(format!(
                    "{}.{} = (int64_t)({});",
                    target,
                    positional_field(1),
                    exit_code
                ));
            }
            "proc_output" => {
                require_tuple(name, ty, &[Ty::i64(), Ty::i64(), Ty::String, Ty::String]);
                let argv = system_vec_string(self, name, args, 0);
                let directory = system_text(self, name, args, 1);
                let status = self.new_plain_of("int64_t", "proc_status");
                let exit_code = self.new_plain_of("int64_t", "proc_exit");
                let stdout = self.new_plain_of("LsString*", "proc_stdout");
                let stderr = self.new_plain_of("LsString*", "proc_stderr");
                self.line(format!(
                    "{} = ls_proc_output({}, {}, &{}, &{}, &{});",
                    status, argv, directory, exit_code, stdout, stderr
                ));
                let zero = self.raw_zero_expr(ty);
                self.line(format!("{} = {};", target, zero));
                self.line(format!(
                    "{}.{} = (int64_t)({});",
                    target,
                    positional_field(0),
                    status
                ));
                self.line(format!(
                    "{}.{} = (int64_t)({});",
                    target,
                    positional_field(1),
                    exit_code
                ));
                self.line(format!("{}.{} = {};", target, positional_field(2), stdout));
                self.line(format!("{}.{} = {};", target, positional_field(3), stderr));
            }
            _ => return false,
        }
        true
    }
}

/// The argument at `index` of a system intrinsic.
fn system_argument<'e>(name: &str, args: &'e [HExpr], index: usize) -> &'e HExpr {
    match args.get(index) {
        Some(argument) => argument,
        None => util::internal_error(format!(
            "intrinsic {} is called without argument {}",
            name, index
        )),
    }
}

/// Borrow a text argument as a `const LsString*`.
fn system_text(emitter: &mut Gen<'_>, name: &str, args: &[HExpr], index: usize) -> String {
    let argument = system_argument(name, args, index);
    let context = format!("intrinsic {}", name);
    emitter.string_text_argument(argument, &context)
}

/// Evaluate an integer argument.
fn system_int(emitter: &mut Gen<'_>, name: &str, args: &[HExpr], index: usize) -> String {
    let argument = system_argument(name, args, index);
    let value = emitter.gen_borrowed(argument);
    format!("(int64_t)({})", value)
}

/// The data pointer and length of a borrowed byte slice argument.
fn system_buffer(
    emitter: &mut Gen<'_>,
    name: &str,
    args: &[HExpr],
    index: usize,
) -> (String, String) {
    let argument = system_argument(name, args, index);
    if !is_byte_slice_reference(&argument.ty) {
        util::internal_error(format!(
            "intrinsic {} expects a borrowed byte slice argument",
            name
        ));
    }
    let fat = emitter.gen_borrowed(argument);
    (
        format!("({}).data", fat),
        format!("(int64_t)(({}).len)", fat),
    )
}

/// Borrow a `&Vec<String>` argument as a `const LsVec_String*`.
fn system_vec_string(emitter: &mut Gen<'_>, name: &str, args: &[HExpr], index: usize) -> String {
    let argument = system_argument(name, args, index);
    let is_vec = match &argument.ty {
        Ty::Vec(element) => **element == Ty::String,
        Ty::Ref { inner, .. } => match &**inner {
            Ty::Vec(element) => **element == Ty::String,
            _ => false,
        },
        _ => false,
    };
    if !is_vec {
        util::internal_error(format!("intrinsic {} expects a Vec<String> argument", name));
    }
    let value = emitter.gen_borrowed(argument);
    format!("(const LsVec_String*)({})", value)
}

/// True when the type is a borrowed byte slice (a frozen buffer view).
fn is_byte_slice_reference(ty: &Ty) -> bool {
    match ty {
        Ty::Ref { inner, .. } => match &**inner {
            Ty::Slice(element) => matches!(**element, Ty::Int(IntTy::U8)),
            _ => false,
        },
        _ => false,
    }
}

fn require_i64(name: &str, ty: &Ty) {
    if !matches!(ty, Ty::Int(IntTy::I64)) {
        util::internal_error(format!(
            "intrinsic {} must produce i64, found {:?}",
            name, ty
        ));
    }
}

fn require_tuple(name: &str, ty: &Ty, expected: &[Ty]) {
    let matches = match ty {
        Ty::Tuple(fields) => fields == expected,
        _ => false,
    };
    if !matches {
        util::internal_error(format!(
            "intrinsic {} must produce a tuple of {:?}, found {:?}",
            name, expected, ty
        ));
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
        "char_is_alphabetic" => HBuiltinFn::CharIsAlphabetic,
        "char_is_numeric" => HBuiltinFn::CharIsNumeric,
        "char_is_whitespace" => HBuiltinFn::CharIsWhitespace,
        "char_to_uppercase" => HBuiltinFn::CharToUppercase,
        "char_to_lowercase" => HBuiltinFn::CharToLowercase,
        "read_line" => HBuiltinFn::ReadLine,
        "args" => HBuiltinFn::Args,
        "exit" => HBuiltinFn::Exit,
        "read_file" => HBuiltinFn::ReadFile,
        "write_file" => HBuiltinFn::WriteFile,
        _ => return None,
    };
    Some(function)
}
