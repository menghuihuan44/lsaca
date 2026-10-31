//! The parser: tokens to an AST.
//!
//! The parser is a hand written recursive descent parser that follows the
//! grammar in docs/language/02-program-structure.md. It accumulates diagnostics instead of panicking:
//! on malformed input it reports a clear error, recovers to a statement or item
//! boundary, and keeps going so that one run can report several independent
//! problems. The returned AST is always well formed structurally (all required
//! nodes are present), though it may contain error placeholders.

use crate::ast::*;
use crate::diag::{Diagnostic, Diagnostics};
use crate::lexer::{Token, TokenKind};
use crate::span::{SourceFile, Span};
use crate::types::SelfKind;

/// Parse a token stream into a program. The token stream should end with
/// TokenKind::Eof (the lexer guarantees that); a missing Eof is tolerated.
///
/// The recursive descent parser spends roughly twenty kilobytes of debug-build
/// stack per nesting level, and MAX_NESTING_DEPTH levels need more than the
/// 1 MiB stack that Windows gives to the main thread. Parsing therefore runs on
/// a dedicated thread with a generous stack, so deeply nested (but still
/// accepted) source code cannot abort the process. Parsing itself never
/// panics; the fallback below only guards against a failed thread spawn.
pub fn parse(tokens: &[Token], file: &SourceFile) -> (Program, Diagnostics) {
    const PARSE_STACK_SIZE: usize = 16 * 1024 * 1024;
    std::thread::scope(|scope| {
        let spawned = std::thread::Builder::new()
            .name("lsaca-parser".to_string())
            .stack_size(PARSE_STACK_SIZE)
            .spawn_scoped(scope, || {
                let mut parser = Parser::new(tokens, file);
                let mut program = parser.parse_program();
                parser.check_ast_depth(&mut program);
                (program, parser.diagnostics)
            });
        match spawned {
            Ok(handle) => handle.join().unwrap_or_else(|_| parser_failure()),
            // A thread could not be spawned (for example because the process
            // hit a resource limit); parse on the current thread instead.
            Err(_) => {
                let mut parser = Parser::new(tokens, file);
                let mut program = parser.parse_program();
                parser.check_ast_depth(&mut program);
                (program, parser.diagnostics)
            }
        }
    })
}

/// Result returned when the parser thread itself failed.
fn parser_failure() -> (Program, Diagnostics) {
    let mut diagnostics = Diagnostics::new();
    diagnostics.error(
        "internal parser error: the parser thread failed",
        Span::new(0, 0),
    );
    (Program { items: Vec::new() }, diagnostics)
}

/// Names that are sugar for Option and Result variants in patterns. A bare
/// occurrence of one of these names is a path pattern, not a binding, so that
/// 'match x { None => ... }' matches the option variant instead of binding it.
const SUGAR_VARIANT_NAMES: [&str; 4] = ["None", "Some", "Ok", "Err"];

/// Maximum recursion depth of the recursive descent parser. Deeper input is
/// rejected with a diagnostic instead of risking a stack overflow, so that
/// pathological source code cannot abort the compiler. A debug build needs
/// roughly 1.3 MB of stack for 64 expression nesting levels; [parse] runs the
/// parser on a dedicated 16 MiB thread so this limit fits comfortably, and no
/// sane program comes close.
const MAX_NESTING_DEPTH: usize = 64;

/// Maximum structural depth of the finished AST, counted as the number of AST
/// nodes on the deepest path from an item down to a leaf. The recursive descent
/// parser folds long operator, unary and postfix chains iteratively, so
/// MAX_NESTING_DEPTH never sees them; every later pass (type checking, HIR
/// lowering, borrow and move checking, code generation) walks the tree
/// recursively, one stack frame per level, so pathologically deep chains would
/// overflow their stacks. Parser::check_ast_depth validates the finished tree
/// with an explicit stack and rejects anything deeper than this limit with the
/// same "this code is nested too deeply" diagnostic as the recursive descent
/// guard (code E0001).
///
/// 128 levels is far below the roughly 300-800 expression levels that overflow
/// a 1 MiB debug stack, and far above the nesting that the shipped prelude,
/// examples and fixtures contain.
const MAX_AST_DEPTH: usize = 128;

/// Diagnostic code used for the syntax restrictions that target/v15/DESIGN.md
/// specifies as E0001: empty generic parameter lists, generic type aliases,
/// trait declaration shape violations and unsupported trait paths, plus the
/// post-parse check that rejects pathologically deep syntax.
const E_SYNTAX: &str = "E0001";

/// Maximum number of tokens scanned to find the top-level 'for' of an impl head.
const MAX_IMPL_HEAD_TOKENS: usize = 128;

/// A prefix operator collected by parse_unary before its operand is parsed.
/// Prefix operators are right associative, so they are folded from the
/// innermost one outwards after the postfix operand has been parsed.
#[derive(Clone, Copy)]
enum PrefixOp {
    Neg,
    Not,
    Ref,
    RefMut,
    Deref,
}

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
    file: &'a SourceFile,
    diagnostics: Diagnostics,
    /// Set when a '>>' token has been split into two '>'. The second '>' is
    /// reported by peek_kind and consumed by the next eat_gt call.
    pending_gt: Option<Span>,
    /// Rust-like restriction: struct literals are not allowed in the condition
    /// of if/while, in for iterables or in match scrutinees, because the opening
    /// brace would be ambiguous with the following block.
    allow_struct_literal: bool,
    /// Synthetic end-of-file token used when the token slice is empty or does
    /// not end with Eof.
    eof: Token,
    /// Current recursion depth of the expression, type, pattern and block
    /// parsers; see MAX_NESTING_DEPTH.
    depth: usize,
    /// Whether the "nested too deeply" diagnostic has already been reported, so
    /// that recovery does not repeat it at every truncated recursion level.
    reported_deep_nesting: bool,
    /// Span of the first "nested too deeply" report. The post-parse depth check
    /// uses it to emit that diagnostic when the recursive descent guard fired
    /// even if the recovered tree turned out shallow enough to keep.
    deep_nesting_span: Option<Span>,
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a [Token], file: &'a SourceFile) -> Parser<'a> {
        let eof = match tokens.last() {
            Some(token) if matches!(token.kind, TokenKind::Eof) => token.clone(),
            _ => Token {
                kind: TokenKind::Eof,
                span: Span::empty(file.len() as u32),
            },
        };
        Parser {
            tokens,
            pos: 0,
            file,
            diagnostics: Diagnostics::new(),
            pending_gt: None,
            allow_struct_literal: true,
            eof,
            depth: 0,
            reported_deep_nesting: false,
            deep_nesting_span: None,
        }
    }

    // ---------------------------------------------------------------------
    // Token helpers
    // ---------------------------------------------------------------------

    fn current(&self) -> &Token {
        match self.tokens.get(self.pos) {
            Some(token) => token,
            None => &self.eof,
        }
    }

    fn kind(&self) -> &TokenKind {
        &self.current().kind
    }

    fn span(&self) -> Span {
        self.current().span
    }

    /// The kind the parser would see, honouring a split '>>' token.
    fn peek_kind(&self) -> TokenKind {
        if self.pending_gt.is_some() {
            return TokenKind::Gt;
        }
        self.kind().clone()
    }

    /// Kind of a token at an offset from the current position, ignoring any
    /// pending split. Used for small positive lookaheads.
    fn peek_kind_at(&self, offset: usize) -> Option<TokenKind> {
        self.tokens
            .get(self.pos.saturating_add(offset))
            .map(|token| token.kind.clone())
    }

    fn at(&self, kind: TokenKind) -> bool {
        if self.pending_gt.is_some() {
            return matches!(kind, TokenKind::Gt);
        }
        std::mem::discriminant(self.kind()) == std::mem::discriminant(&kind)
    }

    fn at_eof(&self) -> bool {
        matches!(self.kind(), TokenKind::Eof)
    }

    fn prev_span(&self) -> Span {
        match self.pos.checked_sub(1).and_then(|pos| self.tokens.get(pos)) {
            Some(token) => token.span,
            None => self.span(),
        }
    }

    /// Consume and return the current token. Never advances past Eof. A pending
    /// split '>' is consumed as a virtual Gt token.
    fn bump(&mut self) -> Token {
        if let Some(span) = self.pending_gt.take() {
            return Token {
                kind: TokenKind::Gt,
                span,
            };
        }
        let token = self.current().clone();
        if !matches!(token.kind, TokenKind::Eof) {
            self.pos += 1;
        }
        token
    }

    fn eat(&mut self, kind: TokenKind) -> Option<Token> {
        if self.at(kind) {
            Some(self.bump())
        } else {
            None
        }
    }

    fn expect(&mut self, kind: TokenKind, what: &str) -> Option<Token> {
        if self.at(kind.clone()) {
            Some(self.bump())
        } else {
            let span = self.span();
            let found = self.peek_kind().describe();
            self.error(format!("expected {}, found {}", what, found), span);
            None
        }
    }

    fn error(&mut self, message: impl Into<String>, span: Span) {
        self.diagnostics.push(Diagnostic::error(message, span));
    }

    /// Push an error that carries the stable E0001 code used by the v1.5
    /// syntax restrictions named in the design contract.
    fn error_coded(&mut self, code: &'static str, message: impl Into<String>, span: Span) {
        self.diagnostics
            .push(Diagnostic::error(message, span).with_code(code));
    }

    fn token_text(&self, span: Span) -> String {
        self.file
            .text()
            .get(span.start as usize..span.end as usize)
            .map(|text| text.to_string())
            .unwrap_or_default()
    }

    /// Consume one '>' from either a Gt token or the first half of a Shr token.
    fn eat_gt(&mut self) -> Option<Span> {
        if let Some(span) = self.pending_gt.take() {
            return Some(span);
        }
        match self.kind() {
            TokenKind::Gt => {
                let span = self.span();
                self.pos += 1;
                Some(span)
            }
            TokenKind::Shr => {
                let span = self.span();
                self.pos += 1;
                // Split '>>' into '>' + '>', leaving the second one pending.
                let first = Span::new(span.start, span.start.saturating_add(1));
                let second = Span::new(span.start.saturating_add(1), span.end);
                self.pending_gt = Some(second);
                Some(first)
            }
            _ => None,
        }
    }

    fn at_gt(&self) -> bool {
        self.pending_gt.is_some() || matches!(self.kind(), TokenKind::Gt)
    }

    /// Guarantee forward progress in a recovery loop.
    fn ensure_progress(&mut self, before: usize) {
        if self.pos != before {
            return;
        }
        // A split '>>' leaves a virtual '>' pending. Consuming it counts as
        // progress so that nested generic argument lists advance normally.
        if self.pending_gt.take().is_some() {
            return;
        }
        if !self.at_eof() {
            let span = self.span();
            self.error("unexpected token", span);
            self.pos += 1;
        }
    }

    // ---------------------------------------------------------------------
    // Recursion guard
    // ---------------------------------------------------------------------

    /// Enter one level of recursive descent. Returns false, after reporting a
    /// diagnostic once, when the input is nested too deeply; the caller then
    /// returns a placeholder instead of recursing any further.
    fn enter_nesting(&mut self) -> bool {
        if self.depth >= MAX_NESTING_DEPTH {
            if !self.reported_deep_nesting {
                self.reported_deep_nesting = true;
                let span = self.span();
                self.deep_nesting_span = Some(span);
                self.error("this code is nested too deeply", span);
            }
            return false;
        }
        self.depth += 1;
        true
    }

    /// Leave one level of recursive descent entered with enter_nesting.
    fn leave_nesting(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }

    /// Validate the structural depth of the finished AST after parsing, and
    /// discard trees that are too deep for the later recursive passes.
    ///
    /// The recursive descent parser folds long operator, unary and postfix
    /// chains iteratively, so `enter_nesting` never sees them and the finished
    /// tree can be arbitrarily deep even though parsing itself used little
    /// stack. Later passes walk the tree recursively, so this method measures
    /// the whole tree with an explicit stack of its own (it can never overflow)
    /// and reports the "nested too deeply" diagnostic (E0001) when the deepest
    /// path is deeper than MAX_AST_DEPTH.
    ///
    /// A too-deep tree is deliberately leaked before it is returned: dropping a
    /// 20 000 level chain of `Box`ed expressions recurses through the drop glue
    /// and would overflow the stack before the driver could even print the
    /// diagnostic (the driver drops the AST when parsing fails). The leak is
    /// bounded by the size of the input and only happens for input that is
    /// already rejected, so the process is about to report errors instead of
    /// compiling it.
    fn check_ast_depth(&mut self, program: &mut Program) {
        let exceeded = {
            let mut stack: Vec<(AstNode<'_>, usize)> = Vec::new();
            for item in &program.items {
                stack.push((AstNode::Item(item), 1));
            }
            let mut exceeded = None;
            while let Some((node, depth)) = stack.pop() {
                if depth > MAX_AST_DEPTH {
                    exceeded = Some(node.span());
                    break;
                }
                node.push_children(&mut stack, depth + 1);
            }
            exceeded
        };
        let Some(span) = exceeded else {
            // The recovered tree is shallow enough to keep, but the recursive
            // descent guard may still have rejected part of the input.
            if self.reported_deep_nesting {
                let span = self.deep_nesting_span.unwrap_or_else(|| Span::new(0, 0));
                self.report_depth_error_only(span);
            }
            return;
        };
        // The tree is too deep for the recursive passes and may also be far too
        // deep to drop recursively, so replace it with an empty program and
        // deliberately leak the rejected tree instead of dropping it.
        let too_deep = std::mem::replace(program, Program { items: Vec::new() });
        std::mem::forget(too_deep);
        self.report_depth_error_only(span);
    }

    /// Replace every parser diagnostic with a single "nested too deeply" error.
    ///
    /// Error recovery on pathological input emits tens of thousands of
    /// diagnostics (5 000 opening parentheses already produce nearly 15 000),
    /// and every diagnostic renders the whole source line below it. On a huge
    /// single-line file that means hundreds of megabytes of output and tens of
    /// seconds of rendering for one file that is rejected anyway, so a
    /// depth-rejected file reports only the one diagnostic that explains why it
    /// cannot be compiled at all.
    fn report_depth_error_only(&mut self, span: Span) {
        self.reported_deep_nesting = true;
        self.diagnostics = Diagnostics::new();
        self.error_coded(E_SYNTAX, "this code is nested too deeply", span);
    }

    /// The placeholder expression used when a construct had to be abandoned.
    fn error_expr(&self) -> Expr {
        let span = self.span();
        Expr::new(
            ExprKind::Path(PathExpr {
                segments: vec![PathSegment {
                    name: "<error>".to_string(),
                    span,
                    generics: Vec::new(),
                }],
                span,
            }),
            span,
        )
    }

    fn expect_ident(&mut self, what: &str) -> (String, Span) {
        match self.kind() {
            TokenKind::Ident(name) => {
                let name = name.clone();
                let span = self.span();
                self.bump();
                (name, span)
            }
            _ => {
                let found = self.peek_kind().describe();
                let span = self.span();
                self.error(format!("expected {}, found {}", what, found), span);
                if !self.at_eof() && !self.at(TokenKind::RBrace) {
                    self.bump();
                }
                ("<error>".to_string(), span)
            }
        }
    }

    /// Accepts a plain identifier or the keyword `Self` as a type name.
    fn expect_type_name_ident(&mut self) -> (String, Span) {
        match self.kind() {
            TokenKind::KwSelfTy => {
                let span = self.span();
                self.bump();
                ("Self".to_string(), span)
            }
            _ => self.expect_ident("type name"),
        }
    }

    fn expect_path_ident(&mut self) -> (String, Span) {
        match self.kind() {
            TokenKind::Ident(name) => {
                let name = name.clone();
                let span = self.span();
                self.bump();
                (name, span)
            }
            TokenKind::KwSelf => {
                let span = self.span();
                self.bump();
                ("self".to_string(), span)
            }
            TokenKind::KwSelfTy => {
                let span = self.span();
                self.bump();
                ("Self".to_string(), span)
            }
            _ => self.expect_ident("identifier"),
        }
    }

    fn is_item_start(&self) -> bool {
        let mut offset = 0usize;
        if matches!(self.kind(), TokenKind::KwPub) {
            offset = 1;
        }
        match self.tokens.get(self.pos.saturating_add(offset)) {
            Some(token) => matches!(
                token.kind,
                TokenKind::KwFn
                    | TokenKind::KwStruct
                    | TokenKind::KwEnum
                    | TokenKind::KwImpl
                    | TokenKind::KwConst
                    | TokenKind::KwType
                    | TokenKind::KwTrait
            ),
            None => false,
        }
    }

    fn starts_expression(&self) -> bool {
        matches!(
            self.peek_kind(),
            TokenKind::Int(_)
                | TokenKind::Float(_)
                | TokenKind::Str(_)
                | TokenKind::Char(_)
                | TokenKind::Ident(_)
                | TokenKind::KwTrue
                | TokenKind::KwFalse
                | TokenKind::KwIf
                | TokenKind::KwMatch
                | TokenKind::KwLoop
                | TokenKind::KwWhile
                | TokenKind::KwFor
                | TokenKind::KwMove
                | TokenKind::KwSelf
                | TokenKind::KwSelfTy
                | TokenKind::LParen
                | TokenKind::LBrace
                | TokenKind::LBracket
                | TokenKind::Minus
                | TokenKind::Bang
                | TokenKind::Amp
                | TokenKind::AmpAmp
                | TokenKind::Star
                | TokenKind::Pipe
                | TokenKind::PipePipe
        )
    }

    // ---------------------------------------------------------------------
    // Recovery
    // ---------------------------------------------------------------------

    /// Skip to the next item keyword at the top level.
    fn synchronize_item(&mut self) {
        while !self.at_eof() && !self.is_item_start() {
            self.bump();
        }
    }

    // ---------------------------------------------------------------------
    // Program and items
    // ---------------------------------------------------------------------

    fn parse_program(&mut self) -> Program {
        let mut items = Vec::new();
        while !self.at_eof() {
            let before = self.pos;
            if let Some(item) = self.parse_item() {
                items.push(item);
            }
            self.ensure_progress(before);
        }
        Program { items }
    }

    fn parse_item(&mut self) -> Option<Item> {
        let had_pub = self.eat(TokenKind::KwPub).is_some();
        match self.peek_kind() {
            TokenKind::KwFn => Some(Item::Fn(self.parse_fn_decl(None))),
            TokenKind::KwStruct => Some(Item::Struct(self.parse_struct_decl())),
            TokenKind::KwEnum => Some(Item::Enum(self.parse_enum_decl())),
            TokenKind::KwImpl => Some(Item::Impl(self.parse_impl_block())),
            TokenKind::KwConst => Some(Item::Const(self.parse_const_decl())),
            TokenKind::KwType => Some(Item::TypeAlias(self.parse_type_alias())),
            TokenKind::KwTrait => Some(Item::Trait(self.parse_trait_decl())),
            TokenKind::Eof => None,
            _ => {
                let found = self.peek_kind().describe();
                let span = self.span();
                self.error(
                    format!(
                        "expected an item (fn, struct, enum, impl, const, type or trait), found {}",
                        found
                    ),
                    span,
                );
                if had_pub {
                    self.error("expected an item after pub", span);
                }
                self.synchronize_item();
                None
            }
        }
    }

    fn parse_fn_decl(&mut self, impl_ty: Option<&TypeExpr>) -> FnDecl {
        let fn_token = self.expect(TokenKind::KwFn, "fn");
        let start = fn_token
            .map(|token| token.span)
            .unwrap_or_else(|| self.span());
        let (name, name_span) = self.expect_ident("function name");
        let generics = if self.at(TokenKind::Lt) {
            self.parse_generic_params()
        } else {
            Vec::new()
        };
        let (params, self_kind) = self.parse_params(impl_ty);
        let ret = if self.eat(TokenKind::Arrow).is_some() {
            Some(self.parse_type())
        } else {
            None
        };
        let body = if self.at(TokenKind::LBrace) {
            self.parse_block()
        } else {
            let span = self.span();
            self.error("expected function body", span);
            Block::empty(Span::empty(span.start))
        };
        let body_span = body.span;
        FnDecl {
            name,
            name_span,
            generics,
            params,
            ret,
            body,
            self_kind,
            span: start.join(body_span),
        }
    }

    fn parse_params(&mut self, impl_ty: Option<&TypeExpr>) -> (Vec<Param>, Option<SelfKind>) {
        let mut params = Vec::new();
        let mut self_kind = None;
        self.expect(TokenKind::LParen, "'('");
        let mut first = true;
        loop {
            if self.at(TokenKind::RParen) || self.at_eof() {
                break;
            }
            let before = self.pos;
            if first && impl_ty.is_some() {
                if self.at(TokenKind::KwSelf) {
                    let token = self.bump();
                    let ty = impl_ty.cloned().unwrap_or_else(|| unit_type(token.span));
                    self_kind = Some(SelfKind::Value);
                    params.push(Param {
                        name: "self".to_string(),
                        name_span: token.span,
                        ty,
                    });
                } else if self.at(TokenKind::Amp) {
                    let amp = self.bump();
                    let mutable = self.eat(TokenKind::KwMut).is_some();
                    if self.at(TokenKind::KwSelf) {
                        let token = self.bump();
                        let ty = impl_ty.cloned().unwrap_or_else(|| unit_type(amp.span));
                        self_kind = Some(if mutable {
                            SelfKind::RefMut
                        } else {
                            SelfKind::Ref
                        });
                        params.push(Param {
                            name: "self".to_string(),
                            name_span: token.span,
                            ty,
                        });
                    } else {
                        let span = self.span();
                        self.error("expected self after &", span);
                    }
                } else {
                    params.push(self.parse_param());
                }
            } else if first && self.at(TokenKind::KwSelf) {
                let token = self.bump();
                self.error("self parameter is only allowed in impl blocks", token.span);
                let ty = unit_type(token.span);
                params.push(Param {
                    name: "self".to_string(),
                    name_span: token.span,
                    ty,
                });
            } else {
                params.push(self.parse_param());
            }
            first = false;
            self.ensure_progress(before);
            if self.eat(TokenKind::Comma).is_some() {
                continue;
            }
            break;
        }
        self.expect(TokenKind::RParen, "')'");
        (params, self_kind)
    }

    fn parse_param(&mut self) -> Param {
        let pattern = self.parse_pattern_atomic();
        self.expect(TokenKind::Colon, "':' after parameter name");
        let ty = self.parse_type();
        match &pattern.kind {
            PatternKind::Binding { name, .. } => Param {
                name: name.clone(),
                name_span: pattern.span,
                ty,
            },
            _ => {
                self.error("parameter pattern must be a simple name", pattern.span);
                Param {
                    name: "_".to_string(),
                    name_span: pattern.span,
                    ty,
                }
            }
        }
    }

    fn parse_struct_decl(&mut self) -> StructDecl {
        let struct_token = self.expect(TokenKind::KwStruct, "struct");
        let start = struct_token
            .map(|token| token.span)
            .unwrap_or_else(|| self.span());
        let (name, name_span) = self.expect_ident("struct name");
        let generics = if self.at(TokenKind::Lt) {
            self.parse_generic_params()
        } else {
            Vec::new()
        };
        let mut fields = Vec::new();
        self.expect(TokenKind::LBrace, "'{'");
        loop {
            if self.at(TokenKind::RBrace) || self.at_eof() {
                break;
            }
            let before = self.pos;
            // A written `pub` on a field is accepted and ignored: version 1.5
            // has no field visibility.
            self.eat(TokenKind::KwPub);
            let (field_name, field_span) = self.expect_ident("field name");
            self.expect(TokenKind::Colon, "':' after field name");
            let ty = self.parse_type();
            fields.push(FieldDef {
                name: field_name,
                name_span: field_span,
                ty,
            });
            self.ensure_progress(before);
            if self.eat(TokenKind::Comma).is_some() {
                continue;
            }
            break;
        }
        let end = self
            .expect(TokenKind::RBrace, "'}'")
            .map(|token| token.span)
            .unwrap_or_else(|| self.span());
        StructDecl {
            name,
            name_span,
            generics,
            fields,
            span: start.join(end),
        }
    }

    fn parse_enum_decl(&mut self) -> EnumDecl {
        let enum_token = self.expect(TokenKind::KwEnum, "enum");
        let start = enum_token
            .map(|token| token.span)
            .unwrap_or_else(|| self.span());
        let (name, name_span) = self.expect_ident("enum name");
        let generics = if self.at(TokenKind::Lt) {
            self.parse_generic_params()
        } else {
            Vec::new()
        };
        let mut variants = Vec::new();
        self.expect(TokenKind::LBrace, "'{'");
        loop {
            if self.at(TokenKind::RBrace) || self.at_eof() {
                break;
            }
            let before = self.pos;
            // A written `pub` on a variant is accepted and ignored: version
            // 1.5 has no variant visibility.
            self.eat(TokenKind::KwPub);
            let (variant_name, variant_span) = self.expect_ident("variant name");
            let mut fields = Vec::new();
            let mut end = variant_span;
            if self.eat(TokenKind::LParen).is_some() {
                loop {
                    if self.at(TokenKind::RParen) || self.at_eof() {
                        break;
                    }
                    let before_field = self.pos;
                    fields.push(self.parse_type());
                    self.ensure_progress(before_field);
                    if self.eat(TokenKind::Comma).is_some() {
                        continue;
                    }
                    break;
                }
                end = self
                    .expect(TokenKind::RParen, "')'")
                    .map(|token| token.span)
                    .unwrap_or_else(|| self.span());
            }
            variants.push(VariantDef {
                name: variant_name,
                name_span: variant_span,
                fields,
                span: variant_span.join(end),
            });
            self.ensure_progress(before);
            if self.eat(TokenKind::Comma).is_some() {
                continue;
            }
            break;
        }
        let end = self
            .expect(TokenKind::RBrace, "'}'")
            .map(|token| token.span)
            .unwrap_or_else(|| self.span());
        EnumDecl {
            name,
            name_span,
            generics,
            variants,
            span: start.join(end),
        }
    }

    fn parse_impl_block(&mut self) -> ImplBlock {
        let impl_token = self.expect(TokenKind::KwImpl, "impl");
        let start = impl_token
            .map(|token| token.span)
            .unwrap_or_else(|| self.span());
        let generics = if self.at(TokenKind::Lt) {
            self.parse_generic_params()
        } else {
            Vec::new()
        };
        let (trait_name, ty) = self.parse_impl_head();
        let ty_span = ty.span();
        let ty_name = match &ty {
            TypeExpr::Path { name, .. } => name.clone(),
            other => other.describe(),
        };
        let mut fns = Vec::new();
        self.expect(TokenKind::LBrace, "'{'");
        loop {
            if self.at(TokenKind::RBrace) || self.at_eof() {
                break;
            }
            let before = self.pos;
            let had_pub = self.eat(TokenKind::KwPub).is_some();
            if self.at(TokenKind::KwFn) {
                fns.push(self.parse_fn_decl(Some(&ty)));
            } else {
                let span = self.span();
                self.error(
                    if had_pub {
                        "expected a function after pub in impl block"
                    } else {
                        "expected a function in impl block"
                    },
                    span,
                );
                while !self.at_eof()
                    && !self.at(TokenKind::RBrace)
                    && !self.at(TokenKind::KwFn)
                    && !self.at(TokenKind::KwPub)
                {
                    self.bump();
                }
            }
            self.ensure_progress(before);
        }
        let end = self
            .expect(TokenKind::RBrace, "'}'")
            .map(|token| token.span)
            .unwrap_or_else(|| self.span());
        ImplBlock {
            generics,
            ty,
            ty_name,
            ty_span,
            trait_name,
            fns,
            span: start.join(end),
        }
    }

    /// Parse the head of an impl block: an optional trait reference followed by
    /// "for" and the self type, or just the inherent self type.
    fn parse_impl_head(&mut self) -> (Option<TraitRef>, TypeExpr) {
        if self.at(TokenKind::KwFor) {
            let span = self.span();
            let found = self.token_text(span);
            self.error(
                format!("expected a type or trait name, found {}", found),
                span,
            );
            // Recovery: the stray 'for' is skipped and the next type is taken
            // as the inherent impl self type.
            self.bump();
            return (None, self.parse_type());
        }
        let for_offset = match self.find_impl_head_for() {
            Some(offset) => offset,
            None => return (None, self.parse_type()),
        };
        if !matches!(self.kind(), TokenKind::Ident(_)) {
            let span = self.span();
            let found = self.token_text(span);
            self.error(
                format!(
                    "a trait impl must name the trait with a single identifier, found {}",
                    found
                ),
                span,
            );
            self.skip_impl_head_to_for(for_offset);
            return (None, self.parse_type());
        }
        let (name, name_span) = self.expect_ident("trait name");
        let args = if self.at(TokenKind::Lt) {
            self.parse_type_args()
        } else {
            Vec::new()
        };
        let head_end = if args.is_empty() {
            name_span
        } else {
            // Include the closing '>' of the argument list in the span.
            self.prev_span()
        };
        let trait_ref = TraitRef {
            name,
            name_span,
            args,
            span: name_span.join(head_end),
        };
        // find_impl_head_for found a top-level for, so it is the current token
        // unless a malformed argument list already consumed it; the skip helper
        // re-synchronizes either way.
        if self.at(TokenKind::KwFor) {
            self.bump();
            return (Some(trait_ref), self.parse_type());
        }
        let span = self.span();
        self.error_coded(
            E_SYNTAX,
            "trait paths are not supported in impl blocks; a trait impl must name the trait with a single identifier",
            span,
        );
        self.skip_impl_head_to_for(for_offset);
        (None, self.parse_type())
    }

    /// Advance to the previously located top-level for keyword and consume it.
    fn skip_impl_head_to_for(&mut self, for_offset: usize) {
        while self.pos < for_offset && !self.at_eof() {
            self.bump();
        }
        if self.at(TokenKind::KwFor) {
            self.bump();
        }
    }

    /// Locate the top-level for that separates a trait reference from the self
    /// type in an impl head. Returns the absolute token index of for, or None
    /// when the head is an inherent impl (no top-level for before the body).
    fn find_impl_head_for(&self) -> Option<usize> {
        let mut offset = 0usize;
        let mut angle_depth = 0i32;
        while offset < MAX_IMPL_HEAD_TOKENS {
            let kind = self.peek_kind_at(offset)?;
            match kind {
                TokenKind::Lt => angle_depth += 1,
                TokenKind::Gt => angle_depth = (angle_depth - 1).max(0),
                TokenKind::Shr => angle_depth = (angle_depth - 2).max(0),
                TokenKind::KwFor if angle_depth == 0 => return Some(self.pos + offset),
                TokenKind::LBrace | TokenKind::RBrace | TokenKind::Semi | TokenKind::Eof
                    if angle_depth == 0 =>
                {
                    return None;
                }
                _ => {}
            }
            offset += 1;
        }
        None
    }

    /// Parse a trait declaration. Version 1.5 traits only carry method
    /// signatures: every method takes a receiver and ends with a semicolon.
    fn parse_trait_decl(&mut self) -> TraitDecl {
        let start = self.span();
        self.expect(TokenKind::KwTrait, "'trait'");
        let (name, name_span) = self.expect_ident("trait name");
        let generics = if self.at(TokenKind::Lt) {
            self.parse_generic_params()
        } else {
            Vec::new()
        };
        if self.at(TokenKind::Colon) {
            let span = self.span();
            self.error_coded(
                E_SYNTAX,
                "supertraits are not supported; declare the inherited methods in this trait instead",
                span,
            );
            while !self.at(TokenKind::LBrace)
                && !self.at(TokenKind::RBrace)
                && !self.at(TokenKind::Semi)
                && !self.at_eof()
            {
                self.bump();
            }
        }
        let mut methods = Vec::new();
        if self
            .expect(TokenKind::LBrace, "'{' after trait name")
            .is_none()
        {
            let end = self.prev_span();
            return TraitDecl {
                name,
                name_span,
                generics,
                methods,
                span: start.join(end),
            };
        }
        loop {
            if self.at(TokenKind::RBrace) || self.at_eof() {
                break;
            }
            let before = self.pos;
            // A written pub before a trait method is accepted and ignored.
            self.eat(TokenKind::KwPub);
            if self.at(TokenKind::KwFn) {
                methods.push(self.parse_trait_method());
            } else {
                let span = self.span();
                self.error("expected a method signature in trait body", span);
                while !self.at(TokenKind::RBrace)
                    && !self.at(TokenKind::KwFn)
                    && !self.at(TokenKind::KwPub)
                    && !self.at_eof()
                {
                    self.bump();
                }
            }
            self.ensure_progress(before);
        }
        let end = self.prev_span();
        self.expect(TokenKind::RBrace, "'}' after trait body");
        TraitDecl {
            name,
            name_span,
            generics,
            methods,
            span: start.join(end),
        }
    }

    /// Parse one trait method signature, which must end with a semicolon.
    fn parse_trait_method(&mut self) -> TraitMethod {
        let start = self.span();
        self.expect(TokenKind::KwFn, "'fn'");
        let (name, name_span) = self.expect_ident("trait method name");
        if self.at(TokenKind::Lt) {
            let span = self.span();
            self.error_coded(
                E_SYNTAX,
                "trait methods cannot be generic; remove the type parameters from the method signature",
                span,
            );
            let _ = self.parse_generic_params();
        }
        let (params, self_kind) = self.parse_trait_params(name_span);
        let ret = if self.eat(TokenKind::Arrow).is_some() {
            Some(self.parse_type())
        } else {
            None
        };
        if self.at(TokenKind::LBrace) {
            let span = self.span();
            self.error_coded(
                E_SYNTAX,
                "default method bodies are not supported; end the signature with ';' and write the body in each impl block",
                span,
            );
            let _ = self.parse_block();
        } else if self.eat(TokenKind::Semi).is_none() {
            let span = self.span();
            self.error("expected ';' after trait method signature", span);
        }
        let end = self.prev_span();
        TraitMethod {
            name,
            name_span,
            params,
            ret,
            self_kind,
            span: start.join(end),
        }
    }

    /// Parse the parameter list of a trait method. The first parameter must be
    /// a receiver; a missing receiver is reported once per method.
    fn parse_trait_params(&mut self, method_span: Span) -> (Vec<Param>, Option<SelfKind>) {
        let mut params = Vec::new();
        let mut self_kind = None;
        let mut reported_missing_receiver = false;
        if self
            .expect(TokenKind::LParen, "'(' after trait method name")
            .is_none()
        {
            return (params, self_kind);
        }
        let mut first = true;
        loop {
            if self.at(TokenKind::RParen) || self.at_eof() {
                break;
            }
            let before = self.pos;
            if first {
                if self.at(TokenKind::KwSelf) {
                    let token = self.bump();
                    self_kind = Some(SelfKind::Value);
                    params.push(Param {
                        name: "self".to_string(),
                        name_span: token.span,
                        ty: self_type(token.span),
                    });
                } else if self.at(TokenKind::Amp) {
                    let amp = self.bump();
                    let mutable = self.eat(TokenKind::KwMut).is_some();
                    self.expect(TokenKind::KwSelf, "'self' after '&' in receiver");
                    self_kind = Some(if mutable {
                        SelfKind::RefMut
                    } else {
                        SelfKind::Ref
                    });
                    params.push(Param {
                        name: "self".to_string(),
                        name_span: amp.span,
                        ty: TypeExpr::Ref {
                            mutable,
                            inner: Box::new(self_type(amp.span)),
                            span: amp.span,
                        },
                    });
                } else {
                    let span = self.span();
                    self.error_coded(
                        E_SYNTAX,
                        "trait methods must take a self receiver; add '&self', '&mut self', or 'self' as the first parameter",
                        span,
                    );
                    reported_missing_receiver = true;
                    params.push(self.parse_param());
                }
                first = false;
            } else {
                params.push(self.parse_param());
            }
            self.ensure_progress(before);
            if self.eat(TokenKind::Comma).is_some() {
                continue;
            }
            break;
        }
        self.expect(TokenKind::RParen, "')' after trait method parameters");
        if self_kind.is_none() && !reported_missing_receiver {
            self.error_coded(
                E_SYNTAX,
                "trait methods must take a self receiver; add '&self', '&mut self', or 'self' as the first parameter",
                method_span,
            );
        }
        (params, self_kind)
    }

    fn parse_const_decl(&mut self) -> ConstDecl {
        let const_token = self.expect(TokenKind::KwConst, "const");
        let start = const_token
            .map(|token| token.span)
            .unwrap_or_else(|| self.span());
        let (name, name_span) = self.expect_ident("constant name");
        self.expect(TokenKind::Colon, "':' after constant name");
        let ty = self.parse_type();
        self.expect(TokenKind::Eq, "'=' in constant declaration");
        let value = self.parse_expr();
        let end = self
            .expect(TokenKind::Semi, "';' after constant declaration")
            .map(|token| token.span)
            .unwrap_or_else(|| self.prev_span());
        ConstDecl {
            name,
            name_span,
            ty,
            value,
            span: start.join(end),
        }
    }

    fn parse_type_alias(&mut self) -> TypeAliasDecl {
        let type_token = self.expect(TokenKind::KwType, "type");
        let start = type_token
            .map(|token| token.span)
            .unwrap_or_else(|| self.span());
        let (name, name_span) = self.expect_ident("type alias name");
        let generics = if self.at(TokenKind::Lt) {
            self.parse_generic_params()
        } else {
            Vec::new()
        };
        if let (Some(first), Some(last)) = (generics.first(), generics.last()) {
            self.error_coded(
                E_SYNTAX,
                "type aliases cannot be generic; remove the type parameters from the alias",
                first.name_span.join(last.name_span),
            );
        }
        self.expect(TokenKind::Eq, "'=' in type alias");
        let ty = self.parse_type();
        let end = self
            .expect(TokenKind::Semi, "';' after type alias")
            .map(|token| token.span)
            .unwrap_or_else(|| self.prev_span());
        TypeAliasDecl {
            name,
            name_span,
            generics,
            ty,
            span: start.join(end),
        }
    }

    // ---------------------------------------------------------------------
    // Types
    // ---------------------------------------------------------------------

    fn unit_type_here(&self) -> TypeExpr {
        unit_type(self.span())
    }

    fn parse_type(&mut self) -> TypeExpr {
        if !self.enter_nesting() {
            return self.unit_type_here();
        }
        let ty = self.parse_type_inner();
        self.leave_nesting();
        ty
    }

    fn parse_type_inner(&mut self) -> TypeExpr {
        // Reference types: &T and &mut T. The lexer scans a written '&&' as a
        // single AmpAmp token; it is two nested shared references, &(&T).
        if self.at(TokenKind::AmpAmp) {
            let amp = self.bump();
            let mutable = self.eat(TokenKind::KwMut).is_some();
            let inner = TypeExpr::Ref {
                mutable,
                inner: Box::new(self.parse_type()),
                span: amp.span,
            };
            return TypeExpr::Ref {
                mutable: false,
                inner: Box::new(inner),
                span: amp.span,
            };
        }
        if self.at(TokenKind::Amp) {
            let amp = self.bump();
            let mutable = self.eat(TokenKind::KwMut).is_some();
            return TypeExpr::Ref {
                mutable,
                inner: Box::new(self.parse_type()),
                span: amp.span,
            };
        }
        if self.at(TokenKind::KwFn) {
            let fn_token = self.bump();
            return self.parse_callable_type(fn_token.span);
        }
        // The callable type / bound form Fn(A, B) -> C: an identifier Fn
        // immediately followed by '(' is the callable form, while a bare Fn
        // stays a path type (and lowercase fn(..) keeps the keyword form).
        if matches!(self.kind(), TokenKind::Ident(name) if name == "Fn")
            && matches!(self.peek_kind_at(1), Some(TokenKind::LParen))
        {
            let token = self.bump();
            return self.parse_callable_type(token.span);
        }
        if self.at(TokenKind::LParen) {
            let lp = self.bump();
            if self.at(TokenKind::RParen) {
                let rp = self.bump();
                return TypeExpr::Tuple {
                    elems: Vec::new(),
                    span: lp.span.join(rp.span),
                };
            }
            let first = self.parse_type();
            if self.eat(TokenKind::Comma).is_none() {
                // Parenthesised type: parentheses do not change the type.
                self.expect(TokenKind::RParen, "')' in type");
                return first;
            }
            let mut elems = vec![first];
            loop {
                if self.at(TokenKind::RParen) || self.at_eof() {
                    break;
                }
                let before = self.pos;
                elems.push(self.parse_type());
                self.ensure_progress(before);
                if self.eat(TokenKind::Comma).is_some() {
                    continue;
                }
                break;
            }
            let rp = self
                .expect(TokenKind::RParen, "')' in tuple type")
                .map(|token| token.span)
                .unwrap_or_else(|| self.prev_span());
            return TypeExpr::Tuple {
                elems,
                span: lp.span.join(rp),
            };
        }
        // Path type, possibly generic.
        let (mut name, mut name_span) = self.expect_type_name_ident();
        loop {
            if !self.at(TokenKind::ColonColon) {
                break;
            }
            let turbofish = matches!(self.peek_kind_at(1), Some(TokenKind::Lt));
            self.bump();
            if turbofish {
                let generics = self.parse_type_args();
                return TypeExpr::Path {
                    name,
                    name_span,
                    generics,
                };
            }
            let (part, part_span) = self.expect_type_name_ident();
            name = format!("{}::{}", name, part);
            name_span = name_span.join(part_span);
        }
        let generics = if self.at(TokenKind::Lt) {
            self.parse_type_args()
        } else {
            Vec::new()
        };
        TypeExpr::Path {
            name,
            name_span,
            generics,
        }
    }

    fn parse_type_args(&mut self) -> Vec<TypeExpr> {
        let mut args = Vec::new();
        if self.expect(TokenKind::Lt, "'<'").is_none() {
            return args;
        }
        loop {
            if self.at_gt() || self.at_eof() {
                break;
            }
            let before = self.pos;
            args.push(self.parse_type());
            self.ensure_progress(before);
            if self.eat(TokenKind::Comma).is_some() {
                continue;
            }
            break;
        }
        if self.eat_gt().is_none() {
            let span = self.span();
            self.error("expected '>' to close type arguments", span);
        }
        args
    }

    /// Parse the parameter and return type of a callable type after its fn or
    /// Fn name token has been consumed. Used for the lowercase fn(..) type form
    /// and for the Fn(A, B) -> C bound form.
    fn parse_callable_type(&mut self, start: Span) -> TypeExpr {
        let mut params = Vec::new();
        self.expect(TokenKind::LParen, "'(' in function type");
        loop {
            if self.at(TokenKind::RParen) || self.at_eof() {
                break;
            }
            let before = self.pos;
            params.push(self.parse_type());
            self.ensure_progress(before);
            if self.eat(TokenKind::Comma).is_some() {
                continue;
            }
            break;
        }
        self.expect(TokenKind::RParen, "')' in function type");
        let ret = if self.eat(TokenKind::Arrow).is_some() {
            self.parse_type()
        } else {
            self.unit_type_here()
        };
        let span = start.join(ret.span());
        TypeExpr::Fn {
            params,
            ret: Box::new(ret),
            span,
        }
    }

    /// Parse a generic parameter list: <T, U: Bound + Bound2>.
    ///
    /// The caller has checked that the current token is '<'. An empty list is
    /// rejected with E0001; a trailing comma is allowed. Bounds are parsed as
    /// type expressions so that trait paths with arguments (Iterator<i64>), the
    /// builtin Drop bound and the callable form Fn(A, B) -> C all go through the
    /// normal type parser.
    fn parse_generic_params(&mut self) -> Vec<GenericParam> {
        let mut params = Vec::new();
        if self
            .expect(TokenKind::Lt, "'<' before generic parameters")
            .is_none()
        {
            return params;
        }
        if self.at_gt() {
            let span = self.span();
            self.error_coded(
                E_SYNTAX,
                "generic parameter lists must not be empty; write <T> with at least one parameter",
                span,
            );
            self.eat_gt();
            return params;
        }
        loop {
            if self.at_gt() || self.at_eof() {
                break;
            }
            let before = self.pos;
            let (name, name_span) = self.expect_ident("generic parameter name");
            let mut bounds = Vec::new();
            if self.eat(TokenKind::Colon).is_some() {
                loop {
                    let bound_start = self.pos;
                    bounds.push(self.parse_type());
                    self.ensure_progress(bound_start);
                    if self.eat(TokenKind::Plus).is_none() {
                        break;
                    }
                    if self.at_gt() || self.at(TokenKind::Comma) || self.at_eof() {
                        let span = self.span();
                        self.error("expected a trait bound after '+'", span);
                        break;
                    }
                }
            }
            params.push(GenericParam {
                name,
                name_span,
                bounds,
            });
            self.ensure_progress(before);
            if self.eat(TokenKind::Comma).is_some() {
                continue;
            }
            break;
        }
        if self.eat_gt().is_none() {
            let span = self.span();
            self.error("expected '>' to close the generic parameter list", span);
        }
        params
    }

    // ---------------------------------------------------------------------
    // Patterns
    // ---------------------------------------------------------------------

    fn parse_pattern(&mut self) -> Pattern {
        if !self.enter_nesting() {
            let span = self.span();
            return Pattern {
                kind: PatternKind::Wild,
                span,
            };
        }
        let pattern = self.parse_pattern_inner();
        self.leave_nesting();
        pattern
    }

    fn parse_pattern_inner(&mut self) -> Pattern {
        let first = self.parse_pattern_atomic();
        if !self.at(TokenKind::Pipe) {
            return first;
        }
        let start = first.span;
        let mut alternatives = vec![first];
        while self.eat(TokenKind::Pipe).is_some() {
            alternatives.push(self.parse_pattern_atomic());
        }
        let end = alternatives
            .last()
            .map(|pattern| pattern.span)
            .unwrap_or(start);
        Pattern {
            kind: PatternKind::Or(alternatives),
            span: start.join(end),
        }
    }

    fn parse_pattern_atomic(&mut self) -> Pattern {
        let start = self.span();
        match self.peek_kind() {
            TokenKind::Underscore => {
                self.bump();
                Pattern {
                    kind: PatternKind::Wild,
                    span: start,
                }
            }
            TokenKind::KwMut => {
                self.bump();
                let (name, name_span) = self.expect_ident("binding name");
                Pattern {
                    kind: PatternKind::Binding {
                        name,
                        mutable: true,
                    },
                    span: start.join(name_span),
                }
            }
            TokenKind::KwTrue => {
                self.bump();
                Pattern {
                    kind: PatternKind::Lit(Lit::Bool(true)),
                    span: start,
                }
            }
            TokenKind::KwFalse => {
                self.bump();
                Pattern {
                    kind: PatternKind::Lit(Lit::Bool(false)),
                    span: start,
                }
            }
            TokenKind::Int(value) => {
                let raw = self.token_text(start);
                self.bump();
                let value = self.small_int(value, start);
                Pattern {
                    kind: PatternKind::Lit(Lit::Int { value, raw }),
                    span: start,
                }
            }
            TokenKind::Float(value) => {
                let raw = self.token_text(start);
                self.bump();
                Pattern {
                    kind: PatternKind::Lit(Lit::Float { value, raw }),
                    span: start,
                }
            }
            TokenKind::Str(value) => {
                self.bump();
                Pattern {
                    kind: PatternKind::Lit(Lit::Str(value)),
                    span: start,
                }
            }
            TokenKind::Char(value) => {
                self.bump();
                Pattern {
                    kind: PatternKind::Lit(Lit::Char(value)),
                    span: start,
                }
            }
            TokenKind::Minus => {
                self.bump();
                self.parse_negative_literal_pattern(start)
            }
            TokenKind::Amp | TokenKind::AmpAmp => {
                let token = self.bump();
                self.error(
                    "reference patterns are not supported; match a reference scrutinee directly, bindings become references",
                    token.span,
                );
                // Recover by parsing the pattern after the rejected reference so
                // that one mistake does not derail the enclosing match or let
                // statement. The AST has no reference pattern node, so the inner
                // pattern is kept on its own.
                self.eat(TokenKind::KwMut);
                self.parse_pattern()
            }
            TokenKind::LParen => self.parse_tuple_pattern(),
            TokenKind::Ident(_) | TokenKind::KwSelfTy => self.parse_path_pattern(),
            _ => {
                let found = self.peek_kind().describe();
                let span = self.span();
                self.error(format!("expected a pattern, found {}", found), span);
                if !self.at_eof() {
                    self.bump();
                }
                Pattern {
                    kind: PatternKind::Wild,
                    span,
                }
            }
        }
    }

    fn parse_negative_literal_pattern(&mut self, start: Span) -> Pattern {
        match self.peek_kind() {
            TokenKind::Int(value) => {
                let number_span = self.span();
                let raw = self.token_text(number_span);
                self.bump();
                let value = self.small_int(value, number_span);
                Pattern {
                    kind: PatternKind::Lit(Lit::Int {
                        value: -value,
                        raw: format!("-{}", raw),
                    }),
                    span: start.join(number_span),
                }
            }
            TokenKind::Float(value) => {
                let number_span = self.span();
                let raw = self.token_text(number_span);
                self.bump();
                Pattern {
                    kind: PatternKind::Lit(Lit::Float {
                        value: -value,
                        raw: format!("-{}", raw),
                    }),
                    span: start.join(number_span),
                }
            }
            _ => {
                let span = self.span();
                self.error("expected a numeric literal after '-' in pattern", span);
                Pattern {
                    kind: PatternKind::Wild,
                    span: start,
                }
            }
        }
    }

    fn parse_tuple_pattern(&mut self) -> Pattern {
        let lp = self.bump();
        if self.at(TokenKind::RParen) {
            let rp = self.bump();
            return Pattern {
                kind: PatternKind::Tuple(Vec::new()),
                span: lp.span.join(rp.span),
            };
        }
        let first = self.parse_pattern();
        if self.eat(TokenKind::Comma).is_none() {
            self.expect(TokenKind::RParen, "')' in pattern");
            return first;
        }
        let mut elems = vec![first];
        loop {
            if self.at(TokenKind::RParen) || self.at_eof() {
                break;
            }
            let before = self.pos;
            elems.push(self.parse_pattern());
            self.ensure_progress(before);
            if self.eat(TokenKind::Comma).is_some() {
                continue;
            }
            break;
        }
        let rp = self
            .expect(TokenKind::RParen, "')' in tuple pattern")
            .map(|token| token.span)
            .unwrap_or_else(|| self.prev_span());
        Pattern {
            kind: PatternKind::Tuple(elems),
            span: lp.span.join(rp),
        }
    }

    fn parse_path_pattern(&mut self) -> Pattern {
        let path = self.parse_path();
        let start = path.span;
        if self.at(TokenKind::LParen) {
            let lp = self.bump();
            let mut args = Vec::new();
            loop {
                if self.at(TokenKind::RParen) || self.at_eof() {
                    break;
                }
                let before = self.pos;
                args.push(self.parse_pattern());
                self.ensure_progress(before);
                if self.eat(TokenKind::Comma).is_some() {
                    continue;
                }
                break;
            }
            let rp = self
                .expect(TokenKind::RParen, "')' in pattern")
                .map(|token| token.span)
                .unwrap_or_else(|| self.prev_span());
            let _ = lp;
            return Pattern {
                kind: PatternKind::Path { path, args },
                span: start.join(rp),
            };
        }
        if self.at(TokenKind::LBrace) {
            let lb = self.bump();
            let mut fields = Vec::new();
            let mut has_rest = false;
            loop {
                if self.at(TokenKind::RBrace) || self.at_eof() {
                    break;
                }
                if self.at(TokenKind::DotDot) {
                    self.bump();
                    has_rest = true;
                    break;
                }
                let before = self.pos;
                let (name, name_span) = self.expect_ident("field name");
                let pat = if self.eat(TokenKind::Colon).is_some() {
                    self.parse_pattern()
                } else {
                    Pattern {
                        kind: PatternKind::Binding {
                            name: name.clone(),
                            mutable: false,
                        },
                        span: name_span,
                    }
                };
                fields.push(FieldPat {
                    name,
                    name_span,
                    pat,
                });
                self.ensure_progress(before);
                if self.eat(TokenKind::Comma).is_some() {
                    continue;
                }
                break;
            }
            let rb = self
                .expect(TokenKind::RBrace, "'}' in struct pattern")
                .map(|token| token.span)
                .unwrap_or_else(|| self.prev_span());
            let _ = lb;
            return Pattern {
                kind: PatternKind::Struct {
                    path,
                    fields,
                    has_rest,
                },
                span: start.join(rb),
            };
        }
        // A bare single-segment path is a binding, unless it is one of the
        // Option/Result variant names used directly as a pattern.
        let single_binding = path.segments.len() == 1
            && path.segments[0].generics.is_empty()
            && !SUGAR_VARIANT_NAMES.contains(&path.segments[0].name.as_str());
        if single_binding {
            let name = path.segments[0].name.clone();
            return Pattern {
                kind: PatternKind::Binding {
                    name,
                    mutable: false,
                },
                span: start,
            };
        }
        Pattern {
            kind: PatternKind::Path {
                path,
                args: Vec::new(),
            },
            span: start,
        }
    }

    fn small_int(&mut self, value: u128, span: Span) -> i128 {
        match i128::try_from(value) {
            Ok(value) => value,
            Err(_) => {
                self.error("integer literal is too large", span);
                i128::MAX
            }
        }
    }

    // ---------------------------------------------------------------------
    // Blocks and statements
    // ---------------------------------------------------------------------

    fn parse_block(&mut self) -> Block {
        if !self.enter_nesting() {
            return Block::empty(self.span());
        }
        let block = self.parse_block_inner();
        self.leave_nesting();
        block
    }

    fn parse_block_inner(&mut self) -> Block {
        let lb = self.expect(TokenKind::LBrace, "'{'");
        let start = lb.map(|token| token.span).unwrap_or_else(|| self.span());
        let saved = self.allow_struct_literal;
        self.allow_struct_literal = true;
        let mut stmts = Vec::new();
        let mut tail: Option<Box<Expr>> = None;
        loop {
            if self.at(TokenKind::RBrace) || self.at_eof() {
                break;
            }
            let before = self.pos;
            if self.at(TokenKind::Semi) {
                let token = self.bump();
                stmts.push(Stmt::Empty(token.span));
                continue;
            }
            if self.at(TokenKind::KwLet) {
                stmts.push(Stmt::Let(self.parse_let_stmt()));
                self.ensure_progress(before);
                continue;
            }
            if self.is_item_start() {
                if let Some(item) = self.parse_item() {
                    stmts.push(Stmt::Item(item));
                }
                self.ensure_progress(before);
                continue;
            }
            let expr = self.parse_expr();
            if self.eat(TokenKind::Semi).is_some() {
                stmts.push(Stmt::Expr(expr));
            } else if self.at(TokenKind::RBrace) || self.at_eof() {
                tail = Some(Box::new(expr));
                break;
            } else if is_block_like(&expr) {
                stmts.push(Stmt::Expr(expr));
            } else {
                let span = self.span();
                self.error("expected ';' or '}' after expression", span);
                stmts.push(Stmt::Expr(expr));
            }
            self.ensure_progress(before);
        }
        let rb = self
            .expect(TokenKind::RBrace, "'}'")
            .map(|token| token.span)
            .unwrap_or_else(|| self.prev_span());
        self.allow_struct_literal = saved;
        Block {
            stmts,
            tail,
            span: start.join(rb),
        }
    }

    fn parse_let_stmt(&mut self) -> LetStmt {
        let let_token = self.bump();
        let pattern = self.parse_pattern();
        let ty = if self.eat(TokenKind::Colon).is_some() {
            Some(self.parse_type())
        } else {
            None
        };
        let init = if self.eat(TokenKind::Eq).is_some() {
            Some(self.parse_expr())
        } else {
            None
        };
        let end = self
            .expect(TokenKind::Semi, "';' after let statement")
            .map(|token| token.span)
            .unwrap_or_else(|| self.prev_span());
        LetStmt {
            pat: pattern,
            ty,
            init,
            span: let_token.span.join(end),
        }
    }

    // ---------------------------------------------------------------------
    // Expressions
    // ---------------------------------------------------------------------

    fn parse_expr_no_struct(&mut self) -> Expr {
        let saved = self.allow_struct_literal;
        self.allow_struct_literal = false;
        let expr = self.parse_expr();
        self.allow_struct_literal = saved;
        expr
    }

    fn parse_expr(&mut self) -> Expr {
        self.parse_assign()
    }

    fn parse_assign(&mut self) -> Expr {
        if !self.enter_nesting() {
            return self.error_expr();
        }
        let expr = self.parse_assign_inner();
        self.leave_nesting();
        expr
    }

    fn parse_assign_inner(&mut self) -> Expr {
        let lhs = self.parse_or();
        if let Some(op) = self.assignment_op() {
            self.bump();
            let rhs = self.parse_assign();
            let span = lhs.span.join(rhs.span);
            return Expr::new(
                ExprKind::Assign {
                    op,
                    target: Box::new(lhs),
                    value: Box::new(rhs),
                },
                span,
            );
        }
        lhs
    }

    fn assignment_op(&self) -> Option<Option<BinaryOp>> {
        let op = match self.kind() {
            TokenKind::Eq => None,
            TokenKind::PlusEq => Some(BinaryOp::Add),
            TokenKind::MinusEq => Some(BinaryOp::Sub),
            TokenKind::StarEq => Some(BinaryOp::Mul),
            TokenKind::SlashEq => Some(BinaryOp::Div),
            TokenKind::PercentEq => Some(BinaryOp::Rem),
            TokenKind::AmpEq => Some(BinaryOp::BitAnd),
            TokenKind::PipeEq => Some(BinaryOp::BitOr),
            TokenKind::CaretEq => Some(BinaryOp::BitXor),
            TokenKind::ShlEq => Some(BinaryOp::Shl),
            TokenKind::ShrEq => Some(BinaryOp::Shr),
            _ => return None,
        };
        Some(op)
    }

    fn parse_or(&mut self) -> Expr {
        let mut lhs = self.parse_and();
        while self.at(TokenKind::PipePipe) {
            self.bump();
            let rhs = self.parse_and();
            lhs = binary(BinaryOp::Or, lhs, rhs);
        }
        lhs
    }

    fn parse_and(&mut self) -> Expr {
        let mut lhs = self.parse_comparison();
        while self.at(TokenKind::AmpAmp) {
            self.bump();
            let rhs = self.parse_comparison();
            lhs = binary(BinaryOp::And, lhs, rhs);
        }
        lhs
    }

    fn parse_comparison(&mut self) -> Expr {
        let mut lhs = self.parse_bitor();
        loop {
            let op = match self.kind() {
                TokenKind::EqEq => BinaryOp::Eq,
                TokenKind::Ne => BinaryOp::Ne,
                TokenKind::Lt => BinaryOp::Lt,
                TokenKind::Le => BinaryOp::Le,
                TokenKind::Gt => BinaryOp::Gt,
                TokenKind::Ge => BinaryOp::Ge,
                _ => break,
            };
            self.bump();
            let rhs = self.parse_bitor();
            lhs = binary(op, lhs, rhs);
        }
        lhs
    }

    fn parse_bitor(&mut self) -> Expr {
        let mut lhs = self.parse_bitxor();
        while self.at(TokenKind::Pipe) {
            self.bump();
            let rhs = self.parse_bitxor();
            lhs = binary(BinaryOp::BitOr, lhs, rhs);
        }
        lhs
    }

    fn parse_bitxor(&mut self) -> Expr {
        let mut lhs = self.parse_bitand();
        while self.at(TokenKind::Caret) {
            self.bump();
            let rhs = self.parse_bitand();
            lhs = binary(BinaryOp::BitXor, lhs, rhs);
        }
        lhs
    }

    fn parse_bitand(&mut self) -> Expr {
        let mut lhs = self.parse_shift();
        while self.at(TokenKind::Amp) {
            self.bump();
            let rhs = self.parse_shift();
            lhs = binary(BinaryOp::BitAnd, lhs, rhs);
        }
        lhs
    }

    fn parse_shift(&mut self) -> Expr {
        let mut lhs = self.parse_add();
        loop {
            let op = match self.kind() {
                TokenKind::Shl => BinaryOp::Shl,
                TokenKind::Shr => BinaryOp::Shr,
                _ => break,
            };
            self.bump();
            let rhs = self.parse_add();
            lhs = binary(op, lhs, rhs);
        }
        lhs
    }

    fn parse_add(&mut self) -> Expr {
        let mut lhs = self.parse_mul();
        loop {
            let op = match self.kind() {
                TokenKind::Plus => BinaryOp::Add,
                TokenKind::Minus => BinaryOp::Sub,
                _ => break,
            };
            self.bump();
            let rhs = self.parse_mul();
            lhs = binary(op, lhs, rhs);
        }
        lhs
    }

    fn parse_mul(&mut self) -> Expr {
        let mut lhs = self.parse_cast();
        loop {
            let op = match self.kind() {
                TokenKind::Star => BinaryOp::Mul,
                TokenKind::Slash => BinaryOp::Div,
                TokenKind::Percent => BinaryOp::Rem,
                _ => break,
            };
            self.bump();
            let rhs = self.parse_cast();
            lhs = binary(op, lhs, rhs);
        }
        lhs
    }

    fn parse_cast(&mut self) -> Expr {
        let mut expr = self.parse_unary();
        while self.eat(TokenKind::KwAs).is_some() {
            let ty = self.parse_type();
            let span = expr.span.join(ty.span());
            expr = Expr::new(
                ExprKind::Cast {
                    expr: Box::new(expr),
                    ty,
                },
                span,
            );
        }
        expr
    }

    fn parse_unary(&mut self) -> Expr {
        // Iterative so that a long run of prefix operators cannot exhaust the
        // stack. Prefix operators are right associative, so the operators are
        // collected first and folded from the innermost one outwards.
        let mut operators: Vec<(PrefixOp, Span)> = Vec::new();
        loop {
            let mut op = match self.kind() {
                TokenKind::Minus => PrefixOp::Neg,
                TokenKind::Bang => PrefixOp::Not,
                TokenKind::Amp => PrefixOp::Ref,
                TokenKind::Star => PrefixOp::Deref,
                TokenKind::AmpAmp => {
                    // A written '&&' is one token but two nested shared
                    // borrows, &(&x). A 'mut' directly after it belongs to the
                    // inner borrow: '&&mut x' means &(&mut x).
                    let token = self.bump();
                    operators.push((PrefixOp::Ref, token.span));
                    let inner = if self.eat(TokenKind::KwMut).is_some() {
                        PrefixOp::RefMut
                    } else {
                        PrefixOp::Ref
                    };
                    operators.push((inner, token.span));
                    continue;
                }
                _ => break,
            };
            let token = self.bump();
            if matches!(op, PrefixOp::Ref) && self.eat(TokenKind::KwMut).is_some() {
                op = PrefixOp::RefMut;
            }
            operators.push((op, token.span));
        }
        let mut expr = self.parse_postfix();
        for (op, op_span) in operators.into_iter().rev() {
            let span = op_span.join(expr.span);
            let kind = match op {
                PrefixOp::Neg => ExprKind::Unary {
                    op: UnaryOp::Neg,
                    expr: Box::new(expr),
                },
                PrefixOp::Not => ExprKind::Unary {
                    op: UnaryOp::Not,
                    expr: Box::new(expr),
                },
                PrefixOp::Ref => ExprKind::Ref {
                    mutable: false,
                    expr: Box::new(expr),
                },
                PrefixOp::RefMut => ExprKind::Ref {
                    mutable: true,
                    expr: Box::new(expr),
                },
                PrefixOp::Deref => ExprKind::Deref {
                    expr: Box::new(expr),
                },
            };
            expr = Expr::new(kind, span);
        }
        expr
    }

    fn parse_postfix(&mut self) -> Expr {
        // A block-like expression that starts the postfix chain ends at its
        // closing brace: a following '(', '.', '[' or '::' begins a new
        // statement/tail expression rather than a postfix operation. This is
        // the Rust statement rule that lets
        //     for v in values { ... }
        //     (lo, hi)
        // parse as a for statement followed by a tuple tail.
        //
        // The exception is the postfix '?' operator: version 1.5 accepts
        // (if c { Ok(1) } else { Err("e") })?. The check looks at the leading
        // token, not at the finished node, so a parenthesized form such as
        // (if c { f } else { g }).call() still gets its postfixes; only a
        // block-like primary used directly is affected.
        let mut block_like = matches!(
            self.peek_kind(),
            TokenKind::KwIf
                | TokenKind::KwMatch
                | TokenKind::KwLoop
                | TokenKind::KwWhile
                | TokenKind::KwFor
                | TokenKind::LBrace
        );
        let mut expr = self.parse_primary();
        loop {
            // Only '?' may follow a block-like expression directly.
            if block_like && !self.at(TokenKind::Question) {
                break;
            }
            block_like = false;
            if self.at(TokenKind::LParen) {
                let (args, rparen) = self.parse_call_args();
                let span = expr.span.join(rparen);
                expr = Expr::new(
                    ExprKind::Call {
                        callee: Box::new(expr),
                        args,
                    },
                    span,
                );
            } else if self.at(TokenKind::Dot) {
                self.bump();
                let kind = self.peek_kind();
                match kind {
                    TokenKind::Int(value) => {
                        let token = self.bump();
                        let index = match usize::try_from(value) {
                            Ok(index) => index,
                            Err(_) => {
                                self.error("tuple index is too large", token.span);
                                0
                            }
                        };
                        let span = expr.span.join(token.span);
                        expr = Expr::new(
                            ExprKind::TupleField {
                                base: Box::new(expr),
                                index,
                                index_span: token.span,
                            },
                            span,
                        );
                    }
                    TokenKind::Ident(name) => {
                        let name_span = self.span();
                        self.bump();
                        let mut turbofish = Vec::new();
                        if self.at(TokenKind::ColonColon)
                            && matches!(self.peek_kind_at(1), Some(TokenKind::Lt))
                        {
                            self.bump();
                            turbofish = self.parse_type_args();
                        }
                        if self.at(TokenKind::LParen) {
                            let (args, rparen) = self.parse_call_args();
                            let span = expr.span.join(rparen);
                            expr = Expr::new(
                                ExprKind::MethodCall {
                                    receiver: Box::new(expr),
                                    method: name,
                                    method_span: name_span,
                                    turbofish,
                                    args,
                                },
                                span,
                            );
                        } else {
                            if !turbofish.is_empty() {
                                self.error("expected '(' after method turbofish", name_span);
                            }
                            let span = expr.span.join(name_span);
                            expr = Expr::new(
                                ExprKind::Field {
                                    base: Box::new(expr),
                                    name,
                                    name_span,
                                },
                                span,
                            );
                        }
                    }
                    _ => {
                        let span = self.span();
                        self.error("expected a field name or tuple index after '.'", span);
                        if !self.at_eof() {
                            self.bump();
                        }
                        break;
                    }
                }
            } else if self.at(TokenKind::LBracket) {
                self.bump();
                let saved = self.allow_struct_literal;
                self.allow_struct_literal = true;
                let index = self.parse_expr();
                self.allow_struct_literal = saved;
                let rb = self
                    .expect(TokenKind::RBracket, "']' after index")
                    .map(|token| token.span)
                    .unwrap_or_else(|| self.prev_span());
                let span = expr.span.join(rb);
                expr = Expr::new(
                    ExprKind::Index {
                        base: Box::new(expr),
                        index: Box::new(index),
                    },
                    span,
                );
            } else if self.at(TokenKind::Question) {
                // Postfix try operator. It is the lowest-precedence postfix, so it
                // chains with calls and fields: f()?.field?.method().
                let question = self.bump();
                let span = expr.span.join(question.span);
                expr = Expr::new(
                    ExprKind::Try {
                        expr: Box::new(expr),
                    },
                    span,
                );
            } else {
                break;
            }
        }
        expr
    }

    fn parse_call_args(&mut self) -> (Vec<Expr>, Span) {
        let lp = self.expect(TokenKind::LParen, "'('");
        let start = lp.map(|token| token.span).unwrap_or_else(|| self.span());
        let saved = self.allow_struct_literal;
        self.allow_struct_literal = true;
        let mut args = Vec::new();
        loop {
            if self.at(TokenKind::RParen) || self.at_eof() {
                break;
            }
            let before = self.pos;
            args.push(self.parse_expr());
            self.ensure_progress(before);
            if self.eat(TokenKind::Comma).is_some() {
                continue;
            }
            break;
        }
        let end = self
            .expect(TokenKind::RParen, "')' after arguments")
            .map(|token| token.span)
            .unwrap_or_else(|| self.prev_span());
        self.allow_struct_literal = saved;
        let _ = start;
        (args, end)
    }

    fn parse_primary(&mut self) -> Expr {
        let start = self.span();
        match self.peek_kind() {
            TokenKind::Int(value) => {
                let raw = self.token_text(start);
                self.bump();
                let value = self.small_int(value, start);
                Expr::new(ExprKind::Lit(Lit::Int { value, raw }), start)
            }
            TokenKind::Float(value) => {
                let raw = self.token_text(start);
                self.bump();
                Expr::new(ExprKind::Lit(Lit::Float { value, raw }), start)
            }
            TokenKind::Str(value) => {
                self.bump();
                Expr::new(ExprKind::Lit(Lit::Str(value)), start)
            }
            TokenKind::Char(value) => {
                self.bump();
                Expr::new(ExprKind::Lit(Lit::Char(value)), start)
            }
            TokenKind::KwTrue => {
                self.bump();
                Expr::new(ExprKind::Lit(Lit::Bool(true)), start)
            }
            TokenKind::KwFalse => {
                self.bump();
                Expr::new(ExprKind::Lit(Lit::Bool(false)), start)
            }
            TokenKind::LParen => self.parse_paren_expr(),
            TokenKind::LBracket => {
                let span = self.span();
                self.error(
                    "array literals are not supported; use vec! for vectors",
                    span,
                );
                self.skip_balanced(TokenKind::LBracket, TokenKind::RBracket);
                Expr::new(ExprKind::Tuple(Vec::new()), span)
            }
            TokenKind::LBrace => {
                let block = self.parse_block();
                let span = block.span;
                Expr::new(ExprKind::Block(block), span)
            }
            TokenKind::KwIf => self.parse_if_expr(),
            TokenKind::KwMatch => self.parse_match_expr(),
            TokenKind::KwLoop => self.parse_loop_expr(),
            TokenKind::KwWhile => self.parse_while_expr(),
            TokenKind::KwFor => self.parse_for_expr(),
            TokenKind::KwMove | TokenKind::Pipe | TokenKind::PipePipe => self.parse_closure_expr(),
            TokenKind::KwBreak => {
                let token = self.bump();
                let value = if self.starts_expression() {
                    Some(Box::new(self.parse_expr()))
                } else {
                    None
                };
                let span = match &value {
                    Some(value) => token.span.join(value.span),
                    None => token.span,
                };
                Expr::new(ExprKind::Break(value), span)
            }
            TokenKind::KwContinue => {
                let token = self.bump();
                Expr::new(ExprKind::Continue, token.span)
            }
            TokenKind::KwReturn => {
                let token = self.bump();
                let value = if self.starts_expression() {
                    Some(Box::new(self.parse_expr()))
                } else {
                    None
                };
                let span = match &value {
                    Some(value) => token.span.join(value.span),
                    None => token.span,
                };
                Expr::new(ExprKind::Return(value), span)
            }
            TokenKind::KwSelf | TokenKind::KwSelfTy => {
                let path = self.parse_path();
                let span = path.span;
                // Self { .. } is a struct literal like any other named struct:
                // keep it in expression position whenever struct literals are
                // allowed, so Self::new / Self { .. } resolve through the impl
                // self type in typeck.
                if self.allow_struct_literal && self.at(TokenKind::LBrace) {
                    return self.parse_struct_literal(path);
                }
                Expr::new(ExprKind::Path(path), span)
            }
            TokenKind::Ident(name) => {
                if matches!(self.peek_kind_at(1), Some(TokenKind::Bang)) {
                    self.bump();
                    self.bump();
                    return self.parse_macro_call(name, start);
                }
                let path = self.parse_path();
                let span = path.span;
                if self.allow_struct_literal && self.at(TokenKind::LBrace) {
                    return self.parse_struct_literal(path);
                }
                Expr::new(ExprKind::Path(path), span)
            }
            _ => {
                let found = self.peek_kind().describe();
                self.error(format!("expected an expression, found {}", found), start);
                if !self.at_eof() {
                    self.bump();
                }
                Expr::new(
                    ExprKind::Lit(Lit::Int {
                        value: 0,
                        raw: "0".to_string(),
                    }),
                    start,
                )
            }
        }
    }

    fn skip_balanced(&mut self, open: TokenKind, close: TokenKind) {
        if self.eat(open.clone()).is_none() {
            return;
        }
        let mut depth: usize = 1;
        while depth > 0 && !self.at_eof() {
            if self.at(open.clone()) {
                depth += 1;
                self.bump();
            } else if self.at(close.clone()) {
                depth -= 1;
                self.bump();
            } else {
                self.bump();
            }
        }
    }

    fn parse_paren_expr(&mut self) -> Expr {
        let lp = self.bump();
        let saved = self.allow_struct_literal;
        self.allow_struct_literal = true;
        let result;
        if self.at(TokenKind::RParen) {
            let rp = self.bump();
            result = Expr::new(ExprKind::Tuple(Vec::new()), lp.span.join(rp.span));
        } else {
            let first = self.parse_expr();
            if self.eat(TokenKind::Comma).is_some() {
                let mut elems = vec![first];
                loop {
                    if self.at(TokenKind::RParen) || self.at_eof() {
                        break;
                    }
                    let before = self.pos;
                    elems.push(self.parse_expr());
                    self.ensure_progress(before);
                    if self.eat(TokenKind::Comma).is_some() {
                        continue;
                    }
                    break;
                }
                let rp = self
                    .expect(TokenKind::RParen, "')' after tuple elements")
                    .map(|token| token.span)
                    .unwrap_or_else(|| self.prev_span());
                result = Expr::new(ExprKind::Tuple(elems), lp.span.join(rp));
            } else {
                self.expect(TokenKind::RParen, "')'");
                result = first;
            }
        }
        self.allow_struct_literal = saved;
        result
    }

    fn parse_if_expr(&mut self) -> Expr {
        if !self.enter_nesting() {
            return self.error_expr();
        }
        let expr = self.parse_if_expr_inner();
        self.leave_nesting();
        expr
    }

    fn parse_if_expr_inner(&mut self) -> Expr {
        let if_token = self.expect(TokenKind::KwIf, "if");
        let start = if_token
            .map(|token| token.span)
            .unwrap_or_else(|| self.span());
        if self.at(TokenKind::KwLet) {
            return self.parse_if_let_expr(start);
        }
        let cond = self.parse_expr_no_struct();
        let then_block = self.parse_block();
        let then_span = then_block.span;
        let else_expr = self.parse_else_branch();
        let end = else_expr
            .as_ref()
            .map(|expr| expr.span)
            .unwrap_or(then_span);
        let span = start.join(end);
        Expr::new(
            ExprKind::If(IfExpr {
                cond: Box::new(cond),
                then_block,
                else_expr: else_expr.map(Box::new),
                span,
            }),
            span,
        )
    }

    fn parse_else_branch(&mut self) -> Option<Expr> {
        self.eat(TokenKind::KwElse)?;
        if self.at(TokenKind::KwIf) {
            return Some(self.parse_if_expr());
        }
        let block = self.parse_block();
        let span = block.span;
        Some(Expr::new(ExprKind::Block(block), span))
    }

    fn parse_if_let_expr(&mut self, start: Span) -> Expr {
        // 'if' has been consumed and 'let' is the current token.
        self.bump();
        let pattern = self.parse_pattern();
        let pattern_span = pattern.span;
        self.expect(TokenKind::Eq, "'=' after if-let pattern");
        let scrutinee = self.parse_expr_no_struct();
        let then_block = self.parse_block();
        let then_span = then_block.span;
        let else_expr = self.parse_else_branch();
        let end = else_expr
            .as_ref()
            .map(|expr| expr.span)
            .unwrap_or(then_span);
        let wildcard_span = Span::empty(then_span.end);
        let then_body = Expr::new(ExprKind::Block(then_block), then_span);
        let then_arm = MatchArm {
            pat: pattern,
            guard: None,
            body: then_body,
            span: pattern_span.join(then_span),
        };
        let else_body = match else_expr {
            Some(expr) => expr,
            None => {
                let empty = Block::empty(wildcard_span);
                Expr::new(ExprKind::Block(empty), wildcard_span)
            }
        };
        let wildcard = Pattern {
            kind: PatternKind::Wild,
            span: wildcard_span,
        };
        let else_arm_span = wildcard_span.join(else_body.span);
        let else_arm = MatchArm {
            pat: wildcard,
            guard: None,
            body: else_body,
            span: else_arm_span,
        };
        let arms = vec![then_arm, else_arm];
        let span = start.join(end);
        let match_expr = MatchExpr {
            scrutinee: Box::new(scrutinee),
            arms,
            span,
        };
        Expr::new(ExprKind::Match(match_expr), span)
    }

    fn parse_match_expr(&mut self) -> Expr {
        let match_token = self.expect(TokenKind::KwMatch, "match");
        let start = match_token
            .map(|token| token.span)
            .unwrap_or_else(|| self.span());
        let scrutinee = self.parse_expr_no_struct();
        self.expect(TokenKind::LBrace, "'{' after match value");
        let mut arms = Vec::new();
        loop {
            if self.at(TokenKind::RBrace) || self.at_eof() {
                break;
            }
            let before = self.pos;
            let pattern = self.parse_pattern();
            let guard = if self.eat(TokenKind::KwIf).is_some() {
                Some(self.parse_expr())
            } else {
                None
            };
            self.expect(TokenKind::FatArrow, "'=>' in match arm");
            let body = self.parse_expr();
            let span = pattern.span.join(body.span);
            arms.push(MatchArm {
                pat: pattern,
                guard,
                body,
                span,
            });
            self.ensure_progress(before);
            if self.eat(TokenKind::Comma).is_some() {
                continue;
            }
        }
        let end = self
            .expect(TokenKind::RBrace, "'}' after match arms")
            .map(|token| token.span)
            .unwrap_or_else(|| self.prev_span());
        let span = start.join(end);
        Expr::new(
            ExprKind::Match(MatchExpr {
                scrutinee: Box::new(scrutinee),
                arms,
                span,
            }),
            span,
        )
    }

    fn parse_loop_expr(&mut self) -> Expr {
        let loop_token = self.expect(TokenKind::KwLoop, "loop");
        let start = loop_token
            .map(|token| token.span)
            .unwrap_or_else(|| self.span());
        let body = self.parse_block();
        let span = start.join(body.span);
        Expr::new(ExprKind::Loop { body }, span)
    }

    fn parse_while_expr(&mut self) -> Expr {
        let while_token = self.expect(TokenKind::KwWhile, "while");
        let start = while_token
            .map(|token| token.span)
            .unwrap_or_else(|| self.span());
        if self.at(TokenKind::KwLet) {
            return self.parse_while_let_expr(start);
        }
        let cond = self.parse_expr_no_struct();
        let body = self.parse_block();
        let span = start.join(body.span);
        Expr::new(
            ExprKind::While {
                cond: Box::new(cond),
                body,
            },
            span,
        )
    }

    fn parse_while_let_expr(&mut self, start: Span) -> Expr {
        self.bump();
        let pattern = self.parse_pattern();
        let pattern_span = pattern.span;
        self.expect(TokenKind::Eq, "'=' after while-let pattern");
        let scrutinee = self.parse_expr_no_struct();
        let body = self.parse_block();
        let body_span = body.span;
        let wildcard_span = Span::empty(body_span.end);
        let then_body = Expr::new(ExprKind::Block(body), body_span);
        let then_arm = MatchArm {
            pat: pattern,
            guard: None,
            body: then_body,
            span: pattern_span.join(body_span),
        };
        let wildcard = Pattern {
            kind: PatternKind::Wild,
            span: wildcard_span,
        };
        let break_expr = Expr::new(ExprKind::Break(None), wildcard_span);
        let else_arm = MatchArm {
            pat: wildcard,
            guard: None,
            body: break_expr,
            span: wildcard_span,
        };
        let match_expr = MatchExpr {
            scrutinee: Box::new(scrutinee),
            arms: vec![then_arm, else_arm],
            span: body_span,
        };
        let match_body = Expr::new(ExprKind::Match(match_expr), body_span);
        let loop_block = Block {
            stmts: Vec::new(),
            tail: Some(Box::new(match_body)),
            span: body_span,
        };
        let span = start.join(body_span);
        Expr::new(ExprKind::Loop { body: loop_block }, span)
    }

    fn parse_for_expr(&mut self) -> Expr {
        let for_token = self.expect(TokenKind::KwFor, "for");
        let start = for_token
            .map(|token| token.span)
            .unwrap_or_else(|| self.span());
        let pattern = self.parse_pattern();
        self.expect(TokenKind::KwIn, "'in' after for pattern");
        let saved = self.allow_struct_literal;
        self.allow_struct_literal = false;
        let lhs = self.parse_expr();
        let inclusive = if self.at(TokenKind::DotDot) {
            self.bump();
            false
        } else if self.at(TokenKind::DotDotEq) {
            self.bump();
            true
        } else {
            self.allow_struct_literal = saved;
            let body = self.parse_block();
            let span = start.join(body.span);
            return Expr::new(
                ExprKind::For {
                    pat: pattern,
                    iter: Box::new(lhs),
                    body,
                },
                span,
            );
        };
        let rhs = self.parse_expr();
        self.allow_struct_literal = saved;
        let range_span = lhs.span.join(rhs.span);
        let iter = Expr::new(
            ExprKind::Range {
                start: Box::new(lhs),
                end: Box::new(rhs),
                inclusive,
            },
            range_span,
        );
        let body = self.parse_block();
        let span = start.join(body.span);
        Expr::new(
            ExprKind::For {
                pat: pattern,
                iter: Box::new(iter),
                body,
            },
            span,
        )
    }

    fn parse_closure_expr(&mut self) -> Expr {
        let start = self.span();
        let is_move = self.eat(TokenKind::KwMove).is_some();
        let mut params = Vec::new();
        let empty_params = self.eat(TokenKind::PipePipe).is_some();
        if !empty_params {
            self.expect(TokenKind::Pipe, "'|'");
            if !self.at(TokenKind::Pipe) {
                loop {
                    if self.at(TokenKind::Pipe) || self.at_eof() {
                        break;
                    }
                    let before = self.pos;
                    let pattern = self.parse_pattern_atomic();
                    let ty = if self.eat(TokenKind::Colon).is_some() {
                        Some(self.parse_type())
                    } else {
                        None
                    };
                    params.push(ClosureParam { pat: pattern, ty });
                    self.ensure_progress(before);
                    if self.eat(TokenKind::Comma).is_some() {
                        continue;
                    }
                    break;
                }
            }
            self.expect(TokenKind::Pipe, "'|' after closure parameters");
        }
        let ret = if self.eat(TokenKind::Arrow).is_some() {
            Some(self.parse_type())
        } else {
            None
        };
        let body = self.parse_expr();
        let span = start.join(body.span);
        Expr::new(
            ExprKind::Closure(ClosureExpr {
                is_move,
                params,
                ret,
                body: Box::new(body),
            }),
            span,
        )
    }

    fn parse_struct_literal(&mut self, path: PathExpr) -> Expr {
        let saved = self.allow_struct_literal;
        self.allow_struct_literal = true;
        self.expect(TokenKind::LBrace, "'{' in struct literal");
        let mut fields = Vec::new();
        loop {
            if self.at(TokenKind::RBrace) || self.at_eof() {
                break;
            }
            let before = self.pos;
            let (name, name_span) = self.expect_ident("field name");
            let value = if self.eat(TokenKind::Colon).is_some() {
                self.parse_expr()
            } else {
                Expr::new(
                    ExprKind::Path(PathExpr {
                        segments: vec![PathSegment {
                            name: name.clone(),
                            span: name_span,
                            generics: Vec::new(),
                        }],
                        span: name_span,
                    }),
                    name_span,
                )
            };
            fields.push(FieldInit {
                name,
                name_span,
                value,
            });
            self.ensure_progress(before);
            if self.eat(TokenKind::Comma).is_some() {
                continue;
            }
            break;
        }
        let end = self
            .expect(TokenKind::RBrace, "'}' after struct literal")
            .map(|token| token.span)
            .unwrap_or_else(|| self.prev_span());
        self.allow_struct_literal = saved;
        let span = path.span.join(end);
        Expr::new(ExprKind::StructLit { path, fields }, span)
    }

    fn parse_macro_call(&mut self, name: String, name_span: Span) -> Expr {
        if name == "vec" {
            return self.parse_vec_literal(name_span);
        }
        let mut args = Vec::new();
        self.expect(TokenKind::LParen, "'(' after macro name");
        loop {
            if self.at(TokenKind::RParen) || self.at_eof() {
                break;
            }
            let before = self.pos;
            args.push(self.parse_expr());
            self.ensure_progress(before);
            if self.eat(TokenKind::Comma).is_some() {
                continue;
            }
            break;
        }
        let end = self
            .expect(TokenKind::RParen, "')' after macro arguments")
            .map(|token| token.span)
            .unwrap_or_else(|| self.prev_span());
        let span = name_span.join(end);
        Expr::new(
            ExprKind::Macro(MacroCall {
                name,
                name_span,
                args,
                span,
            }),
            span,
        )
    }

    fn parse_vec_literal(&mut self, name_span: Span) -> Expr {
        let bracket = if self.at(TokenKind::LBracket) {
            self.bump();
            true
        } else {
            self.expect(TokenKind::LParen, "'[' after vec!");
            false
        };
        let close = if bracket {
            TokenKind::RBracket
        } else {
            TokenKind::RParen
        };
        let mut elements: Vec<Expr> = Vec::new();
        let mut repeat: Option<(Expr, Expr)> = None;
        if !self.at(close.clone()) && !self.at_eof() {
            let first = self.parse_expr();
            if self.eat(TokenKind::Semi).is_some() {
                let count = self.parse_expr();
                repeat = Some((first, count));
            } else {
                elements.push(first);
                while self.eat(TokenKind::Comma).is_some() {
                    if self.at(close.clone()) || self.at_eof() {
                        break;
                    }
                    let before = self.pos;
                    elements.push(self.parse_expr());
                    self.ensure_progress(before);
                }
            }
        }
        let end = self
            .expect(
                close,
                if bracket {
                    "']' after vec!"
                } else {
                    "')' after vec!"
                },
            )
            .map(|token| token.span)
            .unwrap_or_else(|| self.prev_span());
        let span = name_span.join(end);
        let literal = match repeat {
            Some((value, count)) => VecLit::Repeat {
                value: Box::new(value),
                count: Box::new(count),
            },
            None => VecLit::List(elements),
        };
        Expr::new(ExprKind::Vec(literal), span)
    }

    fn parse_path(&mut self) -> PathExpr {
        let start = self.span();
        let (name, span) = self.expect_path_ident();
        let mut segments = vec![PathSegment {
            name,
            span,
            generics: Vec::new(),
        }];
        let mut end = span;
        loop {
            if !self.at(TokenKind::ColonColon) {
                break;
            }
            if matches!(self.peek_kind_at(1), Some(TokenKind::Lt)) {
                self.bump();
                let generics = self.parse_type_args();
                if let Some(last) = segments.last_mut() {
                    last.generics = generics;
                }
                end = self.prev_span();
                continue;
            }
            self.bump();
            let (name, span) = self.expect_path_ident();
            end = span;
            segments.push(PathSegment {
                name,
                span,
                generics: Vec::new(),
            });
        }
        PathExpr {
            segments,
            span: start.join(end),
        }
    }
}

/// One node of the iterative AST depth validation run by
/// [Parser::check_ast_depth] at the end of parsing. The walk is driven by an
/// explicit stack, so validating the tree does not itself recurse and cannot
/// overflow the stack it is meant to protect.
enum AstNode<'a> {
    Item(&'a Item),
    Fn(&'a FnDecl),
    Block(&'a Block),
    Stmt(&'a Stmt),
    Expr(&'a Expr),
    Pattern(&'a Pattern),
    Type(&'a TypeExpr),
}

impl<'a> AstNode<'a> {
    /// Span of this node, used for the diagnostic when the limit is exceeded.
    fn span(&self) -> Span {
        match self {
            AstNode::Item(item) => item.span(),
            AstNode::Fn(decl) => decl.span,
            AstNode::Block(block) => block.span,
            AstNode::Stmt(stmt) => stmt.span(),
            AstNode::Expr(expr) => expr.span,
            AstNode::Pattern(pattern) => pattern.span,
            AstNode::Type(ty) => ty.span(),
        }
    }

    /// Push every direct child of this node with `depth` as the child's depth.
    fn push_children(self, stack: &mut Vec<(AstNode<'a>, usize)>, depth: usize) {
        match self {
            AstNode::Item(item) => match item {
                Item::Fn(decl) => stack.push((AstNode::Fn(decl), depth)),
                Item::Struct(decl) => {
                    push_generic_bounds(stack, &decl.generics, depth);
                    for field in &decl.fields {
                        stack.push((AstNode::Type(&field.ty), depth));
                    }
                }
                Item::Enum(decl) => {
                    push_generic_bounds(stack, &decl.generics, depth);
                    for variant in &decl.variants {
                        for ty in &variant.fields {
                            stack.push((AstNode::Type(ty), depth));
                        }
                    }
                }
                Item::Impl(block) => {
                    push_generic_bounds(stack, &block.generics, depth);
                    stack.push((AstNode::Type(&block.ty), depth));
                    if let Some(trait_ref) = &block.trait_name {
                        for arg in &trait_ref.args {
                            stack.push((AstNode::Type(arg), depth));
                        }
                    }
                    for decl in &block.fns {
                        stack.push((AstNode::Fn(decl), depth));
                    }
                }
                Item::Const(decl) => {
                    stack.push((AstNode::Type(&decl.ty), depth));
                    stack.push((AstNode::Expr(&decl.value), depth));
                }
                Item::TypeAlias(decl) => {
                    push_generic_bounds(stack, &decl.generics, depth);
                    stack.push((AstNode::Type(&decl.ty), depth));
                }
                Item::Trait(decl) => {
                    push_generic_bounds(stack, &decl.generics, depth);
                    for method in &decl.methods {
                        for param in &method.params {
                            stack.push((AstNode::Type(&param.ty), depth));
                        }
                        if let Some(ret) = &method.ret {
                            stack.push((AstNode::Type(ret), depth));
                        }
                    }
                }
            },
            AstNode::Fn(decl) => {
                push_generic_bounds(stack, &decl.generics, depth);
                for param in &decl.params {
                    stack.push((AstNode::Type(&param.ty), depth));
                }
                if let Some(ret) = &decl.ret {
                    stack.push((AstNode::Type(ret), depth));
                }
                stack.push((AstNode::Block(&decl.body), depth));
            }
            AstNode::Block(block) => {
                for stmt in &block.stmts {
                    stack.push((AstNode::Stmt(stmt), depth));
                }
                if let Some(tail) = &block.tail {
                    stack.push((AstNode::Expr(tail), depth));
                }
            }
            AstNode::Stmt(stmt) => match stmt {
                Stmt::Let(lets) => {
                    stack.push((AstNode::Pattern(&lets.pat), depth));
                    if let Some(ty) = &lets.ty {
                        stack.push((AstNode::Type(ty), depth));
                    }
                    if let Some(init) = &lets.init {
                        stack.push((AstNode::Expr(init), depth));
                    }
                }
                Stmt::Expr(expr) => stack.push((AstNode::Expr(expr), depth)),
                Stmt::Item(item) => stack.push((AstNode::Item(item), depth)),
                Stmt::Empty(_) => {}
            },
            AstNode::Expr(expr) => match &expr.kind {
                ExprKind::Lit(_) | ExprKind::Continue => {}
                ExprKind::Path(path) => push_path_generics(stack, path, depth),
                ExprKind::Unary { expr, .. } => stack.push((AstNode::Expr(expr), depth)),
                ExprKind::Binary { lhs, rhs, .. } => {
                    stack.push((AstNode::Expr(lhs), depth));
                    stack.push((AstNode::Expr(rhs), depth));
                }
                ExprKind::Assign { target, value, .. } => {
                    stack.push((AstNode::Expr(target), depth));
                    stack.push((AstNode::Expr(value), depth));
                }
                ExprKind::Cast { expr, ty } => {
                    stack.push((AstNode::Expr(expr), depth));
                    stack.push((AstNode::Type(ty), depth));
                }
                ExprKind::Call { callee, args } => {
                    stack.push((AstNode::Expr(callee), depth));
                    for arg in args {
                        stack.push((AstNode::Expr(arg), depth));
                    }
                }
                ExprKind::MethodCall {
                    receiver,
                    turbofish,
                    args,
                    ..
                } => {
                    stack.push((AstNode::Expr(receiver), depth));
                    for ty in turbofish {
                        stack.push((AstNode::Type(ty), depth));
                    }
                    for arg in args {
                        stack.push((AstNode::Expr(arg), depth));
                    }
                }
                ExprKind::Field { base, .. } => {
                    stack.push((AstNode::Expr(base), depth));
                }
                ExprKind::TupleField { base, .. } => {
                    stack.push((AstNode::Expr(base), depth));
                }
                ExprKind::Index { base, index } => {
                    stack.push((AstNode::Expr(base), depth));
                    stack.push((AstNode::Expr(index), depth));
                }
                ExprKind::If(if_expr) => {
                    stack.push((AstNode::Expr(&if_expr.cond), depth));
                    stack.push((AstNode::Block(&if_expr.then_block), depth));
                    if let Some(else_expr) = &if_expr.else_expr {
                        stack.push((AstNode::Expr(else_expr), depth));
                    }
                }
                ExprKind::Match(match_expr) => {
                    stack.push((AstNode::Expr(&match_expr.scrutinee), depth));
                    for arm in &match_expr.arms {
                        stack.push((AstNode::Pattern(&arm.pat), depth));
                        if let Some(guard) = &arm.guard {
                            stack.push((AstNode::Expr(guard), depth));
                        }
                        stack.push((AstNode::Expr(&arm.body), depth));
                    }
                }
                ExprKind::Block(block) => stack.push((AstNode::Block(block), depth)),
                ExprKind::Loop { body } => stack.push((AstNode::Block(body), depth)),
                ExprKind::While { cond, body } => {
                    stack.push((AstNode::Expr(cond), depth));
                    stack.push((AstNode::Block(body), depth));
                }
                ExprKind::For { pat, iter, body } => {
                    stack.push((AstNode::Pattern(pat), depth));
                    stack.push((AstNode::Expr(iter), depth));
                    stack.push((AstNode::Block(body), depth));
                }
                ExprKind::Range { start, end, .. } => {
                    stack.push((AstNode::Expr(start), depth));
                    stack.push((AstNode::Expr(end), depth));
                }
                ExprKind::Closure(closure) => {
                    for param in &closure.params {
                        stack.push((AstNode::Pattern(&param.pat), depth));
                        if let Some(ty) = &param.ty {
                            stack.push((AstNode::Type(ty), depth));
                        }
                    }
                    if let Some(ret) = &closure.ret {
                        stack.push((AstNode::Type(ret), depth));
                    }
                    stack.push((AstNode::Expr(&closure.body), depth));
                }
                ExprKind::StructLit { path, fields } => {
                    push_path_generics(stack, path, depth);
                    for field in fields {
                        stack.push((AstNode::Expr(&field.value), depth));
                    }
                }
                ExprKind::Tuple(elems) => {
                    for elem in elems {
                        stack.push((AstNode::Expr(elem), depth));
                    }
                }
                ExprKind::Vec(vec_lit) => match vec_lit {
                    VecLit::List(elems) => {
                        for elem in elems {
                            stack.push((AstNode::Expr(elem), depth));
                        }
                    }
                    VecLit::Repeat { value, count } => {
                        stack.push((AstNode::Expr(value), depth));
                        stack.push((AstNode::Expr(count), depth));
                    }
                },
                ExprKind::Macro(call) => {
                    for arg in &call.args {
                        stack.push((AstNode::Expr(arg), depth));
                    }
                }
                ExprKind::Break(value) | ExprKind::Return(value) => {
                    if let Some(value) = value {
                        stack.push((AstNode::Expr(value), depth));
                    }
                }
                ExprKind::Ref { expr, .. } => stack.push((AstNode::Expr(expr), depth)),
                ExprKind::Deref { expr } => stack.push((AstNode::Expr(expr), depth)),
                ExprKind::Try { expr } => stack.push((AstNode::Expr(expr), depth)),
            },
            AstNode::Pattern(pattern) => match &pattern.kind {
                PatternKind::Wild | PatternKind::Binding { .. } | PatternKind::Lit(_) => {}
                PatternKind::Tuple(pats) => {
                    for pat in pats {
                        stack.push((AstNode::Pattern(pat), depth));
                    }
                }
                PatternKind::Path { path, args } => {
                    push_path_generics(stack, path, depth);
                    for pat in args {
                        stack.push((AstNode::Pattern(pat), depth));
                    }
                }
                PatternKind::Struct { path, fields, .. } => {
                    push_path_generics(stack, path, depth);
                    for field in fields {
                        stack.push((AstNode::Pattern(&field.pat), depth));
                    }
                }
                PatternKind::Or(pats) => {
                    for pat in pats {
                        stack.push((AstNode::Pattern(pat), depth));
                    }
                }
            },
            AstNode::Type(ty) => match ty {
                TypeExpr::Path { generics, .. } => {
                    for inner in generics {
                        stack.push((AstNode::Type(inner), depth));
                    }
                }
                TypeExpr::Tuple { elems, .. } => {
                    for inner in elems {
                        stack.push((AstNode::Type(inner), depth));
                    }
                }
                TypeExpr::Fn { params, ret, .. } => {
                    for param in params {
                        stack.push((AstNode::Type(param), depth));
                    }
                    stack.push((AstNode::Type(ret), depth));
                }
                TypeExpr::Ref { inner, .. } => stack.push((AstNode::Type(inner), depth)),
            },
        }
    }
}

/// Push every bound of the given generic parameters as type children.
fn push_generic_bounds<'a>(
    stack: &mut Vec<(AstNode<'a>, usize)>,
    generics: &'a [GenericParam],
    depth: usize,
) {
    for param in generics {
        for bound in &param.bounds {
            stack.push((AstNode::Type(bound), depth));
        }
    }
}

/// Push the generic arguments written on every segment of a path.
fn push_path_generics<'a>(stack: &mut Vec<(AstNode<'a>, usize)>, path: &'a PathExpr, depth: usize) {
    for segment in &path.segments {
        for ty in &segment.generics {
            stack.push((AstNode::Type(ty), depth));
        }
    }
}
/// The synthesized type of a trait method receiver. The type checker replaces
/// it with the concrete impl type when it dispatches a trait method call.
fn self_type(span: Span) -> TypeExpr {
    TypeExpr::Path {
        name: "Self".to_string(),
        name_span: span,
        generics: Vec::new(),
    }
}

fn unit_type(span: Span) -> TypeExpr {
    TypeExpr::Tuple {
        elems: Vec::new(),
        span,
    }
}

fn binary(op: BinaryOp, lhs: Expr, rhs: Expr) -> Expr {
    let span = lhs.span.join(rhs.span);
    Expr::new(
        ExprKind::Binary {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        },
        span,
    )
}

fn is_block_like(expr: &Expr) -> bool {
    matches!(
        expr.kind,
        ExprKind::If(_)
            | ExprKind::Match(_)
            | ExprKind::Loop { .. }
            | ExprKind::While { .. }
            | ExprKind::For { .. }
            | ExprKind::Block(_)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diag::Severity;

    fn parse_source(source: &str) -> (Program, Diagnostics) {
        let file = SourceFile::new("<test>", source);
        let (tokens, lexer_diagnostics) = crate::lexer::tokenize(source);
        assert!(
            !lexer_diagnostics.has_errors(),
            "unexpected lexer errors: {:?}",
            messages(&lexer_diagnostics)
        );
        parse(&tokens, &file)
    }

    fn messages(diagnostics: &Diagnostics) -> Vec<String> {
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.clone())
            .collect()
    }

    fn parse_ok(source: &str) -> Program {
        let (program, diagnostics) = parse_source(source);
        assert!(
            !diagnostics.has_errors(),
            "unexpected parse errors: {:?}",
            messages(&diagnostics)
        );
        program
    }

    fn parse_errors(source: &str) -> Vec<String> {
        let file = SourceFile::new("<test>", source);
        let (tokens, _) = crate::lexer::tokenize(source);
        let (_program, diagnostics) = parse(&tokens, &file);
        diagnostics
            .into_vec()
            .into_iter()
            .filter(|diagnostic| diagnostic.severity == Severity::Error)
            .map(|diagnostic| diagnostic.message)
            .collect()
    }

    fn fn_named<'a>(program: &'a Program, name: &str) -> &'a FnDecl {
        program
            .items
            .iter()
            .find_map(|item| match item {
                Item::Fn(function) if function.name == name => Some(function),
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing function {}", name))
    }

    fn main_fn(program: &Program) -> &FnDecl {
        fn_named(program, "main")
    }

    fn main_stmts(program: &Program) -> &[Stmt] {
        &main_fn(program).body.stmts
    }

    fn main_tail(program: &Program) -> &Expr {
        main_fn(program)
            .body
            .tail
            .as_deref()
            .expect("expected a tail expression")
    }

    fn parse_tail(source: &str) -> Expr {
        let wrapped = format!("fn main() {{ {} }}", source);
        let program = parse_ok(&wrapped);
        main_tail(&program).clone()
    }

    fn stmt_expr(stmt: &Stmt) -> &Expr {
        match stmt {
            Stmt::Expr(expr) => expr,
            other => panic!("expected an expression statement, got {:?}", other),
        }
    }

    fn let_init(stmt: &Stmt) -> &Expr {
        match stmt {
            Stmt::Let(statement) => statement.init.as_ref().expect("expected a let initializer"),
            other => panic!("expected a let statement, got {:?}", other),
        }
    }

    fn as_binary(expr: &Expr) -> (&BinaryOp, &Expr, &Expr) {
        match &expr.kind {
            ExprKind::Binary { op, lhs, rhs } => (op, lhs, rhs),
            other => panic!("expected a binary expression, got {:?}", other),
        }
    }

    fn as_int(expr: &Expr) -> i128 {
        match &expr.kind {
            ExprKind::Lit(Lit::Int { value, .. }) => *value,
            other => panic!("expected an integer literal, got {:?}", other),
        }
    }

    fn as_path_name(expr: &Expr) -> String {
        match &expr.kind {
            ExprKind::Path(path) => path.display_name(),
            other => panic!("expected a path expression, got {:?}", other),
        }
    }

    #[test]
    fn empty_source_parses_to_empty_program() {
        let program = parse_ok("");
        assert!(program.items.is_empty());
    }

    #[test]
    fn parses_hello_world() {
        let source = r#"fn main() { println!("hello, world!"); }"#;
        let program = parse_ok(source);
        assert_eq!(program.items.len(), 1);
        let main = main_fn(&program);
        assert!(main.params.is_empty());
        assert!(main.ret.is_none());
        assert!(main.self_kind.is_none());
        assert_eq!(main.body.stmts.len(), 1);
        let expr = stmt_expr(&main.body.stmts[0]);
        match &expr.kind {
            ExprKind::Macro(call) => {
                assert_eq!(call.name, "println");
                assert_eq!(call.args.len(), 1);
                assert_eq!(call.args[0].as_string_literal(), Some("hello, world!"));
            }
            other => panic!("expected a macro call, got {:?}", other),
        }
    }

    #[test]
    fn parses_function_signatures() {
        let program = parse_ok("fn add(a: i64, b: i64) -> i64 { a + b }");
        let add = fn_named(&program, "add");
        assert_eq!(add.params.len(), 2);
        assert_eq!(add.params[0].name, "a");
        assert_eq!(add.params[1].name, "b");
        assert!(matches!(&add.params[0].ty, TypeExpr::Path { .. }));
        match &add.ret {
            Some(TypeExpr::Path { name, generics, .. }) => {
                assert_eq!(name, "i64");
                assert!(generics.is_empty());
            }
            other => panic!("expected an i64 return type, got {:?}", other),
        }
        let tail = add
            .body
            .tail
            .as_deref()
            .expect("expected a tail expression");
        let (op, lhs, rhs) = as_binary(tail);
        assert_eq!(*op, BinaryOp::Add);
        assert_eq!(as_path_name(lhs), "a");
        assert_eq!(as_path_name(rhs), "b");

        let program = parse_ok("fn noop() {}");
        let noop = fn_named(&program, "noop");
        assert!(noop.ret.is_none());
        assert!(noop.body.stmts.is_empty());
        assert!(noop.body.tail.is_none());
    }

    #[test]
    fn parses_items_with_pub_visibility() {
        let source = r#"pub fn exported(x: i64) -> i64 { x }
pub struct Point { x: f64, y: f64 }
pub const MAX: i64 = 100;"#;
        let program = parse_ok(source);
        assert_eq!(program.items.len(), 3);
        assert_eq!(fn_named(&program, "exported").params.len(), 1);
        match &program.items[1] {
            Item::Struct(structure) => {
                assert_eq!(structure.name, "Point");
                assert_eq!(structure.fields.len(), 2);
                assert_eq!(structure.fields[0].name, "x");
                assert_eq!(structure.fields[1].name, "y");
            }
            other => panic!("expected a struct, got {:?}", other),
        }
        match &program.items[2] {
            Item::Const(constant) => {
                assert_eq!(constant.name, "MAX");
                assert_eq!(as_int(&constant.value), 100);
            }
            other => panic!("expected a const, got {:?}", other),
        }
    }

    #[test]
    fn parses_enum_declarations() {
        let source = "enum Shape { Circle(f64), Rectangle(f64, f64), Empty, }";
        let program = parse_ok(source);
        match &program.items[0] {
            Item::Enum(enum_decl) => {
                assert_eq!(enum_decl.name, "Shape");
                assert_eq!(enum_decl.variants.len(), 3);
                assert_eq!(enum_decl.variants[0].name, "Circle");
                assert_eq!(enum_decl.variants[0].fields.len(), 1);
                assert_eq!(enum_decl.variants[1].fields.len(), 2);
                assert!(enum_decl.variants[2].fields.is_empty());
            }
            other => panic!("expected an enum, got {:?}", other),
        }
    }

    #[test]
    fn parses_impl_block_receivers() {
        let source = r#"struct Point { x: f64 }
impl Point {
    fn new(x: f64) -> Point { Point { x: x } }
    fn get(&self) -> f64 { self.x }
    fn set(&mut self, value: f64) { self.x = value; }
    fn take(self) -> f64 { self.x }
}"#;
        let program = parse_ok(source);
        let implementation = program
            .items
            .iter()
            .find_map(|item| match item {
                Item::Impl(block) => Some(block),
                _ => None,
            })
            .expect("expected an impl block");
        assert_eq!(implementation.ty_name, "Point");
        assert!(implementation.trait_name.is_none());
        assert_eq!(implementation.fns.len(), 4);
        assert!(implementation.fns[0].self_kind.is_none());
        assert!(matches!(
            &implementation.fns[1].self_kind,
            Some(SelfKind::Ref)
        ));
        assert!(matches!(
            &implementation.fns[2].self_kind,
            Some(SelfKind::RefMut)
        ));
        assert!(matches!(
            &implementation.fns[3].self_kind,
            Some(SelfKind::Value)
        ));
        let get = &implementation.fns[1];
        assert_eq!(get.params.len(), 1);
        assert_eq!(get.params[0].name, "self");
        match &get.params[0].ty {
            TypeExpr::Path { name, .. } => assert_eq!(name, "Point"),
            other => panic!("expected the impl type as receiver type, got {:?}", other),
        }
        assert_eq!(implementation.fns[0].params.len(), 1);
    }

    fn first_impl_block(program: &Program) -> &ImplBlock {
        program
            .items
            .iter()
            .find_map(|item| match item {
                Item::Impl(block) => Some(block),
                _ => None,
            })
            .expect("expected an impl block")
    }

    #[test]
    fn inherent_impl_keeps_trait_name_none() {
        let source = r#"struct Point { x: i64 }
impl Point {
    fn get(&self) -> i64 { self.x }
}"#;
        let program = parse_ok(source);
        let implementation = first_impl_block(&program);
        assert!(implementation.trait_name.is_none());
        assert_eq!(implementation.ty_name, "Point");
    }

    #[test]
    fn parses_drop_trait_impl() {
        let source = r#"struct Point { x: i64 }
impl Drop for Point {
    fn drop(&mut self) {
        self.x = 0;
    }
}"#;
        let program = parse_ok(source);
        let implementation = first_impl_block(&program);
        assert_eq!(implementation.ty_name, "Point");
        let trait_ref = implementation
            .trait_name
            .as_ref()
            .expect("expected a trait reference");
        assert_eq!(trait_ref.name, "Drop");
        assert_eq!(
            &source[trait_ref.span.start as usize..trait_ref.span.end as usize],
            "Drop"
        );
        assert_eq!(implementation.fns.len(), 1);
        assert_eq!(implementation.fns[0].name, "drop");
        assert!(matches!(
            implementation.fns[0].self_kind,
            Some(SelfKind::RefMut)
        ));
    }

    #[test]
    fn parses_non_drop_trait_impl_for_typeck_validation() {
        // The parser accepts any single identifier as a trait name; only the
        // type checker decides which traits are meaningful.
        let source = r#"struct Point { x: i64 }
impl Other for Point {
    fn method(&self) -> i64 { self.x }
}"#;
        let program = parse_ok(source);
        let implementation = first_impl_block(&program);
        assert_eq!(implementation.ty_name, "Point");
        let trait_ref = implementation
            .trait_name
            .as_ref()
            .expect("expected a trait reference");
        assert_eq!(trait_ref.name, "Other");
    }

    #[test]
    fn rejects_trait_path_in_impl() {
        let source = "struct Point { x: i64 }\nimpl Trait::Sub for Point { }";
        let (program, diagnostics) = parse_source(source);
        let messages = messages(&diagnostics);
        assert!(
            diagnostics.has_errors(),
            "expected a parse error, got {:?}",
            messages
        );
        assert!(
            messages
                .iter()
                .any(|message| message.contains("single identifier")),
            "expected a malformed trait reference diagnostic, got {:?}",
            messages
        );
        // Recovery still parses the implemented type and the empty body.
        let implementation = first_impl_block(&program);
        assert!(implementation.trait_name.is_none());
        assert_eq!(implementation.ty_name, "Point");
        assert!(implementation.fns.is_empty());
    }

    #[test]
    fn parses_generic_trait_impl() {
        let source = "struct Point { x: i64 }\nimpl Trait<i64> for Point { }";
        let program = parse_ok(source);
        let implementation = first_impl_block(&program);
        assert_eq!(implementation.ty_name, "Point");
        let trait_ref = implementation
            .trait_name
            .as_ref()
            .expect("expected a trait reference");
        assert_eq!(trait_ref.name, "Trait");
        assert_eq!(trait_ref.args.len(), 1);
        assert!(matches!(
            &trait_ref.args[0],
            TypeExpr::Path { name, .. } if name == "i64"
        ));
        // The trait reference span covers the name and its arguments.
        assert_eq!(
            &source[trait_ref.span.start as usize..trait_ref.span.end as usize],
            "Trait<i64>"
        );
    }

    #[test]
    fn rejects_impl_without_trait_or_type() {
        let source = "struct Point { x: i64 }\nimpl for Point { }";
        let (program, diagnostics) = parse_source(source);
        let messages = messages(&diagnostics);
        assert!(
            diagnostics.has_errors(),
            "expected a parse error, got {:?}",
            messages
        );
        assert!(
            messages
                .iter()
                .any(|message| message.contains("expected a type or trait name")),
            "expected a missing type diagnostic, got {:?}",
            messages
        );
        // Recovery: the stray 'for' is skipped and Point becomes the impl type.
        let implementation = first_impl_block(&program);
        assert!(implementation.trait_name.is_none());
        assert_eq!(implementation.ty_name, "Point");
    }

    #[test]
    fn parses_generic_function_parameters_and_bounds() {
        let source = r#"
fn make<T, U: Iterator<i64> + Drop>(seed: T, other: U) -> U {
    other
}
"#;
        let program = parse_ok(source);
        let function = fn_named(&program, "make");
        assert_eq!(function.generics.len(), 2);
        assert_eq!(function.generics[0].name, "T");
        assert!(function.generics[0].bounds.is_empty());
        assert_eq!(function.generics[1].name, "U");
        assert_eq!(function.generics[1].bounds.len(), 2);
        assert!(matches!(
            &function.generics[1].bounds[0],
            TypeExpr::Path { name, generics, .. } if name == "Iterator" && generics.len() == 1
        ));
        assert!(matches!(
            &function.generics[1].bounds[1],
            TypeExpr::Path { name, .. } if name == "Drop"
        ));
    }

    #[test]
    fn parses_fn_callable_type_and_fn_bound() {
        let source = r#"
fn apply<T: Fn(i64, String) -> bool>(value: i64, callback: fn(i64) -> bool) -> bool {
    callback(value)
}
"#;
        let program = parse_ok(source);
        let function = fn_named(&program, "apply");
        match &function.generics[0].bounds[0] {
            TypeExpr::Fn { params, ret, .. } => {
                assert_eq!(params.len(), 2);
                assert!(matches!(ret.as_ref(), TypeExpr::Path { name, .. } if name == "bool"));
            }
            other => panic!("expected a callable bound, got {:?}", other),
        }
        assert!(matches!(
            &function.params[1].ty,
            TypeExpr::Fn { params, .. } if params.len() == 1
        ));
    }

    #[test]
    fn parses_generic_struct_enum_and_impl() {
        let source = r#"
struct Pair<T> { first: T, second: T }
enum Maybe<T> { Some(T), None }
impl<T> Pair<T> {
    fn first(&self) -> T { self.first }
}
"#;
        let program = parse_ok(source);
        let struct_decl = program
            .items
            .iter()
            .find_map(|item| match item {
                Item::Struct(decl) => Some(decl),
                _ => None,
            })
            .expect("expected a struct declaration");
        assert_eq!(struct_decl.generics.len(), 1);
        assert_eq!(struct_decl.generics[0].name, "T");
        let enum_decl = program
            .items
            .iter()
            .find_map(|item| match item {
                Item::Enum(decl) => Some(decl),
                _ => None,
            })
            .expect("expected an enum declaration");
        assert_eq!(enum_decl.generics.len(), 1);
        let implementation = first_impl_block(&program);
        assert_eq!(implementation.generics.len(), 1);
        assert_eq!(implementation.generics[0].name, "T");
        assert_eq!(implementation.ty_name, "Pair");
    }

    #[test]
    fn rejects_empty_generic_parameter_list() {
        for source in ["struct Empty<> { }", "fn f<>() { }", "impl<> Point { }"] {
            let (_, diagnostics) = parse_source(source);
            assert!(
                diagnostics.has_errors(),
                "expected an error for {:?}",
                source
            );
            assert!(
                messages(&diagnostics)
                    .iter()
                    .any(|message| message.contains("must not be empty")),
                "expected an empty generic list diagnostic for {:?}, got {:?}",
                source,
                messages(&diagnostics)
            );
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.code == Some("E0001")),
                "expected code E0001 for {:?}",
                source
            );
        }
    }

    #[test]
    fn generic_parameter_lists_allow_a_trailing_comma() {
        let program = parse_ok("fn f<T,>(value: T) -> T { value }");
        let function = fn_named(&program, "f");
        assert_eq!(function.generics.len(), 1);
    }

    #[test]
    fn parses_trait_declaration_with_receivers() {
        let source = r#"
pub trait Shape<T> {
    pub fn area(&self) -> i64;
    fn rotate(&mut self);
    fn into_id(self) -> i64;
}
"#;
        let program = parse_ok(source);
        let trait_decl = program
            .items
            .iter()
            .find_map(|item| match item {
                Item::Trait(decl) => Some(decl),
                _ => None,
            })
            .expect("expected a trait declaration");
        assert_eq!(trait_decl.name, "Shape");
        assert_eq!(trait_decl.generics.len(), 1);
        assert_eq!(trait_decl.methods.len(), 3);
        assert_eq!(trait_decl.methods[0].name, "area");
        assert!(matches!(
            trait_decl.methods[0].self_kind,
            Some(SelfKind::Ref)
        ));
        assert!(trait_decl.methods[0].ret.is_some());
        assert_eq!(trait_decl.methods[0].params.len(), 1);
        match &trait_decl.methods[0].params[0].ty {
            TypeExpr::Ref { mutable, inner, .. } => {
                assert!(!*mutable);
                assert!(matches!(inner.as_ref(), TypeExpr::Path { name, .. } if name == "Self"));
            }
            other => panic!("expected a reference receiver type, got {:?}", other),
        }
        assert!(matches!(
            trait_decl.methods[1].self_kind,
            Some(SelfKind::RefMut)
        ));
        assert!(matches!(
            trait_decl.methods[2].self_kind,
            Some(SelfKind::Value)
        ));
        assert_eq!(trait_decl.methods[1].params.len(), 1);
        assert_eq!(trait_decl.methods[2].params.len(), 1);
    }

    #[test]
    fn rejects_default_trait_method_body() {
        let source = "trait Shape { fn area(&self) -> i64 { 1 } }";
        let (program, diagnostics) = parse_source(source);
        assert!(
            messages(&diagnostics)
                .iter()
                .any(|message| message.contains("default method bodies are not supported")),
            "expected a default body diagnostic, got {:?}",
            messages(&diagnostics)
        );
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == Some("E0001"))
        );
        let trait_decl = program
            .items
            .iter()
            .find_map(|item| match item {
                Item::Trait(decl) => Some(decl),
                _ => None,
            })
            .expect("expected a trait declaration");
        assert_eq!(trait_decl.methods.len(), 1);
    }

    #[test]
    fn rejects_generic_trait_method() {
        let source = "trait Shape { fn convert<T>(&self, value: T); }";
        let (_, diagnostics) = parse_source(source);
        assert!(
            messages(&diagnostics)
                .iter()
                .any(|message| message.contains("cannot be generic")),
            "expected a generic method diagnostic, got {:?}",
            messages(&diagnostics)
        );
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == Some("E0001"))
        );
    }

    #[test]
    fn rejects_trait_method_without_receiver() {
        let source = "trait Shape { fn area(width: i64) -> i64; }";
        let (program, diagnostics) = parse_source(source);
        assert!(
            messages(&diagnostics)
                .iter()
                .any(|message| message.contains("must take a self receiver")),
            "expected a missing receiver diagnostic, got {:?}",
            messages(&diagnostics)
        );
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == Some("E0001"))
        );
        let trait_decl = program
            .items
            .iter()
            .find_map(|item| match item {
                Item::Trait(decl) => Some(decl),
                _ => None,
            })
            .expect("expected a trait declaration");
        assert!(trait_decl.methods[0].self_kind.is_none());
    }

    #[test]
    fn rejects_supertraits() {
        let source = "trait A: B { fn run(&self); }";
        let (program, diagnostics) = parse_source(source);
        assert!(
            messages(&diagnostics)
                .iter()
                .any(|message| message.contains("supertraits are not supported")),
            "expected a supertrait diagnostic, got {:?}",
            messages(&diagnostics)
        );
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == Some("E0001"))
        );
        let trait_decl = program
            .items
            .iter()
            .find_map(|item| match item {
                Item::Trait(decl) => Some(decl),
                _ => None,
            })
            .expect("expected a trait declaration");
        assert_eq!(trait_decl.methods.len(), 1);
    }

    #[test]
    fn generic_type_alias_is_rejected() {
        let source = "type Pair<T> = Vec<T>;";
        let (_, diagnostics) = parse_source(source);
        assert!(
            messages(&diagnostics)
                .iter()
                .any(|message| message.contains("cannot be generic")),
            "expected a generic type alias diagnostic, got {:?}",
            messages(&diagnostics)
        );
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == Some("E0001"))
        );
    }

    #[test]
    fn syntax_restriction_messages_do_not_mention_a_version() {
        let sources = [
            "trait A: B { fn run(&self); }",
            "trait Shape { fn convert<T>(&self, value: T); }",
            "trait Shape { fn area(&self) -> i64 { 1 } }",
            "trait Shape { fn area(width: i64) -> i64; }",
            "type Pair<T> = Vec<T>;",
        ];
        for source in sources {
            let (_, diagnostics) = parse_source(source);
            assert!(
                diagnostics.has_errors(),
                "expected syntax diagnostics for {source:?}"
            );
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.code == Some("E0001")),
                "expected an E0001 diagnostic for {source:?}, got {:?}",
                messages(&diagnostics)
            );
            for message in messages(&diagnostics) {
                assert!(
                    !message.contains("version"),
                    "syntax restriction message should not mention a version: {message:?}"
                );
            }
        }
    }

    #[test]
    fn accepts_pub_on_fields_variants_and_trait_methods() {
        let source = r#"
pub struct Point { pub x: i64, pub y: i64 }
pub enum Maybe<T> { pub Some(T), pub None }
pub trait Shape { pub fn area(&self) -> i64; }
pub fn make() -> Point { Point { x: 0, y: 0 } }
"#;
        parse_ok(source);
    }

    #[test]
    fn parses_turbofish_calls_struct_literals_and_associated_items() {
        let source = r#"
struct Wrap<T> { value: T }
impl<T> Wrap<T> {
    fn value(&self) -> T { self.value }
}
fn wrap<T>(value: T) -> Wrap<T> { Wrap { value } }
fn main() {
    let wrapped = wrap::<i64>(1);
    let literal = Wrap::<i64> { value: 2 };
    let value = Wrap::<i64>::value(&wrapped);
    let chained = wrapped.value::<i64>();
    let computed = literal.value;
}
"#;
        parse_ok(source);
    }

    #[test]
    fn parses_impl_of_generic_trait_for_generic_type() {
        let source = r#"
trait IntoValue<T> { fn into_value(&self) -> T; }
struct Wrap<T> { value: T }
impl<T> IntoValue<T> for Wrap<T> {
    fn into_value(&self) -> T { self.value }
}
"#;
        let program = parse_ok(source);
        let implementation = first_impl_block(&program);
        assert_eq!(implementation.ty_name, "Wrap");
        assert_eq!(implementation.generics.len(), 1);
        let trait_ref = implementation
            .trait_name
            .as_ref()
            .expect("expected a trait reference");
        assert_eq!(trait_ref.name, "IntoValue");
        assert_eq!(trait_ref.args.len(), 1);
    }

    #[test]
    fn parses_nested_angle_arguments_in_impl_trait_ref() {
        // The impl head scan must split a '>>' token into two closing angle
        // brackets when the trait reference has nested generic arguments.
        let source = r#"trait Collect<T> {
    fn collect_into(self) -> T;
}
struct Wrap<T> {
    value: T,
}
impl<T> Collect<Vec<i64>> for Wrap<T> {
    fn collect_into(self) -> Vec<i64> {
        vec![]
    }
}"#;
        let program = parse_ok(source);
        let implementation = first_impl_block(&program);
        assert_eq!(implementation.generics.len(), 1);
        let trait_ref = implementation.trait_name.as_ref().expect("trait reference");
        assert_eq!(trait_ref.name, "Collect");
        assert_eq!(trait_ref.args.len(), 1);
        match &trait_ref.args[0] {
            TypeExpr::Path { name, generics, .. } => {
                assert_eq!(name, "Vec");
                assert_eq!(generics.len(), 1);
            }
            other => panic!("expected a Vec<i64> trait argument, got {:?}", other),
        }
        assert_eq!(implementation.ty_name, "Wrap");
    }

    #[test]
    fn parses_drop_impl_for_generic_type() {
        let source = r#"
struct Pair<T> { first: T }
impl Drop for Pair<i64> {
    fn drop(&mut self) { }
}
"#;
        let program = parse_ok(source);
        let implementation = first_impl_block(&program);
        let trait_ref = implementation
            .trait_name
            .as_ref()
            .expect("expected a trait reference");
        assert_eq!(trait_ref.name, "Drop");
        assert!(trait_ref.args.is_empty());
        assert_eq!(implementation.ty_name, "Pair");
    }

    #[test]
    fn try_operator_is_allowed_after_block_like_expressions() {
        let source = r#"
fn pick_if(flag: bool) -> i64 {
    if flag { Ok(1) } else { Err(2) }?
}

fn pick_match(flag: bool) -> i64 {
    match flag { true => Ok(1), false => Err(2) }?
}

fn pick_let(flag: bool) -> i64 {
    let from_if = if flag { Ok(1) } else { Err(2) }?;
    let from_match = match flag { true => Ok(1), false => Err(2) }?;
    from_if + from_match
}
"#;
        let program = parse_ok(source);
        for name in ["pick_if", "pick_match"] {
            let function = fn_named(&program, name);
            let tail = function
                .body
                .tail
                .as_ref()
                .unwrap_or_else(|| panic!("expected a tail expression in {}", name));
            assert!(
                matches!(&tail.kind, ExprKind::Try { .. }),
                "expected a try expression as the tail of {}",
                name
            );
        }
        let function = fn_named(&program, "pick_let");
        let mut inits = Vec::new();
        for stmt in &function.body.stmts {
            if let Stmt::Let(let_stmt) = stmt
                && let Some(init) = &let_stmt.init
            {
                inits.push(init);
            }
        }
        assert_eq!(inits.len(), 2);
        assert!(matches!(&inits[0].kind, ExprKind::Try { .. }));
        assert!(matches!(&inits[1].kind, ExprKind::Try { .. }));
    }

    #[test]
    fn for_patterns_allow_mut_bindings() {
        let source = "fn main() { for mut value in values { value = 1; } }";
        let program = parse_ok(source);
        let body = &fn_named(&program, "main").body;
        let for_expr = body
            .tail
            .as_deref()
            .or_else(|| {
                body.stmts.iter().find_map(|stmt| match stmt {
                    Stmt::Expr(expr) => Some(expr),
                    _ => None,
                })
            })
            .expect("expected a for expression");
        match &for_expr.kind {
            ExprKind::For { pat, .. } => {
                assert!(matches!(
                    &pat.kind,
                    PatternKind::Binding { mutable, .. } if *mutable
                ));
            }
            other => panic!("expected a for expression, got {:?}", other),
        }
    }

    #[test]
    fn parses_self_type_in_impl() {
        let source = r#"impl Counter {
    fn new(start: i64) -> Self {
        Counter { value: start }
    }

    fn bump(&mut self) -> Self {
        self
    }
}
"#;
        let program = parse_ok(source);
        let implementation = program
            .items
            .iter()
            .find_map(|item| match item {
                Item::Impl(block) => Some(block),
                _ => None,
            })
            .expect("expected an impl block");
        assert_eq!(implementation.fns.len(), 2);
        match &implementation.fns[0].ret {
            Some(TypeExpr::Path { name, .. }) => assert_eq!(name, "Self"),
            other => panic!("expected a Self return type, got {:?}", other),
        }
        match &implementation.fns[0]
            .body
            .tail
            .as_deref()
            .expect("tail")
            .kind
        {
            ExprKind::StructLit { path, fields } => {
                assert_eq!(path.display_name(), "Counter");
                assert_eq!(fields.len(), 1);
            }
            other => panic!("expected a struct literal, got {:?}", other),
        }
        match &implementation.fns[1].ret {
            Some(TypeExpr::Path { name, .. }) => assert_eq!(name, "Self"),
            other => panic!("expected a Self return type, got {:?}", other),
        }
        // Outside an impl block the parser still accepts the type; the type checker
        // reports "Self is only available inside an impl block".
        let outside = parse_ok("fn helper() -> Self { loop {} }");
        match &fn_named(&outside, "helper").ret {
            Some(TypeExpr::Path { name, .. }) => assert_eq!(name, "Self"),
            other => panic!("expected a Self return type, got {:?}", other),
        }
    }

    #[test]
    fn parses_reference_expressions() {
        let tail = parse_tail("&value");
        match &tail.kind {
            ExprKind::Ref { mutable, expr } => {
                assert!(!mutable);
                assert_eq!(as_path_name(expr), "value");
            }
            other => panic!("expected a reference expression, got {:?}", other),
        }
    }

    #[test]
    fn parses_mutable_reference_expressions() {
        let tail = parse_tail("&mut value");
        match &tail.kind {
            ExprKind::Ref { mutable, expr } => {
                assert!(mutable);
                assert_eq!(as_path_name(expr), "value");
            }
            other => panic!("expected a mutable reference expression, got {:?}", other),
        }
    }

    #[test]
    fn parses_deref_expressions() {
        let tail = parse_tail("*pointer");
        match &tail.kind {
            ExprKind::Deref { expr } => assert_eq!(as_path_name(expr), "pointer"),
            other => panic!("expected a dereference, got {:?}", other),
        }

        // Dereference chains are right associative.
        let tail = parse_tail("**pointer");
        match &tail.kind {
            ExprKind::Deref { expr } => match &expr.kind {
                ExprKind::Deref { expr } => assert_eq!(as_path_name(expr), "pointer"),
                other => panic!("expected an inner dereference, got {:?}", other),
            },
            other => panic!("expected a dereference, got {:?}", other),
        }
    }

    #[test]
    fn double_ampersand_is_nested_shared_refs() {
        let tail = parse_tail("&&x");
        match &tail.kind {
            ExprKind::Ref {
                mutable: false,
                expr,
            } => match &expr.kind {
                ExprKind::Ref {
                    mutable: false,
                    expr,
                } => assert_eq!(as_path_name(expr), "x"),
                other => panic!("expected an inner reference, got {:?}", other),
            },
            other => panic!("expected a reference, got {:?}", other),
        }

        // A written '&&' is one token; a following 'mut' belongs to the inner
        // borrow, so &&mut x is &(&mut x).
        let tail = parse_tail("&&mut x");
        match &tail.kind {
            ExprKind::Ref {
                mutable: false,
                expr,
            } => match &expr.kind {
                ExprKind::Ref {
                    mutable: true,
                    expr,
                } => assert_eq!(as_path_name(expr), "x"),
                other => panic!("expected an inner mutable reference, got {:?}", other),
            },
            other => panic!("expected a reference, got {:?}", other),
        }
    }

    #[test]
    fn reference_binds_tighter_than_binary_operators() {
        let root = parse_tail("&a + b");
        let (op, lhs, rhs) = as_binary(&root);
        assert_eq!(*op, BinaryOp::Add);
        assert!(matches!(&lhs.kind, ExprKind::Ref { mutable: false, .. }));
        assert_eq!(as_path_name(rhs), "b");

        let root = parse_tail("a + &b");
        let (op, lhs, rhs) = as_binary(&root);
        assert_eq!(*op, BinaryOp::Add);
        assert_eq!(as_path_name(lhs), "a");
        assert!(matches!(&rhs.kind, ExprKind::Ref { .. }));

        let root = parse_tail("a & &b");
        let (op, _, rhs) = as_binary(&root);
        assert_eq!(*op, BinaryOp::BitAnd);
        assert!(matches!(&rhs.kind, ExprKind::Ref { .. }));
    }

    #[test]
    fn reference_binds_postfix_operations() {
        let tail = parse_tail("&point.x");
        match &tail.kind {
            ExprKind::Ref {
                mutable: false,
                expr,
            } => match &expr.kind {
                ExprKind::Field { base, name, .. } => {
                    assert_eq!(as_path_name(base), "point");
                    assert_eq!(name, "x");
                }
                other => panic!("expected a field access, got {:?}", other),
            },
            other => panic!("expected a reference, got {:?}", other),
        }

        let tail = parse_tail("&mut values[0]");
        match &tail.kind {
            ExprKind::Ref {
                mutable: true,
                expr,
            } => {
                assert!(matches!(&expr.kind, ExprKind::Index { .. }));
            }
            other => panic!("expected a mutable reference, got {:?}", other),
        }
    }

    #[test]
    fn ref_and_deref_chain() {
        let tail = parse_tail("&*p");
        match &tail.kind {
            ExprKind::Ref { expr, .. } => assert!(matches!(&expr.kind, ExprKind::Deref { .. })),
            other => panic!("expected a reference, got {:?}", other),
        }

        let tail = parse_tail("*&x");
        match &tail.kind {
            ExprKind::Deref { expr } => assert!(matches!(&expr.kind, ExprKind::Ref { .. })),
            other => panic!("expected a dereference, got {:?}", other),
        }
    }

    #[test]
    fn unary_not_and_neg_apply_to_references() {
        let tail = parse_tail("!&x");
        match &tail.kind {
            ExprKind::Unary {
                op: UnaryOp::Not,
                expr,
            } => assert!(matches!(&expr.kind, ExprKind::Ref { .. })),
            other => panic!("expected a logical not, got {:?}", other),
        }

        let tail = parse_tail("-&x");
        match &tail.kind {
            ExprKind::Unary {
                op: UnaryOp::Neg,
                expr,
            } => assert!(matches!(&expr.kind, ExprKind::Ref { .. })),
            other => panic!("expected a negation, got {:?}", other),
        }
    }

    #[test]
    fn binary_ampersand_and_star_still_work() {
        let root = parse_tail("a & b");
        let (op, lhs, rhs) = as_binary(&root);
        assert_eq!(*op, BinaryOp::BitAnd);
        assert_eq!(as_path_name(lhs), "a");
        assert_eq!(as_path_name(rhs), "b");

        let root = parse_tail("a * b");
        let (op, _, _) = as_binary(&root);
        assert_eq!(*op, BinaryOp::Mul);

        let root = parse_tail("a && b");
        let (op, _, _) = as_binary(&root);
        assert_eq!(*op, BinaryOp::And);
    }

    #[test]
    fn parses_reference_types() {
        let program = parse_ok("fn f(a: &i64, b: &mut String) -> &&f64 { loop {} }");
        let function = fn_named(&program, "f");
        match &function.params[0].ty {
            TypeExpr::Ref { mutable, inner, .. } => {
                assert!(!mutable);
                match inner.as_ref() {
                    TypeExpr::Path { name, .. } => assert_eq!(name, "i64"),
                    other => panic!("expected the i64 referent, got {:?}", other),
                }
            }
            other => panic!("expected a reference type, got {:?}", other),
        }
        match &function.params[1].ty {
            TypeExpr::Ref { mutable, inner, .. } => {
                assert!(mutable);
                match inner.as_ref() {
                    TypeExpr::Path { name, .. } => assert_eq!(name, "String"),
                    other => panic!("expected the String referent, got {:?}", other),
                }
            }
            other => panic!("expected a reference type, got {:?}", other),
        }
        match &function.ret {
            Some(TypeExpr::Ref { mutable, inner, .. }) => {
                assert!(!mutable);
                match inner.as_ref() {
                    TypeExpr::Ref { mutable, inner, .. } => {
                        assert!(!mutable);
                        assert!(
                            matches!(inner.as_ref(), TypeExpr::Path { name, .. } if name == "f64")
                        );
                    }
                    other => panic!("expected a nested reference type, got {:?}", other),
                }
            }
            other => panic!("expected a nested reference return type, got {:?}", other),
        }
    }

    #[test]
    fn parses_compound_reference_types() {
        let program = parse_ok(
            "fn f(a: &(i64, i64), b: &fn(i64) -> i64, c: &(), d: &mut &Vec<i64>, e: &&mut i64) {}",
        );
        let function = fn_named(&program, "f");
        let descriptions: Vec<String> = function
            .params
            .iter()
            .map(|param| param.ty.describe())
            .collect();
        assert_eq!(
            descriptions,
            vec![
                "&(i64, i64)",
                "&fn(i64) -> i64",
                "&()",
                "&mut &Vec<i64>",
                "&&mut i64"
            ]
        );
    }

    #[test]
    fn reference_type_spans_cover_the_ampersand() {
        let source = "fn f(a: &i64, b: &&i64) {}";
        let program = parse_ok(source);
        let function = fn_named(&program, "f");
        let span = function.params[0].ty.span();
        assert_eq!(&source[span.start as usize..span.end as usize], "&");
        let span = function.params[1].ty.span();
        assert_eq!(&source[span.start as usize..span.end as usize], "&&");
    }

    #[test]
    fn rejects_reference_patterns() {
        for source in [
            "fn f(x: i64) { match x { &mut y => 1, _ => 2 } }",
            "fn f(x: i64) { match x { &&z => 1, _ => 2 } }",
            "fn f(x: i64) { let &w = x; }",
            "fn f(x: i64) { if let &v = &x {} }",
        ] {
            let errors = parse_errors(source);
            assert!(
                errors
                    .iter()
                    .any(|message| message.contains("reference patterns are not supported")),
                "expected a reference pattern diagnostic for {:?}, got {:?}",
                source,
                errors
            );
        }
    }

    #[test]
    fn reference_pattern_recovery_keeps_parsing() {
        let (program, diagnostics) = parse_source("fn f(x: i64) { match x { &y => 1, _ => 2 } }");
        let errors: Vec<String> = diagnostics
            .into_vec()
            .into_iter()
            .filter(|diagnostic| diagnostic.severity == Severity::Error)
            .map(|diagnostic| diagnostic.message)
            .collect();
        assert_eq!(errors.len(), 1, "expected one diagnostic, got {:?}", errors);
        assert!(errors[0].contains("reference patterns are not supported"));

        let function = fn_named(&program, "f");
        match &function.body.tail.as_deref().expect("tail").kind {
            ExprKind::Match(match_expr) => {
                assert_eq!(match_expr.arms.len(), 2);
                assert_eq!(as_int(&match_expr.arms[0].body), 1);
                assert_eq!(as_int(&match_expr.arms[1].body), 2);
            }
            other => panic!("expected a match expression, got {:?}", other),
        }
    }

    #[test]
    fn receivers_still_parse_with_reference_syntax() {
        let source = r#"struct Point { x: i64 }
impl Point {
    fn get(&self) -> &i64 { &self.x }
    fn get_mut(&mut self) -> &mut i64 { &mut self.x }
}"#;
        let program = parse_ok(source);
        let implementation = program
            .items
            .iter()
            .find_map(|item| match item {
                Item::Impl(block) => Some(block),
                _ => None,
            })
            .expect("expected an impl block");
        assert!(matches!(
            &implementation.fns[0].self_kind,
            Some(SelfKind::Ref)
        ));
        assert!(matches!(
            &implementation.fns[1].self_kind,
            Some(SelfKind::RefMut)
        ));
        assert_eq!(implementation.fns[0].params.len(), 1);
        assert_eq!(implementation.fns[0].params[0].name, "self");

        match &implementation.fns[0].ret {
            Some(TypeExpr::Ref { mutable, inner, .. }) => {
                assert!(!mutable);
                assert!(matches!(inner.as_ref(), TypeExpr::Path { name, .. } if name == "i64"));
            }
            other => panic!("expected a shared reference return type, got {:?}", other),
        }
        match &implementation.fns[1].ret {
            Some(TypeExpr::Ref { mutable, .. }) => assert!(mutable),
            other => panic!("expected a mutable reference return type, got {:?}", other),
        }

        match &implementation.fns[0]
            .body
            .tail
            .as_deref()
            .expect("tail")
            .kind
        {
            ExprKind::Ref {
                mutable: false,
                expr,
            } => match &expr.kind {
                ExprKind::Field { base, name, .. } => {
                    assert_eq!(as_path_name(base), "self");
                    assert_eq!(name, "x");
                }
                other => panic!("expected a field access, got {:?}", other),
            },
            other => panic!("expected a reference expression, got {:?}", other),
        }
        match &implementation.fns[1]
            .body
            .tail
            .as_deref()
            .expect("tail")
            .kind
        {
            ExprKind::Ref {
                mutable: true,
                expr,
            } => {
                assert!(matches!(&expr.kind, ExprKind::Field { .. }));
            }
            other => panic!("expected a mutable reference expression, got {:?}", other),
        }
    }

    #[test]
    fn parses_ref_and_deref_statements() {
        let source = "fn main() { let mut n = 1; let m = &mut n; *m = *m + 1; }";
        let program = parse_ok(source);
        let stmts = main_stmts(&program);
        assert_eq!(stmts.len(), 3);
        assert!(matches!(
            &let_init(&stmts[1]).kind,
            ExprKind::Ref { mutable: true, .. }
        ));

        match &stmt_expr(&stmts[2]).kind {
            ExprKind::Assign {
                op: None,
                target,
                value,
            } => {
                assert!(matches!(&target.kind, ExprKind::Deref { .. }));
                let (op, lhs, rhs) = as_binary(value);
                assert_eq!(*op, BinaryOp::Add);
                assert!(matches!(&lhs.kind, ExprKind::Deref { .. }));
                assert_eq!(as_int(rhs), 1);
            }
            other => panic!("expected an assignment, got {:?}", other),
        }
    }

    #[test]
    fn return_of_reference_value() {
        let program = parse_ok("fn f(x: i64) -> &i64 { return &x; }");
        let function = fn_named(&program, "f");
        assert_eq!(
            function.ret.as_ref().map(TypeExpr::describe),
            Some("&i64".to_string())
        );
        assert_eq!(function.body.stmts.len(), 1);
        match &stmt_expr(&function.body.stmts[0]).kind {
            ExprKind::Return(Some(value)) => {
                assert!(matches!(&value.kind, ExprKind::Ref { mutable: false, .. }));
            }
            other => panic!("expected a return expression, got {:?}", other),
        }
    }

    #[test]
    fn reference_in_for_head() {
        // A block-like expression in final position is the block tail, so the
        // for loop is the tail expression rather than a statement.
        let program = parse_ok("fn main() { for x in &items {} }");
        match &main_tail(&program).kind {
            ExprKind::For { iter, .. } => {
                assert!(matches!(&iter.kind, ExprKind::Ref { mutable: false, .. }));
            }
            other => panic!("expected a for expression, got {:?}", other),
        }
    }

    #[test]
    fn parses_const_and_type_alias() {
        let program =
            parse_ok("const MAX: i64 = 100; type Pair = (i64, i64); fn pair() -> Pair { (1, 2) }");
        assert_eq!(program.items.len(), 3);
        match &program.items[0] {
            Item::Const(constant) => {
                assert_eq!(constant.name, "MAX");
                assert!(matches!(&constant.ty, TypeExpr::Path { name, .. } if name == "i64"));
                assert_eq!(as_int(&constant.value), 100);
            }
            other => panic!("expected a const, got {:?}", other),
        }
        match &program.items[1] {
            Item::TypeAlias(alias) => {
                assert_eq!(alias.name, "Pair");
                match &alias.ty {
                    TypeExpr::Tuple { elems, .. } => assert_eq!(elems.len(), 2),
                    other => panic!("expected a tuple type, got {:?}", other),
                }
            }
            other => panic!("expected a type alias, got {:?}", other),
        }
        let pair = fn_named(&program, "pair");
        match &pair.ret {
            Some(TypeExpr::Path { name, .. }) => assert_eq!(name, "Pair"),
            other => panic!("expected a Pair return type, got {:?}", other),
        }
        match &pair.body.tail.as_deref().expect("tail").kind {
            ExprKind::Tuple(elements) => assert_eq!(elements.len(), 2),
            other => panic!("expected a tuple, got {:?}", other),
        }
    }

    #[test]
    fn parses_nested_items_in_blocks() {
        let source = "fn main() { fn helper() -> i64 { 1 } helper() }";
        let program = parse_ok(source);
        let stmts = main_stmts(&program);
        assert_eq!(stmts.len(), 1);
        match &stmts[0] {
            Stmt::Item(Item::Fn(helper)) => assert_eq!(helper.name, "helper"),
            other => panic!("expected a nested function item, got {:?}", other),
        }
        match &main_tail(&program).kind {
            ExprKind::Call { callee, args } => {
                assert_eq!(as_path_name(callee), "helper");
                assert!(args.is_empty());
            }
            other => panic!("expected a call, got {:?}", other),
        }
    }

    #[test]
    fn parses_let_statements() {
        let source = "fn main() { let x: i64 = 1; let mut y = 2; let z; }";
        let program = parse_ok(source);
        let stmts = main_stmts(&program);
        assert_eq!(stmts.len(), 3);
        match &stmts[0] {
            Stmt::Let(statement) => {
                match &statement.pat.kind {
                    PatternKind::Binding { name, mutable } => {
                        assert_eq!(name, "x");
                        assert!(!*mutable);
                    }
                    other => panic!("expected a binding, got {:?}", other),
                }
                assert!(statement.ty.is_some());
                assert_eq!(as_int(statement.init.as_ref().expect("init")), 1);
            }
            other => panic!("expected a let statement, got {:?}", other),
        }
        match &stmts[1] {
            Stmt::Let(statement) => match &statement.pat.kind {
                PatternKind::Binding { name, mutable } => {
                    assert_eq!(name, "y");
                    assert!(*mutable);
                }
                other => panic!("expected a mutable binding, got {:?}", other),
            },
            other => panic!("expected a let statement, got {:?}", other),
        }
        match &stmts[2] {
            Stmt::Let(statement) => {
                assert!(statement.ty.is_none());
                assert!(statement.init.is_none());
            }
            other => panic!("expected a let statement, got {:?}", other),
        }
    }

    #[test]
    fn parses_empty_statement_and_block_tail() {
        let program = parse_ok("fn main() { ; 1 + 2 }");
        let stmts = main_stmts(&program);
        assert_eq!(stmts.len(), 1);
        assert!(matches!(&stmts[0], Stmt::Empty(_)));
        let (op, lhs, rhs) = as_binary(main_tail(&program));
        assert_eq!(*op, BinaryOp::Add);
        assert_eq!(as_int(lhs), 1);
        assert_eq!(as_int(rhs), 2);
    }

    #[test]
    fn arithmetic_precedence() {
        let expr = parse_tail("1 + 2 * 3");
        let (op, lhs, rhs) = as_binary(&expr);
        assert_eq!(*op, BinaryOp::Add);
        assert_eq!(as_int(lhs), 1);
        let (inner_op, inner_lhs, inner_rhs) = as_binary(rhs);
        assert_eq!(*inner_op, BinaryOp::Mul);
        assert_eq!(as_int(inner_lhs), 2);
        assert_eq!(as_int(inner_rhs), 3);

        let expr = parse_tail("(1 + 2) * 3");
        let (op, lhs, rhs) = as_binary(&expr);
        assert_eq!(*op, BinaryOp::Mul);
        let (inner_op, _, _) = as_binary(lhs);
        assert_eq!(*inner_op, BinaryOp::Add);
        assert_eq!(as_int(rhs), 3);
    }

    #[test]
    fn bitwise_and_logical_precedence() {
        let expr = parse_tail("1 | 2 ^ 3 & 4");
        let (top, lhs, rhs) = as_binary(&expr);
        assert_eq!(*top, BinaryOp::BitOr);
        assert_eq!(as_int(lhs), 1);
        let (middle, middle_lhs, middle_rhs) = as_binary(rhs);
        assert_eq!(*middle, BinaryOp::BitXor);
        assert_eq!(as_int(middle_lhs), 2);
        let (low, low_lhs, low_rhs) = as_binary(middle_rhs);
        assert_eq!(*low, BinaryOp::BitAnd);
        assert_eq!(as_int(low_lhs), 3);
        assert_eq!(as_int(low_rhs), 4);

        let expr = parse_tail("a && b || c");
        let (top, lhs, rhs) = as_binary(&expr);
        assert_eq!(*top, BinaryOp::Or);
        let (inner, _, _) = as_binary(lhs);
        assert_eq!(*inner, BinaryOp::And);
        assert_eq!(as_path_name(rhs), "c");

        let expr = parse_tail("1 << 2 >> 3");
        let (top, lhs, rhs) = as_binary(&expr);
        assert_eq!(*top, BinaryOp::Shr);
        assert_eq!(as_int(rhs), 3);
        let (inner, inner_lhs, inner_rhs) = as_binary(lhs);
        assert_eq!(*inner, BinaryOp::Shl);
        assert_eq!(as_int(inner_lhs), 1);
        assert_eq!(as_int(inner_rhs), 2);
    }

    #[test]
    fn assignment_is_right_associative() {
        let expr = parse_tail("a = b = c");
        match &expr.kind {
            ExprKind::Assign { op, target, value } => {
                assert!(op.is_none());
                assert_eq!(as_path_name(target), "a");
                match &value.kind {
                    ExprKind::Assign {
                        target: inner_target,
                        value: inner_value,
                        ..
                    } => {
                        assert_eq!(as_path_name(inner_target), "b");
                        assert_eq!(as_path_name(inner_value), "c");
                    }
                    other => panic!("expected a nested assignment, got {:?}", other),
                }
            }
            other => panic!("expected an assignment, got {:?}", other),
        }
    }

    #[test]
    fn compound_assignments() {
        let expr = parse_tail("x += 1");
        match &expr.kind {
            ExprKind::Assign { op, target, .. } => {
                assert_eq!(*op, Some(BinaryOp::Add));
                assert_eq!(as_path_name(target), "x");
            }
            other => panic!("expected an assignment, got {:?}", other),
        }
        let expr = parse_tail("x <<= 2");
        match &expr.kind {
            ExprKind::Assign { op, .. } => assert_eq!(*op, Some(BinaryOp::Shl)),
            other => panic!("expected an assignment, got {:?}", other),
        }
    }

    #[test]
    fn unary_and_cast() {
        let expr = parse_tail("-x as i64");
        match &expr.kind {
            ExprKind::Cast { expr: inner, ty } => {
                match &inner.kind {
                    ExprKind::Unary { op, expr } => {
                        assert_eq!(*op, UnaryOp::Neg);
                        assert_eq!(as_path_name(expr), "x");
                    }
                    other => panic!("expected a unary expression, got {:?}", other),
                }
                match ty {
                    TypeExpr::Path { name, .. } => assert_eq!(name, "i64"),
                    other => panic!("expected an i64 type, got {:?}", other),
                }
            }
            other => panic!("expected a cast, got {:?}", other),
        }
        let expr = parse_tail("!a");
        match &expr.kind {
            ExprKind::Unary { op, expr } => {
                assert_eq!(*op, UnaryOp::Not);
                assert_eq!(as_path_name(expr), "a");
            }
            other => panic!("expected a unary expression, got {:?}", other),
        }
    }

    #[test]
    fn postfix_chains() {
        let expr = parse_tail("f(1).g(2)[0].h");
        match &expr.kind {
            ExprKind::Field { base, name, .. } => {
                assert_eq!(name, "h");
                match &base.kind {
                    ExprKind::Index { base, index } => {
                        assert_eq!(as_int(index), 0);
                        match &base.kind {
                            ExprKind::MethodCall {
                                receiver,
                                method,
                                args,
                                ..
                            } => {
                                assert_eq!(method, "g");
                                assert_eq!(args.len(), 1);
                                match &receiver.kind {
                                    ExprKind::Call { callee, args } => {
                                        assert_eq!(as_path_name(callee), "f");
                                        assert_eq!(args.len(), 1);
                                    }
                                    other => panic!("expected a call, got {:?}", other),
                                }
                            }
                            other => panic!("expected a method call, got {:?}", other),
                        }
                    }
                    other => panic!("expected an index expression, got {:?}", other),
                }
            }
            other => panic!("expected a field access, got {:?}", other),
        }
    }

    #[test]
    fn tuple_fields() {
        let program = parse_ok("fn main() { let a = t.0; let b = t.0.1; }");
        let stmts = main_stmts(&program);
        match &let_init(&stmts[0]).kind {
            ExprKind::TupleField { base, index, .. } => {
                assert_eq!(*index, 0);
                assert_eq!(as_path_name(base), "t");
            }
            other => panic!("expected a tuple field, got {:?}", other),
        }
        match &let_init(&stmts[1]).kind {
            ExprKind::TupleField { base, index, .. } => {
                assert_eq!(*index, 1);
                match &base.kind {
                    ExprKind::TupleField { index, .. } => assert_eq!(*index, 0),
                    other => panic!("expected a tuple field, got {:?}", other),
                }
            }
            other => panic!("expected a tuple field, got {:?}", other),
        }
    }

    #[test]
    fn method_turbofish_and_associated_calls() {
        let program =
            parse_ok("fn main() { let n = s.parse::<i64>(); let v = Vec::<i64>::new(); }");
        let stmts = main_stmts(&program);
        match &let_init(&stmts[0]).kind {
            ExprKind::MethodCall {
                method, turbofish, ..
            } => {
                assert_eq!(method, "parse");
                assert_eq!(turbofish.len(), 1);
                match &turbofish[0] {
                    TypeExpr::Path { name, .. } => assert_eq!(name, "i64"),
                    other => panic!("expected an i64 type argument, got {:?}", other),
                }
            }
            other => panic!("expected a method call, got {:?}", other),
        }
        match &let_init(&stmts[1]).kind {
            ExprKind::Call { callee, .. } => match &callee.kind {
                ExprKind::Path(path) => {
                    assert_eq!(path.segments.len(), 2);
                    assert_eq!(path.segments[0].name, "Vec");
                    assert_eq!(path.segments[0].generics.len(), 1);
                    assert_eq!(path.segments[1].name, "new");
                }
                other => panic!("expected a path callee, got {:?}", other),
            },
            other => panic!("expected a call, got {:?}", other),
        }
    }

    #[test]
    fn nested_generic_types() {
        let program = parse_ok("fn main() { let v: Vec<Vec<i64>> = Vec::new(); }");
        let stmts = main_stmts(&program);
        match &stmts[0] {
            Stmt::Let(statement) => match statement.ty.as_ref() {
                Some(TypeExpr::Path { name, generics, .. }) => {
                    assert_eq!(name, "Vec");
                    assert_eq!(generics.len(), 1);
                    match &generics[0] {
                        TypeExpr::Path { name, generics, .. } => {
                            assert_eq!(name, "Vec");
                            assert_eq!(generics.len(), 1);
                        }
                        other => panic!("expected a nested Vec type, got {:?}", other),
                    }
                }
                other => panic!("expected a Vec type, got {:?}", other),
            },
            other => panic!("expected a let statement, got {:?}", other),
        }
    }

    #[test]
    fn parses_if_else_chain() {
        let program = parse_ok("fn main() { if a { 1 } else if b { 2 } else { 3 } }");
        match &main_tail(&program).kind {
            ExprKind::If(if_expr) => {
                match &if_expr.else_expr {
                    Some(else_expr) => match &else_expr.kind {
                        ExprKind::If(inner) => match &inner.else_expr {
                            Some(inner_else) => {
                                assert!(matches!(&inner_else.kind, ExprKind::Block(_)))
                            }
                            other => panic!("expected a final else block, got {:?}", other),
                        },
                        other => panic!("expected an else-if, got {:?}", other),
                    },
                    other => panic!("expected an else branch, got {:?}", other),
                }
                assert!(matches!(
                    &if_expr.then_block.tail.as_deref().map(|e| &e.kind),
                    Some(ExprKind::Lit(Lit::Int { value: 1, .. }))
                ));
            }
            other => panic!("expected an if expression, got {:?}", other),
        }
    }

    #[test]
    fn parses_match_with_guards_and_or_patterns() {
        let source = r#"fn main() {
    match n {
        1 | 2 | 3 => 10,
        x if x > 3 => 20,
        _ => 0,
    }
}"#;
        let program = parse_ok(source);
        match &main_tail(&program).kind {
            ExprKind::Match(match_expr) => {
                assert_eq!(match_expr.arms.len(), 3);
                match &match_expr.arms[0].pat.kind {
                    PatternKind::Or(alternatives) => assert_eq!(alternatives.len(), 3),
                    other => panic!("expected an or pattern, got {:?}", other),
                }
                assert!(match_expr.arms[1].guard.is_some());
                match &match_expr.arms[1].pat.kind {
                    PatternKind::Binding { name, mutable } => {
                        assert_eq!(name, "x");
                        assert!(!*mutable);
                    }
                    other => panic!("expected a binding, got {:?}", other),
                }
                assert!(matches!(&match_expr.arms[2].pat.kind, PatternKind::Wild));
            }
            other => panic!("expected a match expression, got {:?}", other),
        }
    }

    #[test]
    fn parses_enum_patterns() {
        let source = r#"fn main() {
    match shape {
        Shape::Circle(r) => r,
        Shape::Rectangle(w, h) => w,
        Shape::Empty => 0.0,
    }
}"#;
        let program = parse_ok(source);
        match &main_tail(&program).kind {
            ExprKind::Match(match_expr) => {
                assert_eq!(match_expr.arms.len(), 3);
                match &match_expr.arms[0].pat.kind {
                    PatternKind::Path { path, args } => {
                        assert_eq!(path.display_name(), "Shape::Circle");
                        assert_eq!(args.len(), 1);
                        assert!(matches!(&args[0].kind, PatternKind::Binding { .. }));
                    }
                    other => panic!("expected a path pattern, got {:?}", other),
                }
                match &match_expr.arms[1].pat.kind {
                    PatternKind::Path { path, args } => {
                        assert_eq!(path.display_name(), "Shape::Rectangle");
                        assert_eq!(args.len(), 2);
                    }
                    other => panic!("expected a path pattern, got {:?}", other),
                }
                match &match_expr.arms[2].pat.kind {
                    PatternKind::Path { path, args } => {
                        assert_eq!(path.display_name(), "Shape::Empty");
                        assert!(args.is_empty());
                    }
                    other => panic!("expected a path pattern, got {:?}", other),
                }
            }
            other => panic!("expected a match expression, got {:?}", other),
        }
    }

    #[test]
    fn parses_struct_patterns_with_rest() {
        let source = r#"fn main() {
    match p {
        Point { x, y } => x,
        Point { x, .. } => 0.0,
    }
}"#;
        let program = parse_ok(source);
        match &main_tail(&program).kind {
            ExprKind::Match(match_expr) => {
                match &match_expr.arms[0].pat.kind {
                    PatternKind::Struct {
                        path,
                        fields,
                        has_rest,
                    } => {
                        assert_eq!(path.display_name(), "Point");
                        assert_eq!(fields.len(), 2);
                        assert_eq!(fields[0].name, "x");
                        assert_eq!(fields[1].name, "y");
                        assert!(!*has_rest);
                    }
                    other => panic!("expected a struct pattern, got {:?}", other),
                }
                match &match_expr.arms[1].pat.kind {
                    PatternKind::Struct {
                        fields, has_rest, ..
                    } => {
                        assert_eq!(fields.len(), 1);
                        assert!(*has_rest);
                    }
                    other => panic!("expected a struct pattern, got {:?}", other),
                }
            }
            other => panic!("expected a match expression, got {:?}", other),
        }
    }

    #[test]
    fn parses_pattern_forms() {
        let source = r#"fn main() {
    match v {
        -1 => 0,
        1.5 => 1,
        "text" => 2,
        true => 3,
        (a, b) => 4,
        Shape::Circle(r) => 5,
        _ => 6,
    }
}"#;
        let program = parse_ok(source);
        match &main_tail(&program).kind {
            ExprKind::Match(match_expr) => {
                assert_eq!(match_expr.arms.len(), 7);
                match &match_expr.arms[0].pat.kind {
                    PatternKind::Lit(Lit::Int { value, raw }) => {
                        assert_eq!(*value, -1);
                        assert_eq!(raw, "-1");
                    }
                    other => panic!("expected a negative int literal, got {:?}", other),
                }
                match &match_expr.arms[1].pat.kind {
                    PatternKind::Lit(Lit::Float { value, .. }) => assert_eq!(*value, 1.5),
                    other => panic!("expected a float literal, got {:?}", other),
                }
                match &match_expr.arms[2].pat.kind {
                    PatternKind::Lit(Lit::Str(text)) => assert_eq!(text, "text"),
                    other => panic!("expected a string literal, got {:?}", other),
                }
                assert!(matches!(
                    &match_expr.arms[3].pat.kind,
                    PatternKind::Lit(Lit::Bool(true))
                ));
                match &match_expr.arms[4].pat.kind {
                    PatternKind::Tuple(elements) => assert_eq!(elements.len(), 2),
                    other => panic!("expected a tuple pattern, got {:?}", other),
                }
                assert!(matches!(&match_expr.arms[6].pat.kind, PatternKind::Wild));
            }
            other => panic!("expected a match expression, got {:?}", other),
        }
    }

    #[test]
    fn parses_char_literal_expressions() {
        match &parse_tail("'a'").kind {
            ExprKind::Lit(Lit::Char(value)) => assert_eq!(*value, 'a'),
            other => panic!("expected a char literal, got {:?}", other),
        }
        match &parse_tail(r#"'\n'"#).kind {
            ExprKind::Lit(Lit::Char(value)) => assert_eq!(*value, '\n'),
            other => panic!("expected an escaped char literal, got {:?}", other),
        }
        match &parse_tail(r#"'\u{1F600}'"#).kind {
            ExprKind::Lit(Lit::Char(value)) => assert_eq!(*value, '\u{1F600}'),
            other => panic!("expected a unicode char literal, got {:?}", other),
        }

        let source = r#"fn main() {
    let c: char = 'x';
    let order = 'a' < 'b';
    let code = 'a' as i64;
}"#;
        let program = parse_ok(source);
        let stmts = main_stmts(&program);
        match &let_init(&stmts[0]).kind {
            ExprKind::Lit(Lit::Char(value)) => assert_eq!(*value, 'x'),
            other => panic!("expected a char literal, got {:?}", other),
        }
        let (op, lhs, rhs) = as_binary(let_init(&stmts[1]));
        assert_eq!(*op, BinaryOp::Lt);
        assert!(matches!(&lhs.kind, ExprKind::Lit(Lit::Char('a'))));
        assert!(matches!(&rhs.kind, ExprKind::Lit(Lit::Char('b'))));
        match &let_init(&stmts[2]).kind {
            ExprKind::Cast { expr, ty } => {
                assert!(matches!(&expr.kind, ExprKind::Lit(Lit::Char('a'))));
                match ty {
                    TypeExpr::Path { name, .. } => assert_eq!(name, "i64"),
                    other => panic!("expected an i64 cast target, got {:?}", other),
                }
            }
            other => panic!("expected a cast, got {:?}", other),
        }

        // Character literals can be passed to macros and used in postfix forms.
        let source = r#"fn main() { println!("{}", 'x'); }"#;
        let program = parse_ok(source);
        match &stmt_expr(&main_stmts(&program)[0]).kind {
            ExprKind::Macro(call) => match &call.args[1].kind {
                ExprKind::Lit(Lit::Char(value)) => assert_eq!(*value, 'x'),
                other => panic!("expected a char macro argument, got {:?}", other),
            },
            other => panic!("expected a macro call, got {:?}", other),
        }
    }

    #[test]
    fn parses_char_literal_patterns() {
        let source = r#"fn main() {
    match c {
        'a' => 1,
        'b' | 'c' => 2,
        _ => 3,
    }
}"#;
        let program = parse_ok(source);
        match &main_tail(&program).kind {
            ExprKind::Match(match_expr) => {
                assert_eq!(match_expr.arms.len(), 3);
                assert!(matches!(
                    &match_expr.arms[0].pat.kind,
                    PatternKind::Lit(Lit::Char('a'))
                ));
                match &match_expr.arms[1].pat.kind {
                    PatternKind::Or(alternatives) => {
                        assert_eq!(alternatives.len(), 2);
                        assert!(matches!(
                            &alternatives[0].kind,
                            PatternKind::Lit(Lit::Char('b'))
                        ));
                        assert!(matches!(
                            &alternatives[1].kind,
                            PatternKind::Lit(Lit::Char('c'))
                        ));
                    }
                    other => panic!("expected an or pattern, got {:?}", other),
                }
                assert!(matches!(&match_expr.arms[2].pat.kind, PatternKind::Wild));
            }
            other => panic!("expected a match expression, got {:?}", other),
        }
    }

    #[test]
    fn parses_char_type_paths() {
        let source = r#"type CharAlias = char;
struct Tagged { ch: char, flags: Vec<char> }
enum Maybe { None, Some(char) }
fn identity(value: char) -> char { value }
const LETTER: char = 'a';
fn main() {
    let c: char = 'z';
    let maybe: Option<char> = None;
    let counts: HashMap<char, i64> = HashMap::new();
}"#;
        let program = parse_ok(source);
        let alias = program
            .items
            .iter()
            .find_map(|item| match item {
                Item::TypeAlias(alias) => Some(alias),
                _ => None,
            })
            .expect("expected a type alias");
        assert_eq!(alias.name, "CharAlias");
        match &alias.ty {
            TypeExpr::Path { name, .. } => assert_eq!(name, "char"),
            other => panic!("expected a char alias target, got {:?}", other),
        }

        let tagged = program
            .items
            .iter()
            .find_map(|item| match item {
                Item::Struct(decl) => Some(decl),
                _ => None,
            })
            .expect("expected a struct");
        match &tagged.fields[0].ty {
            TypeExpr::Path { name, .. } => assert_eq!(name, "char"),
            other => panic!("expected a char field, got {:?}", other),
        }
        match &tagged.fields[1].ty {
            TypeExpr::Path { name, generics, .. } => {
                assert_eq!(name, "Vec");
                assert_eq!(generics.len(), 1);
                match &generics[0] {
                    TypeExpr::Path { name, .. } => assert_eq!(name, "char"),
                    other => panic!("expected a char type argument, got {:?}", other),
                }
            }
            other => panic!("expected a Vec field, got {:?}", other),
        }

        let maybe = program
            .items
            .iter()
            .find_map(|item| match item {
                Item::Enum(decl) => Some(decl),
                _ => None,
            })
            .expect("expected an enum");
        match &maybe.variants[1].fields[0] {
            TypeExpr::Path { name, .. } => assert_eq!(name, "char"),
            other => panic!("expected a char enum payload, got {:?}", other),
        }

        let identity = fn_named(&program, "identity");
        match &identity.params[0].ty {
            TypeExpr::Path { name, .. } => assert_eq!(name, "char"),
            other => panic!("expected a char parameter, got {:?}", other),
        }
        match identity.ret.as_ref().expect("expected a return type") {
            TypeExpr::Path { name, .. } => assert_eq!(name, "char"),
            other => panic!("expected a char return type, got {:?}", other),
        }

        let letter = program
            .items
            .iter()
            .find_map(|item| match item {
                Item::Const(decl) => Some(decl),
                _ => None,
            })
            .expect("expected a const");
        match &letter.ty {
            TypeExpr::Path { name, .. } => assert_eq!(name, "char"),
            other => panic!("expected a char const type, got {:?}", other),
        }
        assert!(matches!(&letter.value.kind, ExprKind::Lit(Lit::Char('a'))));
    }

    #[test]
    fn parses_loops_and_for_ranges() {
        let source = r#"fn main() {
    loop { break; }
    while i < 10 { i = i + 1; }
    for i in 0..10 { print(i); }
    for i in 0..=10 { print(i); }
    for x in values { print(x); };
}"#;
        let program = parse_ok(source);
        let stmts = main_stmts(&program);
        assert_eq!(stmts.len(), 5);
        assert!(matches!(&stmt_expr(&stmts[0]).kind, ExprKind::Loop { .. }));
        assert!(matches!(&stmt_expr(&stmts[1]).kind, ExprKind::While { .. }));
        match &stmt_expr(&stmts[2]).kind {
            ExprKind::For { pat, iter, .. } => {
                assert!(matches!(&pat.kind, PatternKind::Binding { .. }));
                match &iter.kind {
                    ExprKind::Range {
                        start,
                        end,
                        inclusive,
                    } => {
                        assert!(!*inclusive);
                        assert_eq!(as_int(start), 0);
                        assert_eq!(as_int(end), 10);
                    }
                    other => panic!("expected a range, got {:?}", other),
                }
            }
            other => panic!("expected a for expression, got {:?}", other),
        }
        match &stmt_expr(&stmts[3]).kind {
            ExprKind::For { iter, .. } => match &iter.kind {
                ExprKind::Range { inclusive, .. } => assert!(*inclusive),
                other => panic!("expected a range, got {:?}", other),
            },
            other => panic!("expected a for expression, got {:?}", other),
        }
        match &stmt_expr(&stmts[4]).kind {
            ExprKind::For { iter, .. } => assert_eq!(as_path_name(iter), "values"),
            other => panic!("expected a for expression, got {:?}", other),
        }
    }

    #[test]
    fn for_range_binds_looser_than_addition() {
        let program = parse_ok("fn main() { for i in 0..n + 1 { print(i); }; }");
        let stmts = main_stmts(&program);
        match &stmt_expr(&stmts[0]).kind {
            ExprKind::For { iter, .. } => match &iter.kind {
                ExprKind::Range { end, .. } => {
                    let (op, lhs, rhs) = as_binary(end);
                    assert_eq!(*op, BinaryOp::Add);
                    assert_eq!(as_path_name(lhs), "n");
                    assert_eq!(as_int(rhs), 1);
                }
                other => panic!("expected a range, got {:?}", other),
            },
            other => panic!("expected a for expression, got {:?}", other),
        }
    }

    #[test]
    fn desugars_if_let_without_else() {
        let program = parse_ok("fn main() { if let Some(x) = opt { print(x); } }");
        match &main_tail(&program).kind {
            ExprKind::Match(match_expr) => {
                assert_eq!(match_expr.arms.len(), 2);
                match &match_expr.arms[0].pat.kind {
                    PatternKind::Path { args, .. } => assert_eq!(args.len(), 1),
                    other => panic!("expected a path pattern, got {:?}", other),
                }
                assert!(matches!(&match_expr.arms[1].pat.kind, PatternKind::Wild));
                match &match_expr.arms[0].body.kind {
                    ExprKind::Block(block) => assert_eq!(block.stmts.len(), 1),
                    other => panic!("expected a block, got {:?}", other),
                }
                match &match_expr.arms[1].body.kind {
                    ExprKind::Block(block) => {
                        assert!(block.stmts.is_empty());
                        assert!(block.tail.is_none());
                    }
                    other => panic!("expected an empty block, got {:?}", other),
                }
            }
            other => panic!("expected a match expression, got {:?}", other),
        }
    }

    #[test]
    fn desugars_if_let_with_else() {
        let program =
            parse_ok("fn main() { if let Some(x) = opt { print(x); } else { print(0); } }");
        match &main_tail(&program).kind {
            ExprKind::Match(match_expr) => {
                assert_eq!(match_expr.arms.len(), 2);
                match &match_expr.arms[1].body.kind {
                    ExprKind::Block(block) => assert_eq!(block.stmts.len(), 1),
                    other => panic!("expected a block, got {:?}", other),
                }
            }
            other => panic!("expected a match expression, got {:?}", other),
        }
    }

    #[test]
    fn desugars_while_let() {
        let program = parse_ok("fn main() { while let Some(x) = iter.next() { print(x); } }");
        match &main_tail(&program).kind {
            ExprKind::Loop { body } => match body.tail.as_deref().map(|expr| &expr.kind) {
                Some(ExprKind::Match(match_expr)) => {
                    assert_eq!(match_expr.arms.len(), 2);
                    match &match_expr.arms[0].pat.kind {
                        PatternKind::Path { args, .. } => assert_eq!(args.len(), 1),
                        other => panic!("expected a path pattern, got {:?}", other),
                    }
                    assert!(matches!(&match_expr.arms[1].pat.kind, PatternKind::Wild));
                    match &match_expr.arms[1].body.kind {
                        ExprKind::Break(value) => assert!(value.is_none()),
                        other => panic!("expected a break, got {:?}", other),
                    }
                }
                other => panic!("expected a tail match, got {:?}", other),
            },
            other => panic!("expected a loop, got {:?}", other),
        }
    }

    #[test]
    fn block_like_expressions_do_not_take_postfixes() {
        // A block-like expression in statement position ends the statement; a
        // following parenthesized expression is a new statement or tail, not a
        // call of the for/match expression (repro from tests/programs/tuples.ls).
        let program = parse_ok("fn f() { for v in xs { } (1, 2) }");
        let body = &fn_named(&program, "f").body;
        assert_eq!(body.stmts.len(), 1);
        match &body.stmts[0] {
            Stmt::Expr(expr) => assert!(matches!(&expr.kind, ExprKind::For { .. })),
            other => panic!("expected a for statement, got {:?}", other),
        }
        match &body.tail {
            Some(tail) => match &tail.kind {
                ExprKind::Tuple(elems) => {
                    assert_eq!(elems.len(), 2);
                    assert_eq!(as_int(&elems[0]), 1);
                    assert_eq!(as_int(&elems[1]), 2);
                }
                other => panic!("expected a tuple tail, got {:?}", other),
            },
            None => panic!("expected a tuple tail expression"),
        }

        let program = parse_ok("fn g() { match 1 { _ => 2 } (3, 4) }");
        let body = &fn_named(&program, "g").body;
        assert_eq!(body.stmts.len(), 1);
        match &body.stmts[0] {
            Stmt::Expr(expr) => assert!(matches!(&expr.kind, ExprKind::Match(_))),
            other => panic!("expected a match statement, got {:?}", other),
        }
        match &body.tail {
            Some(tail) => assert!(matches!(&tail.kind, ExprKind::Tuple(elems) if elems.len() == 2)),
            None => panic!("expected a tuple tail expression"),
        }

        // Parenthesizing the block-like expression keeps ordinary postfixes.
        let expr = parse_tail("(if true { 1 } else { 2 }).to_string()");
        match &expr.kind {
            ExprKind::MethodCall {
                receiver, method, ..
            } => {
                assert_eq!(method, "to_string");
                assert!(matches!(&receiver.kind, ExprKind::If(_)));
            }
            other => panic!("expected a method call, got {:?}", other),
        }
    }

    #[test]
    fn parses_block_expressions() {
        // Block expressions are primary expressions: they show up as match arm
        // bodies, as bare block statements and as closure bodies.
        let program = parse_ok("fn main() { match 1 { _ => {}, } }");
        match &main_tail(&program).kind {
            ExprKind::Match(match_expr) => {
                assert_eq!(match_expr.arms.len(), 1);
                assert!(matches!(&match_expr.arms[0].body.kind, ExprKind::Block(_)));
            }
            other => panic!("expected a match expression, got {:?}", other),
        }

        let program = parse_ok("fn main() { { let value = 1; } }");
        let body = &fn_named(&program, "main").body;
        assert!(body.stmts.is_empty());
        match &body.tail {
            Some(tail) => match &tail.kind {
                ExprKind::Block(block) => {
                    assert_eq!(block.stmts.len(), 1);
                    assert!(block.tail.is_none());
                }
                other => panic!("expected a block expression tail, got {:?}", other),
            },
            None => panic!("expected a block expression tail"),
        }

        let program = parse_ok("fn main() { let bump = || { let counter = 0; counter + 1 }; }");
        let stmts = main_stmts(&program);
        match &let_init(&stmts[0]).kind {
            ExprKind::Closure(closure) => match &closure.body.kind {
                ExprKind::Block(block) => {
                    assert_eq!(block.stmts.len(), 1);
                    assert!(block.tail.is_some());
                }
                other => panic!("expected a block closure body, got {:?}", other),
            },
            other => panic!("expected a closure, got {:?}", other),
        }

        // A block expression followed by '(' starts a new statement instead of
        // taking a call postfix.
        let program = parse_ok("fn h() { { 1; } (2, 3) }");
        let body = &fn_named(&program, "h").body;
        assert_eq!(body.stmts.len(), 1);
        assert!(
            matches!(&body.stmts[0], Stmt::Expr(expr) if matches!(&expr.kind, ExprKind::Block(_)))
        );
        match &body.tail {
            Some(tail) => assert!(matches!(&tail.kind, ExprKind::Tuple(elems) if elems.len() == 2)),
            None => panic!("expected a tuple tail expression"),
        }
    }

    #[test]
    fn parses_try_expressions() {
        // The question mark is a postfix operator: it binds tighter than the
        // binary operators, applies to calls and method calls, and nests.
        let program = parse_ok(
            "fn main() { let a = lookup(1)?; let b = values?.len(); let c = a??; let d = calc()? + 1; }",
        );
        let stmts = main_stmts(&program);

        match &let_init(&stmts[0]).kind {
            ExprKind::Try { expr } => assert!(matches!(&expr.kind, ExprKind::Call { .. })),
            other => panic!("expected a try expression, got {:?}", other),
        }

        match &let_init(&stmts[1]).kind {
            ExprKind::MethodCall {
                receiver, method, ..
            } => {
                assert_eq!(method, "len");
                assert!(matches!(&receiver.kind, ExprKind::Try { .. }));
            }
            other => panic!("expected a method call, got {:?}", other),
        }

        match &let_init(&stmts[2]).kind {
            ExprKind::Try { expr } => match &expr.kind {
                ExprKind::Try { expr } => assert!(matches!(&expr.kind, ExprKind::Path(_))),
                other => panic!("expected a nested try expression, got {:?}", other),
            },
            other => panic!("expected a try expression, got {:?}", other),
        }

        let (op, lhs, rhs) = as_binary(let_init(&stmts[3]));
        assert_eq!(*op, BinaryOp::Add);
        assert!(matches!(&lhs.kind, ExprKind::Try { .. }));
        assert_eq!(as_int(rhs), 1);
    }

    #[test]
    fn parses_closures() {
        let source = r#"fn main() {
    let f = |x: i64| x + 1;
    let g = move |x| x * 2;
    let h = || 42;
}"#;
        let program = parse_ok(source);
        let stmts = main_stmts(&program);
        assert_eq!(stmts.len(), 3);
        match &let_init(&stmts[0]).kind {
            ExprKind::Closure(closure) => {
                assert!(!closure.is_move);
                assert_eq!(closure.params.len(), 1);
                assert!(closure.params[0].ty.is_some());
                assert!(matches!(&closure.body.kind, ExprKind::Binary { .. }));
            }
            other => panic!("expected a closure, got {:?}", other),
        }
        match &let_init(&stmts[1]).kind {
            ExprKind::Closure(closure) => {
                assert!(closure.is_move);
                assert_eq!(closure.params.len(), 1);
                assert!(closure.params[0].ty.is_none());
            }
            other => panic!("expected a closure, got {:?}", other),
        }
        match &let_init(&stmts[2]).kind {
            ExprKind::Closure(closure) => {
                assert!(closure.params.is_empty());
                assert_eq!(as_int(&closure.body), 42);
            }
            other => panic!("expected a closure, got {:?}", other),
        }
    }

    #[test]
    fn parses_macros_and_vec_literals() {
        let source = r#"fn main() {
    println!("hello");
    println!();
    format!();
    assert!(true);
    assert_eq!(1, 2);
    panic!("boom");
    let a = vec![1, 2, 3];
    let b = vec![0; 5];
    let c = vec![];
}"#;
        let program = parse_ok(source);
        let stmts = main_stmts(&program);
        assert_eq!(stmts.len(), 9);
        match &stmt_expr(&stmts[0]).kind {
            ExprKind::Macro(call) => {
                assert_eq!(call.name, "println");
                assert_eq!(call.args.len(), 1);
            }
            other => panic!("expected a macro call, got {:?}", other),
        }
        assert!(
            matches!(&stmt_expr(&stmts[1]).kind, ExprKind::Macro(call) if call.args.is_empty())
        );
        assert!(
            matches!(&stmt_expr(&stmts[2]).kind, ExprKind::Macro(call) if call.name == "format")
        );
        assert!(
            matches!(&stmt_expr(&stmts[3]).kind, ExprKind::Macro(call) if call.name == "assert")
        );
        assert!(
            matches!(&stmt_expr(&stmts[4]).kind, ExprKind::Macro(call) if call.name == "assert_eq")
        );
        assert!(
            matches!(&stmt_expr(&stmts[5]).kind, ExprKind::Macro(call) if call.name == "panic")
        );
        match &let_init(&stmts[6]).kind {
            ExprKind::Vec(VecLit::List(elements)) => assert_eq!(elements.len(), 3),
            other => panic!("expected a vec list, got {:?}", other),
        }
        match &let_init(&stmts[7]).kind {
            ExprKind::Vec(VecLit::Repeat { value, count }) => {
                assert_eq!(as_int(value), 0);
                assert_eq!(as_int(count), 5);
            }
            other => panic!("expected a vec repeat, got {:?}", other),
        }
        match &let_init(&stmts[8]).kind {
            ExprKind::Vec(VecLit::List(elements)) => assert!(elements.is_empty()),
            other => panic!("expected an empty vec, got {:?}", other),
        }
    }

    #[test]
    fn parses_tuples_and_unit() {
        let expr = parse_tail("()");
        match &expr.kind {
            ExprKind::Tuple(elements) => assert!(elements.is_empty()),
            other => panic!("expected the unit literal, got {:?}", other),
        }
        let expr = parse_tail("(1,)");
        match &expr.kind {
            ExprKind::Tuple(elements) => {
                assert_eq!(elements.len(), 1);
                assert_eq!(as_int(&elements[0]), 1);
            }
            other => panic!("expected a one element tuple, got {:?}", other),
        }
        let expr = parse_tail("(1, 2)");
        match &expr.kind {
            ExprKind::Tuple(elements) => assert_eq!(elements.len(), 2),
            other => panic!("expected a tuple, got {:?}", other),
        }
    }

    #[test]
    fn parses_struct_literals() {
        let expr = parse_tail("Point { x: 1.0, y: 2.0 }");
        match &expr.kind {
            ExprKind::StructLit { path, fields } => {
                assert_eq!(path.display_name(), "Point");
                assert_eq!(fields.len(), 2);
                assert_eq!(fields[0].name, "x");
                assert_eq!(fields[1].name, "y");
                assert!(matches!(
                    &fields[0].value.kind,
                    ExprKind::Lit(Lit::Float { .. })
                ));
            }
            other => panic!("expected a struct literal, got {:?}", other),
        }
    }

    #[test]
    fn parses_self_struct_literals_and_associated_items() {
        // Self in expression position: Self { .. } is a struct literal and
        // Self::new(..) is an associated function path inside impls.
        let source = r#"struct Counter {
    value: i64,
}
impl Counter {
    fn new(value: i64) -> Self {
        Self { value: value }
    }
    fn bump(&mut self) -> Self {
        Self::new(self.value)
    }
}"#;
        let program = parse_ok(source);
        let implementation = first_impl_block(&program);
        let new_fn = implementation
            .fns
            .iter()
            .find(|function| function.name == "new")
            .expect("new method");
        match new_fn.body.tail.as_deref().map(|expr| &expr.kind) {
            Some(ExprKind::StructLit { path, fields }) => {
                assert_eq!(path.display_name(), "Self");
                assert_eq!(fields.len(), 1);
                assert_eq!(fields[0].name, "value");
            }
            other => panic!("expected a Self struct literal tail, got {:?}", other),
        }

        let bump = implementation
            .fns
            .iter()
            .find(|function| function.name == "bump")
            .expect("bump method");
        match bump.body.tail.as_deref().map(|expr| &expr.kind) {
            Some(ExprKind::Call { callee, args }) => {
                assert_eq!(as_path_name(callee), "Self::new");
                assert_eq!(args.len(), 1);
            }
            other => panic!("expected a Self::new call tail, got {:?}", other),
        }
    }

    #[test]
    fn parses_path_expressions() {
        let program = parse_ok("fn main() { Foo::BAR; Point::new(1.0, 2.0); }");
        let stmts = main_stmts(&program);
        assert_eq!(as_path_name(stmt_expr(&stmts[0])), "Foo::BAR");
        match &stmt_expr(&stmts[1]).kind {
            ExprKind::Call { callee, args } => {
                assert_eq!(as_path_name(callee), "Point::new");
                assert_eq!(args.len(), 2);
            }
            other => panic!("expected a call, got {:?}", other),
        }
    }

    #[test]
    fn parses_jump_expressions() {
        let program = parse_ok("fn main() { return 1; }");
        match &stmt_expr(&main_stmts(&program)[0]).kind {
            ExprKind::Return(value) => {
                assert_eq!(as_int(value.as_deref().expect("return value")), 1)
            }
            other => panic!("expected a return, got {:?}", other),
        }

        let source = r#"fn helper() -> i64 {
    loop {
        break 1;
        continue;
    }
}"#;
        let program = parse_ok(source);
        let helper = fn_named(&program, "helper");
        match helper.body.tail.as_deref().map(|expr| &expr.kind) {
            Some(ExprKind::Loop { body }) => {
                assert_eq!(body.stmts.len(), 2);
                match &stmt_expr(&body.stmts[0]).kind {
                    ExprKind::Break(value) => {
                        assert_eq!(as_int(value.as_deref().expect("break value")), 1)
                    }
                    other => panic!("expected a break, got {:?}", other),
                }
                assert!(matches!(
                    &stmt_expr(&body.stmts[1]).kind,
                    ExprKind::Continue
                ));
            }
            other => panic!("expected a loop tail, got {:?}", other),
        }
    }

    #[test]
    fn spans_cover_source() {
        let source = "fn main() { let x = 1; }";
        let program = parse_ok(source);
        let main = main_fn(&program);
        assert_eq!(
            &source[main.span.start as usize..main.span.end as usize],
            source
        );
        let block_text = &source[main.body.span.start as usize..main.body.span.end as usize];
        assert_eq!(block_text, "{ let x = 1; }");
        match &main.body.stmts[0] {
            Stmt::Let(statement) => {
                assert_eq!(
                    &source[statement.span.start as usize..statement.span.end as usize],
                    "let x = 1;"
                );
                let init = statement.init.as_ref().expect("init");
                assert_eq!(
                    &source[init.span.start as usize..init.span.end as usize],
                    "1"
                );
            }
            other => panic!("expected a let statement, got {:?}", other),
        }
    }

    #[test]
    fn reports_errors_and_recovers() {
        let source = "fn main( { let x = ; } struct Good { x: i64 }";
        let (program, diagnostics) = parse_source(source);
        assert!(diagnostics.has_errors());
        assert!(
            program
                .items
                .iter()
                .any(|item| matches!(item, Item::Struct(structure) if structure.name == "Good"))
        );

        let errors = parse_errors("fn main() { let x = 1 }");
        assert!(!errors.is_empty());
    }

    #[test]
    fn never_panics_on_garbage() {
        let sources = [
            "",
            "fn",
            "fn main",
            "fn main(",
            "fn main() {",
            "struct",
            "enum E {",
            "impl",
            "const",
            "type",
            "match",
            "if",
            "|",
            "(",
            ")",
            "}",
            "{",
            "vec![",
            "Point {",
            "let",
            "fn main() { let x = ; }",
            "fn main() { if }",
            "@#$%",
            "/* unterminated",
            r#""unterminated"#,
            "fn main() { match x { } }",
        ];
        for source in sources {
            let file = SourceFile::new("<test>", source);
            let (tokens, _) = crate::lexer::tokenize(source);
            let _ = parse(&tokens, &file);
        }
    }

    #[test]
    fn random_source_never_panics() {
        // Deterministic pseudo-random source soup: whatever the input, the
        // parser must terminate, report diagnostics and return a program.
        let alphabet: Vec<char> = vec![
            'a', 'b', 'X', 'Y', '_', '0', '1', '2', '9', ' ', '/', '*', '+', '-', '=', '!', '&',
            '|', '^', '~', '#', '{', '}', '[', ']', '(', ')', '.', ',', ';', ':', '<', '>', '%',
            '"', '?', '@',
        ];
        let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
        for _ in 0..300 {
            let length = 16 + (state % 64) as usize;
            let mut source = String::new();
            for _ in 0..length {
                state = state
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                let index = ((state >> 33) as usize) % alphabet.len();
                source.push(alphabet[index]);
            }
            let file = SourceFile::new("<random>", &source);
            let (tokens, _) = crate::lexer::tokenize(&source);
            let (program, _diagnostics) = parse(&tokens, &file);
            assert!(program.items.len() < 1024);
        }
    }

    #[test]
    fn handles_missing_eof_token() {
        let file = SourceFile::new("<test>", "fn");
        let tokens = vec![Token {
            kind: TokenKind::KwFn,
            span: Span::new(0, 2),
        }];
        let (program, diagnostics) = parse(&tokens, &file);
        assert!(diagnostics.has_errors());
        assert!(program.items.len() <= 1);
    }

    #[test]
    fn parses_shipped_examples() {
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let examples = manifest_dir.join("..").join("..").join("examples");
        let mut count = 0;
        for entry in std::fs::read_dir(&examples).expect("examples directory") {
            let path = entry.expect("directory entry").path();
            if path.extension().and_then(|extension| extension.to_str()) != Some("ls") {
                continue;
            }
            let source = std::fs::read_to_string(&path).expect("example source");
            let (program, diagnostics) = parse_source(&source);
            assert!(
                !diagnostics.has_errors(),
                "parse errors in {:?}: {:?}",
                path,
                messages(&diagnostics)
            );
            assert!(!program.items.is_empty(), "no items parsed from {:?}", path);
            count += 1;
        }
        assert!(
            count >= 7,
            "expected at least 7 example programs, found {}",
            count
        );
    }

    #[test]
    fn parses_the_shipped_prelude() {
        // The standard prelude ships as library source; it must always be
        // accepted by the parser. This guards against syntax regressions in
        // both the prelude and the parser.
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let prelude = manifest_dir.join("src").join("prelude.ls");
        let source = std::fs::read_to_string(&prelude)
            .unwrap_or_else(|error| panic!("failed to read {prelude:?}: {error}"));
        let (program, diagnostics) = parse_source(&source);
        assert!(
            diagnostics.is_empty(),
            "unexpected diagnostics in {prelude:?}: {:?}",
            messages(&diagnostics)
        );
        assert!(
            !program.items.is_empty(),
            "no items parsed from {prelude:?}"
        );
    }

    /// Run a parser test on a thread with a generous stack so that tests about
    /// deep nesting measure the parser's own nesting limit rather than the
    /// default test thread stack.
    fn with_big_stack<F: FnOnce() + Send + 'static>(f: F) {
        let handle = std::thread::Builder::new()
            .stack_size(16 * 1024 * 1024)
            .spawn(f)
            .expect("spawn parser thread");
        handle.join().expect("parser thread panicked");
    }

    #[test]
    fn deep_parentheses_do_not_overflow() {
        // 48 levels stay below MAX_NESTING_DEPTH, so this parses successfully.
        with_big_stack(|| {
            let mut source = String::from("fn main() { ");
            for _ in 0..48 {
                source.push('(');
            }
            source.push('1');
            for _ in 0..48 {
                source.push(')');
            }
            source.push_str(" }");
            let program = parse_ok(&source);
            assert_eq!(as_int(main_tail(&program)), 1);
        });
    }

    #[test]
    fn deep_nesting_is_reported_not_fatal() {
        // Pathological nesting must produce a diagnostic instead of aborting
        // the process with a stack overflow.
        with_big_stack(|| {
            let mut parens = String::from("fn main() { ");
            for _ in 0..5000 {
                parens.push('(');
            }
            parens.push('1');
            for _ in 0..5000 {
                parens.push(')');
            }
            parens.push_str(" }");
            let (program, diagnostics) = parse_source(&parens);
            // Error recovery from the nesting guard can still leave an AST
            // deeper than MAX_AST_DEPTH; such a tree is discarded instead of
            // returned, so that dropping it cannot overflow the stack.
            assert!(program.items.is_empty());
            assert!(diagnostics.has_errors());
            // Deeply nested input is rejected as a whole, so the recovery noise
            // is replaced by the single diagnostic that explains the rejection.
            assert_eq!(
                diagnostics.len(),
                1,
                "a depth-rejected file reports exactly one diagnostic, got {:?}",
                messages(&diagnostics)
            );
            assert_eq!(
                messages(&diagnostics)
                    .iter()
                    .filter(|message| message.contains("nested too deeply"))
                    .count(),
                1,
                "the nesting diagnostic should be reported exactly once"
            );

            let mut types = String::from("fn main() { let v: ");
            for _ in 0..5000 {
                types.push_str("Vec<");
            }
            types.push_str("i64");
            for _ in 0..5000 {
                types.push('>');
            }
            types.push_str(" = x; }");
            let (program, diagnostics) = parse_source(&types);
            // Error recovery from the nesting guard can still leave an AST
            // deeper than MAX_AST_DEPTH; such a tree is discarded instead of
            // returned, so that dropping it cannot overflow the stack.
            assert!(program.items.is_empty());
            assert!(diagnostics.has_errors());
            assert_eq!(
                diagnostics.len(),
                1,
                "a depth-rejected file reports exactly one diagnostic, got {:?}",
                messages(&diagnostics)
            );
            assert!(
                messages(&diagnostics)
                    .iter()
                    .any(|message| message.contains("nested too deeply"))
            );

            // Nested item bodies exercise the block nesting guard.
            let mut nested_items = String::from("fn main() {");
            for _ in 0..5000 {
                nested_items.push_str(" fn f() {");
            }
            for _ in 0..5000 {
                nested_items.push_str(" }");
            }
            nested_items.push_str(" }");
            let (program, diagnostics) = parse_source(&nested_items);
            // Error recovery from the nesting guard can still leave an AST
            // deeper than MAX_AST_DEPTH; such a tree is discarded instead of
            // returned, so that dropping it cannot overflow the stack.
            assert!(program.items.is_empty());
            assert!(diagnostics.has_errors());
            assert_eq!(
                diagnostics.len(),
                1,
                "a depth-rejected file reports exactly one diagnostic, got {:?}",
                messages(&diagnostics)
            );
            assert!(
                messages(&diagnostics)
                    .iter()
                    .any(|message| message.contains("nested too deeply"))
            );
        });
    }

    #[test]
    fn pathological_single_line_input_reports_one_small_diagnostic() {
        // 30 000 nested parentheses on one line used to make recovery emit tens
        // of thousands of diagnostics, each rendering the whole 60 KB source
        // line. That took tens of seconds and could exhaust the output pipe, so a
        // rejected file must report a single diagnostic whose rendered form stays
        // proportional to one source line.
        with_big_stack(|| {
            let mut source = String::from("fn main() { ");
            source.push_str(&"(".repeat(30_000));
            source.push('1');
            source.push_str(&")".repeat(30_000));
            source.push_str(" }");
            assert!(source.len() > 60_000);

            let file = SourceFile::new("<test>", &source);
            let (tokens, lexer_diagnostics) = crate::lexer::tokenize(&source);
            assert!(
                !lexer_diagnostics.has_errors(),
                "{:?}",
                messages(&lexer_diagnostics)
            );
            let (program, diagnostics) = parse(&tokens, &file);
            assert!(program.items.is_empty(), "the rejected tree is discarded");
            assert_eq!(
                diagnostics.len(),
                1,
                "a depth-rejected file reports exactly one diagnostic, got {:?}",
                messages(&diagnostics)
            );
            let diagnostic = &diagnostics.as_slice()[0];
            assert!(diagnostic.message.contains("nested too deeply"));
            assert_eq!(diagnostic.code, Some("E0001"));
            assert_eq!(diagnostic.severity, Severity::Error);
            let rendered = diagnostics.render(&file);
            assert!(
                rendered.len() < source.len() * 4,
                "rendered diagnostics must stay proportional to one source line, \
                 got {} bytes for a {} byte source",
                rendered.len(),
                source.len()
            );
        });
    }

    fn add_chain_source(terms: usize) -> String {
        let mut source = String::from("fn main() -> i64 { 1");
        for _ in 1..terms {
            source.push_str(" + 1");
        }
        source.push_str(" }");
        source
    }

    fn method_chain_source(calls: usize) -> String {
        let mut source = String::from("fn main() { let s = 1; let t = s");
        for _ in 0..calls {
            source.push_str(".clone()");
        }
        source.push_str("; }");
        source
    }

    fn assert_rejected_as_too_deep(source: &str) {
        let (program, diagnostics) = parse_source(source);
        assert_eq!(
            diagnostics.len(),
            1,
            "a depth-rejected file reports exactly one diagnostic, got {:?}",
            messages(&diagnostics)
        );
        let deep: Vec<_> = diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.message.contains("nested too deeply"))
            .collect();
        assert_eq!(
            deep.len(),
            1,
            "expected exactly one nesting diagnostic, got {:?}",
            messages(&diagnostics)
        );
        let diagnostic = deep[0];
        assert_eq!(diagnostic.severity, Severity::Error);
        assert_eq!(diagnostic.code, Some("E0001"));
        assert!(
            diagnostic.span.start < source.len() as u32,
            "diagnostic span start {} is outside the {} byte source",
            diagnostic.span.start,
            source.len()
        );
        assert!(
            diagnostic.span.end <= source.len() as u32,
            "diagnostic span end {} is outside the {} byte source",
            diagnostic.span.end,
            source.len()
        );
        assert!(diagnostic.span.start <= diagnostic.span.end);
        // A rejected tree is discarded instead of being returned and dropped,
        // because dropping a pathologically deep tree would overflow the stack.
        assert!(
            program.items.is_empty(),
            "a too-deep AST is discarded, so no items are returned"
        );
    }

    #[test]
    fn accepts_moderately_long_expression_chains() {
        // Folding long operator and method chains iteratively is legal; only the
        // structural depth of the finished AST is limited.
        parse_ok(&add_chain_source(100));
        parse_ok(&method_chain_source(100));
    }

    #[test]
    fn rejects_deep_binary_operator_chain() {
        assert_rejected_as_too_deep(&add_chain_source(400));
    }

    #[test]
    fn rejects_deep_method_call_chain() {
        assert_rejected_as_too_deep(&method_chain_source(2000));
    }

    #[test]
    fn rejects_very_deep_binary_operator_chain() {
        // A tree this deep would overflow the stack when the driver drops it if
        // the parser returned or dropped it normally.
        assert_rejected_as_too_deep(&add_chain_source(8000));
    }

    #[test]
    fn rejects_deep_unary_operator_chain() {
        let mut source = String::from("fn main() { let b: bool = ");
        for _ in 0..400 {
            source.push('!');
        }
        source.push_str("true; }");
        assert_rejected_as_too_deep(&source);
    }
}
