//! The abstract syntax tree produced by the parser.
//!
//! The AST is a faithful, mostly untyped record of the source program. All
//! semantic questions (what a name resolves to, what the type of an expression
//! is, whether a pattern is exhaustive) are answered by the type checker, which
//! lowers this tree into [crate::hir].

use crate::span::Span;
use crate::types::SelfKind;

/// A whole source file: a list of top-level items.
#[derive(Debug, Clone)]
pub struct Program {
    pub items: Vec<Item>,
}

/// A top-level item.
#[derive(Debug, Clone)]
pub enum Item {
    Fn(FnDecl),
    Struct(StructDecl),
    Enum(EnumDecl),
    Impl(ImplBlock),
    Const(ConstDecl),
    TypeAlias(TypeAliasDecl),
}

impl Item {
    pub fn span(&self) -> Span {
        match self {
            Item::Fn(item) => item.span,
            Item::Struct(item) => item.span,
            Item::Enum(item) => item.span,
            Item::Impl(item) => item.span,
            Item::Const(item) => item.span,
            Item::TypeAlias(item) => item.span,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Item::Fn(item) => &item.name,
            Item::Struct(item) => &item.name,
            Item::Enum(item) => &item.name,
            Item::Impl(item) => &item.ty_name,
            Item::Const(item) => &item.name,
            Item::TypeAlias(item) => &item.name,
        }
    }
}

/// Function or method declaration.
#[derive(Debug, Clone)]
pub struct FnDecl {
    pub name: String,
    pub name_span: Span,
    pub params: Vec<Param>,
    /// None means the function returns the unit type.
    pub ret: Option<TypeExpr>,
    pub body: Block,
    /// Set when the function is declared inside an impl block.
    pub self_kind: Option<SelfKind>,
    pub span: Span,
}

/// How a method receives its receiver, mirroring Rust's self, &self, &mut self.
/// (Re-exported from the type model so the AST and HIR agree.)
pub use crate::types::SelfKind as ReceiverKind;

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub name_span: Span,
    pub ty: TypeExpr,
}

#[derive(Debug, Clone)]
pub struct StructDecl {
    pub name: String,
    pub name_span: Span,
    pub fields: Vec<FieldDef>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct FieldDef {
    pub name: String,
    pub name_span: Span,
    pub ty: TypeExpr,
}

#[derive(Debug, Clone)]
pub struct EnumDecl {
    pub name: String,
    pub name_span: Span,
    pub variants: Vec<VariantDef>,
    pub span: Span,
}

/// An enum variant: Unit (no fields), Tuple(T) or Struct { a: T } style is not
/// supported; only tuple variants and unit variants exist.
#[derive(Debug, Clone)]
pub struct VariantDef {
    pub name: String,
    pub name_span: Span,
    pub fields: Vec<TypeExpr>,
    pub span: Span,
}

/// An impl block. Inline type parameter lists are not supported.
#[derive(Debug, Clone)]
pub struct ImplBlock {
    pub ty: TypeExpr,
    /// Flattened display name of the implemented type, for messages.
    pub ty_name: String,
    pub ty_span: Span,
    pub fns: Vec<FnDecl>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ConstDecl {
    pub name: String,
    pub name_span: Span,
    pub ty: TypeExpr,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct TypeAliasDecl {
    pub name: String,
    pub name_span: Span,
    pub ty: TypeExpr,
    pub span: Span,
}

// ---------------------------------------------------------------------------
// Blocks and statements
// ---------------------------------------------------------------------------

/// A braced block: statements followed by an optional tail expression.
#[derive(Debug, Clone)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub tail: Option<Box<Expr>>,
    pub span: Span,
}

impl Block {
    pub fn empty(span: Span) -> Block {
        Block {
            stmts: Vec::new(),
            tail: None,
            span,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Let(LetStmt),
    /// An expression statement: the value is evaluated and discarded.
    Expr(Expr),
    /// A nested item declaration. Nested items are hoisted to the top level by
    /// the type checker; they do not capture anything from the enclosing block.
    Item(Item),
    /// A stray semicolon.
    Empty(Span),
}

impl Stmt {
    pub fn span(&self) -> Span {
        match self {
            Stmt::Let(stmt) => stmt.span,
            Stmt::Expr(expr) => expr.span,
            Stmt::Item(item) => item.span(),
            Stmt::Empty(span) => *span,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LetStmt {
    pub pat: Pattern,
    pub ty: Option<TypeExpr>,
    pub init: Option<Expr>,
    pub span: Span,
}

// ---------------------------------------------------------------------------
// Expressions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

impl Expr {
    pub fn new(kind: ExprKind, span: Span) -> Expr {
        Expr { kind, span }
    }

    /// The literal value of a string literal, if this is one. Used by macro
    /// parsing (format strings) and const evaluation.
    pub fn as_string_literal(&self) -> Option<&str> {
        match &self.kind {
            ExprKind::Lit(Lit::Str(text)) => Some(text),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ExprKind {
    Lit(Lit),
    /// A path expression such as `x`, `Point`, `Point::new`, `Vec::<i64>::new` or `None`.
    Path(PathExpr),
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Binary {
        op: BinaryOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    /// Assignment or compound assignment. The parser only produces this for
    /// place expressions; the type checker verifies assignability.
    Assign {
        op: Option<BinaryOp>,
        target: Box<Expr>,
        value: Box<Expr>,
    },
    Cast {
        expr: Box<Expr>,
        ty: TypeExpr,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },
    MethodCall {
        receiver: Box<Expr>,
        method: String,
        method_span: Span,
        turbofish: Vec<TypeExpr>,
        args: Vec<Expr>,
    },
    /// Named field access: value.field
    Field {
        base: Box<Expr>,
        name: String,
        name_span: Span,
    },
    /// Numeric tuple field access: `value.0`
    TupleField {
        base: Box<Expr>,
        index: usize,
        index_span: Span,
    },
    /// Indexing: `value[index]`
    Index {
        base: Box<Expr>,
        index: Box<Expr>,
    },
    If(IfExpr),
    Match(MatchExpr),
    Block(Block),
    Loop {
        body: Block,
    },
    While {
        cond: Box<Expr>,
        body: Block,
    },
    For {
        pat: Pattern,
        iter: Box<Expr>,
        body: Block,
    },
    /// Range expression, only produced inside `for` heads: `a..b` or `a..=b`.
    /// Ranges are not first-class values in this language version.
    Range {
        start: Box<Expr>,
        end: Box<Expr>,
        inclusive: bool,
    },
    Closure(ClosureExpr),
    /// Struct literal: Point { x: 1, y: 2 }
    StructLit {
        path: PathExpr,
        fields: Vec<FieldInit>,
    },
    /// Tuple literal: (a, b) or (a,) for a one-tuple.
    Tuple(Vec<Expr>),
    /// vec![a, b, c] or vec![value; count]
    Vec(VecLit),
    /// A macro invocation other than vec!: println!, print!, format!, panic!,
    /// assert!, assert_eq!, eprintln!, eprint!.
    Macro(MacroCall),
    Break(Option<Box<Expr>>),
    Continue,
    Return(Option<Box<Expr>>),
}

#[derive(Debug, Clone)]
pub enum VecLit {
    List(Vec<Expr>),
    Repeat { value: Box<Expr>, count: Box<Expr> },
}

/// A resolved, syntactically valid path like `Point::new` or `Vec::<i64>::new`.
#[derive(Debug, Clone)]
pub struct PathExpr {
    pub segments: Vec<PathSegment>,
    pub span: Span,
}

impl PathExpr {
    /// The plain name when the path has exactly one segment without generic
    /// arguments (for example the local variable x or the constructor None).
    pub fn simple_name(&self) -> Option<&str> {
        if self.segments.len() == 1 && self.segments[0].generics.is_empty() {
            Some(&self.segments[0].name)
        } else {
            None
        }
    }

    pub fn last_name(&self) -> Option<&str> {
        self.segments.last().map(|segment| segment.name.as_str())
    }

    pub fn is_plain_name(&self) -> bool {
        self.segments.len() == 1
    }

    pub fn display_name(&self) -> String {
        self.segments
            .iter()
            .map(|segment| segment.name.clone())
            .collect::<Vec<_>>()
            .join("::")
    }
}

#[derive(Debug, Clone)]
pub struct PathSegment {
    pub name: String,
    pub span: Span,
    /// Generic arguments written with a turbofish: `Vec::<i64>::new` has one
    /// generic argument on the first segment.
    pub generics: Vec<TypeExpr>,
}

#[derive(Debug, Clone)]
pub struct IfExpr {
    pub cond: Box<Expr>,
    pub then_block: Block,
    /// The else branch as an expression: either a block or another if.
    pub else_expr: Option<Box<Expr>>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct MatchExpr {
    pub scrutinee: Box<Expr>,
    pub arms: Vec<MatchArm>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pat: Pattern,
    pub guard: Option<Expr>,
    pub body: Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct FieldInit {
    pub name: String,
    pub name_span: Span,
    pub value: Expr,
}

#[derive(Debug, Clone)]
pub struct ClosureExpr {
    pub is_move: bool,
    pub params: Vec<ClosureParam>,
    pub ret: Option<TypeExpr>,
    /// The body is either a block or an expression.
    pub body: Box<Expr>,
}

#[derive(Debug, Clone)]
pub struct ClosureParam {
    pub pat: Pattern,
    pub ty: Option<TypeExpr>,
}

#[derive(Debug, Clone)]
pub struct MacroCall {
    pub name: String,
    pub name_span: Span,
    pub args: Vec<Expr>,
    pub span: Span,
}

// ---------------------------------------------------------------------------
// Patterns
// ---------------------------------------------------------------------------

/// A pattern, used by let statements, match arms, for loops and closures.
#[derive(Debug, Clone)]
pub struct Pattern {
    pub kind: PatternKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum PatternKind {
    /// The wildcard pattern _
    Wild,
    /// A binding like x or mut x. The parser produces this for any plain
    /// identifier; the type checker turns it into an enum-variant pattern when
    /// the name resolves to a unit variant or constant.
    Binding {
        name: String,
        mutable: bool,
    },
    /// A literal pattern such as 1, 'text', true or -1.
    Lit(Lit),
    Tuple(Vec<Pattern>),
    /// A path pattern with optional payload: None, Some(x), Color::Red,
    /// Shape::Circle(r).
    Path {
        path: PathExpr,
        args: Vec<Pattern>,
    },
    /// A struct pattern: Point { x, y: other } or Point { x, .. }
    Struct {
        path: PathExpr,
        fields: Vec<FieldPat>,
        has_rest: bool,
    },
    /// Or patterns in match arms: 1 | 2 | 3
    Or(Vec<Pattern>),
}

#[derive(Debug, Clone)]
pub struct FieldPat {
    pub name: String,
    pub name_span: Span,
    pub pat: Pattern,
}

// ---------------------------------------------------------------------------
// Literals, operators, type expressions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum Lit {
    /// Integer literal. The value is always non-negative in the AST; a leading
    /// minus is a unary expression (or folded by the pattern parser).
    Int {
        value: i128,
        raw: String,
    },
    Float {
        value: f64,
        raw: String,
    },
    Bool(bool),
    Str(String),
}

impl Lit {
    pub fn describe(&self) -> String {
        match self {
            Lit::Int { raw, .. } => raw.clone(),
            Lit::Float { raw, .. } => raw.clone(),
            Lit::Bool(value) => value.to_string(),
            Lit::Str(_) => "string".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
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
    And,
    Or,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
}

impl BinaryOp {
    pub fn symbol(self) -> &'static str {
        match self {
            BinaryOp::Add => "+",
            BinaryOp::Sub => "-",
            BinaryOp::Mul => "*",
            BinaryOp::Div => "/",
            BinaryOp::Rem => "%",
            BinaryOp::Eq => "==",
            BinaryOp::Ne => "!=",
            BinaryOp::Lt => "<",
            BinaryOp::Le => "<=",
            BinaryOp::Gt => ">",
            BinaryOp::Ge => ">=",
            BinaryOp::And => "&&",
            BinaryOp::Or => "||",
            BinaryOp::BitAnd => "&",
            BinaryOp::BitOr => "|",
            BinaryOp::BitXor => "^",
            BinaryOp::Shl => "<<",
            BinaryOp::Shr => ">>",
        }
    }

    pub fn is_comparison(self) -> bool {
        matches!(
            self,
            BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge
        )
    }

    pub fn is_logical(self) -> bool {
        matches!(self, BinaryOp::And | BinaryOp::Or)
    }

    pub fn is_assignment_op(self) -> bool {
        matches!(
            self,
            BinaryOp::Add
                | BinaryOp::Sub
                | BinaryOp::Mul
                | BinaryOp::Div
                | BinaryOp::Rem
                | BinaryOp::BitAnd
                | BinaryOp::BitOr
                | BinaryOp::BitXor
                | BinaryOp::Shl
                | BinaryOp::Shr
        )
    }
}

/// A type as written in source code. Resolved to [crate::types::Ty] by the type
/// checker.
#[derive(Debug, Clone)]
pub enum TypeExpr {
    /// A named type with optional generic arguments: `i64`, `String`,
    /// `Vec<i64>`, `Option<String>`, `Point`.
    Path {
        name: String,
        name_span: Span,
        generics: Vec<TypeExpr>,
    },
    /// A tuple type, including the empty tuple ().
    Tuple { elems: Vec<TypeExpr>, span: Span },
    /// A function type: fn(i64, String) -> bool
    Fn {
        params: Vec<TypeExpr>,
        ret: Box<TypeExpr>,
        span: Span,
    },
}

impl TypeExpr {
    pub fn span(&self) -> Span {
        match self {
            TypeExpr::Path { name_span, .. } => *name_span,
            TypeExpr::Tuple { span, .. } => *span,
            TypeExpr::Fn { span, .. } => *span,
        }
    }

    /// Unambiguous text for this type expression, used in diagnostics.
    pub fn describe(&self) -> String {
        match self {
            TypeExpr::Path { name, generics, .. } => {
                if generics.is_empty() {
                    name.clone()
                } else {
                    let args = generics
                        .iter()
                        .map(TypeExpr::describe)
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("{}<{}>", name, args)
                }
            }
            TypeExpr::Tuple { elems, .. } => {
                if elems.is_empty() {
                    "()".to_string()
                } else if elems.len() == 1 {
                    format!("({},)", elems[0].describe())
                } else {
                    let inner = elems
                        .iter()
                        .map(TypeExpr::describe)
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("({})", inner)
                }
            }
            TypeExpr::Fn { params, ret, .. } => {
                let params = params
                    .iter()
                    .map(TypeExpr::describe)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("fn({}) -> {}", params, ret.describe())
            }
        }
    }
}
