//! The typed tree produced by the checker and consumed by lowering.
//!
//! It mirrors [crate::hir] but keeps inference types and bookkeeping the
//! lowering step needs (mutability of places, closure capture expressions).

use crate::hir::{
    FormatSpec, HBinaryOp, HBuiltinAssoc, HBuiltinFn, HBuiltinMethod, HFieldRef, HMacroKind,
    HUnaryOp,
};
use crate::span::Span;

use super::itype::ITy;

#[derive(Clone, Debug)]
pub struct TParam {
    pub name: String,
    pub c_name: String,
    pub ty: ITy,
}

#[derive(Clone, Debug)]
pub struct TCapture {
    pub name: String,
    pub ty: ITy,
    /// Expression reading the captured value in the enclosing function.
    pub parent_expr: TExpr,
}

#[derive(Clone, Debug)]
pub struct TClosure {
    pub id: usize,
    pub c_name: String,
    pub params: Vec<TParam>,
    pub ret: ITy,
    pub captures: Vec<TCapture>,
    pub body: TBlock,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct TExpr {
    pub ty: ITy,
    pub kind: TExprKind,
    pub span: Span,
}

impl TExpr {
    pub fn new(ty: ITy, kind: TExprKind, span: Span) -> TExpr {
        TExpr { ty, kind, span }
    }
}

#[derive(Clone, Debug)]
pub enum TExprKind {
    Lit(TLit),
    Local {
        name: String,
        c_name: String,
        mutable: bool,
    },
    Capture {
        index: usize,
        name: String,
    },
    ConstRef {
        const_id: usize,
    },
    FnRef {
        fn_id: usize,
    },
    Closure {
        closure_id: usize,
    },
    Call {
        callee: Box<TExpr>,
        args: Vec<TExpr>,
    },
    DirectCall {
        fn_id: usize,
        receiver: Option<Box<TExpr>>,
        args: Vec<TExpr>,
    },
    BuiltinMethod {
        receiver: Box<TExpr>,
        method: HBuiltinMethod,
        args: Vec<TExpr>,
    },
    BuiltinAssoc {
        func: HBuiltinAssoc,
        args: Vec<TExpr>,
        type_args: Vec<ITy>,
    },
    BuiltinCall {
        func: HBuiltinFn,
        args: Vec<TExpr>,
    },
    Field {
        base: Box<TExpr>,
        field: HFieldRef,
    },
    Index {
        base: Box<TExpr>,
        index: Box<TExpr>,
    },
    Unary {
        op: HUnaryOp,
        expr: Box<TExpr>,
    },
    Binary {
        op: HBinaryOp,
        lhs: Box<TExpr>,
        rhs: Box<TExpr>,
    },
    Assign {
        target: Box<TExpr>,
        op: Option<HBinaryOp>,
        value: Box<TExpr>,
    },
    Cast {
        expr: Box<TExpr>,
    },
    Block(TBlock),
    If {
        cond: Box<TExpr>,
        then_block: TBlock,
        else_expr: Option<Box<TExpr>>,
    },
    Match {
        scrutinee: Box<TExpr>,
        arms: Vec<TArm>,
    },
    Loop {
        label: Option<String>,
        body: TBlock,
    },
    While {
        label: Option<String>,
        cond: Box<TExpr>,
        body: TBlock,
    },
    ForRange {
        label: Option<String>,
        pat: TPat,
        start: Box<TExpr>,
        end: Box<TExpr>,
        inclusive: bool,
        body: TBlock,
    },
    ForVec {
        label: Option<String>,
        pat: TPat,
        vec: Box<TExpr>,
        body: TBlock,
    },
    /// A loop over a user iterator: the value is moved into the loop and each
    /// iteration drives the instantiated `Iterator::next` function.
    ForIter {
        label: Option<String>,
        iter: Box<TExpr>,
        next_fn: usize,
        pat: TPat,
        body: TBlock,
    },
    StructLit {
        struct_id: usize,
        fields: Vec<(usize, TExpr)>,
    },
    Tuple(Vec<TExpr>),
    VecLit {
        elems: Vec<TExpr>,
    },
    VecRepeat {
        value: Box<TExpr>,
        count: Box<TExpr>,
    },
    /// An array literal [a, b, c].
    Array {
        elems: Vec<TExpr>,
    },
    /// An array repeat expression [value; count] with a constant count.
    ArrayRepeat {
        value: Box<TExpr>,
        len: usize,
    },
    EnumLit {
        enum_id: usize,
        variant: usize,
        args: Vec<TExpr>,
    },
    OptionSome(Box<TExpr>),
    OptionNone,
    ResultOk(Box<TExpr>),
    ResultErr(Box<TExpr>),
    Ref {
        mutable: bool,
        expr: Box<TExpr>,
    },
    Deref {
        expr: Box<TExpr>,
    },
    Format {
        parts: Vec<TFormatPart>,
    },
    Macro(TMacroCall),
    Break {
        label: Option<String>,
    },
    Continue {
        label: Option<String>,
    },
    Return(Option<Box<TExpr>>),
    /// Coerce a concrete pointer value to a trait object pointer: `&T` to
    /// `&dyn Trait`, `Box<T>` to `Box<dyn Trait>`, and so on.
    DynCoerce {
        value: Box<TExpr>,
        /// Index into `Checker::vtables`.
        vtable: usize,
    },
    /// Call a method through a trait object's vtable slot.
    DynCall {
        /// Receiver exactly as written (`Box<dyn Trait>`, `Rc<dyn Trait>`,
        /// `&dyn Trait` or `&mut dyn Trait`).
        object: Box<TExpr>,
        /// Index into `Checker::trait_objects`.
        trait_object: usize,
        /// Zero-based method slot inside the trait object.
        slot: usize,
        args: Vec<TExpr>,
    },
    /// Slice of a vector, string or existing slice: `base[start..end]`.
    /// Always wrapped in a reference on the HIR level; the expression type is
    /// `&[T]`, `&mut [T]`, `&str` or `&mut str`.
    Slice {
        /// Container place expression (the vector, string or slice).
        base: Box<TExpr>,
        start: Option<Box<TExpr>>,
        end: Option<Box<TExpr>>,
        inclusive: bool,
    },
    /// Postfix try operator. Lowered into a match plus an early return that
    /// propagates None or Err out of the enclosing function or closure.
    Try {
        expr: Box<TExpr>,
        /// Declared return type of the enclosing frame (Option or Result).
        ret: ITy,
        /// Fresh local bound to the payload value in the success arm.
        value_c_name: String,
        /// Fresh local bound to the error value in the failure arm (Result only).
        error_c_name: String,
        /// Instantiating `from` function of the `impl From<Found> for Expected`
        /// that converts the propagated error, when the error types differ.
        error_conversion: Option<usize>,
    },
}

#[derive(Clone, Debug)]
pub enum TLit {
    Int(i128),
    Float(f64),
    Bool(bool),
    Char(char),
    Str(String),
    Unit,
}

#[derive(Clone, Debug)]
pub struct TFormatPart {
    pub kind: TFormatPartKind,
}

#[derive(Clone, Debug)]
pub enum TFormatPartKind {
    Literal(String),
    Arg { expr: Box<TExpr>, spec: FormatSpec },
}

#[derive(Clone, Debug)]
pub struct TMacroCall {
    pub kind: HMacroKind,
    pub parts: Vec<TFormatPart>,
    pub args: Vec<TExpr>,
}

#[derive(Clone, Debug)]
pub struct TArm {
    pub pat: TPat,
    pub guard: Option<TExpr>,
    pub body: TExpr,
}

#[derive(Clone, Debug)]
pub struct TBlock {
    pub stmts: Vec<TStmt>,
    pub tail: Option<Box<TExpr>>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum TStmt {
    Let {
        pat: TPat,
        init: Option<TExpr>,
        span: Span,
    },
    Expr(TExpr),
}

#[derive(Clone, Debug)]
pub struct TPat {
    pub ty: ITy,
    pub kind: TPatKind,
    pub span: Span,
}

impl TPat {
    pub fn new(ty: ITy, kind: TPatKind, span: Span) -> TPat {
        TPat { ty, kind, span }
    }
}

#[derive(Clone, Debug)]
pub enum TPatKind {
    Wild,
    Bind {
        name: String,
        c_name: String,
        mutable: bool,
    },
    Lit(TLit),
    Tuple(Vec<TPat>),
    Struct {
        struct_id: usize,
        fields: Vec<(usize, TPat)>,
        has_rest: bool,
    },
    Enum {
        enum_id: usize,
        variant: usize,
        args: Vec<TPat>,
    },
    OptionSome(Box<TPat>),
    OptionNone,
    ResultOk(Box<TPat>),
    ResultErr(Box<TPat>),
    Or(Vec<TPat>),
}
