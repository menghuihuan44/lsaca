//! The standard-library prelude: lsaca source compiled into every program.
//!
//! The prelude is written in lsaca itself (see `prelude.ls` next to this
//! file) and parsed once per process. It provides the `Iterator` trait and the
//! lazy iterator adapters that the compiler's method sugar lowers to, so
//! ordinary library code can be written in the language it serves.
//!
//! Every span of the parsed prelude program is rewritten to `Span::empty(0)`
//! so prelude-origin diagnostics and generated location information never point
//! into the user's source file. The type checker labels diagnostics produced
//! by prelude items as internal standard-library errors.

use std::sync::OnceLock;

use crate::ast;
use crate::diag::{Diagnostic, Diagnostics};
use crate::lexer;
use crate::parser;
use crate::span::{SourceFile, Span};

/// The name reported for the synthetic prelude source file.
pub const FILE_NAME: &str = "<lsaca prelude>";

/// The prelude source code.
pub const SOURCE: &str = include_str!("prelude.ls");

/// The parsed prelude program, parsed on first use and cached afterwards.
///
/// A prelude that fails to parse indicates a compiler bug, so the error
/// diagnostics are rewritten into internal-error messages before they reach
/// the user.
pub fn program() -> Result<&'static ast::Program, Diagnostics> {
    static CACHE: OnceLock<Result<ast::Program, Diagnostics>> = OnceLock::new();
    match CACHE.get_or_init(parse) {
        Ok(program) => Ok(program),
        Err(errors) => Err(errors.clone()),
    }
}

fn parse() -> Result<ast::Program, Diagnostics> {
    let file = SourceFile::new(FILE_NAME, SOURCE);
    let (tokens, mut diagnostics) = lexer::tokenize(file.text());
    let (mut program, parse_diagnostics) = parser::parse(&tokens, &file);
    diagnostics.extend(parse_diagnostics);
    if diagnostics.has_errors() {
        return Err(internal_errors(diagnostics));
    }
    program.for_each_span_mut(&mut |span| *span = Span::empty(0));
    Ok(program)
}

/// Rewrite a diagnostic produced while processing the prelude into an internal
/// standard-library error, so users never see compiler-library source positions.
pub fn internal_error(diagnostic: Diagnostic) -> Diagnostic {
    let mut message = String::from("internal standard library error: ");
    message.push_str(&diagnostic.message);
    let mut rewritten = Diagnostic::error(message, Span::empty(0));
    rewritten.code = diagnostic.code;
    for note in diagnostic.notes {
        rewritten = rewritten.with_note(note);
    }
    rewritten
}

fn internal_errors(diagnostics: Diagnostics) -> Diagnostics {
    let mut errors = Diagnostics::new();
    for diagnostic in diagnostics.into_vec() {
        errors.push(internal_error(diagnostic));
    }
    errors
}
