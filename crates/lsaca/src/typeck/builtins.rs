//! Built-in method, associated function and free function signatures.
//!
//! The signature tables are derived from docs/LANGUAGE.md. Generic element
//! types are substituted with the receiver type's parameters.

use super::itype::ITy;
use crate::hir::{HBuiltinAssoc, HBuiltinFn, HBuiltinMethod};

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
        _ => None,
    }
}

/// A piece of a parsed format string.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FormatPiece {
    Literal(String),
    Hole { debug: bool },
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
                let debug = match spec.as_str() {
                    "" => false,
                    "?" | ":?" => true,
                    other => {
                        return Err(format!(
                            "unsupported format specifier '{{{}}}' in format string",
                            other
                        ));
                    }
                };
                if !literal.is_empty() {
                    pieces.push(FormatPiece::Literal(std::mem::take(&mut literal)));
                }
                pieces.push(FormatPiece::Hole { debug });
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
