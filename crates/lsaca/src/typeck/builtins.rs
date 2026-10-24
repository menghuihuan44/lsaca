//! Built-in method, associated function and free function signatures.
//!
//! The signature tables are derived from docs/LANGUAGE.md. Generic element
//! types are substituted with the receiver type's parameters.

use super::itype::ITy;
use crate::hir::{FormatAlign, FormatSpec, HBuiltinAssoc, HBuiltinFn, HBuiltinMethod};

/// A resolved built-in method signature.
#[derive(Clone, Debug)]
pub struct BuiltinMethodInfo {
    pub method: HBuiltinMethod,
    pub params: Vec<ITy>,
    pub ret: ITy,
    pub mutable_receiver: bool,
}

fn method(method: HBuiltinMethod, params: Vec<ITy>, ret: ITy) -> BuiltinMethodInfo {
    BuiltinMethodInfo {
        method,
        params,
        ret,
        mutable_receiver: false,
    }
}

fn mutating(method: HBuiltinMethod, params: Vec<ITy>, ret: ITy) -> BuiltinMethodInfo {
    BuiltinMethodInfo {
        method,
        params,
        ret,
        mutable_receiver: true,
    }
}

fn i64() -> ITy {
    ITy::i64()
}

/// Look up a built-in method on a resolved receiver type.
pub fn builtin_method(recv: &ITy, name: &str) -> Option<BuiltinMethodInfo> {
    match recv {
        ITy::String => string_method(name),
        ITy::Vec(elem) => vec_method(elem, name),
        ITy::Rc(elem) => rc_method(elem, name),
        ITy::Box(elem) => box_method(elem, name),
        ITy::Option(elem) => option_method(elem, name),
        ITy::Result(ok, err) => result_method(ok, err, name),
        _ => None,
    }
}

fn string_method(name: &str) -> Option<BuiltinMethodInfo> {
    use HBuiltinMethod::*;
    let info = match name {
        "len" => method(StrLen, vec![], i64()),
        "is_empty" => method(StrIsEmpty, vec![], ITy::bool_()),
        "push_str" => mutating(StrPushStr, vec![ITy::string()], ITy::Unit),
        "clear" => mutating(StrClear, vec![], ITy::Unit),
        "contains" => method(StrContains, vec![ITy::string()], ITy::bool_()),
        "starts_with" => method(StrStartsWith, vec![ITy::string()], ITy::bool_()),
        "ends_with" => method(StrEndsWith, vec![ITy::string()], ITy::bool_()),
        "find" => method(StrFind, vec![ITy::string()], ITy::option(i64())),
        "replace" => method(
            StrReplace,
            vec![ITy::string(), ITy::string()],
            ITy::string(),
        ),
        "to_uppercase" => method(StrToUppercase, vec![], ITy::string()),
        "to_lowercase" => method(StrToLowercase, vec![], ITy::string()),
        "trim" => method(StrTrim, vec![], ITy::string()),
        "split" => method(StrSplit, vec![ITy::string()], ITy::vec(ITy::string())),
        "substring" => method(StrSubstring, vec![i64(), i64()], ITy::string()),
        "repeat" => method(StrRepeat, vec![i64()], ITy::string()),
        "parse_i64" => method(StrParseI64, vec![], ITy::result(i64(), ITy::string())),
        "parse_f64" => method(StrParseF64, vec![], ITy::result(ITy::f64(), ITy::string())),
        _ => return None,
    };
    Some(info)
}

fn vec_method(elem: &ITy, name: &str) -> Option<BuiltinMethodInfo> {
    use HBuiltinMethod::*;
    let elem = elem.clone();
    let info = match name {
        "len" => method(VecLen, vec![], i64()),
        "is_empty" => method(VecIsEmpty, vec![], ITy::bool_()),
        "push" => mutating(VecPush, vec![elem], ITy::Unit),
        "pop" => mutating(VecPop, vec![], ITy::option(elem)),
        "insert" => mutating(VecInsert, vec![i64(), elem], ITy::Unit),
        "remove" => mutating(VecRemove, vec![i64()], elem),
        "clear" => mutating(VecClear, vec![], ITy::Unit),
        "first" => method(VecFirst, vec![], ITy::option(elem)),
        "last" => method(VecLast, vec![], ITy::option(elem)),
        "get" => method(VecGet, vec![i64()], ITy::option(elem)),
        "extend" => mutating(VecExtend, vec![ITy::vec(elem)], ITy::Unit),
        "reverse" => mutating(VecReverse, vec![], ITy::Unit),
        _ => return None,
    };
    Some(info)
}

fn rc_method(elem: &ITy, name: &str) -> Option<BuiltinMethodInfo> {
    use HBuiltinMethod::*;
    let elem = elem.clone();
    let info = match name {
        "get" => method(RcGet, vec![], elem),
        // The cell is shared and mutated through the handle, so `set` needs
        // only a shared (immutable) binding, not a mutable one.
        "set" => method(RcSet, vec![elem], ITy::Unit),
        "strong_count" => method(RcStrongCount, vec![], i64()),
        _ => return None,
    };
    Some(info)
}

fn box_method(elem: &ITy, name: &str) -> Option<BuiltinMethodInfo> {
    use HBuiltinMethod::*;
    let elem = elem.clone();
    let info = match name {
        // `Box<T>` is an owned value, so `get` copies the payload out and
        // leaves the box untouched.
        "get" => method(BoxGet, vec![], elem),
        // `set` replaces the boxed payload and therefore needs a mutable
        // binding, like any other value mutation.
        "set" => mutating(BoxSet, vec![elem], ITy::Unit),
        _ => return None,
    };
    Some(info)
}

fn option_method(elem: &ITy, name: &str) -> Option<BuiltinMethodInfo> {
    use HBuiltinMethod::*;
    let elem = elem.clone();
    let info = match name {
        "is_some" => method(OptIsSome, vec![], ITy::bool_()),
        "is_none" => method(OptIsNone, vec![], ITy::bool_()),
        "unwrap" => method(OptUnwrap, vec![], elem),
        "unwrap_or" => method(OptUnwrapOr, vec![elem.clone()], elem),
        "expect" => method(OptExpect, vec![ITy::string()], elem),
        _ => return None,
    };
    Some(info)
}

fn result_method(ok: &ITy, err: &ITy, name: &str) -> Option<BuiltinMethodInfo> {
    use HBuiltinMethod::*;
    let ok = ok.clone();
    let err = err.clone();
    let info = match name {
        "is_ok" => method(ResIsOk, vec![], ITy::bool_()),
        "is_err" => method(ResIsErr, vec![], ITy::bool_()),
        "unwrap" => method(ResUnwrap, vec![], ok),
        "unwrap_or" => method(ResUnwrapOr, vec![ok.clone()], ok),
        "expect" => method(ResExpect, vec![ITy::string()], ok),
        "ok" => method(ResOk, vec![], ITy::option(ok)),
        "err" => method(ResErr, vec![], ITy::option(err)),
        _ => return None,
    };
    Some(info)
}

/// The turbofish type argument accepted by String::parse, if any.
pub fn parse_alias(target: &ITy) -> Option<(HBuiltinMethod, ITy)> {
    match target {
        ITy::Int(crate::types::IntTy::I64) => Some((
            HBuiltinMethod::StrParseI64,
            ITy::result(ITy::i64(), ITy::string()),
        )),
        ITy::Float(crate::types::FloatTy::F64) => Some((
            HBuiltinMethod::StrParseF64,
            ITy::result(ITy::f64(), ITy::string()),
        )),
        _ => None,
    }
}

/// Look up a free built-in function by name.
pub fn builtin_fn(name: &str) -> Option<HBuiltinFn> {
    use HBuiltinFn::*;
    let func = match name {
        "sqrt" => Sqrt,
        "abs" => Abs,
        "min" => Min,
        "max" => Max,
        "pow" => Pow,
        "floor" => Floor,
        "ceil" => Ceil,
        "round" => Round,
        "sin" => Sin,
        "cos" => Cos,
        "tan" => Tan,
        "ln" => Ln,
        "log10" => Log10,
        "exp" => Exp,
        "read_line" => ReadLine,
        "args" => Args,
        "exit" => Exit,
        "read_file" => ReadFile,
        "write_file" => WriteFile,
        _ => return None,
    };
    Some(func)
}

/// Look up an associated function on a built-in type.
pub fn builtin_assoc(type_name: &str, name: &str) -> Option<HBuiltinAssoc> {
    use HBuiltinAssoc::*;
    match (type_name, name) {
        ("String", "new") => Some(StringNew),
        ("String", "from") => Some(StringFrom),
        ("Vec", "new") => Some(VecNew),
        ("Vec", "with_capacity") => Some(VecWithCapacity),
        ("Rc", "new") => Some(RcNew),
        ("Box", "new") => Some(BoxNew),
        _ => None,
    }
}

/// A piece of a parsed format string.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FormatPiece {
    Literal(String),
    Hole { spec: FormatSpec },
}

/// Largest accepted field width or precision.
const MAX_FORMAT_FIELD: i64 = 10_000;

fn parse_format_align(ch: char) -> FormatAlign {
    match ch {
        '<' => FormatAlign::Left,
        '^' => FormatAlign::Center,
        _ => FormatAlign::Right,
    }
}

/// Parse the text between `{` and `}` into a [FormatSpec].
///
/// The accepted grammar is
/// `[fill]align|align][+][0][width][.precision][?]`, with an optional leading
/// `` carried over from the historical `{:?}` spelling.
fn parse_format_spec(raw: &str) -> Result<FormatSpec, String> {
    let rest = raw.strip_prefix(':').unwrap_or(raw);
    let mut spec = FormatSpec::plain();
    let rest = match rest.strip_suffix('?') {
        Some(stripped) => {
            spec.debug = true;
            stripped
        }
        None => rest,
    };
    let chars: Vec<char> = rest.chars().collect();
    let unsupported = || {
        format!(
            "unsupported format specifier '{{{}}}' in format string",
            raw
        )
    };
    let mut index = 0;

    // [fill]align
    if chars.len() >= 2 && matches!(chars[1], '<' | '^' | '>') {
        if chars[0] == '{' || chars[0] == '}' {
            return Err(unsupported());
        }
        spec.fill = chars[0];
        spec.align = Some(parse_format_align(chars[1]));
        index = 2;
    } else if index < chars.len() && matches!(chars[index], '<' | '^' | '>') {
        spec.align = Some(parse_format_align(chars[index]));
        index += 1;
    }

    // sign
    if index < chars.len() && chars[index] == '+' {
        spec.sign_plus = true;
        index += 1;
    }

    // zero padding flag
    if index < chars.len() && chars[index] == '0' {
        let has_width = index + 1 < chars.len() && chars[index + 1].is_ascii_digit();
        if !has_width {
            return Err("the '0' flag in a format specifier needs a width".to_string());
        }
        spec.zero_pad = true;
        index += 1;
    }

    // width
    let width_start = index;
    while index < chars.len() && chars[index].is_ascii_digit() {
        index += 1;
    }
    if index > width_start {
        let digits: String = chars[width_start..index].iter().collect();
        let width = digits
            .parse::<i64>()
            .map_err(|_| "format width is too large".to_string())?;
        if width > MAX_FORMAT_FIELD {
            return Err(format!("format width must be at most {}", MAX_FORMAT_FIELD));
        }
        spec.width = Some(width);
    }

    // precision
    if index < chars.len() && chars[index] == '.' {
        index += 1;
        let precision_start = index;
        while index < chars.len() && chars[index].is_ascii_digit() {
            index += 1;
        }
        if index == precision_start {
            return Err("the precision of a format specifier needs a number".to_string());
        }
        let digits: String = chars[precision_start..index].iter().collect();
        let precision = digits
            .parse::<i64>()
            .map_err(|_| "format precision is too large".to_string())?;
        if precision > MAX_FORMAT_FIELD {
            return Err(format!(
                "format precision must be at most {}",
                MAX_FORMAT_FIELD
            ));
        }
        spec.precision = Some(precision);
    }

    if index != chars.len() {
        return Err(unsupported());
    }
    Ok(spec)
}

/// The category of a format argument, after literal inference variables are
/// classified as integers or floats.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormatValueKind {
    String,
    Bool,
    SignedInt,
    UnsignedInt,
    Float,
    /// A compound value (struct, enum, tuple, Vec, ...) formatted through its
    /// debug helper.
    Other,
}

/// Check a format specification against the category of its argument.
///
/// Returns a diagnostic message when the specification cannot be applied to
/// the value; `None` when the combination is valid.
pub fn format_spec_error(spec: &FormatSpec, kind: FormatValueKind) -> Option<String> {
    let is_string = kind == FormatValueKind::String;
    let is_float = kind == FormatValueKind::Float;
    let is_signed_int = kind == FormatValueKind::SignedInt;
    let is_unsigned_int = kind == FormatValueKind::UnsignedInt;
    let is_numeric = is_signed_int || is_unsigned_int || is_float;

    if spec.precision.is_some() {
        if spec.debug {
            return Some("precision cannot be combined with debug formatting ('?')".to_string());
        }
        if !is_float && !is_string {
            return Some("precision is only supported for floats and strings".to_string());
        }
    }
    if spec.sign_plus && !is_signed_int && !is_float {
        if is_unsigned_int {
            return Some("the '+' sign flag is not supported for unsigned integers".to_string());
        }
        return Some(
            "the '+' sign flag is only supported for signed integers and floats".to_string(),
        );
    }
    if spec.zero_pad && !is_numeric {
        return Some("the '0' flag is only supported for numeric values".to_string());
    }
    None
}

/// Parse a macro format string into literal chunks and placeholders.
pub fn parse_format_string(text: &str) -> Result<Vec<FormatPiece>, String> {
    let mut pieces = Vec::new();
    let mut literal = String::new();
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '{' => {
                if chars.peek() == Some(&'{') {
                    chars.next();
                    literal.push('{');
                    continue;
                }
                let mut spec = String::new();
                let mut closed = false;
                for inner in chars.by_ref() {
                    if inner == '}' {
                        closed = true;
                        break;
                    }
                    spec.push(inner);
                }
                if !closed {
                    return Err("unclosed '{' in format string".to_string());
                }
                let spec = parse_format_spec(&spec)?;
                if !literal.is_empty() {
                    pieces.push(FormatPiece::Literal(std::mem::take(&mut literal)));
                }
                pieces.push(FormatPiece::Hole { spec });
            }
            '}' => {
                if chars.peek() == Some(&'}') {
                    chars.next();
                    literal.push('}');
                    continue;
                }
                return Err("unmatched '}' in format string".to_string());
            }
            other => literal.push(other),
        }
    }
    if !literal.is_empty() {
        pieces.push(FormatPiece::Literal(literal));
    }
    Ok(pieces)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(raw: &str) -> FormatSpec {
        parse_format_spec(raw).unwrap_or_else(|error| panic!("{:?}: {}", raw, error))
    }

    #[test]
    fn parses_plain_and_debug_holes() {
        assert_eq!(spec(""), FormatSpec::plain());
        assert_eq!(spec(":"), FormatSpec::plain());
        assert_eq!(
            spec("?"),
            FormatSpec {
                debug: true,
                ..FormatSpec::plain()
            }
        );
        // The historical "colon question mark" spelling is still accepted.
        assert_eq!(spec(":?"), spec("?"));
        assert!(!spec(":?").is_plain());
    }

    #[test]
    fn parses_alignment_fill_and_width() {
        let right = spec(":>8");
        assert_eq!(right.align, Some(FormatAlign::Right));
        assert_eq!(right.width, Some(8));

        let bare_width = spec("8");
        assert_eq!(bare_width.align, None);
        assert_eq!(bare_width.width, Some(8));

        let centered = spec(":*^9");
        assert_eq!(centered.fill, '*');
        assert_eq!(centered.align, Some(FormatAlign::Center));
        assert_eq!(centered.width, Some(9));

        let left = spec(":<5");
        assert_eq!(left.fill, ' ');
        assert_eq!(left.align, Some(FormatAlign::Left));
        assert_eq!(left.width, Some(5));
    }

    #[test]
    fn parses_sign_zero_padding_and_precision() {
        let combined = spec(":+08.2");
        assert!(combined.sign_plus);
        assert!(combined.zero_pad);
        assert_eq!(combined.width, Some(8));
        assert_eq!(combined.precision, Some(2));

        let precision_only = spec(":.0");
        assert_eq!(precision_only.precision, Some(0));

        let width_and_precision = spec(":12.3");
        assert_eq!(width_and_precision.width, Some(12));
        assert_eq!(width_and_precision.precision, Some(3));
        assert!(!width_and_precision.debug);
    }

    #[test]
    fn rejects_invalid_format_specifications() {
        let cases: &[(&str, &str)] = &[
            ("w", "unsupported format specifier"),
            ("0", "needs a width"),
            (".", "needs a number"),
            ("10001", "at most 10000"),
            (".10001", "at most 10000"),
            ("{>3", "unsupported format specifier"),
        ];
        for (raw, message) in cases {
            let error = parse_format_spec(raw).expect_err(raw);
            assert!(
                error.contains(message),
                "{:?} produced {:?}, expected {:?}",
                raw,
                error,
                message
            );
        }
    }

    #[test]
    fn parses_format_strings_into_literal_chunks_and_holes() {
        let pieces = parse_format_string("a{{b}}c{:>4}").expect("valid format string");
        assert_eq!(pieces.len(), 2);
        assert!(matches!(&pieces[0], FormatPiece::Literal(text) if text == "a{b}c"));
        assert!(matches!(
            &pieces[1],
            FormatPiece::Hole { spec } if spec.width == Some(4) && spec.align == Some(FormatAlign::Right)
        ));
    }

    #[test]
    fn parses_two_holes_in_one_format_string() {
        let pieces = parse_format_string("{:>3}-{:.1}").expect("valid format string");
        assert_eq!(pieces.len(), 3);
        assert!(matches!(&pieces[0], FormatPiece::Hole { spec } if spec.width == Some(3)));
        assert!(matches!(&pieces[1], FormatPiece::Literal(text) if text == "-"));
        assert!(matches!(&pieces[2], FormatPiece::Hole { spec } if spec.precision == Some(1)));
    }

    #[test]
    fn validates_specifications_against_value_kinds() {
        use FormatValueKind::*;
        // Valid combinations.
        assert_eq!(format_spec_error(&spec(":.2"), Float), None);
        assert_eq!(format_spec_error(&spec(":.3"), String), None);
        assert_eq!(format_spec_error(&spec(":+"), SignedInt), None);
        assert_eq!(format_spec_error(&spec(":08"), SignedInt), None);
        assert_eq!(format_spec_error(&spec(":08.2"), Float), None);
        assert_eq!(format_spec_error(&spec(":*^9"), Bool), None);
        assert_eq!(format_spec_error(&spec(":>8?"), Other), None);

        // Invalid combinations.
        let precision = format_spec_error(&spec(":.2"), SignedInt).expect("precision on int");
        assert!(precision.contains("precision is only supported"));
        let debug_precision =
            format_spec_error(&spec(":.2?"), Float).expect("precision with debug");
        assert!(debug_precision.contains("debug formatting"));
        let unsigned = format_spec_error(&spec(":+"), UnsignedInt).expect("plus on unsigned");
        assert!(unsigned.contains("unsigned integers"));
        let plus_string = format_spec_error(&spec(":+"), String).expect("plus on string");
        assert!(plus_string.contains("signed integers and floats"));
        let zero_string = format_spec_error(&spec(":05"), String).expect("zero on string");
        assert!(zero_string.contains("numeric values"));
    }
}
