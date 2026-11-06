//! Lowering of slice and str values and their pointer-and-length intrinsics.
//!
//! A shared or mutable slice is a by-value fat struct that borrows a buffer:
//! a data pointer plus a length. A str value uses the same representation
//! under a distinct C type. Both are Copy, so creating one only copies the
//! pointer and the length; nothing owns an allocation.
//!
//! Range checks run before the view is built and panic with the Rust wording.
//! str ranges additionally ask the runtime to validate the byte range and the
//! UTF-8 scalar boundaries, which keeps the frozen runtime panic wording. The
//! std library's impl [T] / impl str methods are one-line forwarding wrappers
//! over the intrinsic names implemented here.

use super::emit::Gen;
use super::ops;
use super::types::{OPTION_SOME_MEMBER, TAG_FIELD, UNION_FIELD};
use super::util;
use crate::hir::HExpr;
use crate::span::Span;
use crate::types::Ty;

/// The borrowed buffer a slice expression starts from.
struct SliceView {
    /// C expression for the data pointer.
    data: String,
    /// C expression for the length, already converted to int64_t.
    len: String,
}

/// One piece of a generated panic message.
enum MessagePiece<'a> {
    /// Literal text.
    Text(&'a str),
    /// A number expression formatted by ls_fmt_i64.
    Number(&'a str),
}

/// True when the value is a str view: a fat value by value, no pointer layer.
fn is_str_value(ty: &Ty) -> bool {
    match ty {
        Ty::Str => true,
        Ty::Ref { inner, .. } => inner.is_str(),
        _ => false,
    }
}

/// True when the value is a slice view.
fn is_slice_value(ty: &Ty) -> bool {
    match ty {
        Ty::Slice(_) => true,
        Ty::Ref { inner, .. } => inner.is_slice(),
        _ => false,
    }
}

/// Element type of a slice view.
fn slice_element(ty: &Ty) -> Option<Ty> {
    match ty {
        Ty::Slice(elem) => Some((**elem).clone()),
        Ty::Ref { inner, .. } => match &**inner {
            Ty::Slice(elem) => Some((**elem).clone()),
            _ => None,
        },
        _ => None,
    }
}

/// True for the references that expose a Vec buffer as a slice.
fn is_vector_reference(ty: &Ty) -> bool {
    match ty {
        Ty::Ref { inner, .. } => matches!(&**inner, Ty::Vec(_)),
        _ => false,
    }
}

/// True for the references that expose a String buffer as str.
fn is_string_reference(ty: &Ty) -> bool {
    match ty {
        Ty::Ref { inner, .. } => inner.is_string(),
        _ => false,
    }
}

/// The Unicode White_Space predicate, as a C boolean expression over one
/// decoded scalar. Matches Rust's char::is_whitespace, which str::trim uses.
fn whitespace_expression(scalar: &str) -> String {
    format!(
        "(({scalar} >= 0x09U && {scalar} <= 0x0DU) || {scalar} == 0x20U || {scalar} == 0x85U || {scalar} == 0xA0U || {scalar} == 0x1680U || ({scalar} >= 0x2000U && {scalar} <= 0x200AU) || {scalar} == 0x2028U || {scalar} == 0x2029U || {scalar} == 0x202FU || {scalar} == 0x205FU || {scalar} == 0x3000U)",
        scalar = scalar
    )
}

impl<'a> Gen<'a> {
    /// Lower a slice expression into the fat value in target.
    ///
    /// The expression type is either a slice or str value, possibly behind a
    /// reference. Start and end are signed 64 bit values; inclusive ranges add
    /// one to the end before the checks.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn gen_slice(
        &mut self,
        base: &HExpr,
        start: Option<&HExpr>,
        end: Option<&HExpr>,
        inclusive: bool,
        ty: &Ty,
        target: &str,
        span: Span,
    ) {
        let pointee = match ty {
            Ty::Ref { inner, .. } => (**inner).clone(),
            other => other.clone(),
        };
        let (elem, is_str) = match &pointee {
            Ty::Slice(elem) => (Some((**elem).clone()), false),
            Ty::Str => (None, true),
            _ => util::internal_error("a slice expression must have slice or str type"),
        };
        let view = self.slice_view(base);
        if is_str && start.is_none() && end.is_none() {
            // A whole value str conversion such as the implicit &String -> &str
            // coercion, or an explicit full range like &s[..], always covers
            // the entire buffer: the endpoints are char boundaries by
            // construction, so the fat value can point straight at the
            // existing bytes with no validation and no copy.
            let out_ctype = self.ctype(ty);
            self.line(format!(
                "{} = ({}){{ .data = ((uint8_t*)({})), .len = (size_t)({}) }};",
                target, out_ctype, view.data, view.len
            ));
            return;
        }
        let start_name = self.new_plain_of("int64_t", "slice_start");
        match start {
            Some(start) => {
                let value = self.gen_owned(start);
                self.line(format!("{} = (int64_t)({});", start_name, value));
            }
            None => self.line(format!("{} = 0;", start_name)),
        }
        let end_name = self.new_plain_of("int64_t", "slice_end");
        match end {
            Some(end) => {
                let value = self.gen_owned(end);
                if inclusive {
                    self.line(format!("{} = (int64_t)({}) + 1;", end_name, value));
                } else {
                    self.line(format!("{} = (int64_t)({});", end_name, value));
                }
            }
            None => self.line(format!("{} = {};", end_name, view.len)),
        }
        // The order of the checks matches Rust: an inverted range first, then
        // the range end, then the range start. Signed values are compared as
        // unsigned so that a negative index is reported as out of range.
        self.line(format!(
            "if ((uint64_t)({}) > (uint64_t)({})) {{",
            start_name, end_name
        ));
        self.w.indent();
        self.emit_message_panic(
            span,
            &[
                MessagePiece::Text("slice index starts at "),
                MessagePiece::Number(&start_name),
                MessagePiece::Text(" but ends at "),
                MessagePiece::Number(&end_name),
            ],
        );
        self.w.dedent();
        self.line("}");
        if !is_str {
            let length = view.len.clone();
            self.line(format!(
                "if ((uint64_t)({}) > (uint64_t)({})) {{",
                end_name, length
            ));
            self.w.indent();
            self.emit_message_panic(
                span,
                &[
                    MessagePiece::Text("range end index "),
                    MessagePiece::Number(&end_name),
                    MessagePiece::Text(" out of range for slice of length "),
                    MessagePiece::Number(&length),
                ],
            );
            self.w.dedent();
            self.line("}");
            self.line(format!(
                "if ((uint64_t)({}) > (uint64_t)({})) {{",
                start_name, length
            ));
            self.w.indent();
            self.emit_message_panic(
                span,
                &[
                    MessagePiece::Text("range start index "),
                    MessagePiece::Number(&start_name),
                    MessagePiece::Text(" out of range for slice of length "),
                    MessagePiece::Number(&length),
                ],
            );
            self.w.dedent();
            self.line("}");
        }
        let out_ctype = self.ctype(ty);
        if is_str {
            // The runtime validates the byte range and both endpoints of the
            // UTF-8 scalar the range would cut, with the frozen wording. It
            // only reads the argument, so a read only view avoids copying the
            // base string.
            let length = format!("(size_t){}", view.len);
            let handle = self.read_only_string_view(&view.data, &length);
            let checked = self.declare_temp(&Ty::String);
            self.line(format!(
                "{} = ls_str_substring({}, (size_t)({}), (size_t)(({}) - ({})), {});",
                checked,
                handle,
                start_name,
                end_name,
                start_name,
                self.loc(span)
            ));
            self.release_handle(&checked);
            self.line(format!(
                "{} = ({}){{ .data = ((uint8_t*)({})) + (size_t)({}), .len = (size_t)(({}) - ({})) }};",
                target, out_ctype, view.data, start_name, end_name, start_name
            ));
        } else {
            let elem = elem.unwrap_or_else(|| {
                util::internal_error("a slice expression must have an element type")
            });
            let elem_ctype = self.ctype(&elem);
            let pointer_ctype = format!("{}*", elem_ctype);
            let out_data = self.new_plain_of(&pointer_ctype, "slice_output");
            self.line(format!("if (({}) != NULL) {{", view.data));
            self.w.indent();
            self.line(format!(
                "{} = (({})({})) + (size_t)({});",
                out_data, pointer_ctype, view.data, start_name
            ));
            self.w.dedent();
            self.line("} else {");
            self.w.indent();
            self.line(format!("{} = ({})(NULL);", out_data, pointer_ctype));
            self.w.dedent();
            self.line("}");
            self.line(format!(
                "{} = ({}){{ .data = {}, .len = (size_t)(({}) - ({})) }};",
                target, out_ctype, out_data, end_name, start_name
            ));
        }
    }

    /// The borrowed buffer of a slice expression base value.
    fn slice_view(&mut self, base: &HExpr) -> SliceView {
        match &base.ty {
            Ty::Vec(elem) => {
                let elem = (**elem).clone();
                let place = self.gen_place(base);
                self.vector_view(&place, &elem)
            }
            Ty::String => {
                let place = self.gen_place(base);
                self.string_view(&place)
            }
            Ty::Ref { inner, .. } => match &**inner {
                Ty::Vec(elem) => {
                    let elem = (**elem).clone();
                    let place = self.place_through_reference(base);
                    self.vector_view(&place, &elem)
                }
                Ty::String => {
                    let place = self.place_through_reference(base);
                    self.string_view(&place)
                }
                Ty::Slice(_) => {
                    let fat = self.fat_value(base, "slice_base");
                    SliceView {
                        data: format!("({}).data", fat),
                        len: format!("(int64_t)(({}).len)", fat),
                    }
                }
                Ty::Str => {
                    let fat = self.fat_value(base, "slice_base");
                    SliceView {
                        data: format!("(const uint8_t*)(({}).data)", fat),
                        len: format!("(int64_t)(({}).len)", fat),
                    }
                }
                _ => util::internal_error("cannot take a slice of this value"),
            },
            Ty::Slice(_) => {
                let fat = self.fat_value(base, "slice_base");
                SliceView {
                    data: format!("({}).data", fat),
                    len: format!("(int64_t)(({}).len)", fat),
                }
            }
            Ty::Str => {
                let fat = self.fat_value(base, "slice_base");
                SliceView {
                    data: format!("(const uint8_t*)(({}).data)", fat),
                    len: format!("(int64_t)(({}).len)", fat),
                }
            }
            _ => util::internal_error("cannot take a slice of this value"),
        }
    }

    /// The data pointer and length of a Vec place.
    fn vector_view(&mut self, place: &str, elem: &Ty) -> SliceView {
        let elem_ctype = self.ctype(elem);
        let data = self.new_plain_of(&format!("{}*", elem_ctype), "slice_data");
        self.line(format!("{} = ({}).data;", data, place));
        let length = self.new_plain_of("size_t", "slice_len");
        self.line(format!("{} = ({}).len;", length, place));
        SliceView {
            data,
            len: format!("(int64_t)({})", length),
        }
    }

    /// The data pointer and byte length of a String place.
    fn string_view(&mut self, place: &str) -> SliceView {
        let data = self.new_plain_of("const char*", "slice_data");
        self.line(format!("{} = ls_str_data({});", data, place));
        let length = self.new_plain_of("size_t", "slice_len");
        self.line(format!("{} = ls_str_len({});", length, place));
        SliceView {
            data,
            len: format!("(int64_t)({})", length),
        }
    }

    /// Materialise a slice or str value into a plain temporary.
    fn fat_value(&mut self, expr: &HExpr, prefix: &str) -> String {
        let fat = self.new_plain(&expr.ty, prefix);
        let value = self.gen_borrowed(expr);
        self.line(format!("{} = {};", fat, value));
        fat
    }

    /// Build a read only stack view over bytes borrowed from a str value.
    ///
    /// The read only string queries (char_count, char_at, is_char_boundary,
    /// contains, starts_with, ends_with, find, split, parse, to_uppercase,
    /// to_lowercase and substring) take a const LsString pointer and only read
    /// the len and data members: they never retain, release or mutate the
    /// argument. A stack descriptor over the borrowed bytes is therefore safe
    /// and avoids copying the whole string for every query. The cap member
    /// mirrors len so the descriptor is well formed even though it owns no
    /// buffer.
    fn read_only_string_view(&mut self, data: &str, len: &str) -> String {
        let view = self.fresh("str_view");
        self.line(format!(
            "LsString {} = (LsString){{ .len = (size_t)({}), .cap = (size_t)({}), .data = (char*)({}) }};",
            view, len, len, data
        ));
        format!("&({})", view)
    }

    /// Read-only LsString view over a borrowed str argument.
    ///
    /// Runtime path and name parameters take a const LsString pointer and only
    /// read it, so a `&str` argument is bridged with a stack descriptor over
    /// its borrowed bytes: no copy and no allocation.
    pub(crate) fn str_argument_view(&mut self, argument: &HExpr) -> String {
        let fat = self.fat_value(argument, "str_arg");
        let data = format!("(({}).data)", fat);
        let len = format!("(size_t)(({}).len)", fat);
        self.read_only_string_view(&data, &len)
    }

    /// C expression for the text of a String or &str argument as a
    /// const LsString* pointer.
    ///
    /// `context` names the caller in internal errors only; the front end
    /// decides whether a text parameter is an owned String or a borrowed
    /// slice, so both shapes are accepted here.
    pub(crate) fn string_text_argument(&mut self, argument: &HExpr, context: &str) -> String {
        if argument.ty.is_string() {
            return self.gen_borrowed(argument);
        }
        if let Ty::Ref { inner, .. } = &argument.ty
            && inner.is_string()
        {
            return self.place_through_reference(argument);
        }
        if is_str_value(&argument.ty) {
            return self.str_argument_view(argument);
        }
        util::internal_error(format!(
            "{} expects a String or &str argument, found {:?}",
            context, argument.ty
        ))
    }

    /// Release one owned handle and mark it released.
    fn release_handle(&mut self, handle: &str) {
        self.emit_release(&Ty::String, handle);
        self.mark_moved(handle);
    }

    /// Build and raise a panic message from literal text and number pieces.
    fn emit_message_panic(&mut self, span: Span, pieces: &[MessagePiece<'_>]) {
        let loc = self.loc(span);
        let message = self.new_plain_of("LsString*", "message");
        self.line(format!("{} = ls_format_begin(0);", message));
        for piece in pieces {
            match piece {
                MessagePiece::Text(text) => self.line(format!(
                    "{} = ls_format_push_cstr({}, {});",
                    message,
                    message,
                    util::c_string_literal(text)
                )),
                MessagePiece::Number(value) => self.line(format!(
                    "{} = ls_fmt_i64({}, (int64_t)({}));",
                    message, message, value
                )),
            }
        }
        self.line(format!("ls_panic_at({}, {});", loc, message));
    }

    /// Decode the UTF-8 scalar that starts at byte offset scan.
    ///
    /// The input is valid UTF-8 (the runtime only stores valid strings), so the
    /// lead byte determines the width; the continuation loop is bounded by the
    /// view length against a corrupted buffer.
    fn emit_utf8_scalar(&mut self, data: &str, scan: &str, length: &str) -> (String, String) {
        let lead = self.new_plain_of("unsigned char", "lead");
        self.line(format!("{} = (unsigned char)(({})[{}]);", lead, data, scan));
        let scalar = self.new_plain_of("uint32_t", "scalar");
        let width = self.new_plain_of("size_t", "width");
        self.line(format!("if (({}) < 0x80U) {{", lead));
        self.w.indent();
        self.line(format!("{} = (uint32_t)({});", scalar, lead));
        self.line(format!("{} = 1;", width));
        self.w.dedent();
        self.line(format!("}} else if ((({}) & 0xE0U) == 0xC0U) {{", lead));
        self.w.indent();
        self.line(format!("{} = (uint32_t)(({}) & 0x1FU);", scalar, lead));
        self.line(format!("{} = 2;", width));
        self.w.dedent();
        self.line(format!("}} else if ((({}) & 0xF0U) == 0xE0U) {{", lead));
        self.w.indent();
        self.line(format!("{} = (uint32_t)(({}) & 0x0FU);", scalar, lead));
        self.line(format!("{} = 3;", width));
        self.w.dedent();
        self.line("} else {");
        self.w.indent();
        self.line(format!("{} = (uint32_t)(({}) & 0x07U);", scalar, lead));
        self.line(format!("{} = 4;", width));
        self.w.dedent();
        self.line("}");
        let index = self.fresh("k");
        self.line(format!(
            "for (size_t {} = 1; {} < {} && (({}) + {}) < {}; {} += 1) {{",
            index, index, width, scan, index, length, index
        ));
        self.w.indent();
        self.line(format!(
            "{} = ({} << 6) | ((uint32_t)(({})[({}) + ({})]) & 0x3FU);",
            scalar, scalar, data, scan, index
        ));
        self.w.dedent();
        self.line("}");
        (scalar, width)
    }

    /// Emit one slice/str intrinsic call, returning false when the name is not
    /// one of the pointer-and-length intrinsics or the argument shape does not
    /// match. The caller then falls back to the legacy builtin tables.
    pub(crate) fn try_gen_slice_intrinsic(
        &mut self,
        name: &str,
        args: &[HExpr],
        ty: &Ty,
        target: &str,
        span: Span,
    ) -> bool {
        let name = name.strip_prefix("__").unwrap_or(name);
        match name {
            "slice_len" => {
                let Some(arg) = args.first() else {
                    return false;
                };
                if !is_slice_value(&arg.ty) {
                    return false;
                }
                let fat = self.fat_value(arg, "slice");
                let ctype = self.ctype(ty);
                self.line(format!("{} = ({})(({}).len);", target, ctype, fat));
                true
            }
            "slice_get" => {
                let (Some(arg), Some(index)) = (args.first(), args.get(1)) else {
                    return false;
                };
                let Some(elem) = slice_element(&arg.ty) else {
                    return false;
                };
                let fat = self.fat_value(arg, "slice");
                let position = self.new_plain_of("int64_t", "slice_index");
                let value = self.gen_owned(index);
                self.line(format!("{} = (int64_t)({});", position, value));
                let zero = self.zero_expr(ty);
                self.line(format!("{} = {};", target, zero));
                self.line(format!(
                    "if ({} >= 0 && (uint64_t)({}) < (uint64_t)(({}).len)) {{",
                    position, position, fat
                ));
                self.w.indent();
                self.line(format!("{}.{} = 1;", target, TAG_FIELD));
                let lvalue = format!("({}).data[(size_t)({})]", fat, position);
                let copied = ops::copy_expr(self.names, &elem, &lvalue);
                self.line(format!(
                    "{}.{}.{} = {};",
                    target, UNION_FIELD, OPTION_SOME_MEMBER, copied
                ));
                self.w.dedent();
                self.line("}");
                true
            }
            "slice_slice" => {
                let Some(arg) = args.first() else {
                    return false;
                };
                if !is_slice_value(&arg.ty) || args.len() < 3 {
                    return false;
                }
                self.gen_slice(
                    &args[0],
                    Some(&args[1]),
                    Some(&args[2]),
                    false,
                    ty,
                    target,
                    span,
                );
                true
            }
            "str_slice" => {
                let Some(arg) = args.first() else {
                    return false;
                };
                if !is_str_value(&arg.ty) || args.len() < 3 {
                    return false;
                }
                self.gen_slice(
                    &args[0],
                    Some(&args[1]),
                    Some(&args[2]),
                    false,
                    ty,
                    target,
                    span,
                );
                true
            }
            "str_len" => {
                let Some(arg) = args.first() else {
                    return false;
                };
                if !is_str_value(&arg.ty) {
                    return false;
                }
                let fat = self.fat_value(arg, "str");
                let ctype = self.ctype(ty);
                self.line(format!("{} = ({})(({}).len);", target, ctype, fat));
                true
            }
            "str_as_bytes" => {
                let Some(arg) = args.first() else {
                    return false;
                };
                if !is_str_value(&arg.ty) {
                    return false;
                }
                let fat = self.fat_value(arg, "str");
                let out_ctype = self.ctype(ty);
                self.line(format!(
                    "{} = ({}){{ .data = ({}).data, .len = ({}).len }};",
                    target, out_ctype, fat, fat
                ));
                true
            }
            "str_char_count" => {
                let Some(arg) = args.first() else {
                    return false;
                };
                if !is_str_value(&arg.ty) {
                    return false;
                }
                let fat = self.fat_value(arg, "str");
                let handle = self.read_only_string_view(
                    &format!("({}).data", fat),
                    &format!("(size_t)(({}).len)", fat),
                );
                self.line(format!("{} = ls_str_char_count({});", target, handle));
                true
            }
            "str_char_at" => {
                let (Some(arg), Some(index)) = (args.first(), args.get(1)) else {
                    return false;
                };
                if !is_str_value(&arg.ty) {
                    return false;
                }
                let fat = self.fat_value(arg, "str");
                let handle = self.read_only_string_view(
                    &format!("({}).data", fat),
                    &format!("(size_t)(({}).len)", fat),
                );
                let position = self.new_plain_of("int64_t", "char_index");
                let value = self.gen_owned(index);
                self.line(format!("{} = (int64_t)({});", position, value));
                let scalar = self.new_plain_of("uint32_t", "char_value");
                let present = self.new_plain_of("bool", "char_present");
                self.line(format!(
                    "{} = ls_str_char_at({}, {}, &{});",
                    present, handle, position, scalar
                ));
                let zero = self.zero_expr(ty);
                self.line(format!("{} = {};", target, zero));
                self.line(format!("if ({}) {{", present));
                self.w.indent();
                self.line(format!("{}.{} = 1;", target, TAG_FIELD));
                self.line(format!(
                    "{}.{}.{} = {};",
                    target, UNION_FIELD, OPTION_SOME_MEMBER, scalar
                ));
                self.w.dedent();
                self.line("}");
                true
            }
            "str_is_char_boundary" => {
                let (Some(arg), Some(index)) = (args.first(), args.get(1)) else {
                    return false;
                };
                if !is_str_value(&arg.ty) {
                    return false;
                }
                let fat = self.fat_value(arg, "str");
                let handle = self.read_only_string_view(
                    &format!("({}).data", fat),
                    &format!("(size_t)(({}).len)", fat),
                );
                let position = self.new_plain_of("int64_t", "byte_index");
                let value = self.gen_owned(index);
                self.line(format!("{} = (int64_t)({});", position, value));
                self.line(format!(
                    "{} = ls_str_is_char_boundary({}, {});",
                    target, handle, position
                ));
                true
            }
            "str_contains" | "str_starts_with" | "str_ends_with" => {
                let (Some(left), Some(right)) = (args.first(), args.get(1)) else {
                    return false;
                };
                if !is_str_value(&left.ty) || !is_str_value(&right.ty) {
                    return false;
                }
                let left_fat = self.fat_value(left, "str");
                let right_fat = self.fat_value(right, "str");
                let left_handle = self.read_only_string_view(
                    &format!("({}).data", left_fat),
                    &format!("(size_t)(({}).len)", left_fat),
                );
                let right_handle = self.read_only_string_view(
                    &format!("({}).data", right_fat),
                    &format!("(size_t)(({}).len)", right_fat),
                );
                let function = match name {
                    "str_contains" => "ls_str_contains",
                    "str_starts_with" => "ls_str_starts_with",
                    _ => "ls_str_ends_with",
                };
                self.line(format!(
                    "{} = {}({}, {});",
                    target, function, left_handle, right_handle
                ));
                true
            }
            "str_find" => {
                let (Some(left), Some(right)) = (args.first(), args.get(1)) else {
                    return false;
                };
                if !is_str_value(&left.ty) || !is_str_value(&right.ty) {
                    return false;
                }
                let left_fat = self.fat_value(left, "str");
                let right_fat = self.fat_value(right, "str");
                let left_handle = self.read_only_string_view(
                    &format!("({}).data", left_fat),
                    &format!("(size_t)(({}).len)", left_fat),
                );
                let right_handle = self.read_only_string_view(
                    &format!("({}).data", right_fat),
                    &format!("(size_t)(({}).len)", right_fat),
                );
                let index = self.new_plain_of("int64_t", "find_index");
                self.line(format!(
                    "{} = ls_str_find({}, {});",
                    index, left_handle, right_handle
                ));
                let zero = self.zero_expr(ty);
                self.line(format!("{} = {};", target, zero));
                self.line(format!("if ({} >= 0) {{", index));
                self.w.indent();
                self.line(format!("{}.{} = 1;", target, TAG_FIELD));
                self.line(format!(
                    "{}.{}.{} = {};",
                    target, UNION_FIELD, OPTION_SOME_MEMBER, index
                ));
                self.w.dedent();
                self.line("}");
                true
            }
            "str_split" => {
                let (Some(left), Some(right)) = (args.first(), args.get(1)) else {
                    return false;
                };
                if !is_str_value(&left.ty) || !is_str_value(&right.ty) {
                    return false;
                }
                let left_fat = self.fat_value(left, "str");
                let right_fat = self.fat_value(right, "str");
                let left_handle = self.read_only_string_view(
                    &format!("({}).data", left_fat),
                    &format!("(size_t)(({}).len)", left_fat),
                );
                let right_handle = self.read_only_string_view(
                    &format!("({}).data", right_fat),
                    &format!("(size_t)(({}).len)", right_fat),
                );
                self.line(format!(
                    "{} = ls_str_split({}, {});",
                    target, left_handle, right_handle
                ));
                true
            }
            "str_parse_i64" => {
                let Some(arg) = args.first() else {
                    return false;
                };
                if !is_str_value(&arg.ty) {
                    return false;
                }
                let fat = self.fat_value(arg, "str");
                let handle = self.read_only_string_view(
                    &format!("({}).data", fat),
                    &format!("(size_t)(({}).len)", fat),
                );
                let parsed = self.new_plain_of("int64_t", "parsed");
                let present = self.new_plain_of("bool", "parsed_ok");
                self.line(format!(
                    "{} = ls_str_parse_i64({}, &{});",
                    present, handle, parsed
                ));
                let zero = self.zero_expr(ty);
                self.line(format!("{} = {};", target, zero));
                self.line(format!("if ({}) {{", present));
                self.w.indent();
                self.line(format!("{}.{} = 1;", target, TAG_FIELD));
                self.line(format!(
                    "{}.{}.{} = {};",
                    target, UNION_FIELD, OPTION_SOME_MEMBER, parsed
                ));
                self.w.dedent();
                self.line("}");
                true
            }
            "str_parse_f64" => {
                let Some(arg) = args.first() else {
                    return false;
                };
                if !is_str_value(&arg.ty) {
                    return false;
                }
                let fat = self.fat_value(arg, "str");
                let handle = self.read_only_string_view(
                    &format!("({}).data", fat),
                    &format!("(size_t)(({}).len)", fat),
                );
                let parsed = self.new_plain_of("double", "parsed");
                let present = self.new_plain_of("bool", "parsed_ok");
                self.line(format!(
                    "{} = ls_str_parse_f64({}, &{});",
                    present, handle, parsed
                ));
                let zero = self.zero_expr(ty);
                self.line(format!("{} = {};", target, zero));
                self.line(format!("if ({}) {{", present));
                self.w.indent();
                self.line(format!("{}.{} = 1;", target, TAG_FIELD));
                self.line(format!(
                    "{}.{}.{} = {};",
                    target, UNION_FIELD, OPTION_SOME_MEMBER, parsed
                ));
                self.w.dedent();
                self.line("}");
                true
            }
            "str_to_uppercase" | "str_to_lowercase" => {
                let Some(arg) = args.first() else {
                    return false;
                };
                if !is_str_value(&arg.ty) {
                    return false;
                }
                let fat = self.fat_value(arg, "str");
                let handle = self.read_only_string_view(
                    &format!("({}).data", fat),
                    &format!("(size_t)(({}).len)", fat),
                );
                let function = if name == "str_to_uppercase" {
                    "ls_str_to_uppercase"
                } else {
                    "ls_str_to_lowercase"
                };
                self.line(format!("{} = {}({});", target, function, handle));
                true
            }
            "str_from_str" => {
                let Some(arg) = args.first() else {
                    return false;
                };
                if !is_str_value(&arg.ty) {
                    return false;
                }
                let fat = self.fat_value(arg, "str");
                self.line(format!(
                    "{} = ls_str_from_parts((const char*)(({}).data), (size_t)(({}).len));",
                    target, fat, fat
                ));
                true
            }
            "str_trim" => {
                let Some(arg) = args.first() else {
                    return false;
                };
                if !is_str_value(&arg.ty) {
                    return false;
                }
                self.gen_str_trim(arg, ty, target);
                true
            }
            "vec_as_slice" | "vec_as_mut_slice" => {
                let Some(arg) = args.first() else {
                    return false;
                };
                if !is_vector_reference(&arg.ty) {
                    return false;
                }
                let place = self.place_through_reference(arg);
                let out_ctype = self.ctype(ty);
                self.line(format!(
                    "{} = ({}){{ .data = ({}).data, .len = ({}).len }};",
                    target, out_ctype, place, place
                ));
                true
            }
            "string_as_str" | "string_as_mut_str" => {
                let Some(arg) = args.first() else {
                    return false;
                };
                if !is_string_reference(&arg.ty) {
                    return false;
                }
                let place = self.place_through_reference(arg);
                let out_ctype = self.ctype(ty);
                self.line(format!(
                    "{} = ({}){{ .data = (uint8_t*)ls_str_data({}), .len = ls_str_len({}) }};",
                    target, out_ctype, place, place
                ));
                true
            }
            _ => false,
        }
    }

    /// Trim leading and trailing Unicode White_Space scalars of a str view.
    ///
    /// The result is a borrowed view of the original bytes, so the offsets are
    /// computed inline: a generated helper function would be emitted into every
    /// program even when it is unused. The scalar decoder is the standard UTF-8
    /// length lookup, valid because the runtime only stores valid UTF-8.
    fn gen_str_trim(&mut self, arg: &HExpr, ty: &Ty, target: &str) {
        let fat = self.fat_value(arg, "trim");
        let data = self.new_plain_of("const uint8_t*", "trim_data");
        self.line(format!("{} = (const uint8_t*)(({}).data);", data, fat));
        let length = self.new_plain_of("size_t", "trim_len");
        self.line(format!("{} = (size_t)(({}).len);", length, fat));
        let scan = self.new_plain_of("size_t", "trim_scan");
        let start = self.new_plain_of("size_t", "trim_start");
        let end = self.new_plain_of("size_t", "trim_end");
        let seen = self.new_plain_of("bool", "trim_seen");
        self.line(format!("while ({} < {}) {{", scan, length));
        self.w.indent();
        let (scalar, width) = self.emit_utf8_scalar(&data, &scan, &length);
        self.line(format!("if (!{}) {{", whitespace_expression(&scalar)));
        self.w.indent();
        self.line(format!("if (!{}) {{", seen));
        self.w.indent();
        self.line(format!("{} = {};", start, scan));
        self.line(format!("{} = true;", seen));
        self.w.dedent();
        self.line("}");
        self.line(format!("{} = {} + {};", end, scan, width));
        self.w.dedent();
        self.line("}");
        self.line(format!("{} += {};", scan, width));
        self.w.dedent();
        self.line("}");
        self.line(format!("if (!{}) {{", seen));
        self.w.indent();
        self.line(format!("{} = 0;", start));
        self.line(format!("{} = 0;", end));
        self.w.dedent();
        self.line("}");
        let out_ctype = self.ctype(ty);
        self.line(format!(
            "{} = ({}){{ .data = (({}) != NULL ? ((uint8_t*)({})) + ({}) : (uint8_t*)NULL), .len = ({}) - ({}) }};",
            target, out_ctype, data, data, start, end, start
        ));
    }
}
