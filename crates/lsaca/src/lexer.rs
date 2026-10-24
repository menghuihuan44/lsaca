//! The lexer: source text to tokens.
//!
//! The lexer turns a UTF-8 source string into a flat token stream. It never
//! panics and never aborts: lexical errors are reported as diagnostics and the
//! lexer recovers, so a single run can report several independent problems. The
//! token stream always ends with TokenKind::Eof.

use crate::diag::{Diagnostic, Diagnostics};
use crate::span::Span;

/// A single token.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

/// Token kinds. Keywords are recognised by the lexer; identifiers are stored
/// as-is, without the surrounding whitespace.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Ident(String),
    Int(u128),
    Float(f64),
    Str(String),
    // Keywords
    KwFn,
    KwLet,
    KwMut,
    KwConst,
    KwStruct,
    KwEnum,
    KwImpl,
    KwType,
    KwIf,
    KwElse,
    KwMatch,
    KwWhile,
    KwLoop,
    KwFor,
    KwIn,
    KwReturn,
    KwBreak,
    KwContinue,
    KwTrue,
    KwFalse,
    KwAs,
    KwSelf,
    KwSelfTy,
    KwPub,
    KwMove,
    // Punctuation
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Semi,
    Colon,
    ColonColon,
    Arrow,
    FatArrow,
    Dot,
    DotDot,
    DotDotEq,
    Eq,
    EqEq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Amp,
    AmpAmp,
    Pipe,
    PipePipe,
    Caret,
    Bang,
    Underscore,
    /// Shift left: <<
    Shl,
    /// Shift right: >>
    Shr,
    /// Add-assign: +=
    PlusEq,
    /// Sub-assign: -=
    MinusEq,
    /// Mul-assign: *=
    StarEq,
    /// Div-assign: /=
    SlashEq,
    /// Rem-assign: %=
    PercentEq,
    /// Bit-and-assign: &=
    AmpEq,
    /// Bit-or-assign: |=
    PipeEq,
    /// Bit-xor-assign: ^=
    CaretEq,
    /// Shl-assign: <<=
    ShlEq,
    /// Shr-assign: >>=
    ShrEq,
    Eof,
}

impl TokenKind {
    /// Human readable description used by "expected X, found Y" diagnostics.
    pub fn describe(&self) -> String {
        match self {
            TokenKind::Ident(name) => format!("identifier {}", name),
            TokenKind::Int(value) => format!("integer literal {}", value),
            TokenKind::Float(value) => format!("float literal {}", value),
            TokenKind::Str(_) => "string literal".to_string(),
            TokenKind::Eof => "end of file".to_string(),
            other => format!("{}", other),
        }
    }
}

impl std::fmt::Display for TokenKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenKind::Ident(name) => write!(f, "{}", name),
            TokenKind::Int(value) => write!(f, "{}", value),
            TokenKind::Float(value) => write!(f, "{}", value),
            TokenKind::Str(_) => write!(f, "string literal"),
            TokenKind::KwFn => write!(f, "fn"),
            TokenKind::KwLet => write!(f, "let"),
            TokenKind::KwMut => write!(f, "mut"),
            TokenKind::KwConst => write!(f, "const"),
            TokenKind::KwStruct => write!(f, "struct"),
            TokenKind::KwEnum => write!(f, "enum"),
            TokenKind::KwImpl => write!(f, "impl"),
            TokenKind::KwType => write!(f, "type"),
            TokenKind::KwIf => write!(f, "if"),
            TokenKind::KwElse => write!(f, "else"),
            TokenKind::KwMatch => write!(f, "match"),
            TokenKind::KwWhile => write!(f, "while"),
            TokenKind::KwLoop => write!(f, "loop"),
            TokenKind::KwFor => write!(f, "for"),
            TokenKind::KwIn => write!(f, "in"),
            TokenKind::KwReturn => write!(f, "return"),
            TokenKind::KwBreak => write!(f, "break"),
            TokenKind::KwContinue => write!(f, "continue"),
            TokenKind::KwTrue => write!(f, "true"),
            TokenKind::KwFalse => write!(f, "false"),
            TokenKind::KwAs => write!(f, "as"),
            TokenKind::KwSelf => write!(f, "self"),
            TokenKind::KwSelfTy => write!(f, "Self"),
            TokenKind::KwPub => write!(f, "pub"),
            TokenKind::KwMove => write!(f, "move"),
            TokenKind::LParen => write!(f, "("),
            TokenKind::RParen => write!(f, ")"),
            TokenKind::LBrace => write!(f, "{{"),
            TokenKind::RBrace => write!(f, "}}"),
            TokenKind::LBracket => write!(f, "["),
            TokenKind::RBracket => write!(f, "]"),
            TokenKind::Comma => write!(f, ","),
            TokenKind::Semi => write!(f, ";"),
            TokenKind::Colon => write!(f, ":"),
            TokenKind::ColonColon => write!(f, "::"),
            TokenKind::Arrow => write!(f, "->"),
            TokenKind::FatArrow => write!(f, "=>"),
            TokenKind::Dot => write!(f, "."),
            TokenKind::DotDot => write!(f, ".."),
            TokenKind::DotDotEq => write!(f, "..="),
            TokenKind::Eq => write!(f, "="),
            TokenKind::EqEq => write!(f, "=="),
            TokenKind::Ne => write!(f, "!="),
            TokenKind::Lt => write!(f, "<"),
            TokenKind::Le => write!(f, "<="),
            TokenKind::Gt => write!(f, ">"),
            TokenKind::Ge => write!(f, ">="),
            TokenKind::Plus => write!(f, "+"),
            TokenKind::Minus => write!(f, "-"),
            TokenKind::Star => write!(f, "*"),
            TokenKind::Slash => write!(f, "/"),
            TokenKind::Percent => write!(f, "%"),
            TokenKind::Amp => write!(f, "&"),
            TokenKind::AmpAmp => write!(f, "&&"),
            TokenKind::Pipe => write!(f, "|"),
            TokenKind::PipePipe => write!(f, "||"),
            TokenKind::Caret => write!(f, "^"),
            TokenKind::Bang => write!(f, "!"),
            TokenKind::Underscore => write!(f, "_"),
            TokenKind::Shl => write!(f, "<<"),
            TokenKind::Shr => write!(f, ">>"),
            TokenKind::PlusEq => write!(f, "+="),
            TokenKind::MinusEq => write!(f, "-="),
            TokenKind::StarEq => write!(f, "*="),
            TokenKind::SlashEq => write!(f, "/="),
            TokenKind::PercentEq => write!(f, "%="),
            TokenKind::AmpEq => write!(f, "&="),
            TokenKind::PipeEq => write!(f, "|="),
            TokenKind::CaretEq => write!(f, "^="),
            TokenKind::ShlEq => write!(f, "<<="),
            TokenKind::ShrEq => write!(f, ">>="),
            TokenKind::Eof => write!(f, "end of file"),
        }
    }
}

// Diagnostic codes used by the lexer. Kept stable so tests and tooling can match
// on them (the numbering is lexer-local and does not follow rustc).
const E_UNEXPECTED_CHAR: &str = "E0001";
const E_INT_LITERAL: &str = "E0002";
const E_FLOAT_LITERAL: &str = "E0003";
const E_UNTERMINATED_STRING: &str = "E0004";
const E_ESCAPE: &str = "E0005";
const E_UNTERMINATED_COMMENT: &str = "E0006";

/// Tokenize a whole source file.
///
/// Lexical errors are accumulated in the returned diagnostics; the token stream
/// always ends with TokenKind::Eof so that the parser can recover.
pub fn tokenize(source: &str) -> (Vec<Token>, Diagnostics) {
    let mut lexer = Lexer::new(source);
    lexer.run();
    let Lexer {
        tokens,
        diagnostics,
        ..
    } = lexer;
    (tokens, diagnostics)
}

/// Internal lexer state. Kept private so the public surface stays just
/// Token, TokenKind and tokenize.
struct Lexer<'a> {
    source: &'a str,
    /// Byte offset of the next character to scan. Always on a char boundary.
    pos: usize,
    tokens: Vec<Token>,
    diagnostics: Diagnostics,
}

impl<'a> Lexer<'a> {
    fn new(source: &'a str) -> Lexer<'a> {
        Lexer {
            source,
            pos: 0,
            tokens: Vec::with_capacity(source.len() / 3 + 1),
            diagnostics: Diagnostics::new(),
        }
    }

    fn run(&mut self) {
        loop {
            self.skip_trivia();
            let start = self.pos;
            let ch = match self.peek_char() {
                Some(ch) => ch,
                None => {
                    self.push(TokenKind::Eof, start, start);
                    return;
                }
            };
            if is_ident_start(ch) {
                self.lex_identifier(start);
            } else if ch.is_ascii_digit() {
                self.lex_number(start);
            } else if ch == '"' {
                self.lex_string(start);
            } else {
                self.lex_punctuation(start);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Character and token helpers
    // -----------------------------------------------------------------------

    fn peek_char(&self) -> Option<char> {
        self.source
            .get(self.pos..)
            .and_then(|rest| rest.chars().next())
    }

    /// Look at the character offset bytes past the current position without
    /// consuming anything. Returns None at or beyond the end of the source.
    fn peek_char_at(&self, offset: usize) -> Option<char> {
        self.pos
            .checked_add(offset)
            .and_then(|pos| self.source.get(pos..))
            .and_then(|rest| rest.chars().next())
    }

    fn bump(&mut self) -> Option<char> {
        let ch = self.peek_char()?;
        self.pos += ch.len_utf8();
        Some(ch)
    }

    fn starts_with(&self, prefix: &str) -> bool {
        match self.source.get(self.pos..) {
            Some(rest) => rest.starts_with(prefix),
            None => false,
        }
    }

    /// Consume ch if it is the next character.
    fn eat(&mut self, ch: char) -> bool {
        if self.peek_char() == Some(ch) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn push(&mut self, kind: TokenKind, start: usize, end: usize) {
        self.tokens.push(Token {
            kind,
            span: Span::from_usize(start, end),
        });
    }

    fn report_error(
        &mut self,
        code: &'static str,
        message: impl Into<String>,
        start: usize,
        end: usize,
    ) {
        let end = end.max(start);
        self.diagnostics
            .push(Diagnostic::error(message, Span::from_usize(start, end)).with_code(code));
    }

    // -----------------------------------------------------------------------
    // Trivia: whitespace and comments
    // -----------------------------------------------------------------------

    fn skip_trivia(&mut self) {
        loop {
            match self.peek_char() {
                Some(ch) if ch.is_whitespace() || ch == '\u{feff}' => {
                    self.bump();
                }
                Some('/') if self.starts_with("//") => self.skip_line_comment(),
                Some('/') if self.starts_with("/*") => self.skip_block_comment(),
                _ => return,
            }
        }
    }

    fn skip_line_comment(&mut self) {
        // Skip the two slashes, then up to (but not including) the newline.
        self.pos += 2;
        while let Some(ch) = self.peek_char() {
            if ch == '\n' {
                return;
            }
            self.bump();
        }
    }

    fn skip_block_comment(&mut self) {
        let start = self.pos;
        self.pos += 2;
        let mut depth: usize = 1;
        while depth > 0 {
            if self.starts_with("/*") {
                depth += 1;
                self.pos += 2;
            } else if self.starts_with("*/") {
                depth -= 1;
                self.pos += 2;
            } else if self.bump().is_none() {
                let end = self.pos;
                self.report_error(
                    E_UNTERMINATED_COMMENT,
                    "unterminated block comment",
                    start,
                    end,
                );
                return;
            }
        }
    }

    // -----------------------------------------------------------------------
    // Identifiers and keywords
    // -----------------------------------------------------------------------

    fn lex_identifier(&mut self, start: usize) {
        // The first character is known to be an identifier start.
        self.bump();
        while let Some(ch) = self.peek_char() {
            if is_ident_continue(ch) {
                self.bump();
            } else {
                break;
            }
        }
        let end = self.pos;
        let text = self.source.get(start..end).unwrap_or("");
        let kind = match text {
            "fn" => TokenKind::KwFn,
            "let" => TokenKind::KwLet,
            "mut" => TokenKind::KwMut,
            "const" => TokenKind::KwConst,
            "struct" => TokenKind::KwStruct,
            "enum" => TokenKind::KwEnum,
            "impl" => TokenKind::KwImpl,
            "type" => TokenKind::KwType,
            "if" => TokenKind::KwIf,
            "else" => TokenKind::KwElse,
            "match" => TokenKind::KwMatch,
            "while" => TokenKind::KwWhile,
            "loop" => TokenKind::KwLoop,
            "for" => TokenKind::KwFor,
            "in" => TokenKind::KwIn,
            "return" => TokenKind::KwReturn,
            "break" => TokenKind::KwBreak,
            "continue" => TokenKind::KwContinue,
            "true" => TokenKind::KwTrue,
            "false" => TokenKind::KwFalse,
            "as" => TokenKind::KwAs,
            "self" => TokenKind::KwSelf,
            "Self" => TokenKind::KwSelfTy,
            "pub" => TokenKind::KwPub,
            "move" => TokenKind::KwMove,
            "_" => TokenKind::Underscore,
            _ => TokenKind::Ident(text.to_string()),
        };
        self.push(kind, start, end);
    }

    // -----------------------------------------------------------------------
    // Numeric literals
    // -----------------------------------------------------------------------

    fn lex_number(&mut self, start: usize) {
        if self.starts_with("0x") || self.starts_with("0X") {
            self.pos += 2;
            self.lex_radix_integer(start, 16, "hexadecimal");
        } else if self.starts_with("0o") || self.starts_with("0O") {
            self.pos += 2;
            self.lex_radix_integer(start, 8, "octal");
        } else if self.starts_with("0b") || self.starts_with("0B") {
            self.pos += 2;
            self.lex_radix_integer(start, 2, "binary");
        } else {
            self.lex_decimal(start);
        }
    }

    fn lex_radix_integer(&mut self, start: usize, radix: u32, name: &str) {
        let mut digits: usize = 0;
        let mut value: u128 = 0;
        let mut overflow = false;
        while let Some(ch) = self.peek_char() {
            if ch == '_' {
                self.bump();
                continue;
            }
            let digit = match ch.to_digit(radix) {
                Some(digit) => digit,
                None => break,
            };
            if !overflow {
                match value
                    .checked_mul(u128::from(radix))
                    .and_then(|v| v.checked_add(u128::from(digit)))
                {
                    Some(next) => value = next,
                    None => overflow = true,
                }
            }
            digits += 1;
            self.bump();
        }
        let end = self.pos;
        if digits == 0 {
            self.report_error(
                E_INT_LITERAL,
                format!("{} integer literal has no digits", name),
                start,
                end,
            );
            return;
        }
        // Anything alphanumeric directly after the digits is a bad digit or an
        // unsupported type suffix: consume it and report one clear error.
        let bad_start = self.pos;
        while let Some(ch) = self.peek_char() {
            if ch.is_ascii_alphanumeric() {
                self.bump();
            } else {
                break;
            }
        }
        if self.pos > bad_start {
            self.report_error(
                E_INT_LITERAL,
                format!("invalid digit in {} integer literal", name),
                start,
                self.pos,
            );
            return;
        }
        if overflow {
            self.report_error(
                E_INT_LITERAL,
                "integer literal is too large for any supported integer type",
                start,
                end,
            );
            return;
        }
        self.push(TokenKind::Int(value), start, end);
    }

    fn lex_decimal(&mut self, start: usize) {
        // Integer part. The caller guarantees the first character is a digit.
        let mut integer_digits: usize = 0;
        loop {
            match self.peek_char() {
                Some(ch) if ch.is_ascii_digit() => {
                    self.bump();
                    integer_digits += 1;
                }
                Some('_') => {
                    self.bump();
                }
                _ => break,
            }
        }
        if integer_digits == 0 {
            // Unreachable in practice (lex_decimal is only called on a digit),
            // but keep the recovery path so the lexer can never loop.
            self.bump();
            return;
        }

        let mut is_float = false;
        // A number directly after a dot is a tuple-field index such as t.0,
        // never a float: this keeps t.0.1 lexing as t . 0 . 1.
        let after_dot = matches!(
            self.tokens.last(),
            Some(token) if token.kind == TokenKind::Dot
        );
        // Fractional part: only when the dot is followed by a digit, so that
        // 1..2 lexes as 1 .. 2 and 1.max(2) stays a method call.
        if !after_dot
            && self.peek_char() == Some('.')
            && let Some(next) = self.peek_char_at(1)
            && next.is_ascii_digit()
        {
            is_float = true;
            self.bump(); // .
            loop {
                match self.peek_char() {
                    Some(ch) if ch.is_ascii_digit() => {
                        self.bump();
                    }
                    Some('_') => {
                        self.bump();
                    }
                    _ => break,
                }
            }
        }

        // Exponent: e/E with an optional sign and at least one digit.
        if matches!(self.peek_char(), Some('e') | Some('E')) {
            let exp_start = self.pos;
            self.bump();
            if matches!(self.peek_char(), Some('+') | Some('-')) {
                self.bump();
            }
            let mut exp_digits: usize = 0;
            loop {
                match self.peek_char() {
                    Some(ch) if ch.is_ascii_digit() => {
                        self.bump();
                        exp_digits += 1;
                    }
                    Some('_') if exp_digits > 0 => {
                        self.bump();
                    }
                    _ => break,
                }
            }
            if exp_digits == 0 {
                let end = self.pos;
                self.report_error(
                    E_FLOAT_LITERAL,
                    "invalid float literal: expected at least one digit in the exponent",
                    exp_start,
                    end,
                );
                return;
            }
            is_float = true;
        }

        let end = self.pos;
        // Reject unsupported type suffixes (the language has none) with a clear
        // message instead of producing a confusing parser error later.
        if let Some(ch) = self.peek_char()
            && is_ident_start(ch)
        {
            let suffix_start = self.pos;
            while let Some(inner) = self.peek_char() {
                if is_ident_continue(inner) {
                    self.bump();
                } else {
                    break;
                }
            }
            let suffix = self.source.get(suffix_start..self.pos).unwrap_or("");
            let message = format!("invalid suffix {} on numeric literal", suffix);
            let code = if is_float {
                E_FLOAT_LITERAL
            } else {
                E_INT_LITERAL
            };
            self.report_error(code, message, start, self.pos);
            return;
        }

        let raw: String = self
            .source
            .get(start..end)
            .unwrap_or("")
            .chars()
            .filter(|ch| *ch != '_')
            .collect();

        if is_float {
            match raw.parse::<f64>() {
                Ok(value) if value.is_finite() => {
                    self.push(TokenKind::Float(value), start, end);
                }
                Ok(_) => {
                    self.report_error(
                        E_FLOAT_LITERAL,
                        format!("float literal {} is out of range", raw),
                        start,
                        end,
                    );
                }
                Err(_) => {
                    self.report_error(
                        E_FLOAT_LITERAL,
                        format!("invalid float literal {}", raw),
                        start,
                        end,
                    );
                }
            }
            return;
        }

        let mut value: u128 = 0;
        let mut overflow = false;
        for ch in raw.chars() {
            let digit = match ch.to_digit(10) {
                Some(digit) => digit,
                None => continue,
            };
            if !overflow {
                match value
                    .checked_mul(10)
                    .and_then(|v| v.checked_add(u128::from(digit)))
                {
                    Some(next) => value = next,
                    None => overflow = true,
                }
            }
        }
        if overflow {
            self.report_error(
                E_INT_LITERAL,
                "integer literal is too large for any supported integer type",
                start,
                end,
            );
            return;
        }
        self.push(TokenKind::Int(value), start, end);
    }

    // -----------------------------------------------------------------------
    // String literals
    // -----------------------------------------------------------------------

    fn lex_string(&mut self, start: usize) {
        self.bump(); // opening quote
        let mut value = String::new();
        let mut terminated = false;
        while let Some(ch) = self.peek_char() {
            match ch {
                '"' => {
                    self.bump();
                    terminated = true;
                    break;
                }
                '\\' => {
                    self.bump();
                    self.lex_escape(&mut value);
                }
                _ => {
                    value.push(ch);
                    self.bump();
                }
            }
        }
        let end = self.pos;
        if !terminated {
            self.report_error(
                E_UNTERMINATED_STRING,
                "unterminated string literal",
                start,
                end,
            );
        }
        // Emit the recovered literal even when unterminated so the parser sees a
        // string token and can keep going.
        self.push(TokenKind::Str(value), start, end);
    }

    /// Consume one escape sequence. The leading backslash has already been
    /// consumed; out receives the decoded character when the escape is valid.
    fn lex_escape(&mut self, out: &mut String) {
        let esc_start = self.pos.saturating_sub(1);
        let ch = match self.bump() {
            Some(ch) => ch,
            None => return,
        };
        match ch {
            'n' => out.push('\n'),
            't' => out.push('\t'),
            'r' => out.push('\r'),
            '\\' => out.push('\\'),
            '"' => out.push('"'),
            '\'' => out.push('\''),
            '0' => out.push('\0'),
            'x' => self.lex_hex_escape(esc_start, out),
            'u' => self.lex_unicode_escape(esc_start, out),
            _ => {
                self.report_error(
                    E_ESCAPE,
                    format!("unknown escape sequence \\{}", ch),
                    esc_start,
                    self.pos,
                );
                out.push(ch);
            }
        }
    }

    fn lex_hex_escape(&mut self, start: usize, out: &mut String) {
        let mut value: u32 = 0;
        let mut digits: usize = 0;
        while digits < 2 {
            match self.peek_char().and_then(|ch| ch.to_digit(16)) {
                Some(digit) => {
                    value = value * 16 + digit;
                    digits += 1;
                    self.bump();
                }
                None => break,
            }
        }
        if digits != 2 {
            self.report_error(
                E_ESCAPE,
                "invalid \\x escape: expected two hexadecimal digits",
                start,
                self.pos,
            );
            return;
        }
        if value > 0x7f {
            self.report_error(
                E_ESCAPE,
                format!(
                    "invalid byte escape \\x{:02X}: only ASCII bytes (0x00-0x7F) are allowed",
                    value
                ),
                start,
                self.pos,
            );
            return;
        }
        if let Some(decoded) = char::from_u32(value) {
            out.push(decoded);
        }
    }

    fn lex_unicode_escape(&mut self, start: usize, out: &mut String) {
        if !self.eat('{') {
            self.report_error(
                E_ESCAPE,
                "invalid \\u escape: expected { after \\u",
                start,
                self.pos,
            );
            return;
        }
        let mut value: u32 = 0;
        let mut digits: usize = 0;
        loop {
            match self.peek_char() {
                Some('}') => {
                    self.bump();
                    break;
                }
                Some('_') => {
                    self.bump();
                }
                Some(ch) => {
                    let digit = match ch.to_digit(16) {
                        Some(digit) => digit,
                        None => {
                            self.report_error(
                                E_ESCAPE,
                                format!("invalid character {} in unicode escape", ch),
                                start,
                                self.pos,
                            );
                            return;
                        }
                    };
                    if digits >= 6 {
                        self.report_error(
                            E_ESCAPE,
                            "invalid unicode escape: at most six hexadecimal digits are allowed",
                            start,
                            self.pos,
                        );
                        // Recover by skipping the rest of the escape.
                        while let Some(inner) = self.peek_char() {
                            self.bump();
                            if inner == '}' {
                                break;
                            }
                        }
                        return;
                    }
                    value = value * 16 + digit;
                    digits += 1;
                    self.bump();
                }
                None => {
                    self.report_error(E_ESCAPE, "unterminated unicode escape", start, self.pos);
                    return;
                }
            }
        }
        if digits == 0 {
            self.report_error(
                E_ESCAPE,
                "invalid unicode escape: expected at least one hexadecimal digit",
                start,
                self.pos,
            );
            return;
        }
        match char::from_u32(value) {
            Some(decoded) => out.push(decoded),
            None => {
                self.report_error(
                    E_ESCAPE,
                    format!("invalid unicode scalar value {:X}", value),
                    start,
                    self.pos,
                );
            }
        }
    }

    // -----------------------------------------------------------------------
    // Punctuation
    // -----------------------------------------------------------------------

    fn lex_punctuation(&mut self, start: usize) {
        use TokenKind::*;
        let ch = match self.bump() {
            Some(ch) => ch,
            None => return,
        };
        let kind = match ch {
            '(' => LParen,
            ')' => RParen,
            '{' => LBrace,
            '}' => RBrace,
            '[' => LBracket,
            ']' => RBracket,
            ',' => Comma,
            ';' => Semi,
            ':' => {
                if self.eat(':') {
                    ColonColon
                } else {
                    Colon
                }
            }
            '=' => {
                if self.eat('=') {
                    EqEq
                } else if self.eat('>') {
                    FatArrow
                } else {
                    Eq
                }
            }
            '-' => {
                if self.eat('>') {
                    Arrow
                } else if self.eat('=') {
                    MinusEq
                } else {
                    Minus
                }
            }
            '+' => {
                if self.eat('=') {
                    PlusEq
                } else {
                    Plus
                }
            }
            '*' => {
                if self.eat('=') {
                    StarEq
                } else {
                    Star
                }
            }
            '/' => {
                if self.eat('=') {
                    SlashEq
                } else {
                    Slash
                }
            }
            '%' => {
                if self.eat('=') {
                    PercentEq
                } else {
                    Percent
                }
            }
            '!' => {
                if self.eat('=') {
                    Ne
                } else {
                    Bang
                }
            }
            '<' => {
                if self.eat('<') {
                    if self.eat('=') { ShlEq } else { Shl }
                } else if self.eat('=') {
                    Le
                } else {
                    Lt
                }
            }
            '>' => {
                if self.eat('>') {
                    if self.eat('=') { ShrEq } else { Shr }
                } else if self.eat('=') {
                    Ge
                } else {
                    Gt
                }
            }
            '&' => {
                if self.eat('&') {
                    AmpAmp
                } else if self.eat('=') {
                    AmpEq
                } else {
                    Amp
                }
            }
            '|' => {
                if self.eat('|') {
                    PipePipe
                } else if self.eat('=') {
                    PipeEq
                } else {
                    Pipe
                }
            }
            '^' => {
                if self.eat('=') {
                    CaretEq
                } else {
                    Caret
                }
            }
            '.' => {
                if self.eat('.') {
                    if self.eat('=') { DotDotEq } else { DotDot }
                } else {
                    Dot
                }
            }
            _ => {
                let end = self.pos;
                let message = if ch.is_control() {
                    format!("unexpected control character U+{:04X}", ch as u32)
                } else {
                    format!("unexpected character {}", ch)
                };
                self.report_error(E_UNEXPECTED_CHAR, message, start, end);
                return;
            }
        };
        self.push(kind, start, self.pos);
    }
}

fn is_ident_start(ch: char) -> bool {
    ch == '_' || ch.is_alphabetic()
}

fn is_ident_continue(ch: char) -> bool {
    ch == '_' || ch.is_alphanumeric()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(source: &str) -> Vec<TokenKind> {
        tokenize(source)
            .0
            .into_iter()
            .map(|token| token.kind)
            .collect()
    }

    fn errors(source: &str) -> Vec<Diagnostic> {
        tokenize(source).1.into_vec()
    }

    fn error_messages(source: &str) -> Vec<String> {
        errors(source)
            .into_iter()
            .filter(|diagnostic| diagnostic.severity == crate::diag::Severity::Error)
            .map(|diagnostic| diagnostic.message)
            .collect()
    }

    fn ints(source: &str) -> Vec<u128> {
        tokenize(source)
            .0
            .into_iter()
            .filter_map(|token| match token.kind {
                TokenKind::Int(value) => Some(value),
                _ => None,
            })
            .collect()
    }

    fn floats(source: &str) -> Vec<f64> {
        tokenize(source)
            .0
            .into_iter()
            .filter_map(|token| match token.kind {
                TokenKind::Float(value) => Some(value),
                _ => None,
            })
            .collect()
    }

    fn strings(source: &str) -> Vec<String> {
        tokenize(source)
            .0
            .into_iter()
            .filter_map(|token| match token.kind {
                TokenKind::Str(value) => Some(value),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn empty_source_yields_eof() {
        let (tokens, diagnostics) = tokenize("");
        assert!(diagnostics.is_empty());
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, TokenKind::Eof);
        assert_eq!(tokens[0].span, Span::new(0, 0));
    }

    #[test]
    fn whitespace_and_comments_only() {
        let source = "  \n\t\r\n // a comment\n /* block /* nested */ still */ \n";
        let (tokens, diagnostics) = tokenize(source);
        assert!(diagnostics.is_empty(), "{:?}", diagnostics.into_vec());
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, TokenKind::Eof);
        assert_eq!(tokens[0].span.start as usize, source.len());
    }

    #[test]
    fn identifiers_and_keywords() {
        let source = "fn let mut const struct enum impl type if else match while loop for in \
                      return break continue true false as self Self pub move";
        let expected = vec![
            TokenKind::KwFn,
            TokenKind::KwLet,
            TokenKind::KwMut,
            TokenKind::KwConst,
            TokenKind::KwStruct,
            TokenKind::KwEnum,
            TokenKind::KwImpl,
            TokenKind::KwType,
            TokenKind::KwIf,
            TokenKind::KwElse,
            TokenKind::KwMatch,
            TokenKind::KwWhile,
            TokenKind::KwLoop,
            TokenKind::KwFor,
            TokenKind::KwIn,
            TokenKind::KwReturn,
            TokenKind::KwBreak,
            TokenKind::KwContinue,
            TokenKind::KwTrue,
            TokenKind::KwFalse,
            TokenKind::KwAs,
            TokenKind::KwSelf,
            TokenKind::KwSelfTy,
            TokenKind::KwPub,
            TokenKind::KwMove,
            TokenKind::Eof,
        ];
        assert_eq!(kinds(source), expected);
    }

    #[test]
    fn identifiers_are_not_keywords() {
        let source = "fnord letter mutable consts selfy SELF x1 _1 __ _";
        assert_eq!(
            kinds(source),
            vec![
                TokenKind::Ident("fnord".to_string()),
                TokenKind::Ident("letter".to_string()),
                TokenKind::Ident("mutable".to_string()),
                TokenKind::Ident("consts".to_string()),
                TokenKind::Ident("selfy".to_string()),
                TokenKind::Ident("SELF".to_string()),
                TokenKind::Ident("x1".to_string()),
                TokenKind::Ident("_1".to_string()),
                TokenKind::Ident("__".to_string()),
                TokenKind::Underscore,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn unicode_identifier() {
        let source = "café = 1;";
        assert_eq!(
            kinds(source),
            vec![
                TokenKind::Ident("café".to_string()),
                TokenKind::Eq,
                TokenKind::Int(1),
                TokenKind::Semi,
                TokenKind::Eof,
            ]
        );
        assert!(errors(source).is_empty());
    }

    #[test]
    fn punctuation_and_operators() {
        use TokenKind::*;
        let source = "( ) { } [ ] , ; : :: -> => . .. ..= = == != <= >= && || + - * / % & | ^ ! \
                      += -= *= /= %= &= |= ^= << >> <<= >>=";
        assert_eq!(
            kinds(source),
            vec![
                LParen, RParen, LBrace, RBrace, LBracket, RBracket, Comma, Semi, Colon, ColonColon,
                Arrow, FatArrow, Dot, DotDot, DotDotEq, Eq, EqEq, Ne, Le, Ge, AmpAmp, PipePipe,
                Plus, Minus, Star, Slash, Percent, Amp, Pipe, Caret, Bang, PlusEq, MinusEq, StarEq,
                SlashEq, PercentEq, AmpEq, PipeEq, CaretEq, Shl, Shr, ShlEq, ShrEq, Eof,
            ]
        );
    }

    #[test]
    fn juxtaposed_punctuation_is_split_correctly() {
        use TokenKind::*;
        let source = "a<b>>c<<=d>>=e..=f..g";
        assert_eq!(
            kinds(source),
            vec![
                Ident("a".to_string()),
                Lt,
                Ident("b".to_string()),
                Shr,
                Ident("c".to_string()),
                ShlEq,
                Ident("d".to_string()),
                ShrEq,
                Ident("e".to_string()),
                DotDotEq,
                Ident("f".to_string()),
                DotDot,
                Ident("g".to_string()),
                Eof,
            ]
        );
    }

    #[test]
    fn integer_literals() {
        assert_eq!(ints("0"), vec![0]);
        assert_eq!(ints("123"), vec![123]);
        assert_eq!(ints("1_000_000"), vec![1_000_000]);
        assert_eq!(ints("0xFF"), vec![255]);
        assert_eq!(ints("0xff"), vec![255]);
        assert_eq!(ints("0xDEAD_BEEF"), vec![0xDEAD_BEEF]);
        assert_eq!(ints("0o77"), vec![63]);
        assert_eq!(ints("0b1010"), vec![10]);
        assert_eq!(ints("0b10_10"), vec![10]);
        assert!(errors("0 123 1_000_000 0xFF 0o77 0b1010").is_empty());
    }

    #[test]
    fn integer_literal_boundaries() {
        // u128::MAX still lexes (the type checker rejects it for i64 if needed).
        let source = "340282366920938463463374607431768211455";
        assert_eq!(ints(source), vec![u128::MAX]);
        assert!(errors(source).is_empty());
        // One more does not fit u128.
        let too_big = "340282366920938463463374607431768211456";
        assert!(ints(too_big).is_empty());
        assert_eq!(error_messages(too_big).len(), 1);
        assert!(error_messages(too_big)[0].contains("too large"));
    }

    #[test]
    fn invalid_integer_literals() {
        for source in ["0x", "0o", "0b", "0xZZ", "0b102", "0o8"] {
            let messages = error_messages(source);
            assert!(!messages.is_empty(), "expected an error for {:?}", source);
        }
        // A suffix is not supported: report it instead of lexing an ident.
        let messages = error_messages("123i64");
        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("suffix"));
    }

    #[test]
    fn float_literals() {
        assert_eq!(floats("1.5"), vec![1.5]);
        assert_eq!(floats("2.0"), vec![2.0]);
        assert_eq!(floats("1e10"), vec![1e10]);
        assert!((floats("1.5e-3")[0] - 0.0015).abs() < f64::EPSILON);
        assert!((floats("2.0E+4")[0] - 2.0e4).abs() < f64::EPSILON);
        assert!((floats("1_000.5")[0] - 1000.5).abs() < f64::EPSILON);
        assert!(errors("1.5 2.0 1e10 1.5e-3 2.0E+4").is_empty());
    }

    #[test]
    fn dot_after_integer_is_not_a_float() {
        use TokenKind::*;
        assert_eq!(kinds("1..2"), vec![Int(1), DotDot, Int(2), Eof]);
        assert_eq!(
            kinds("1.max(2)"),
            vec![
                Int(1),
                Dot,
                Ident("max".to_string()),
                LParen,
                Int(2),
                RParen,
                Eof
            ]
        );
        assert!(errors("1..2").is_empty());
    }

    #[test]
    fn invalid_float_literals() {
        for source in ["1e", "1e_", "1.5e", "1.5e+", "1e400"] {
            let messages = error_messages(source);
            assert!(!messages.is_empty(), "expected an error for {:?}", source);
        }
        let messages = error_messages("1.5f32");
        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("suffix"));
    }

    #[test]
    fn string_literals_and_escapes() {
        assert_eq!(strings(r#""hello""#), vec!["hello".to_string()]);
        assert_eq!(
            strings(r#""a\nb\tc\rd\\e\"f\0g""#),
            vec!["a\nb\tc\rd\\e\"f\0g".to_string()]
        );
        assert_eq!(strings(r#""\x41\x7f""#), vec!["A\u{7f}".to_string()]);
        assert_eq!(strings(r#""\u{1F600}""#), vec!["\u{1F600}".to_string()]);
        assert_eq!(strings(r#""\u{48}\u{49}""#), vec!["HI".to_string()]);
        // Non-ASCII text passes through unchanged.
        assert_eq!(
            strings("\"héllo wörld €\""),
            vec!["héllo wörld €".to_string()]
        );
        assert!(errors(r#""\n\t\r\\\"\0\x41\u{1F600}""#).is_empty());
    }

    #[test]
    fn string_escape_errors() {
        assert!(error_messages(r#""\q""#)[0].contains("escape"));
        assert!(error_messages(r#""\x4""#)[0].contains("hexadecimal"));
        assert!(error_messages(r#""\xFF""#)[0].contains("ASCII"));
        assert!(error_messages(r#""\u{}""#)[0].contains("hexadecimal"));
        assert!(error_messages(r#""\u{ZZ}""#)[0].contains("character"));
        assert!(error_messages(r#""\u{110000}""#)[0].contains("unicode"));
        assert!(error_messages(r#""\u1234""#)[0].contains("{"));
    }

    #[test]
    fn unterminated_string_is_recovered() {
        let (tokens, diagnostics) = tokenize(r#"let s = "abc"#);
        assert!(diagnostics.has_errors());
        assert_eq!(diagnostics.error_count(), 1);
        assert!(
            diagnostics
                .iter()
                .next()
                .map(|diagnostic| diagnostic.message.contains("unterminated"))
                .unwrap_or(false)
        );
        assert!(
            tokens
                .iter()
                .any(|token| token.kind == TokenKind::Str("abc".to_string()))
        );
        assert_eq!(
            tokens.last().map(|token| token.kind.clone()),
            Some(TokenKind::Eof)
        );
    }

    #[test]
    fn unterminated_block_comment_is_recovered() {
        let (tokens, diagnostics) = tokenize("let x = 1; /* not closed");
        assert!(diagnostics.has_errors());
        assert_eq!(diagnostics.error_count(), 1);
        assert_eq!(
            tokens
                .iter()
                .filter(|token| token.kind == TokenKind::Eof)
                .count(),
            1
        );
        assert_eq!(
            tokens.last().map(|token| token.kind.clone()),
            Some(TokenKind::Eof)
        );
    }

    #[test]
    fn unexpected_characters_are_reported_and_skipped() {
        for (source, needle) in [
            ("let x = #;", "#"),
            ("let x = $;", "$"),
            ("let x = \\;", "\\"),
            ("let x = @;", "@"),
            ("let x = \u{60};", "\u{60}"),
            ("let x = ~;", "~"),
        ] {
            let messages = error_messages(source);
            assert_eq!(messages.len(), 1, "source {:?}: {:?}", source, messages);
            assert!(
                messages[0].contains(needle),
                "source {:?}: {}",
                source,
                messages[0]
            );
            let (tokens, _) = tokenize(source);
            assert_eq!(
                tokens.last().map(|token| token.kind.clone()),
                Some(TokenKind::Eof)
            );
        }
    }

    #[test]
    fn token_spans_cover_source_text() {
        let source = "let x = 1 + 2;";
        let (tokens, diagnostics) = tokenize(source);
        assert!(diagnostics.is_empty());
        let texts: Vec<&str> = tokens
            .iter()
            .filter(|token| token.kind != TokenKind::Eof)
            .map(|token| &source[token.span.start as usize..token.span.end as usize])
            .collect();
        assert_eq!(texts, vec!["let", "x", "=", "1", "+", "2", ";"]);
        if let Some(eof) = tokens.last() {
            assert_eq!(eof.span, Span::empty(source.len() as u32));
        }
    }

    #[test]
    fn multiline_source_offsets() {
        let source = "fn main() {\n    let x = 1;\n}\n";
        let (tokens, _) = tokenize(source);
        let let_token = tokens
            .iter()
            .find(|token| token.kind == TokenKind::KwLet)
            .expect("let token");
        assert_eq!(let_token.span.start as usize, source.find("let").unwrap());
    }

    #[test]
    fn nested_block_comments() {
        let source = "a /* one /* two /* three */ two */ one */ b";
        assert_eq!(
            kinds(source),
            vec![
                TokenKind::Ident("a".to_string()),
                TokenKind::Ident("b".to_string()),
                TokenKind::Eof,
            ]
        );
        assert!(errors(source).is_empty());
    }

    #[test]
    fn line_comment_runs_to_newline() {
        let source = "a // comment with /* and */ inside\nb";
        assert_eq!(
            kinds(source),
            vec![
                TokenKind::Ident("a".to_string()),
                TokenKind::Ident("b".to_string()),
                TokenKind::Eof,
            ]
        );
        assert!(errors(source).is_empty());
    }

    #[test]
    fn comment_markers_inside_strings_are_literal() {
        assert_eq!(
            strings(r#""// not a comment" "/* neither */""#),
            vec!["// not a comment".to_string(), "/* neither */".to_string()]
        );
    }

    #[test]
    fn describe_and_display() {
        assert_eq!(TokenKind::KwFn.to_string(), "fn");
        assert_eq!(TokenKind::ColonColon.to_string(), "::");
        assert_eq!(TokenKind::Shl.to_string(), "<<");
        assert_eq!(TokenKind::ShrEq.to_string(), ">>=");
        assert_eq!(TokenKind::Eof.to_string(), "end of file");
        assert!(
            TokenKind::Ident("foo".to_string())
                .describe()
                .contains("foo")
        );
        assert_eq!(TokenKind::Eof.describe(), "end of file");
    }

    #[test]
    fn token_stream_always_ends_with_eof() {
        let sources = [
            "",
            " ",
            "/*",
            "//",
            "\"",
            "\"\\",
            "0x",
            "0b2",
            "1e",
            ".",
            "..",
            "..=",
            "#",
            "é(",
            "\u{feff}let",
            "fn main() {",
            "let x = 'a';",
            "①",
            "\u{2028}",
        ];
        for source in sources {
            let (tokens, _) = tokenize(source);
            assert!(!tokens.is_empty(), "no tokens produced for {:?}", source);
            assert_eq!(
                tokens.last().map(|token| token.kind.clone()),
                Some(TokenKind::Eof),
                "missing Eof for {:?}",
                source
            );
            // Every token span must be a valid slice of the source.
            for token in &tokens {
                assert!(
                    source
                        .get(token.span.start as usize..token.span.end as usize)
                        .is_some(),
                    "invalid span {:?} for {:?}",
                    token.span,
                    source
                );
            }
        }
    }

    #[test]
    fn bom_is_ignored() {
        let source = "\u{feff}let x = 1;";
        assert_eq!(
            kinds(source),
            vec![
                TokenKind::KwLet,
                TokenKind::Ident("x".to_string()),
                TokenKind::Eq,
                TokenKind::Int(1),
                TokenKind::Semi,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn windows_line_endings() {
        let source = "let x = 1;\r\nlet y = 2;\r\n";
        assert_eq!(
            kinds(source),
            vec![
                TokenKind::KwLet,
                TokenKind::Ident("x".to_string()),
                TokenKind::Eq,
                TokenKind::Int(1),
                TokenKind::Semi,
                TokenKind::KwLet,
                TokenKind::Ident("y".to_string()),
                TokenKind::Eq,
                TokenKind::Int(2),
                TokenKind::Semi,
                TokenKind::Eof,
            ]
        );
        assert!(errors(source).is_empty());
    }

    #[test]
    fn diagnostic_codes_are_stable() {
        let diagnostics = tokenize("let x = #;").1;
        let codes: Vec<&str> = diagnostics
            .iter()
            .filter_map(|diagnostic| diagnostic.code)
            .collect();
        assert_eq!(codes, vec!["E0001"]);
        let diagnostics = tokenize("/*").1;
        let codes: Vec<&str> = diagnostics
            .iter()
            .filter_map(|diagnostic| diagnostic.code)
            .collect();
        assert_eq!(codes, vec!["E0006"]);
    }

    #[test]
    fn tuple_field_numbers_are_not_floats() {
        use TokenKind::*;
        assert_eq!(kinds("t.0"), vec![Ident("t".to_string()), Dot, Int(0), Eof]);
        assert_eq!(
            kinds("t.0.1"),
            vec![Ident("t".to_string()), Dot, Int(0), Dot, Int(1), Eof]
        );
        assert!(errors("t.0.1").is_empty());
        // A float after any other token still lexes as one token.
        assert_eq!(floats("x + 1.5"), vec![1.5]);
        assert_eq!(floats("(1.5)"), vec![1.5]);
    }

    #[test]
    fn random_input_never_panics() {
        // Deterministic pseudo-random soup with quotes, comment markers and
        // brackets: the lexer must always terminate and produce valid spans.
        let alphabet: Vec<char> = vec![
            'a', 'b', 'X', 'Y', '_', '0', '1', '2', '9', ' ', '/', '*', '+', '-', '=', '!', '&',
            '|', '^', '~', '#', '{', '}', '[', ']', '(', ')', '.', ',', ';', ':', '<', '>', '%',
            '"', '?', '@',
        ];
        let mut state: u64 = 0x2545_F491_4F6C_DD1D;
        for _ in 0..300 {
            let length = 12 + (state % 80) as usize;
            let mut source = String::new();
            for _ in 0..length {
                state = state
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                let index = ((state >> 33) as usize) % alphabet.len();
                source.push(alphabet[index]);
            }
            let (tokens, _diagnostics) = tokenize(&source);
            assert_eq!(
                tokens.last().map(|token| token.kind.clone()),
                Some(TokenKind::Eof),
                "missing Eof for {:?}",
                source
            );
            for token in &tokens {
                assert!(
                    source
                        .get(token.span.start as usize..token.span.end as usize)
                        .is_some(),
                    "invalid span {:?} for {:?}",
                    token.span,
                    source
                );
            }
        }
    }
}
