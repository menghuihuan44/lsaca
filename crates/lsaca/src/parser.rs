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
pub fn parse(tokens: &[Token], file: &SourceFile) -> (Program, Diagnostics) {
    let mut parser = Parser::new(tokens, file);
    let program = parser.parse_program();
    (program, parser.diagnostics)
}

/// Names that are sugar for Option and Result variants in patterns. A bare
/// occurrence of one of these names is a path pattern, not a binding, so that
/// 'match x { None => ... }' matches the option variant instead of binding it.
const SUGAR_VARIANT_NAMES: [&str; 4] = ["None", "Some", "Ok", "Err"];

/// Maximum recursion depth of the recursive descent parser. Deeper input is
/// rejected with a diagnostic instead of risking a stack overflow, so that
/// pathological source code cannot abort the compiler. A debug build needs
/// roughly 1.3 MB of stack for 64 expression nesting levels, which fits on the
/// 2 MiB stack of a test thread; no sane program comes close.
const MAX_NESTING_DEPTH: usize = 64;

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
            TokenKind::Eof => None,
            _ => {
                let found = self.peek_kind().describe();
                let span = self.span();
                self.error(
                    format!(
                        "expected an item (fn, struct, enum, impl, const or type), found {}",
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
        let mut fields = Vec::new();
        self.expect(TokenKind::LBrace, "'{'");
        loop {
            if self.at(TokenKind::RBrace) || self.at_eof() {
                break;
            }
            let before = self.pos;
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
        let mut variants = Vec::new();
        self.expect(TokenKind::LBrace, "'{'");
        loop {
            if self.at(TokenKind::RBrace) || self.at_eof() {
                break;
            }
            let before = self.pos;
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
            variants,
            span: start.join(end),
        }
    }

    fn parse_impl_block(&mut self) -> ImplBlock {
        let impl_token = self.expect(TokenKind::KwImpl, "impl");
        let start = impl_token
            .map(|token| token.span)
            .unwrap_or_else(|| self.span());
        let ty = self.parse_type();
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
            ty,
            ty_name,
            ty_span,
            fns,
            span: start.join(end),
        }
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
        self.expect(TokenKind::Eq, "'=' in type alias");
        let ty = self.parse_type();
        let end = self
            .expect(TokenKind::Semi, "';' after type alias")
            .map(|token| token.span)
            .unwrap_or_else(|| self.prev_span());
        TypeAliasDecl {
            name,
            name_span,
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
        if self.at(TokenKind::KwFn) {
            let fn_token = self.bump();
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
            let span = fn_token.span.join(ret.span());
            return TypeExpr::Fn {
                params,
                ret: Box::new(ret),
                span,
            };
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
            TokenKind::Minus => {
                self.bump();
                self.parse_negative_literal_pattern(start)
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
        // Iterative so that a long run of '-' or '!' cannot exhaust the stack.
        // Unary operators are right associative, so the operators are collected
        // first and folded from the innermost one outwards.
        let mut operators: Vec<(UnaryOp, Span)> = Vec::new();
        loop {
            let op = if self.at(TokenKind::Minus) {
                UnaryOp::Neg
            } else if self.at(TokenKind::Bang) {
                UnaryOp::Not
            } else {
                break;
            };
            let token = self.bump();
            operators.push((op, token.span));
        }
        let mut expr = self.parse_postfix();
        for (op, op_span) in operators.into_iter().rev() {
            let span = op_span.join(expr.span);
            expr = Expr::new(
                ExprKind::Unary {
                    op,
                    expr: Box::new(expr),
                },
                span,
            );
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
        // The check looks at the leading token, not at the finished node, so a
        // parenthesized form such as (if c { f } else { g }).call() still gets
        // its postfixes; only a block-like primary used directly is affected.
        let starts_block_like = matches!(
            self.peek_kind(),
            TokenKind::KwIf
                | TokenKind::KwMatch
                | TokenKind::KwLoop
                | TokenKind::KwWhile
                | TokenKind::KwFor
                | TokenKind::LBrace
        );
        let mut expr = self.parse_primary();
        if starts_block_like {
            return expr;
        }
        loop {
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
            assert!(!program.items.is_empty());
            assert!(diagnostics.has_errors());
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
            assert!(!program.items.is_empty());
            assert!(diagnostics.has_errors());
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
            assert!(!program.items.is_empty());
            assert!(diagnostics.has_errors());
            assert!(
                messages(&diagnostics)
                    .iter()
                    .any(|message| message.contains("nested too deeply"))
            );
        });
    }
}
