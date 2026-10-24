//! Lowering of builtin standard library methods, associated functions, free
//! functions, formatting and the builtin macros.
//!
//! Every builtin writes one owned value into the caller supplied target lvalue,
//! following the same ownership protocol as ordinary expressions.

use crate::hir::{
    FormatAlign, FormatSpec, HBuiltinAssoc, HBuiltinFn, HBuiltinMethod, HExpr, HFormatPart,
    HFormatPartKind, HMacroCall, HMacroKind,
};
use crate::span::Span;
use crate::types::{FloatTy, Ty};

use super::emit::Gen;
use super::ops;
use super::types;
use super::util;

/// Panic messages that are part of the language contract.
const OPTION_UNWRAP_PANIC: &str =
    "called \u{0060}Option::unwrap()\u{0060} on a \u{0060}None\u{0060} value";
const RESULT_UNWRAP_PANIC: &str =
    "called \u{0060}Result::unwrap()\u{0060} on an \u{0060}Err\u{0060} value";
const ASSERT_PANIC: &str = "assertion failed";
const ASSERT_EQ_PANIC: &str = "assertion \u{0060}left == right\u{0060} failed";
const ASSERT_EQ_LEFT: &str = "\n  left: ";
const ASSERT_EQ_RIGHT: &str = "\n  right: ";
const CAPACITY_OVERFLOW: &str = "capacity overflow";
const PARSE_I64_ERROR: &str = "invalid integer";
const PARSE_F64_ERROR: &str = "invalid float";
const IMPLICIT_PANIC: &str = "explicit panic";

impl<'a> Gen<'a> {
    // ----- builtin methods -------------------------------------------------

    pub(crate) fn gen_builtin_method(
        &mut self,
        receiver: &HExpr,
        method: HBuiltinMethod,
        args: &[HExpr],
        ty: &Ty,
        target: &str,
        span: Span,
    ) {
        let loc = self.loc(span);
        match method {
            // ----- String --------------------------------------------------
            HBuiltinMethod::StrLen => {
                let place = self.gen_place(receiver);
                let ctype = self.ctype(ty);
                self.line(format!("{} = ({})ls_str_len({});", target, ctype, place));
            }
            HBuiltinMethod::StrIsEmpty => {
                let place = self.gen_place(receiver);
                self.line(format!("{} = ls_str_is_empty({});", target, place));
            }
            HBuiltinMethod::StrPushStr => {
                let place = self.gen_place(receiver);
                if let Some(arg) = args.first() {
                    let value = self.gen_owned(arg);
                    self.line(format!(
                        "{} = ls_str_push_str({}, {});",
                        place, place, value
                    ));
                }
                self.assign_unit(target);
            }
            HBuiltinMethod::StrClear => {
                let place = self.gen_place(receiver);
                self.line(format!("{} = ls_str_clear({});", place, place));
                self.assign_unit(target);
            }
            HBuiltinMethod::StrContains => {
                self.string_predicate(receiver, args, "ls_str_contains", target);
            }
            HBuiltinMethod::StrStartsWith => {
                self.string_predicate(receiver, args, "ls_str_starts_with", target);
            }
            HBuiltinMethod::StrEndsWith => {
                self.string_predicate(receiver, args, "ls_str_ends_with", target);
            }
            HBuiltinMethod::StrFind => {
                let place = self.gen_place(receiver);
                if let Some(arg) = args.first() {
                    let value = self.gen_owned(arg);
                    let found = self.new_plain_of("int64_t", "found");
                    self.line(format!("{} = ls_str_find({}, {});", found, place, value));
                    self.line(format!("if ({} >= 0) {{", found));
                    self.w.indent();
                    self.line(format!("{}.{} = 1;", target, types::TAG_FIELD));
                    self.line(format!(
                        "{}.{}.{} = (int64_t){};",
                        target,
                        types::UNION_FIELD,
                        types::OPTION_SOME_MEMBER,
                        found
                    ));
                    self.w.dedent();
                    self.line("}");
                }
            }
            HBuiltinMethod::StrReplace => {
                let place = self.gen_place(receiver);
                if let (Some(from), Some(to)) = (args.first(), args.get(1)) {
                    let from_value = self.gen_owned(from);
                    let to_value = self.gen_owned(to);
                    self.line(format!(
                        "{} = ls_str_replace({}, {}, {});",
                        target, place, from_value, to_value
                    ));
                }
            }
            HBuiltinMethod::StrToUppercase => {
                let place = self.gen_place(receiver);
                self.line(format!("{} = ls_str_to_uppercase({});", target, place));
            }
            HBuiltinMethod::StrToLowercase => {
                let place = self.gen_place(receiver);
                self.line(format!("{} = ls_str_to_lowercase({});", target, place));
            }
            HBuiltinMethod::StrTrim => {
                let place = self.gen_place(receiver);
                self.line(format!("{} = ls_str_trim({});", target, place));
            }
            HBuiltinMethod::StrSplit => {
                let place = self.gen_place(receiver);
                if let Some(arg) = args.first() {
                    let value = self.gen_owned(arg);
                    self.line(format!("{} = ls_str_split({}, {});", target, place, value));
                }
            }
            HBuiltinMethod::StrParseI64 => {
                let place = self.gen_place(receiver);
                let parsed = self.new_plain_of("int64_t", "parsed");
                self.line(format!("if (ls_str_parse_i64({}, &{})) {{", place, parsed));
                self.w.indent();
                self.line(format!("{}.{} = 0;", target, types::TAG_FIELD));
                self.line(format!(
                    "{}.{}.{} = (int64_t){};",
                    target,
                    types::UNION_FIELD,
                    types::RESULT_OK_MEMBER,
                    parsed
                ));
                self.w.dedent();
                self.line("} else {");
                self.w.indent();
                self.line(format!("{}.{} = 1;", target, types::TAG_FIELD));
                self.line(format!(
                    "{}.{}.{} = ls_str_cstr({});",
                    target,
                    types::UNION_FIELD,
                    types::RESULT_ERR_MEMBER,
                    util::c_string_literal(PARSE_I64_ERROR)
                ));
                self.w.dedent();
                self.line("}");
            }
            HBuiltinMethod::StrParseF64 => {
                let place = self.gen_place(receiver);
                let parsed = self.new_plain_of("double", "parsed");
                self.line(format!("if (ls_str_parse_f64({}, &{})) {{", place, parsed));
                self.w.indent();
                self.line(format!("{}.{} = 0;", target, types::TAG_FIELD));
                self.line(format!(
                    "{}.{}.{} = (double){};",
                    target,
                    types::UNION_FIELD,
                    types::RESULT_OK_MEMBER,
                    parsed
                ));
                self.w.dedent();
                self.line("} else {");
                self.w.indent();
                self.line(format!("{}.{} = 1;", target, types::TAG_FIELD));
                self.line(format!(
                    "{}.{}.{} = ls_str_cstr({});",
                    target,
                    types::UNION_FIELD,
                    types::RESULT_ERR_MEMBER,
                    util::c_string_literal(PARSE_F64_ERROR)
                ));
                self.w.dedent();
                self.line("}");
            }
            HBuiltinMethod::StrSubstring => {
                let place = self.gen_place(receiver);
                if let (Some(start), Some(length)) = (args.first(), args.get(1)) {
                    let start_value = self.gen_owned(start);
                    let length_value = self.gen_owned(length);
                    self.line(format!(
                        "if ((int64_t)({}) < 0) {{ ls_panic_bounds({}, (int64_t)ls_str_len({}), (int64_t)({})); }}",
                        start_value, loc, place, start_value
                    ));
                    self.line(format!(
                        "if ((int64_t)({}) < 0) {{ ls_panic_bounds({}, (int64_t)ls_str_len({}), (int64_t)({})); }}",
                        length_value, loc, place, length_value
                    ));
                    self.line(format!(
                        "{} = ls_str_substring({}, (size_t)({}), (size_t)({}), {});",
                        target, place, start_value, length_value, loc
                    ));
                }
            }
            HBuiltinMethod::StrRepeat => {
                let place = self.gen_place(receiver);
                if let Some(arg) = args.first() {
                    let count = self.gen_owned(arg);
                    self.line(format!("if ((int64_t)({}) < 0) {{", count));
                    self.w.indent();
                    self.line(format!(
                        "ls_panic_cstr({}, {});",
                        loc,
                        util::c_string_literal(CAPACITY_OVERFLOW)
                    ));
                    self.w.dedent();
                    self.line("}");
                    self.line(format!(
                        "{} = ls_str_repeat({}, (size_t)({}), {});",
                        target, place, count, loc
                    ));
                }
            }

            // ----- Vec<T> --------------------------------------------------
            HBuiltinMethod::VecLen => {
                let place = self.gen_place(receiver);
                let ctype = self.ctype(ty);
                self.line(format!("{} = ({}){}.{};", target, ctype, place, "len"));
            }
            HBuiltinMethod::VecIsEmpty => {
                let place = self.gen_place(receiver);
                self.line(format!("{} = ({}.len == 0);", target, place));
            }
            HBuiltinMethod::VecPush => {
                let place = self.gen_place(receiver);
                if let Some(arg) = args.first() {
                    let value = self.gen_owned(arg);
                    if ops::is_frozen_shared(&receiver.ty) {
                        self.line(format!("ls_vec_string_push(&({}), {});", place, value));
                    } else {
                        let element = vec_element(&receiver.ty);
                        let element_ctype = self.ctype(&element);
                        self.line(format!(
                            "ls_vec_insert_bytes((void**)&({place}).data, &({place}).len, &({place}).cap, ({place}).len, (const void*)&{value}, sizeof({element_ctype}));"
                        ));
                        self.mark_moved(&value);
                    }
                }
                self.assign_unit(target);
            }
            HBuiltinMethod::VecPop => {
                let place = self.gen_place(receiver);
                if ops::is_frozen_shared(&receiver.ty) {
                    let out = self.new_plain_of("LsString*", "out");
                    self.line(format!("if (ls_vec_string_pop(&({}), &{})) {{", place, out));
                    self.w.indent();
                    self.line(format!("{} = ls_opt_string_some({});", target, out));
                    self.w.dedent();
                    self.line("}");
                } else {
                    self.line(format!("if (({}).len > 0) {{", place));
                    self.w.indent();
                    self.line(format!("({}).len -= 1;", place));
                    self.line(format!("{}.{} = 1;", target, types::TAG_FIELD));
                    self.line(format!(
                        "{}.{}.{} = ({}).data[({}).len];",
                        target,
                        types::UNION_FIELD,
                        types::OPTION_SOME_MEMBER,
                        place,
                        place
                    ));
                    self.w.dedent();
                    self.line("}");
                }
            }
            HBuiltinMethod::VecInsert => {
                let place = self.gen_place(receiver);
                if let (Some(index), Some(value)) = (args.first(), args.get(1)) {
                    let index_value = self.gen_owned(index);
                    let element_value = self.gen_owned(value);
                    self.line(format!(
                        "if ((int64_t)({}) < 0 || (uint64_t)({}) > (uint64_t)({}).len) {{ ls_panic_bounds({}, (int64_t)({}).len, (int64_t)({})); }}",
                        index_value, index_value, place, loc, place, index_value
                    ));
                    if ops::is_frozen_shared(&receiver.ty) {
                        self.line(format!(
                            "ls_vec_string_insert(&({}), (size_t)({}), {});",
                            place, index_value, element_value
                        ));
                    } else {
                        let element = vec_element(&receiver.ty);
                        let element_ctype = self.ctype(&element);
                        self.line(format!(
                            "ls_vec_insert_bytes((void**)&({}).data, &({}).len, &({}).cap, (size_t)({}), (const void*)&{}, sizeof({}));",
                            place, place, place, index_value, element_value, element_ctype
                        ));
                        self.mark_moved(&element_value);
                    }
                }
                self.assign_unit(target);
            }
            HBuiltinMethod::VecRemove => {
                let place = self.gen_place(receiver);
                if let Some(index) = args.first() {
                    let index_value = self.gen_owned(index);
                    self.line(format!(
                        "if ((int64_t)({}) < 0 || (uint64_t)({}) >= (uint64_t)({}).len) {{ ls_panic_bounds({}, (int64_t)({}).len, (int64_t)({})); }}",
                        index_value, index_value, place, loc, place, index_value
                    ));
                    if ops::is_frozen_shared(&receiver.ty) {
                        self.line(format!(
                            "{} = ls_vec_string_remove(&({}), (size_t)({}));",
                            target, place, index_value
                        ));
                    } else {
                        let element = vec_element(&receiver.ty);
                        let element_ctype = self.ctype(&element);
                        self.line(format!(
                            "{} = ({}).data[(size_t)({})];",
                            target, place, index_value
                        ));
                        self.line(format!(
                            "memmove(&({}).data[(size_t)({})], &({}).data[(size_t)({}) + 1], (({}).len - (size_t)({}) - 1) * sizeof({}));",
                            place, index_value, place, index_value, place, index_value, element_ctype
                        ));
                        self.line(format!("({}).len -= 1;", place));
                    }
                }
            }
            HBuiltinMethod::VecClear => {
                let place = self.gen_place(receiver);
                if ops::is_frozen_shared(&receiver.ty) {
                    self.line(format!("ls_vec_string_clear(&({}));", place));
                } else {
                    let element = vec_element(&receiver.ty);
                    if ops::is_managed(&element) {
                        let index = self.fresh("i");
                        self.line(format!(
                            "for (size_t {} = 0; {} < ({}).len; {} += 1) {{",
                            index, index, place, index
                        ));
                        self.w.indent();
                        let value = format!("({}).data[{}]", place, index);
                        self.emit_release(&element, &value);
                        self.w.dedent();
                        self.line("}");
                    }
                    self.line(format!("({}).len = 0;", place));
                }
                self.assign_unit(target);
            }
            HBuiltinMethod::VecFirst => {
                self.gen_vec_optional_element(receiver, false, None, target);
            }
            HBuiltinMethod::VecLast => {
                self.gen_vec_optional_element(receiver, true, None, target);
            }
            HBuiltinMethod::VecGet => {
                if let Some(index) = args.first() {
                    self.gen_vec_optional_element(receiver, false, Some(index), target);
                }
            }
            HBuiltinMethod::VecExtend => {
                let place = self.gen_place(receiver);
                if let Some(arg) = args.first() {
                    let other = self.gen_owned(arg);
                    if ops::is_frozen_shared(&receiver.ty) {
                        self.line(format!("ls_vec_string_extend(&({}), &({}));", place, other));
                    } else {
                        let element = vec_element(&receiver.ty);
                        let element_ctype = self.ctype(&element);
                        self.line(format!(
                            "ls_vec_reserve((void**)&({}).data, &({}).cap, ({}).len, ({}).len + ({}).len, sizeof({}));",
                            place, place, place, place, other, element_ctype
                        ));
                        let index = self.fresh("i");
                        self.line(format!(
                            "for (size_t {} = 0; {} < ({}).len; {} += 1) {{",
                            index, index, other, index
                        ));
                        self.w.indent();
                        let names = self.names;
                        let value = ops::copy_expr(
                            names,
                            &element,
                            &format!("({}).data[{}]", other, index),
                        );
                        self.line(format!(
                            "({}).data[({}).len + {}] = {};",
                            place, place, index, value
                        ));
                        self.w.dedent();
                        self.line("}");
                        self.line(format!("({}).len += ({}).len;", place, other));
                    }
                }
                self.assign_unit(target);
            }
            HBuiltinMethod::VecReverse => {
                let place = self.gen_place(receiver);
                if ops::is_frozen_shared(&receiver.ty) {
                    self.line(format!("ls_vec_string_reverse(&({}));", place));
                } else {
                    let element = vec_element(&receiver.ty);
                    let element_ctype = self.ctype(&element);
                    self.line(format!(
                        "ls_vec_reverse_bytes((void*)({}).data, ({}).len, sizeof({}));",
                        place, place, element_ctype
                    ));
                }
                self.assign_unit(target);
            }

            // ----- Option<T> -----------------------------------------------
            HBuiltinMethod::OptIsSome => {
                let place = self.gen_place(receiver);
                self.line(format!(
                    "{} = (({}).{} == 1);",
                    target,
                    place,
                    types::TAG_FIELD
                ));
            }
            HBuiltinMethod::OptIsNone => {
                let place = self.gen_place(receiver);
                self.line(format!(
                    "{} = (({}).{} != 1);",
                    target,
                    place,
                    types::TAG_FIELD
                ));
            }
            HBuiltinMethod::OptUnwrap => {
                let place = self.gen_place(receiver);
                let payload_ty = option_payload(&receiver.ty);
                self.line(format!("if (({}).{} != 1) {{", place, types::TAG_FIELD));
                self.w.indent();
                self.line(format!(
                    "ls_panic_cstr({}, {});",
                    loc,
                    util::c_string_literal(OPTION_UNWRAP_PANIC)
                ));
                self.w.dedent();
                self.line("}");
                let names = self.names;
                let value = ops::copy_expr(
                    names,
                    &payload_ty,
                    &format!(
                        "({}).{}.{}",
                        place,
                        types::UNION_FIELD,
                        types::OPTION_SOME_MEMBER
                    ),
                );
                self.line(format!("{} = {};", target, value));
            }
            HBuiltinMethod::OptUnwrapOr => {
                let place = self.gen_place(receiver);
                let payload_ty = option_payload(&receiver.ty);
                if let Some(default) = args.first() {
                    let default_value = self.gen_owned(default);
                    self.line(format!("{} = {};", target, default_value));
                    self.mark_moved(&default_value);
                    self.line(format!("if (({}).{} == 1) {{", place, types::TAG_FIELD));
                    self.w.indent();
                    self.emit_release(&payload_ty, target);
                    let names = self.names;
                    let value = ops::copy_expr(
                        names,
                        &payload_ty,
                        &format!(
                            "({}).{}.{}",
                            place,
                            types::UNION_FIELD,
                            types::OPTION_SOME_MEMBER
                        ),
                    );
                    self.line(format!("{} = {};", target, value));
                    self.w.dedent();
                    self.line("}");
                }
            }
            HBuiltinMethod::OptExpect => {
                let place = self.gen_place(receiver);
                let payload_ty = option_payload(&receiver.ty);
                if let Some(message) = args.first() {
                    let message_value = self.gen_owned(message);
                    self.line(format!("if (({}).{} != 1) {{", place, types::TAG_FIELD));
                    self.w.indent();
                    self.line(format!("ls_panic_at({}, {});", loc, message_value));
                    self.w.dedent();
                    self.line("}");
                    let names = self.names;
                    let value = ops::copy_expr(
                        names,
                        &payload_ty,
                        &format!(
                            "({}).{}.{}",
                            place,
                            types::UNION_FIELD,
                            types::OPTION_SOME_MEMBER
                        ),
                    );
                    self.line(format!("{} = {};", target, value));
                }
            }

            // ----- Result<T, E> --------------------------------------------
            HBuiltinMethod::ResIsOk => {
                let place = self.gen_place(receiver);
                self.line(format!(
                    "{} = (({}).{} == 0);",
                    target,
                    place,
                    types::TAG_FIELD
                ));
            }
            HBuiltinMethod::ResIsErr => {
                let place = self.gen_place(receiver);
                self.line(format!(
                    "{} = (({}).{} != 0);",
                    target,
                    place,
                    types::TAG_FIELD
                ));
            }
            HBuiltinMethod::ResUnwrap => {
                let place = self.gen_place(receiver);
                let ok_ty = result_ok_type(&receiver.ty);
                self.line(format!("if (({}).{} != 0) {{", place, types::TAG_FIELD));
                self.w.indent();
                self.line(format!(
                    "ls_panic_cstr({}, {});",
                    loc,
                    util::c_string_literal(RESULT_UNWRAP_PANIC)
                ));
                self.w.dedent();
                self.line("}");
                let names = self.names;
                let value = ops::copy_expr(
                    names,
                    &ok_ty,
                    &format!(
                        "({}).{}.{}",
                        place,
                        types::UNION_FIELD,
                        types::RESULT_OK_MEMBER
                    ),
                );
                self.line(format!("{} = {};", target, value));
            }
            HBuiltinMethod::ResUnwrapOr => {
                let place = self.gen_place(receiver);
                let ok_ty = result_ok_type(&receiver.ty);
                if let Some(default) = args.first() {
                    let default_value = self.gen_owned(default);
                    self.line(format!("{} = {};", target, default_value));
                    self.mark_moved(&default_value);
                    self.line(format!("if (({}).{} == 0) {{", place, types::TAG_FIELD));
                    self.w.indent();
                    self.emit_release(&ok_ty, target);
                    let names = self.names;
                    let value = ops::copy_expr(
                        names,
                        &ok_ty,
                        &format!(
                            "({}).{}.{}",
                            place,
                            types::UNION_FIELD,
                            types::RESULT_OK_MEMBER
                        ),
                    );
                    self.line(format!("{} = {};", target, value));
                    self.w.dedent();
                    self.line("}");
                }
            }
            HBuiltinMethod::ResExpect => {
                let place = self.gen_place(receiver);
                let ok_ty = result_ok_type(&receiver.ty);
                if let Some(message) = args.first() {
                    let message_value = self.gen_owned(message);
                    self.line(format!("if (({}).{} != 0) {{", place, types::TAG_FIELD));
                    self.w.indent();
                    self.line(format!("ls_panic_at({}, {});", loc, message_value));
                    self.w.dedent();
                    self.line("}");
                    let names = self.names;
                    let value = ops::copy_expr(
                        names,
                        &ok_ty,
                        &format!(
                            "({}).{}.{}",
                            place,
                            types::UNION_FIELD,
                            types::RESULT_OK_MEMBER
                        ),
                    );
                    self.line(format!("{} = {};", target, value));
                }
            }
            HBuiltinMethod::ResOk => {
                let place = self.gen_place(receiver);
                let ok_ty = result_ok_type(&receiver.ty);
                let names = self.names;
                let value = ops::copy_expr(
                    names,
                    &ok_ty,
                    &format!(
                        "({}).{}.{}",
                        place,
                        types::UNION_FIELD,
                        types::RESULT_OK_MEMBER
                    ),
                );
                self.line(format!("if (({}).{} == 0) {{", place, types::TAG_FIELD));
                self.w.indent();
                self.store_option_some(target, ty, &value);
                self.w.dedent();
                self.line("}");
            }
            HBuiltinMethod::RcGet => {
                if let Ty::Rc(inner) = &receiver.ty {
                    let place = self.gen_place(receiver);
                    let suffix = self.names.mangle(inner);
                    let ctype = self.ctype(ty);
                    self.line(format!(
                        "{} = ({})lsaca_rc_get_{}({});",
                        target, ctype, suffix, place
                    ));
                } else {
                    let zero = self.zero_expr(ty);
                    self.line(format!("{} = {};", target, zero));
                }
            }
            HBuiltinMethod::RcSet => {
                if let (Ty::Rc(inner), Some(arg)) = (&receiver.ty, args.first()) {
                    let place = self.gen_place(receiver);
                    let suffix = self.names.mangle(inner);
                    // The cell takes ownership of the new payload, so the
                    // generated code must not release the argument again.
                    let value = self.gen_owned(arg);
                    self.line(format!("lsaca_rc_set_{}({}, {});", suffix, place, value));
                    self.mark_moved(&value);
                    self.assign_unit(target);
                } else {
                    self.assign_unit(target);
                }
            }
            HBuiltinMethod::RcStrongCount => {
                let place = self.gen_place(receiver);
                let ctype = self.ctype(ty);
                self.line(format!("{} = ({})ls_refcount({});", target, ctype, place));
            }
            HBuiltinMethod::BoxGet => {
                if let Ty::Box(inner) = &receiver.ty {
                    let place = self.gen_place(receiver);
                    let suffix = self.names.mangle(inner);
                    let ctype = self.ctype(ty);
                    self.line(format!(
                        "{} = ({})lsaca_box_get_{}({});",
                        target, ctype, suffix, place
                    ));
                } else {
                    let zero = self.zero_expr(ty);
                    self.line(format!("{} = {};", target, zero));
                }
            }
            HBuiltinMethod::BoxSet => {
                if let (Ty::Box(inner), Some(arg)) = (&receiver.ty, args.first()) {
                    let place = self.gen_place(receiver);
                    let suffix = self.names.mangle(inner);
                    // The cell takes ownership of the new payload, so the
                    // generated code must not release the argument again.
                    let value = self.gen_owned(arg);
                    self.line(format!("lsaca_box_set_{}({}, {});", suffix, place, value));
                    self.mark_moved(&value);
                    self.assign_unit(target);
                } else {
                    self.assign_unit(target);
                }
            }
            HBuiltinMethod::ResErr => {
                let place = self.gen_place(receiver);
                let err_ty = result_err_type(&receiver.ty);
                let names = self.names;
                let value = ops::copy_expr(
                    names,
                    &err_ty,
                    &format!(
                        "({}).{}.{}",
                        place,
                        types::UNION_FIELD,
                        types::RESULT_ERR_MEMBER
                    ),
                );
                self.line(format!("if (({}).{} != 0) {{", place, types::TAG_FIELD));
                self.w.indent();
                self.store_option_some(target, ty, &value);
                self.w.dedent();
                self.line("}");
            }
        }
    }

    // ----- builtin associated functions ------------------------------------

    pub(crate) fn gen_builtin_assoc(
        &mut self,
        func: HBuiltinAssoc,
        args: &[HExpr],
        _type_args: &[Ty],
        ty: &Ty,
        target: &str,
        span: Span,
    ) {
        let loc = self.loc(span);
        match func {
            HBuiltinAssoc::StringNew => {
                self.line(format!("{} = ls_str_new();", target));
            }
            HBuiltinAssoc::StringFrom => {
                if let Some(arg) = args.first() {
                    let value = self.gen_owned(arg);
                    self.line(format!("{} = ls_str_clone({});", target, value));
                }
            }
            HBuiltinAssoc::RcNew => {
                if let (Some(arg), Ty::Rc(inner)) = (args.first(), ty) {
                    // The new cell owns the payload: transfer ownership
                    // instead of releasing the argument at the end of the
                    // statement.
                    let value = self.gen_owned(arg);
                    self.mark_moved(&value);
                    let suffix = self.names.mangle(inner);
                    let ctype = self.ctype(ty);
                    self.line(format!(
                        "{} = ({})lsaca_rc_new_{}({});",
                        target, ctype, suffix, value
                    ));
                }
            }
            HBuiltinAssoc::BoxNew => {
                if let (Some(arg), Ty::Box(inner)) = (args.first(), ty) {
                    // The new cell owns the payload: transfer ownership
                    // instead of releasing the argument at the end of the
                    // statement.
                    let value = self.gen_owned(arg);
                    self.mark_moved(&value);
                    let suffix = self.names.mangle(inner);
                    let ctype = self.ctype(ty);
                    self.line(format!(
                        "{} = ({})lsaca_box_new_{}({});",
                        target, ctype, suffix, value
                    ));
                }
            }
            HBuiltinAssoc::VecNew => {
                let zero = self.zero_expr(ty);
                self.line(format!("{} = {};", target, zero));
            }
            HBuiltinAssoc::VecWithCapacity => {
                if let Some(arg) = args.first() {
                    let capacity = self.gen_owned(arg);
                    self.line(format!("if ((int64_t)({}) < 0) {{", capacity));
                    self.w.indent();
                    self.line(format!(
                        "ls_panic_cstr({}, {});",
                        loc,
                        util::c_string_literal(CAPACITY_OVERFLOW)
                    ));
                    self.w.dedent();
                    self.line("}");
                    if ops::is_frozen_shared(ty) {
                        self.line(format!(
                            "{} = ls_vec_string_with_capacity((size_t)({}));",
                            target, capacity
                        ));
                    } else {
                        let element = vec_element(ty);
                        let element_ctype = self.ctype(&element);
                        // `ls_vec_alloc` returns `void*`: cast to the element pointer type
                        // exactly like the other vector constructors do.
                        self.line(format!(
                            "{}.data = ({}*)ls_vec_alloc((size_t)({}), sizeof({}));",
                            target, element_ctype, capacity, element_ctype
                        ));
                        self.line(format!("{}.cap = (size_t)({});", target, capacity));
                    }
                }
            }
        }
    }

    // ----- builtin free functions ------------------------------------------

    pub(crate) fn gen_builtin_call(
        &mut self,
        func: HBuiltinFn,
        args: &[HExpr],
        ty: &Ty,
        target: &str,
        span: Span,
    ) {
        let loc = self.loc(span);
        match func {
            HBuiltinFn::Sqrt => self.gen_unary_float_math("sqrt", args, ty, target),
            HBuiltinFn::Floor => self.gen_unary_float_math("floor", args, ty, target),
            HBuiltinFn::Ceil => self.gen_unary_float_math("ceil", args, ty, target),
            HBuiltinFn::Round => self.gen_unary_float_math("round", args, ty, target),
            HBuiltinFn::Sin => self.gen_unary_float_math("sin", args, ty, target),
            HBuiltinFn::Cos => self.gen_unary_float_math("cos", args, ty, target),
            HBuiltinFn::Tan => self.gen_unary_float_math("tan", args, ty, target),
            HBuiltinFn::Ln => self.gen_unary_float_math("ln", args, ty, target),
            HBuiltinFn::Log10 => self.gen_unary_float_math("log10", args, ty, target),
            HBuiltinFn::Exp => self.gen_unary_float_math("exp", args, ty, target),
            HBuiltinFn::Pow => {
                if let (Some(base), Some(exponent)) = (args.first(), args.get(1)) {
                    let base_value = self.gen_owned(base);
                    let exponent_value = self.gen_owned(exponent);
                    let suffix = match &base.ty {
                        Ty::Float(float_ty) => float_ty.name(),
                        _ => "f64",
                    };
                    let ctype = self.ctype(ty);
                    self.line(format!(
                        "{} = ({})ls_pow_{}({}, {});",
                        target, ctype, suffix, base_value, exponent_value
                    ));
                }
            }
            HBuiltinFn::Abs => {
                if let Some(arg) = args.first() {
                    let value = self.gen_owned(arg);
                    match ty {
                        Ty::Int(int_ty) => {
                            let suffix = int_ty.name();
                            if int_ty.is_signed() && self.debug_checks {
                                self.line(format!(
                                    "{} = ls_abs_{}_checked({}, {});",
                                    target, suffix, value, loc
                                ));
                            } else {
                                self.line(format!("{} = ls_abs_{}({});", target, suffix, value));
                            }
                        }
                        Ty::Float(float_ty) => {
                            let suffix = float_ty.name();
                            self.line(format!("{} = ls_abs_{}({});", target, suffix, value));
                        }
                        _ => {}
                    }
                }
            }
            HBuiltinFn::Min => self.gen_int_or_float_min_max(true, args, ty, target),
            HBuiltinFn::Max => self.gen_int_or_float_min_max(false, args, ty, target),
            HBuiltinFn::ReadLine => {
                self.line(format!("{} = ls_read_line();", target));
            }
            HBuiltinFn::Args => {
                self.line(format!("{} = ls_args();", target));
            }
            HBuiltinFn::Exit => {
                if let Some(arg) = args.first() {
                    let code = self.gen_owned(arg);
                    self.assign_unit(target);
                    self.line(format!("ls_exit((int32_t)({}));", code));
                    self.block_terminated = true;
                    self.fn_terminated = true;
                }
            }
            HBuiltinFn::ReadFile => {
                if let Some(arg) = args.first() {
                    let path = self.gen_owned(arg);
                    self.line(format!("{} = ls_read_file({});", target, path));
                }
            }
            HBuiltinFn::WriteFile => {
                if let (Some(path), Some(contents)) = (args.first(), args.get(1)) {
                    let path_value = self.gen_owned(path);
                    let contents_value = self.gen_owned(contents);
                    let result = self.fresh("res");
                    self.line("{");
                    self.w.indent();
                    self.line(format!(
                        "LsRes_String_String {} = ls_write_file({}, {});",
                        result, path_value, contents_value
                    ));
                    self.line(format!(
                        "{}.{} = {}.{};",
                        target,
                        types::TAG_FIELD,
                        result,
                        types::TAG_FIELD
                    ));
                    self.line(format!("if ({}.{} == 0) {{", result, types::TAG_FIELD));
                    self.w.indent();
                    self.line(format!(
                        "ls_release({}.{}.{});",
                        result,
                        types::UNION_FIELD,
                        types::RESULT_OK_MEMBER
                    ));
                    self.w.dedent();
                    self.line("} else {");
                    self.w.indent();
                    self.line(format!(
                        "{}.{}.{} = {}.{}.{};",
                        target,
                        types::UNION_FIELD,
                        types::RESULT_ERR_MEMBER,
                        result,
                        types::UNION_FIELD,
                        types::RESULT_ERR_MEMBER
                    ));
                    self.w.dedent();
                    self.line("}");
                    self.w.dedent();
                    self.line("}");
                }
            }
        }
    }

    // ----- formatting ------------------------------------------------------

    pub(crate) fn gen_format(&mut self, parts: &[HFormatPart], target: &str, _span: Span) {
        let accumulator = self.build_format(parts);
        self.line(format!("{} = {};", target, accumulator));
    }

    pub(crate) fn gen_macro(&mut self, call: &HMacroCall, target: &str, span: Span) {
        let loc = self.loc(span);
        match call.kind {
            HMacroKind::Print | HMacroKind::Println | HMacroKind::Eprint | HMacroKind::Eprintln => {
                let accumulator = self.build_format(&call.parts);
                let printer = match call.kind {
                    HMacroKind::Print => "ls_print",
                    HMacroKind::Println => "ls_println",
                    HMacroKind::Eprint => "ls_eprint",
                    _ => "ls_eprintln",
                };
                self.line(format!("{}({});", printer, accumulator));
                self.line(format!("ls_release({});", accumulator));
                self.assign_unit(target);
            }
            HMacroKind::Panic => {
                if call.parts.is_empty() {
                    self.line(format!(
                        "ls_panic_cstr({}, {});",
                        loc,
                        util::c_string_literal(IMPLICIT_PANIC)
                    ));
                } else {
                    let accumulator = self.build_format(&call.parts);
                    self.line(format!("ls_panic_at({}, {});", loc, accumulator));
                }
                self.block_terminated = true;
                self.fn_terminated = true;
            }
            HMacroKind::Assert => self.gen_assert(call, &loc, target),
            HMacroKind::AssertEq => self.gen_assert_eq(call, &loc, target),
        }
    }

    // ----- helpers ---------------------------------------------------------

    fn assign_unit(&mut self, target: &str) {
        let zero = self.zero_expr(&Ty::Unit);
        self.line(format!("{} = {};", target, zero));
    }

    fn string_predicate(&mut self, receiver: &HExpr, args: &[HExpr], func: &str, target: &str) {
        let place = self.gen_place(receiver);
        if let Some(arg) = args.first() {
            let value = self.gen_owned(arg);
            self.line(format!("{} = {}({}, {});", target, func, place, value));
        }
    }

    fn gen_unary_float_math(&mut self, name: &str, args: &[HExpr], ty: &Ty, target: &str) {
        if let Some(arg) = args.first() {
            let value = self.gen_owned(arg);
            let suffix = match &arg.ty {
                Ty::Float(float_ty) => float_ty.name(),
                _ => "f64",
            };
            let ctype = self.ctype(ty);
            self.line(format!(
                "{} = ({})ls_{}_{}({});",
                target, ctype, name, suffix, value
            ));
        }
    }

    fn gen_int_or_float_min_max(&mut self, is_min: bool, args: &[HExpr], ty: &Ty, target: &str) {
        if let (Some(first), Some(second)) = (args.first(), args.get(1)) {
            let first_value = self.gen_owned(first);
            let second_value = self.gen_owned(second);
            let name = if is_min { "min" } else { "max" };
            match ty {
                Ty::Int(int_ty) => {
                    let suffix = int_ty.name();
                    self.line(format!(
                        "{} = ls_{}_{}({}, {});",
                        target, name, suffix, first_value, second_value
                    ));
                }
                Ty::Float(float_ty) => {
                    let suffix = float_ty.name();
                    self.line(format!(
                        "{} = ls_{}_{}({}, {});",
                        target, name, suffix, first_value, second_value
                    ));
                }
                _ => {}
            }
        }
    }

    /// Build the value of a Vec first/last/get optional element expression.
    ///
    /// When "from_end" is set and no explicit index is given the last element
    /// is read; an explicit index is used by Vec::get.
    fn gen_vec_optional_element(
        &mut self,
        receiver: &HExpr,
        from_end: bool,
        index_arg: Option<&HExpr>,
        target: &str,
    ) {
        let place = self.gen_place(receiver);
        let frozen = ops::is_frozen_shared(&receiver.ty);
        let index_value = index_arg.map(|argument| self.gen_owned(argument));
        let condition = match &index_value {
            Some(value) => format!(
                "((int64_t)({}) >= 0 && (uint64_t)({}) < (uint64_t)({}).len)",
                value, value, place
            ),
            None => format!("(({}).len > 0)", place),
        };
        let index_expression = match &index_value {
            Some(value) => format!("(size_t)({})", value),
            None => {
                if from_end {
                    format!("(({}).len - 1)", place)
                } else {
                    String::from("0")
                }
            }
        };
        self.line(format!("if ({}) {{", condition));
        self.w.indent();
        if frozen {
            let out = self.new_plain_of("LsString*", "out");
            self.line(format!(
                "if (ls_vec_string_get(&({}), {}, &{})) {{",
                place, index_expression, out
            ));
            self.w.indent();
            self.line(format!("{} = ls_opt_string_some({});", target, out));
            self.w.dedent();
            self.line("}");
        } else {
            let element = vec_element(&receiver.ty);
            let names = self.names;
            let value = ops::copy_expr(
                names,
                &element,
                &format!("({}).data[{}]", place, index_expression),
            );
            self.line(format!("{}.{} = 1;", target, types::TAG_FIELD));
            self.line(format!(
                "{}.{}.{} = {};",
                target,
                types::UNION_FIELD,
                types::OPTION_SOME_MEMBER,
                value
            ));
        }
        self.w.dedent();
        self.line("}");
    }

    fn store_option_some(&mut self, target: &str, option_ty: &Ty, value: &str) {
        if ops::is_frozen_shared(option_ty) {
            self.line(format!("{} = ls_opt_string_some({});", target, value));
        } else {
            self.line(format!("{}.{} = 1;", target, types::TAG_FIELD));
            self.line(format!(
                "{}.{}.{} = {};",
                target,
                types::UNION_FIELD,
                types::OPTION_SOME_MEMBER,
                value
            ));
        }
    }

    fn build_format(&mut self, parts: &[HFormatPart]) -> String {
        let accumulator = self.new_plain_of("LsString*", "acc");
        self.line(format!("{} = ls_format_begin(0);", accumulator));
        self.append_format_parts(&accumulator, parts);
        accumulator
    }

    fn append_format_parts(&mut self, accumulator: &str, parts: &[HFormatPart]) {
        for part in parts {
            match &part.kind {
                HFormatPartKind::Literal(text) => {
                    self.line(format!(
                        "{} = ls_format_push_cstr({}, {});",
                        accumulator,
                        accumulator,
                        util::c_string_literal(text)
                    ));
                }
                HFormatPartKind::Arg { expr, spec } => {
                    let value = self.gen_owned(expr);
                    if spec.is_plain() {
                        self.append_display(accumulator, &expr.ty, &value);
                    } else {
                        self.append_specified(accumulator, &expr.ty, &value, spec);
                    }
                }
            }
        }
    }

    /// Append one format argument that carries a non-plain format specifier.
    ///
    /// Without a width the value is rendered straight into the accumulator.
    /// With a width the rendered text first goes into a temporary piece so the
    /// runtime can pad it (and so the sign and leading zeros stay in front of
    /// any zero padding).
    fn append_specified(&mut self, accumulator: &str, ty: &Ty, value: &str, spec: &FormatSpec) {
        let has_padding = spec.width.is_some() || spec.align.is_some();
        if !has_padding {
            if spec.debug {
                if let Some(statement) =
                    ops::debug_append_statement(self.names, ty, accumulator, value)
                {
                    self.line(statement);
                }
            } else {
                self.append_display_spec(accumulator, ty, value, spec);
            }
            return;
        }
        self.push_frame();
        // The piece is a real temporary: declare it through declare_temp so the
        // frame cleanup releases it like every other owned handle.
        let piece = self.declare_temp(&Ty::String);
        self.line(format!("{} = ls_format_begin(0);", piece));
        if spec.debug {
            if let Some(statement) = ops::debug_append_statement(self.names, ty, &piece, value) {
                self.line(statement);
            }
        } else {
            self.append_display_spec(&piece, ty, value, spec);
        }
        let width = spec.width.unwrap_or(0);
        let align = match spec.align {
            Some(FormatAlign::Left) => "LS_ALIGN_LEFT",
            Some(FormatAlign::Center) => "LS_ALIGN_CENTER",
            Some(FormatAlign::Right) => "LS_ALIGN_RIGHT",
            None if ty.is_integer() || ty.is_float() => "LS_ALIGN_RIGHT",
            None => "LS_ALIGN_LEFT",
        };
        self.line(format!(
            "{} = ls_fmt_apply_width({}, {}, {}, {}, {}, {});",
            accumulator,
            accumulator,
            piece,
            width,
            align,
            spec.fill as u32,
            if spec.zero_pad { "true" } else { "false" }
        ));
        self.frame_cleanup(self.frame_index());
        self.pop_frame();
    }

    /// Display one value with the precision and sign parts of a format
    /// specification; padding is applied by the caller.
    fn append_display_spec(&mut self, accumulator: &str, ty: &Ty, value: &str, spec: &FormatSpec) {
        let plus = if spec.sign_plus { "true" } else { "false" };
        let precision = spec.precision.unwrap_or(-1);
        match ty {
            Ty::Unit => self.line(format!(
                "{} = ls_format_push_cstr({}, {});",
                accumulator,
                accumulator,
                util::c_string_literal("()")
            )),
            Ty::String => {
                if spec.precision.is_some() {
                    self.line(format!(
                        "{} = ls_fmt_str_prec({}, {}, {});",
                        accumulator, accumulator, value, precision
                    ));
                } else {
                    self.line(format!(
                        "{} = ls_format_push_str({}, {});",
                        accumulator, accumulator, value
                    ));
                }
            }
            Ty::Bool => self.line(format!(
                "{} = ls_fmt_bool({}, {});",
                accumulator, accumulator, value
            )),
            Ty::Int(int_ty) => {
                if int_ty.is_signed() {
                    self.line(format!(
                        "{} = ls_fmt_i64_signed({}, (int64_t)({}), {});",
                        accumulator, accumulator, value, plus
                    ));
                } else {
                    self.line(format!(
                        "{} = ls_fmt_u64_signed({}, (uint64_t)({}), {});",
                        accumulator, accumulator, value, plus
                    ));
                }
            }
            Ty::Float(FloatTy::F32) => self.line(format!(
                "{} = ls_fmt_f32_prec({}, (float)({}), {}, {});",
                accumulator, accumulator, value, precision, plus
            )),
            Ty::Float(FloatTy::F64) => self.line(format!(
                "{} = ls_fmt_f64_prec({}, (double)({}), {}, {});",
                accumulator, accumulator, value, precision, plus
            )),
            Ty::Box(inner) => {
                let inner_value = format!("({})->payload", value);
                self.append_display_spec(accumulator, inner, &inner_value, spec);
            }
            _ => {
                if let Some(statement) =
                    ops::debug_append_statement(self.names, ty, accumulator, value)
                {
                    self.line(statement);
                }
            }
        }
    }

    fn append_display(&mut self, accumulator: &str, ty: &Ty, value: &str) {
        let statement = match ty {
            Ty::Unit => Some(format!(
                "{} = ls_format_push_cstr({}, {});",
                accumulator,
                accumulator,
                util::c_string_literal("()")
            )),
            Ty::String => Some(format!(
                "{} = ls_format_push_str({}, {});",
                accumulator, accumulator, value
            )),
            Ty::Bool => Some(format!(
                "{} = ls_fmt_bool({}, {});",
                accumulator, accumulator, value
            )),
            Ty::Int(int_ty) => {
                if int_ty.is_signed() {
                    Some(format!(
                        "{} = ls_fmt_i64({}, (int64_t)({}));",
                        accumulator, accumulator, value
                    ))
                } else {
                    Some(format!(
                        "{} = ls_fmt_u64({}, (uint64_t)({}));",
                        accumulator, accumulator, value
                    ))
                }
            }
            Ty::Float(float_ty) => match float_ty {
                FloatTy::F32 => Some(format!(
                    "{} = ls_fmt_f32({}, (float)({}));",
                    accumulator, accumulator, value
                )),
                FloatTy::F64 => Some(format!(
                    "{} = ls_fmt_f64({}, (double)({}));",
                    accumulator, accumulator, value
                )),
            },
            Ty::Box(inner) => {
                // Like Rust, displaying a box displays the value inside it.
                self.append_display(accumulator, inner, &format!("({})->payload", value));
                None
            }
            _ => ops::debug_append_statement(self.names, ty, accumulator, value),
        };
        if let Some(statement) = statement {
            self.line(statement);
        }
    }

    fn gen_assert(&mut self, call: &HMacroCall, loc: &str, target: &str) {
        let condition = match call.args.first() {
            Some(argument) => self.gen_owned(argument),
            None => return,
        };
        self.line(format!("if (!({})) {{", condition));
        self.w.indent();
        self.push_frame();
        if call.parts.is_empty() {
            self.line(format!(
                "ls_panic_cstr({}, {});",
                loc,
                util::c_string_literal(ASSERT_PANIC)
            ));
        } else {
            let message = self.build_format(&call.parts);
            self.frame_cleanup(self.frame_index());
            self.line(format!("ls_panic_at({}, {});", loc, message));
        }
        self.pop_frame();
        self.w.dedent();
        self.line("}");
        self.assign_unit(target);
    }

    fn gen_assert_eq(&mut self, call: &HMacroCall, loc: &str, target: &str) {
        if call.args.len() < 2 {
            return;
        }
        let left = self.gen_owned(&call.args[0]);
        let right = self.gen_owned(&call.args[1]);
        let names = self.names;
        let equals = ops::eq_expr(names, &call.args[0].ty, &left, &right);
        self.line(format!("if (!({})) {{", equals));
        self.w.indent();
        self.push_frame();
        if call.parts.is_empty() {
            let accumulator = self.new_plain_of("LsString*", "acc");
            self.line(format!("{} = ls_format_begin(0);", accumulator));
            self.line(format!(
                "{} = ls_format_push_cstr({}, {});",
                accumulator,
                accumulator,
                util::c_string_literal(ASSERT_EQ_PANIC)
            ));
            self.line(format!(
                "{} = ls_format_push_cstr({}, {});",
                accumulator,
                accumulator,
                util::c_string_literal(ASSERT_EQ_LEFT)
            ));
            if let Some(statement) =
                ops::debug_append_statement(names, &call.args[0].ty, &accumulator, &left)
            {
                self.line(statement);
            }
            self.line(format!(
                "{} = ls_format_push_cstr({}, {});",
                accumulator,
                accumulator,
                util::c_string_literal(ASSERT_EQ_RIGHT)
            ));
            if let Some(statement) =
                ops::debug_append_statement(names, &call.args[1].ty, &accumulator, &right)
            {
                self.line(statement);
            }
            self.frame_cleanup(self.frame_index());
            self.line(format!("ls_panic_at({}, {});", loc, accumulator));
        } else {
            let message = self.build_format(&call.parts);
            self.frame_cleanup(self.frame_index());
            self.line(format!("ls_panic_at({}, {});", loc, message));
        }
        self.pop_frame();
        self.w.dedent();
        self.line("}");
        self.assign_unit(target);
    }
}

fn option_payload(ty: &Ty) -> Ty {
    match ty {
        Ty::Option(inner) => (**inner).clone(),
        _ => Ty::Unit,
    }
}

fn result_ok_type(ty: &Ty) -> Ty {
    match ty {
        Ty::Result(ok, _) => (**ok).clone(),
        _ => Ty::Unit,
    }
}

fn result_err_type(ty: &Ty) -> Ty {
    match ty {
        Ty::Result(_, err) => (**err).clone(),
        _ => Ty::Unit,
    }
}

fn vec_element(ty: &Ty) -> Ty {
    match ty {
        Ty::Vec(element) => (**element).clone(),
        _ => Ty::Unit,
    }
}
