//! The high-level intermediate representation.
//!
//! The HIR is produced by the type checker ([crate::typeck]) and consumed by
//! the C code generator ([crate::codegen]). It is a fully resolved tree:
//!
//! * every name is resolved to a local variable, a function, a constant or a
//!   builtin;
//! * every expression carries its concrete [Ty];
//! * struct and enum types are referenced by index into [Program::structs] and
//!   [Program::enums];
//! * syntactic sugar (method calls, macros, impl blocks, type aliases, nested
//!   items) has been lowered away.
//!
//! Ownership is explicit enough for the code generator: every owned value is
//! either a temporary created in place, a copy of a place, or a moved return
//! value. The code generator tracks those with the help of [Ty::is_bitwise_copy].

use std::fmt;

use crate::span::Span;
use crate::types::{FloatTy, IntTy, SelfKind, Ty};

pub type StructId = usize;
pub type EnumId = usize;
pub type FnId = usize;

/// A whole checked program.
#[derive(Debug, Clone)]
pub struct Program {
    /// Stem of the source file, used for the generated executable name.
    pub name: String,
    pub structs: Vec<HStruct>,
    pub enums: Vec<HEnum>,
    pub fns: Vec<HFn>,
    pub consts: Vec<HConst>,
    pub closures: Vec<HClosure>,
    /// Index of the user entry point: fn main().
    pub main_fn: FnId,
}

impl Program {
    /// Display name of a type, resolving struct and enum ids.
    pub fn type_name(&self, ty: &Ty) -> String {
        match ty {
            Ty::Struct(id) => self
                .structs
                .get(*id)
                .map(|item| item.name.clone())
                .unwrap_or_else(|| format!("struct#{}", id)),
            Ty::Enum(id) => self
                .enums
                .get(*id)
                .map(|item| item.name.clone())
                .unwrap_or_else(|| format!("enum#{}", id)),
            Ty::Vec(inner) => format!("Vec<{}>", self.type_name(inner)),
            Ty::Option(inner) => format!("Option<{}>", self.type_name(inner)),
            Ty::Result(ok, err) => {
                format!("Result<{}, {}>", self.type_name(ok), self.type_name(err))
            }
            Ty::Tuple(elems) => {
                let inner = elems
                    .iter()
                    .map(|elem| self.type_name(elem))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("({})", inner)
            }
            Ty::Fn(params, ret) => {
                let params = params
                    .iter()
                    .map(|param| self.type_name(param))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("fn({}) -> {}", params, self.type_name(ret))
            }
            other => format!("{}", other),
        }
    }

    pub fn struct_name(&self, id: StructId) -> &str {
        self.structs
            .get(id)
            .map(|item| item.name.as_str())
            .unwrap_or("<invalid struct>")
    }

    pub fn enum_name(&self, id: EnumId) -> &str {
        self.enums
            .get(id)
            .map(|item| item.name.as_str())
            .unwrap_or("<invalid enum>")
    }

    pub fn variant_name(&self, enum_id: EnumId, variant: usize) -> &str {
        self.enums
            .get(enum_id)
            .and_then(|item| item.variants.get(variant))
            .map(|item| item.name.as_str())
            .unwrap_or("<invalid variant>")
    }
}

/// A user struct with named fields.
#[derive(Debug, Clone)]
pub struct HStruct {
    pub name: String,
    pub fields: Vec<HField>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HField {
    pub name: String,
    pub ty: Ty,
}

/// A user enum.
#[derive(Debug, Clone)]
pub struct HEnum {
    pub name: String,
    pub variants: Vec<HVariant>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HVariant {
    pub name: String,
    /// Index of the variant inside the enum, used as the C tag value.
    pub tag: usize,
    /// Payload types; empty for a unit variant.
    pub fields: Vec<Ty>,
}

/// A checked function or method.
#[derive(Debug, Clone)]
pub struct HFn {
    /// Display name: "main", "add", "Point::new", "Point::len".
    pub name: String,
    /// Unique C identifier for the generated function, for example
    /// lsaca_fn_add or lsaca_fn_Point_len.
    pub c_name: String,
    pub params: Vec<HParam>,
    pub ret: Ty,
    /// None for free functions and associated functions; Some for methods.
    pub self_kind: Option<SelfKind>,
    pub body: HBlock,
    pub span: Span,
}

impl HFn {
    /// Whether the first parameter is the receiver.
    pub fn is_method(&self) -> bool {
        self.self_kind.is_some()
    }
}

#[derive(Debug, Clone)]
pub struct HParam {
    pub name: String,
    /// Unique C identifier of the variable backing this parameter.
    pub c_name: String,
    pub ty: Ty,
}

/// A checked constant.
#[derive(Debug, Clone)]
pub struct HConst {
    pub name: String,
    pub c_name: String,
    pub ty: Ty,
    pub value: ConstValue,
    pub span: Span,
}

/// A compile time constant value. Only scalar constants are representable;
/// String constants are stored as C string literals and materialized on use.
#[derive(Debug, Clone, PartialEq)]
pub enum ConstValue {
    Int(i128),
    Float(f64),
    Bool(bool),
    Str(String),
    Unit,
}

/// A block with an optional tail expression whose value becomes the block's
/// value.
#[derive(Debug, Clone)]
pub struct HBlock {
    pub stmts: Vec<HStmt>,
    pub tail: Option<Box<HExpr>>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum HStmt {
    Let {
        pat: HPat,
        init: Option<HExpr>,
        span: Span,
    },
    /// Evaluate an expression and discard its value.
    Expr(HExpr),
}

/// A checked expression.
#[derive(Debug, Clone)]
pub struct HExpr {
    pub ty: Ty,
    pub kind: HExprKind,
    pub span: Span,
}

impl HExpr {
    pub fn new(ty: Ty, kind: HExprKind, span: Span) -> HExpr {
        HExpr { ty, kind, span }
    }

    pub fn is_place(&self) -> bool {
        matches!(
            self.kind,
            HExprKind::Local { .. }
                | HExprKind::Capture { .. }
                | HExprKind::Field { .. }
                | HExprKind::Index { .. }
        )
    }
}

/// One arm of a match expression.
#[derive(Debug, Clone)]
pub struct HArm {
    pub pat: HPat,
    /// Optional if guard: evaluated after the pattern has bound its variables.
    pub guard: Option<HExpr>,
    pub body: HExpr,
}

#[derive(Debug, Clone)]
pub enum HExprKind {
    Lit(HLit),
    /// A read of a local variable. The reader owns a copy: owned types are
    /// retained by the code generator.
    Local {
        name: String,
        c_name: String,
    },
    /// A read of a captured variable from the current closure environment.
    Capture {
        index: usize,
        name: String,
    },
    /// A read of a constant.
    ConstRef {
        c_name: String,
    },
    /// A named function used as a value.
    FnRef {
        fn_id: FnId,
    },
    /// A closure value: the closure id selects a [HClosure] in the program.
    /// The capture initializers are evaluated in the enclosing environment and
    /// moved into the closure environment.
    Closure {
        closure_id: usize,
        captures: Vec<HExpr>,
    },
    /// An indirect call through a function-typed value.
    Call {
        callee: Box<HExpr>,
        args: Vec<HExpr>,
    },
    /// A direct call of a known function or method. For methods the receiver
    /// is a place expression and is passed as a pointer.
    DirectCall {
        fn_id: FnId,
        receiver: Option<Box<HExpr>>,
        args: Vec<HExpr>,
    },
    /// A call of a builtin (monomorphized) method.
    BuiltinMethod {
        receiver: Box<HExpr>,
        method: HBuiltinMethod,
        args: Vec<HExpr>,
    },
    /// A call of a builtin associated function such as Vec::new.
    BuiltinAssoc {
        func: HBuiltinAssoc,
        args: Vec<HExpr>,
        type_args: Vec<Ty>,
    },
    /// A call of a builtin free function such as sqrt or read_line.
    BuiltinCall {
        func: HBuiltinFn,
        args: Vec<HExpr>,
    },
    Field {
        base: Box<HExpr>,
        field: HFieldRef,
    },
    Index {
        base: Box<HExpr>,
        index: Box<HExpr>,
    },
    Unary {
        op: HUnaryOp,
        expr: Box<HExpr>,
    },
    Binary {
        op: HBinaryOp,
        lhs: Box<HExpr>,
        rhs: Box<HExpr>,
    },
    /// Assignment to a place expression. The expression has unit type.
    Assign {
        target: Box<HExpr>,
        /// None for =, Some(op) for compound assignments such as +=.
        op: Option<HBinaryOp>,
        value: Box<HExpr>,
    },
    /// A numeric or bool cast. Source type is expr.ty, target type is the
    /// enclosing expression's ty.
    Cast {
        expr: Box<HExpr>,
    },
    Block(HBlock),
    If {
        cond: Box<HExpr>,
        then_block: HBlock,
        else_expr: Option<Box<HExpr>>,
    },
    Match {
        scrutinee: Box<HExpr>,
        arms: Vec<HArm>,
    },
    Loop {
        body: HBlock,
    },
    While {
        cond: Box<HExpr>,
        body: HBlock,
    },
    ForRange {
        pat: HPat,
        start: Box<HExpr>,
        end: Box<HExpr>,
        inclusive: bool,
        body: HBlock,
    },
    ForVec {
        pat: HPat,
        vec: Box<HExpr>,
        body: HBlock,
    },
    StructLit {
        struct_id: StructId,
        /// Field index and value, in source order.
        fields: Vec<(usize, HExpr)>,
    },
    Tuple(Vec<HExpr>),
    VecLit {
        elems: Vec<HExpr>,
    },
    VecRepeat {
        value: Box<HExpr>,
        count: Box<HExpr>,
    },
    /// A user-defined enum variant value: Shape::Circle(1.0), Color::Red.
    /// Unit variants have an empty args vector.
    EnumLit {
        enum_id: EnumId,
        variant: usize,
        args: Vec<HExpr>,
    },
    OptionSome(Box<HExpr>),
    OptionNone,
    ResultOk(Box<HExpr>),
    ResultErr(Box<HExpr>),
    /// format!(...) - produces a String.
    Format {
        parts: Vec<HFormatPart>,
    },
    /// A statement-like macro (print, println, panic, assert, ...).
    Macro(HMacroCall),
    Break,
    Continue,
    Return(Option<Box<HExpr>>),
}

/// A reference to a field of a struct or tuple. Indices are checked against the
/// HIR tables; the code generator mangles them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HFieldRef {
    Struct { struct_id: StructId, index: usize },
    Tuple(usize),
}

#[derive(Debug, Clone)]
pub enum HLit {
    Int { value: i128, int_ty: IntTy },
    Float { value: f64, float_ty: FloatTy },
    Bool(bool),
    Str(String),
    Unit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HUnaryOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HBinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
}

impl HBinaryOp {
    pub fn is_comparison(self) -> bool {
        matches!(
            self,
            HBinaryOp::Eq
                | HBinaryOp::Ne
                | HBinaryOp::Lt
                | HBinaryOp::Le
                | HBinaryOp::Gt
                | HBinaryOp::Ge
        )
    }

    pub fn is_bitwise(self) -> bool {
        matches!(
            self,
            HBinaryOp::BitAnd
                | HBinaryOp::BitOr
                | HBinaryOp::BitXor
                | HBinaryOp::Shl
                | HBinaryOp::Shr
        )
    }
}

// ---------------------------------------------------------------------------
// Builtins
// ---------------------------------------------------------------------------

/// Builtin methods on String, Vec, Option and Result. The receiver type
/// decides which instantiation is used; the result type is carried by the
/// enclosing HExpr.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HBuiltinMethod {
    // String
    StrLen,
    StrIsEmpty,
    StrPushStr,
    StrClear,
    StrContains,
    StrStartsWith,
    StrEndsWith,
    StrFind,
    StrReplace,
    StrToUppercase,
    StrToLowercase,
    StrTrim,
    StrSplit,
    StrParseI64,
    StrParseF64,
    StrSubstring,
    StrRepeat,
    // Vec
    VecLen,
    VecIsEmpty,
    VecPush,
    VecPop,
    VecInsert,
    VecRemove,
    VecClear,
    VecFirst,
    VecLast,
    VecGet,
    VecExtend,
    VecReverse,
    // Option
    OptIsSome,
    OptIsNone,
    OptUnwrap,
    OptUnwrapOr,
    OptExpect,
    // Result
    ResIsOk,
    ResIsErr,
    ResUnwrap,
    ResUnwrapOr,
    ResExpect,
    ResOk,
    ResErr,
    // Rc
    RcGet,
    RcSet,
    RcStrongCount,
    // Box
    BoxGet,
    BoxSet,
}

impl HBuiltinMethod {
    /// Human readable name used in diagnostics.
    pub fn name(self) -> &'static str {
        use HBuiltinMethod::*;
        match self {
            StrLen => "len",
            StrIsEmpty => "is_empty",
            StrPushStr => "push_str",
            StrClear => "clear",
            StrContains => "contains",
            StrStartsWith => "starts_with",
            StrEndsWith => "ends_with",
            StrFind => "find",
            StrReplace => "replace",
            StrToUppercase => "to_uppercase",
            StrToLowercase => "to_lowercase",
            StrTrim => "trim",
            StrSplit => "split",
            StrParseI64 => "parse_i64",
            StrParseF64 => "parse_f64",
            StrSubstring => "substring",
            StrRepeat => "repeat",
            VecLen => "len",
            VecIsEmpty => "is_empty",
            VecPush => "push",
            VecPop => "pop",
            VecInsert => "insert",
            VecRemove => "remove",
            VecClear => "clear",
            VecFirst => "first",
            VecLast => "last",
            VecGet => "get",
            VecExtend => "extend",
            VecReverse => "reverse",
            OptIsSome => "is_some",
            OptIsNone => "is_none",
            OptUnwrap => "unwrap",
            OptUnwrapOr => "unwrap_or",
            OptExpect => "expect",
            ResIsOk => "is_ok",
            ResIsErr => "is_err",
            ResUnwrap => "unwrap",
            ResUnwrapOr => "unwrap_or",
            ResExpect => "expect",
            ResOk => "ok",
            ResErr => "err",
            RcGet => "get",
            RcSet => "set",
            RcStrongCount => "strong_count",
            BoxGet => "get",
            BoxSet => "set",
        }
    }
}

/// Builtin associated functions called through a type path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HBuiltinAssoc {
    /// String::new()
    StringNew,
    /// String::from(&str) - the argument is a String, so this is a clone.
    StringFrom,
    /// `Vec::<T>::new()`
    VecNew,
    /// `Vec::<T>::with_capacity(n)`
    VecWithCapacity,
    /// `Rc::<T>::new(value)`
    RcNew,
    /// `Box::<T>::new(value)`
    BoxNew,
}

/// Builtin free functions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HBuiltinFn {
    Sqrt,
    Abs,
    Min,
    Max,
    Pow,
    Floor,
    Ceil,
    Round,
    Sin,
    Cos,
    Tan,
    Ln,
    Log10,
    Exp,
    ReadLine,
    Args,
    Exit,
    ReadFile,
    WriteFile,
}

impl HBuiltinFn {
    pub fn name(self) -> &'static str {
        use HBuiltinFn::*;
        match self {
            Sqrt => "sqrt",
            Abs => "abs",
            Min => "min",
            Max => "max",
            Pow => "pow",
            Floor => "floor",
            Ceil => "ceil",
            Round => "round",
            Sin => "sin",
            Cos => "cos",
            Tan => "tan",
            Ln => "ln",
            Log10 => "log10",
            Exp => "exp",
            ReadLine => "read_line",
            Args => "args",
            Exit => "exit",
            ReadFile => "read_file",
            WriteFile => "write_file",
        }
    }
}

/// Alignment of a padded format field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatAlign {
    /// `<`: content is left aligned, padding on the right.
    Left,
    /// `^`: content is centered, extra padding goes to the right.
    Center,
    /// `>`: content is right aligned, padding on the left.
    Right,
}

/// The parsed specification of one format placeholder, for example
/// `<8`, `+.2` or `*^7?`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatSpec {
    /// Fill character used for padding; a space unless an align was given.
    pub fill: char,
    /// Alignment; `None` means the default (left for most values).
    pub align: Option<FormatAlign>,
    /// `+`: prefix non-negative numbers with a plus sign.
    pub sign_plus: bool,
    /// `0`: pad numbers with zeros after the sign instead of the fill.
    pub zero_pad: bool,
    /// Minimum field width in characters.
    pub width: Option<i64>,
    /// Digits after the decimal point for floats, maximum characters for
    /// strings.
    pub precision: Option<i64>,
    /// `?`: use Debug instead of Display formatting.
    pub debug: bool,
}

impl FormatSpec {
    /// The specification of a plain `{}` placeholder.
    pub fn plain() -> Self {
        Self {
            fill: ' ',
            align: None,
            sign_plus: false,
            zero_pad: false,
            width: None,
            precision: None,
            debug: false,
        }
    }

    /// True when the placeholder is plain `{}`: no width, precision, sign,
    /// zero padding, alignment or debug flag.
    pub fn is_plain(&self) -> bool {
        *self == Self::plain()
    }
}

/// One piece of a formatted string. Literal text is copied verbatim; arguments
/// are formatted with Display or Debug and the parsed format specification.
#[derive(Debug, Clone)]
pub struct HFormatPart {
    pub kind: HFormatPartKind,
}

#[derive(Debug, Clone)]
pub enum HFormatPartKind {
    Literal(String),
    /// An argument formatted with the given specification.
    ///
    /// The specification is boxed to keep this enum (and every HIR expression
    /// that embeds it) small.
    Arg {
        expr: HExpr,
        spec: Box<FormatSpec>,
    },
}

/// A statement-like macro invocation.
#[derive(Debug, Clone)]
pub struct HMacroCall {
    pub kind: HMacroKind,
    /// Format pieces; empty for macros without a format string.
    pub parts: Vec<HFormatPart>,
    /// Extra arguments: the condition for assert, the two operands for
    /// assert_eq. Format arguments live inside parts, not here.
    pub args: Vec<HExpr>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HMacroKind {
    Print,
    Println,
    Eprint,
    Eprintln,
    Panic,
    Assert,
    AssertEq,
}

// ---------------------------------------------------------------------------
// Patterns
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct HPat {
    /// The type of the value matched by this pattern.
    pub ty: Ty,
    pub kind: HPatKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum HPatKind {
    Wild,
    /// Bind the matched value to a new local variable.
    Bind {
        c_name: String,
        name: String,
    },
    Lit(HLit),
    Tuple(Vec<HPat>),
    Struct {
        struct_id: StructId,
        /// Field index and sub-pattern, in source order.
        fields: Vec<(usize, HPat)>,
        has_rest: bool,
    },
    Enum {
        enum_id: EnumId,
        variant: usize,
        args: Vec<HPat>,
    },
    OptionSome(Box<HPat>),
    OptionNone,
    ResultOk(Box<HPat>),
    ResultErr(Box<HPat>),
    Or(Vec<HPat>),
}

/// A closure: a generated C function plus a reference counted environment
/// holding its captures.
#[derive(Debug, Clone)]
pub struct HClosure {
    pub id: usize,
    /// Display name, for example "closure#1".
    pub name: String,
    /// Unique C identifier of the generated C function.
    pub c_name: String,
    pub params: Vec<HParam>,
    pub ret: Ty,
    pub captures: Vec<HCapture>,
    pub body: HBlock,
    pub span: Span,
}

/// One captured variable. The capture order is the environment field order.
#[derive(Debug, Clone)]
pub struct HCapture {
    pub name: String,
    pub ty: Ty,
}

// ---------------------------------------------------------------------------
// Display helpers used by codegen for debug formatting
// ---------------------------------------------------------------------------

impl fmt::Display for HBinaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                HBinaryOp::Add => "+",
                HBinaryOp::Sub => "-",
                HBinaryOp::Mul => "*",
                HBinaryOp::Div => "/",
                HBinaryOp::Rem => "%",
                HBinaryOp::Eq => "==",
                HBinaryOp::Ne => "!=",
                HBinaryOp::Lt => "<",
                HBinaryOp::Le => "<=",
                HBinaryOp::Gt => ">",
                HBinaryOp::Ge => ">=",
                HBinaryOp::BitAnd => "&",
                HBinaryOp::BitOr => "|",
                HBinaryOp::BitXor => "^",
                HBinaryOp::Shl => "<<",
                HBinaryOp::Shr => ">>",
            }
        )
    }
}
