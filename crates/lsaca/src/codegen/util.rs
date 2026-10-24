//! Small helpers shared by the code generator: C identifier and string
//! literal escaping, numeric literal formatting and file/location handling.
//!
//! Everything here is pure and deterministic; the code generator never relies
//! on hash map iteration order.

use crate::span::SourceFile;
use crate::types::{FloatTy, IntTy};

/// Sanitize a name so that it can be used as a C identifier suffix.
pub fn sanitize_ident(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for (index, ch) in name.chars().enumerate() {
        let ok = ch.is_ascii_alphanumeric() || ch == '_';
        if ok {
            if index == 0 && ch.is_ascii_digit() {
                out.push('_');
            }
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        out.push('_');
    }
    out
}

/// Push one byte as a C character escape sequence (no surrounding quotes).
fn push_byte_escape(out: &mut String, byte: u8) {
    match byte {
        b'\\' => out.push_str("\\\\"),
        b'"' => out.push_str("\\\""),
        b'\'' => out.push_str("\\'"),
        b'?' => out.push_str("\\?"),
        b'\n' => out.push_str("\\n"),
        b'\r' => out.push_str("\\r"),
        b'\t' => out.push_str("\\t"),
        0x20..=0x7e => out.push(byte as char),
        _ => {
            // Octal escapes are exactly three digits, so they can never run
            // into a following digit (unlike hexadecimal escapes).
            out.push('\\');
            out.push((b'0' + (byte >> 6)) as char);
            out.push((b'0' + ((byte >> 3) & 0o7)) as char);
            out.push((b'0' + (byte & 0o7)) as char);
        }
    }
}

/// Render a Rust string as a C string literal (including the quotes).
///
/// The result can be handed to ls_str_from_parts together with the byte length
/// of the source string; embedded NUL bytes are preserved.
pub fn c_string_literal(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for &byte in text.as_bytes() {
        push_byte_escape(&mut out, byte);
    }
    out.push('"');
    out
}

/// Render an integer literal as a C expression of the given integer type.
pub fn int_literal(value: i128, ty: IntTy) -> String {
    let c_ty = ty.c_type();
    if ty.is_signed() {
        let (min, _) = ty.range();
        if value == min {
            return format!("(({}){})", c_ty, min_c_macro(ty));
        }
        format!("(({}){})", c_ty, value)
    } else {
        let (_, max) = ty.range();
        if value > max || value < 0 {
            // Defensive: a well typed literal always fits. Wrap it the way the
            // target type would so that code generation never panics.
            let wrapped = (value as u64) as u128;
            format!("(({})0x{:016X}ULL)", c_ty, wrapped & 0xffff_ffff_ffff_ffff)
        } else if value > i128::from(i64::MAX) {
            format!("(({})0x{:016X}ULL)", c_ty, value)
        } else {
            format!("(({}){}u)", c_ty, value)
        }
    }
}

fn min_c_macro(ty: IntTy) -> &'static str {
    match ty {
        IntTy::I8 => "INT8_MIN",
        IntTy::I16 => "INT16_MIN",
        IntTy::I32 => "INT32_MIN",
        IntTy::I64 => "INT64_MIN",
        _ => "0",
    }
}

/// Render a floating point literal as a C expression of the given type.
///
/// Non-finite values use the standard math.h macros, so generated projects
/// include <math.h>.
pub fn float_literal(value: f64, ty: FloatTy) -> String {
    let c_ty = ty.c_type();
    if value.is_nan() {
        return format!("(({})NAN)", c_ty);
    }
    if value.is_infinite() {
        let sign = if value.is_sign_negative() { "-" } else { "" };
        return format!("(({}){}INFINITY)", c_ty, sign);
    }
    // Rust's Debug formatting for f64 is the shortest representation that
    // round-trips; C parses it as a double and the cast rounds it to the
    // target type.
    format!("(({}){:?})", c_ty, value)
}

/// The escaped C file name plus resolved line/column for panic call sites.
pub fn source_location(
    source: Option<&SourceFile>,
    fallback_name: &str,
    span: crate::span::Span,
) -> (String, u32, u32) {
    match source {
        Some(file) => {
            let (line, col) = file.line_col(span.start);
            let line = line.min(u32::MAX as usize) as u32;
            let col = col.min(u32::MAX as usize) as u32;
            (c_string_literal(file.name()), line, col)
        }
        None => (c_string_literal(fallback_name), 0, 0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_c_strings() {
        assert_eq!(c_string_literal("a b"), "\"a b\"");
        assert_eq!(c_string_literal("a\"b"), "\"a\\\"b\"");
        assert_eq!(c_string_literal("a\\b"), "\"a\\\\b\"");
        assert_eq!(c_string_literal("\n"), "\"\\n\"");
        assert_eq!(c_string_literal("\u{00e9}"), "\"\\303\\251\"");
        assert_eq!(c_string_literal("\0"), "\"\\000\"");
    }

    #[test]
    fn formats_int_literals() {
        assert_eq!(int_literal(42, IntTy::I64), "((int64_t)42)");
        assert_eq!(int_literal(-1, IntTy::I32), "((int32_t)-1)");
        assert_eq!(
            int_literal(i128::from(i64::MIN), IntTy::I64),
            "((int64_t)INT64_MIN)"
        );
        assert_eq!(
            int_literal(18446744073709551615u128 as i128, IntTy::U64),
            "((uint64_t)0xFFFFFFFFFFFFFFFFULL)"
        );
    }

    #[test]
    fn formats_float_literals() {
        assert_eq!(float_literal(1.5, FloatTy::F64), "((double)1.5)");
        assert_eq!(float_literal(2.0, FloatTy::F32), "((float)2.0)");
        assert_eq!(float_literal(f64::NAN, FloatTy::F64), "((double)NAN)");
        assert_eq!(
            float_literal(f64::INFINITY, FloatTy::F64),
            "((double)INFINITY)"
        );
    }

    #[test]
    fn sanitizes_identifiers() {
        assert_eq!(sanitize_ident("Point"), "Point");
        assert_eq!(sanitize_ident("2bad"), "_2bad");
        assert_eq!(sanitize_ident("a-b"), "a_b");
        assert_eq!(sanitize_ident(""), "_");
    }
}
