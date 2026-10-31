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
    Trait(TraitDecl),
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
            Item::Trait(item) => item.span,
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
            Item::Trait(item) => &item.name,
        }
    }
}

/// One generic parameter declaration: `T` in `fn f<T>(..)` or the bound form
/// `T: Iterator<i64> + Drop`.
#[derive(Debug, Clone)]
pub struct GenericParam {
    pub name: String,
    pub name_span: Span,
    /// Trait bounds, the callable bound `Fn(A, B) -> C`, or the builtin `Drop`
    /// bound. Stored as written; the type checker resolves them.
    pub bounds: Vec<TypeExpr>,
}

/// Function or method declaration.
#[derive(Debug, Clone)]
pub struct FnDecl {
    pub name: String,
    pub name_span: Span,
    pub generics: Vec<GenericParam>,
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
    pub generics: Vec<GenericParam>,
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
    pub generics: Vec<GenericParam>,
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

/// The trait named in an `impl Trait<Args> for Type` block.
#[derive(Debug, Clone)]
pub struct TraitRef {
    pub name: String,
    pub name_span: Span,
    /// Generic arguments written on the trait: `Iterator<i64>` has one argument.
    pub args: Vec<TypeExpr>,
    /// Covers the whole trait reference (name and generic arguments).
    pub span: Span,
}

/// A method signature inside a trait declaration. Trait methods are
/// signature-only in version 1.5: the body is not part of the AST.
#[derive(Debug, Clone)]
pub struct TraitMethod {
    pub name: String,
    pub name_span: Span,
    /// The first parameter is the receiver, synthesized the same way as in an
    /// impl method (name "self" and the trait self type as the parameter type).
    pub params: Vec<Param>,
    pub ret: Option<TypeExpr>,
    /// Always Some in version 1.5; kept optional to mirror [FnDecl].
    pub self_kind: Option<SelfKind>,
    pub span: Span,
}

/// A trait declaration: `trait Iterator<T> { fn next(&mut self) -> Option<T>; }`.
#[derive(Debug, Clone)]
pub struct TraitDecl {
    pub name: String,
    pub name_span: Span,
    pub generics: Vec<GenericParam>,
    pub methods: Vec<TraitMethod>,
    pub span: Span,
}

/// An impl block: an inherent impl (`impl<T> Pair<T> { .. }`) or a trait impl
/// (`impl Iterator<i64> for Vector { .. }`).
#[derive(Debug, Clone)]
pub struct ImplBlock {
    pub generics: Vec<GenericParam>,
    pub ty: TypeExpr,
    /// Flattened display name of the implemented type, for messages.
    pub ty_name: String,
    pub ty_span: Span,
    /// Some for trait impl blocks. The type checker validates the trait name,
    /// the generic arguments and the method signatures.
    pub trait_name: Option<TraitRef>,
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
    pub generics: Vec<GenericParam>,
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
    /// A borrow expression: `&expr` or `&mut expr`. The operand must be a
    /// place expression; the type checker verifies mutability for `&mut`.
    Ref {
        mutable: bool,
        expr: Box<Expr>,
    },
    /// An explicit dereference of a reference, `*expr`. The result is a place
    /// expression of the referent type.
    Deref {
        expr: Box<Expr>,
    },
    /// Postfix try operator: `expr?`. Only valid where the enclosing function
    /// or closure returns Option or Result; the type checker verifies that and
    /// lowering desugars it into a match plus early return.
    Try {
        expr: Box<Expr>,
    },
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
    /// A character literal such as 'a' or '\n'. The value is a Unicode
    /// scalar value.
    Char(char),
    Str(String),
}

impl Lit {
    pub fn describe(&self) -> String {
        match self {
            Lit::Int { raw, .. } => raw.clone(),
            Lit::Float { raw, .. } => raw.clone(),
            Lit::Bool(value) => value.to_string(),
            Lit::Char(value) => format!("{:?}", value),
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
    /// A reference type: &T or &mut T
    Ref {
        mutable: bool,
        inner: Box<TypeExpr>,
        span: Span,
    },
}

impl TypeExpr {
    pub fn span(&self) -> Span {
        match self {
            TypeExpr::Path { name_span, .. } => *name_span,
            TypeExpr::Tuple { span, .. } => *span,
            TypeExpr::Fn { span, .. } => *span,
            TypeExpr::Ref { span, .. } => *span,
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
            TypeExpr::Ref { mutable, inner, .. } => {
                if *mutable {
                    format!("&mut {}", inner.describe())
                } else {
                    format!("&{}", inner.describe())
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Span walking
// ---------------------------------------------------------------------------

impl Program {
    /// Visit every [Span] stored in the tree.
    ///
    /// The prelude loader uses this to erase the spans of prelude items,
    /// because those declarations do not come from the user's source file.
    /// The walk is exhaustive: every node type that owns a span extends it.
    /// New AST nodes that store spans must add them here.
    pub fn for_each_span_mut(&mut self, visit: &mut dyn FnMut(&mut Span)) {
        for item in &mut self.items {
            item.for_each_span_mut(visit);
        }
    }
}

impl Item {
    fn for_each_span_mut(&mut self, visit: &mut dyn FnMut(&mut Span)) {
        match self {
            Item::Fn(decl) => decl.for_each_span_mut(visit),
            Item::Struct(decl) => decl.for_each_span_mut(visit),
            Item::Enum(decl) => decl.for_each_span_mut(visit),
            Item::Impl(block) => block.for_each_span_mut(visit),
            Item::Const(decl) => decl.for_each_span_mut(visit),
            Item::TypeAlias(decl) => decl.for_each_span_mut(visit),
            Item::Trait(decl) => decl.for_each_span_mut(visit),
        }
    }
}

impl GenericParam {
    fn for_each_span_mut(&mut self, visit: &mut dyn FnMut(&mut Span)) {
        visit(&mut self.name_span);
        for bound in &mut self.bounds {
            bound.for_each_span_mut(visit);
        }
    }
}

impl FnDecl {
    fn for_each_span_mut(&mut self, visit: &mut dyn FnMut(&mut Span)) {
        visit(&mut self.name_span);
        visit(&mut self.span);
        for param in &mut self.generics {
            param.for_each_span_mut(visit);
        }
        for param in &mut self.params {
            param.for_each_span_mut(visit);
        }
        if let Some(ret) = &mut self.ret {
            ret.for_each_span_mut(visit);
        }
        self.body.for_each_span_mut(visit);
    }
}

impl Param {
    fn for_each_span_mut(&mut self, visit: &mut dyn FnMut(&mut Span)) {
        visit(&mut self.name_span);
        self.ty.for_each_span_mut(visit);
    }
}

impl StructDecl {
    fn for_each_span_mut(&mut self, visit: &mut dyn FnMut(&mut Span)) {
        visit(&mut self.name_span);
        visit(&mut self.span);
        for param in &mut self.generics {
            param.for_each_span_mut(visit);
        }
        for field in &mut self.fields {
            field.for_each_span_mut(visit);
        }
    }
}

impl FieldDef {
    fn for_each_span_mut(&mut self, visit: &mut dyn FnMut(&mut Span)) {
        visit(&mut self.name_span);
        self.ty.for_each_span_mut(visit);
    }
}

impl EnumDecl {
    fn for_each_span_mut(&mut self, visit: &mut dyn FnMut(&mut Span)) {
        visit(&mut self.name_span);
        visit(&mut self.span);
        for param in &mut self.generics {
            param.for_each_span_mut(visit);
        }
        for variant in &mut self.variants {
            variant.for_each_span_mut(visit);
        }
    }
}

impl VariantDef {
    fn for_each_span_mut(&mut self, visit: &mut dyn FnMut(&mut Span)) {
        visit(&mut self.name_span);
        visit(&mut self.span);
        for field in &mut self.fields {
            field.for_each_span_mut(visit);
        }
    }
}

impl TraitRef {
    fn for_each_span_mut(&mut self, visit: &mut dyn FnMut(&mut Span)) {
        visit(&mut self.name_span);
        visit(&mut self.span);
        for arg in &mut self.args {
            arg.for_each_span_mut(visit);
        }
    }
}

impl TraitMethod {
    fn for_each_span_mut(&mut self, visit: &mut dyn FnMut(&mut Span)) {
        visit(&mut self.name_span);
        visit(&mut self.span);
        for param in &mut self.params {
            param.for_each_span_mut(visit);
        }
        if let Some(ret) = &mut self.ret {
            ret.for_each_span_mut(visit);
        }
    }
}

impl TraitDecl {
    fn for_each_span_mut(&mut self, visit: &mut dyn FnMut(&mut Span)) {
        visit(&mut self.name_span);
        visit(&mut self.span);
        for param in &mut self.generics {
            param.for_each_span_mut(visit);
        }
        for method in &mut self.methods {
            method.for_each_span_mut(visit);
        }
    }
}

impl ImplBlock {
    fn for_each_span_mut(&mut self, visit: &mut dyn FnMut(&mut Span)) {
        visit(&mut self.ty_span);
        visit(&mut self.span);
        for param in &mut self.generics {
            param.for_each_span_mut(visit);
        }
        self.ty.for_each_span_mut(visit);
        if let Some(trait_ref) = &mut self.trait_name {
            trait_ref.for_each_span_mut(visit);
        }
        for decl in &mut self.fns {
            decl.for_each_span_mut(visit);
        }
    }
}

impl ConstDecl {
    fn for_each_span_mut(&mut self, visit: &mut dyn FnMut(&mut Span)) {
        visit(&mut self.name_span);
        visit(&mut self.span);
        self.ty.for_each_span_mut(visit);
        self.value.for_each_span_mut(visit);
    }
}

impl TypeAliasDecl {
    fn for_each_span_mut(&mut self, visit: &mut dyn FnMut(&mut Span)) {
        visit(&mut self.name_span);
        visit(&mut self.span);
        for param in &mut self.generics {
            param.for_each_span_mut(visit);
        }
        self.ty.for_each_span_mut(visit);
    }
}

impl Block {
    fn for_each_span_mut(&mut self, visit: &mut dyn FnMut(&mut Span)) {
        visit(&mut self.span);
        for stmt in &mut self.stmts {
            stmt.for_each_span_mut(visit);
        }
        if let Some(tail) = &mut self.tail {
            tail.for_each_span_mut(visit);
        }
    }
}

impl Stmt {
    fn for_each_span_mut(&mut self, visit: &mut dyn FnMut(&mut Span)) {
        match self {
            Stmt::Let(stmt) => stmt.for_each_span_mut(visit),
            Stmt::Expr(expr) => expr.for_each_span_mut(visit),
            Stmt::Item(item) => item.for_each_span_mut(visit),
            Stmt::Empty(span) => visit(span),
        }
    }
}

impl LetStmt {
    fn for_each_span_mut(&mut self, visit: &mut dyn FnMut(&mut Span)) {
        visit(&mut self.span);
        self.pat.for_each_span_mut(visit);
        if let Some(ty) = &mut self.ty {
            ty.for_each_span_mut(visit);
        }
        if let Some(init) = &mut self.init {
            init.for_each_span_mut(visit);
        }
    }
}

impl Expr {
    fn for_each_span_mut(&mut self, visit: &mut dyn FnMut(&mut Span)) {
        visit(&mut self.span);
        match &mut self.kind {
            ExprKind::Lit(_) => {}
            ExprKind::Path(path) => path.for_each_span_mut(visit),
            ExprKind::Unary { expr, .. } => expr.for_each_span_mut(visit),
            ExprKind::Binary { lhs, rhs, .. } => {
                lhs.for_each_span_mut(visit);
                rhs.for_each_span_mut(visit);
            }
            ExprKind::Assign { target, value, .. } => {
                target.for_each_span_mut(visit);
                value.for_each_span_mut(visit);
            }
            ExprKind::Cast { expr, ty } => {
                expr.for_each_span_mut(visit);
                ty.for_each_span_mut(visit);
            }
            ExprKind::Call { callee, args } => {
                callee.for_each_span_mut(visit);
                for arg in args {
                    arg.for_each_span_mut(visit);
                }
            }
            ExprKind::MethodCall {
                receiver,
                method_span,
                turbofish,
                args,
                ..
            } => {
                receiver.for_each_span_mut(visit);
                visit(method_span);
                for ty in turbofish {
                    ty.for_each_span_mut(visit);
                }
                for arg in args {
                    arg.for_each_span_mut(visit);
                }
            }
            ExprKind::Field {
                base, name_span, ..
            } => {
                base.for_each_span_mut(visit);
                visit(name_span);
            }
            ExprKind::TupleField {
                base, index_span, ..
            } => {
                base.for_each_span_mut(visit);
                visit(index_span);
            }
            ExprKind::Index { base, index } => {
                base.for_each_span_mut(visit);
                index.for_each_span_mut(visit);
            }
            ExprKind::If(if_expr) => if_expr.for_each_span_mut(visit),
            ExprKind::Match(match_expr) => match_expr.for_each_span_mut(visit),
            ExprKind::Block(block) => block.for_each_span_mut(visit),
            ExprKind::Loop { body } => body.for_each_span_mut(visit),
            ExprKind::While { cond, body } => {
                cond.for_each_span_mut(visit);
                body.for_each_span_mut(visit);
            }
            ExprKind::For { pat, iter, body } => {
                pat.for_each_span_mut(visit);
                iter.for_each_span_mut(visit);
                body.for_each_span_mut(visit);
            }
            ExprKind::Range { start, end, .. } => {
                start.for_each_span_mut(visit);
                end.for_each_span_mut(visit);
            }
            ExprKind::Closure(closure) => closure.for_each_span_mut(visit),
            ExprKind::StructLit { path, fields } => {
                path.for_each_span_mut(visit);
                for field in fields {
                    field.for_each_span_mut(visit);
                }
            }
            ExprKind::Tuple(exprs) => {
                for expr in exprs {
                    expr.for_each_span_mut(visit);
                }
            }
            ExprKind::Vec(lit) => match lit {
                VecLit::List(exprs) => {
                    for expr in exprs {
                        expr.for_each_span_mut(visit);
                    }
                }
                VecLit::Repeat { value, count } => {
                    value.for_each_span_mut(visit);
                    count.for_each_span_mut(visit);
                }
            },
            ExprKind::Macro(call) => call.for_each_span_mut(visit),
            ExprKind::Break(expr) => {
                if let Some(expr) = expr {
                    expr.for_each_span_mut(visit);
                }
            }
            ExprKind::Continue => {}
            ExprKind::Return(expr) => {
                if let Some(expr) = expr {
                    expr.for_each_span_mut(visit);
                }
            }
            ExprKind::Ref { expr, .. } => expr.for_each_span_mut(visit),
            ExprKind::Deref { expr } => expr.for_each_span_mut(visit),
            ExprKind::Try { expr } => expr.for_each_span_mut(visit),
        }
    }
}

impl PathExpr {
    fn for_each_span_mut(&mut self, visit: &mut dyn FnMut(&mut Span)) {
        visit(&mut self.span);
        for segment in &mut self.segments {
            segment.for_each_span_mut(visit);
        }
    }
}

impl PathSegment {
    fn for_each_span_mut(&mut self, visit: &mut dyn FnMut(&mut Span)) {
        visit(&mut self.span);
        for ty in &mut self.generics {
            ty.for_each_span_mut(visit);
        }
    }
}

impl IfExpr {
    fn for_each_span_mut(&mut self, visit: &mut dyn FnMut(&mut Span)) {
        visit(&mut self.span);
        self.cond.for_each_span_mut(visit);
        self.then_block.for_each_span_mut(visit);
        if let Some(else_expr) = &mut self.else_expr {
            else_expr.for_each_span_mut(visit);
        }
    }
}

impl MatchExpr {
    fn for_each_span_mut(&mut self, visit: &mut dyn FnMut(&mut Span)) {
        visit(&mut self.span);
        self.scrutinee.for_each_span_mut(visit);
        for arm in &mut self.arms {
            arm.for_each_span_mut(visit);
        }
    }
}

impl MatchArm {
    fn for_each_span_mut(&mut self, visit: &mut dyn FnMut(&mut Span)) {
        visit(&mut self.span);
        self.pat.for_each_span_mut(visit);
        if let Some(guard) = &mut self.guard {
            guard.for_each_span_mut(visit);
        }
        self.body.for_each_span_mut(visit);
    }
}

impl FieldInit {
    fn for_each_span_mut(&mut self, visit: &mut dyn FnMut(&mut Span)) {
        visit(&mut self.name_span);
        self.value.for_each_span_mut(visit);
    }
}

impl ClosureExpr {
    fn for_each_span_mut(&mut self, visit: &mut dyn FnMut(&mut Span)) {
        for param in &mut self.params {
            param.for_each_span_mut(visit);
        }
        if let Some(ret) = &mut self.ret {
            ret.for_each_span_mut(visit);
        }
        self.body.for_each_span_mut(visit);
    }
}

impl ClosureParam {
    fn for_each_span_mut(&mut self, visit: &mut dyn FnMut(&mut Span)) {
        self.pat.for_each_span_mut(visit);
        if let Some(ty) = &mut self.ty {
            ty.for_each_span_mut(visit);
        }
    }
}

impl MacroCall {
    fn for_each_span_mut(&mut self, visit: &mut dyn FnMut(&mut Span)) {
        visit(&mut self.name_span);
        visit(&mut self.span);
        for arg in &mut self.args {
            arg.for_each_span_mut(visit);
        }
    }
}

impl Pattern {
    fn for_each_span_mut(&mut self, visit: &mut dyn FnMut(&mut Span)) {
        visit(&mut self.span);
        match &mut self.kind {
            PatternKind::Wild => {}
            PatternKind::Binding { .. } => {}
            PatternKind::Lit(_) => {}
            PatternKind::Tuple(patterns) => {
                for pattern in patterns {
                    pattern.for_each_span_mut(visit);
                }
            }
            PatternKind::Path { path, args } => {
                path.for_each_span_mut(visit);
                for arg in args {
                    arg.for_each_span_mut(visit);
                }
            }
            PatternKind::Struct { path, fields, .. } => {
                path.for_each_span_mut(visit);
                for field in fields {
                    field.for_each_span_mut(visit);
                }
            }
            PatternKind::Or(patterns) => {
                for pattern in patterns {
                    pattern.for_each_span_mut(visit);
                }
            }
        }
    }
}

impl FieldPat {
    fn for_each_span_mut(&mut self, visit: &mut dyn FnMut(&mut Span)) {
        visit(&mut self.name_span);
        self.pat.for_each_span_mut(visit);
    }
}

impl TypeExpr {
    fn for_each_span_mut(&mut self, visit: &mut dyn FnMut(&mut Span)) {
        match self {
            TypeExpr::Path {
                name_span,
                generics,
                ..
            } => {
                visit(name_span);
                for arg in generics {
                    arg.for_each_span_mut(visit);
                }
            }
            TypeExpr::Tuple { elems, span } => {
                visit(span);
                for elem in elems {
                    elem.for_each_span_mut(visit);
                }
            }
            TypeExpr::Fn { params, ret, span } => {
                visit(span);
                for param in params {
                    param.for_each_span_mut(visit);
                }
                ret.for_each_span_mut(visit);
            }
            TypeExpr::Ref { inner, span, .. } => {
                visit(span);
                inner.for_each_span_mut(visit);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;
    use crate::parser::parse;
    use crate::span::SourceFile;

    /// A program that exercises every AST node kind.
    const EVERY_NODE_KIND: &str = r#"
struct Point<T> {
    pub x: T,
    pub y: T,
}

struct Wrapper<T> {
    value: T,
}

enum Maybe<T> {
    Nothing,
    Just(T),
}

trait Shape<T> {
    pub fn area(&self) -> i64;
    fn scale(&mut self, factor: i64);
    fn into_point(self) -> Point<i64>;
}

trait IntoValue<T> {
    fn into_value(self) -> T;
}

const LIMIT: i64 = 10;

type Number = i64;

impl<T> Wrapper<T> {
    fn get(&self) -> &T {
        &self.value
    }
}

impl<T> IntoValue<T> for Wrapper<T> {
    fn into_value(self) -> T {
        self.value
    }
}

impl Shape<i64> for Point<i64> {
    fn area(&self) -> i64 {
        self.x
    }

    fn scale(&mut self, factor: i64) {
        self.x = self.x + factor;
    }

    fn into_point(self) -> Point<i64> {
        self
    }
}

impl Drop for Point<i64> {
    fn drop(&mut self) {
    }
}

fn make<T, U: Iterator<i64> + Drop>(first: T, second: U) -> i64 {
    let _ = first;
    let _ = second;
    0
}

fn callable(callback: fn(i64, String) -> bool, generic: Fn(i64) -> bool) -> bool {
    callback(1, "text") && generic(2)
}

fn fallible() -> Option<i64> {
    Some(1)
}

fn pick() -> Option<i64> {
    let value = fallible()?;
    Some(value)
}

fn returns_value() -> i64 {
    return 1;
}

fn main() {
    let mut x = 1;
    let float = 1.5;
    let text = "hello";
    let flag = true;
    let negative = -x;
    let inverted = !flag;
    let arithmetic = x + 1 * 2 - 3 / 1 % 2;
    let comparison = x == 1 || x != 2 && x < 3 && x <= 4 && x > 0 && x >= -1;
    let bits = (x & 1) | (x ^ 2) | (x << 1) | (x >> 1);
    x = 2;
    x += 1;
    let cast = x as u8;
    let called = make::<i64, i64>(x, 2);
    let wrapped = Wrapper::<i64> { value: 3 };
    let borrowed = wrapped.get::<i64>();
    let field = wrapped.value;
    let tuple = (1, 2, 3);
    let first = tuple.0;
    let pair: (i64, i64) = (1, 2);
    let reference: &i64 = &x;
    let mutable_reference: &mut i64 = &mut x;
    let list = vec![1, 2, 3];
    let repeated = vec![0; 3];
    let indexed = list[0];
    let block = {
        const INNER: i64 = 1;
        let inner = INNER;
        ;
        inner
    };
    let chosen = if x > 0 { 1 } else if x == 0 { 0 } else { -1 };
    let mut counter = 0;
    while counter < 3 {
        counter += 1;
    }
    loop {
        break;
    }
    loop {
        continue;
        ;
    }
    for k in 0..3 {
        let _ = k;
    }
    for m in 0..=3 {
        let _ = m;
    }
    for mut n in 0..1 {
        n = 1;
    }
    let matched = match x {
        0 | 1 => 1,
        _ => 0,
    };
    let guarded = match x {
        value if value > 0 => value,
        _ => 0,
    };
    let tuple_matched = match tuple {
        (a, b, c) => a + b + c,
    };
    let maybe = Maybe::Just(4);
    let unwrapped = match maybe {
        Maybe::Nothing => 0,
        Maybe::Just(value) => value,
    };
    let point = Point { x: 1, y: 2 };
    let point_x = match point {
        Point { x, y } => x + y,
    };
    let closure = move |a: i64, b| a + b;
    let typed_closure = |value: i64| -> i64 { value + 1 };
    println!("hello");
    print!("x");
    let shared = &x;
    let unique = &mut x;
    let dereferenced = *shared;
    let _ = (
        float,
        text,
        inverted,
        arithmetic,
        comparison,
        bits,
        cast,
        called,
        borrowed,
        field,
        first,
        pair,
        reference,
        mutable_reference,
        repeated,
        indexed,
        block,
        chosen,
        matched,
        guarded,
        tuple_matched,
        unwrapped,
        point_x,
        closure,
        typed_closure,
        shared,
        unique,
        dereferenced,
        negative,
    );
    return;
}
"#;

    fn parse_test_program() -> Program {
        let file = SourceFile::new("<ast-test>", EVERY_NODE_KIND);
        let (tokens, lex_diagnostics) = tokenize(EVERY_NODE_KIND);
        assert!(
            !lex_diagnostics.has_errors(),
            "AST test source must lex cleanly:
{}",
            lex_diagnostics.render(&file)
        );
        let (program, diagnostics) = parse(&tokens, &file);
        assert!(
            !diagnostics.has_errors(),
            "AST test source must parse cleanly:
{}",
            diagnostics.render(&file)
        );
        program
    }

    /// Every span rendered by the Debug formatter that is not 0..0.
    fn non_zero_spans(program: &Program) -> Vec<String> {
        let text = format!("{:?}", program);
        let bytes = text.as_bytes();
        let mut found = Vec::new();
        let mut index = 0;
        while index < bytes.len() {
            if !bytes[index].is_ascii_digit() {
                index += 1;
                continue;
            }
            let start = index;
            while index < bytes.len() && bytes[index].is_ascii_digit() {
                index += 1;
            }
            if index + 1 < bytes.len() && bytes[index] == b'.' && bytes[index + 1] == b'.' {
                let end_start = index + 2;
                let mut end = end_start;
                while end < bytes.len() && bytes[end].is_ascii_digit() {
                    end += 1;
                }
                if end > end_start {
                    let span = &text[start..end];
                    if span != "0..0" {
                        found.push(span.to_string());
                    }
                    index = end;
                    continue;
                }
            }
        }
        found
    }

    #[test]
    fn every_node_span_is_visited() {
        let mut program = parse_test_program();
        assert!(
            !non_zero_spans(&program).is_empty(),
            "the test source should produce spans before the walk"
        );

        let mut visited = 0usize;
        program.for_each_span_mut(&mut |span: &mut Span| {
            *span = Span::empty(0);
            visited += 1;
        });
        assert!(visited > 0, "the span walk must visit the tree");

        let leftover = non_zero_spans(&program);
        assert!(
            leftover.is_empty(),
            "the span walk missed {:?} (Debug output still has non-zero spans)",
            leftover
        );

        let mut second_visit = 0usize;
        program.for_each_span_mut(&mut |span: &mut Span| {
            assert_eq!(
                *span,
                Span::empty(0),
                "the second walk saw a span the first walk did not clear"
            );
            second_visit += 1;
        });
        assert_eq!(
            visited, second_visit,
            "both walks must visit the same number of spans"
        );
    }
}
