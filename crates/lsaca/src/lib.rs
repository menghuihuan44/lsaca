//! The lsaca compiler library.
//!
//! lsaca is a statically typed, Rust-flavoured language that compiles to C.
//! Its memory management model is ORC: every owned value is reference counted
//! (like Nim's ARC/ORC) and mutation uses copy-on-write, so programs never need
//! a tracing garbage collector.
//!
//! # Compilation pipeline
//!
//! 1. lexer turns source text into tokens.
//! 2. parser builds the ast.
//! 3. typeck resolves names, infers and checks types, and lowers the AST to
//!    the hir.
//! 4. codegen turns the HIR into a self contained C project (main.c,
//!    lsaca_rt.h, lsaca_rt.c, CMakeLists.txt).
//! 5. driver orchestrates the pipeline and the native build; the CLI crate
//!    wraps the driver.

#![forbid(unsafe_code)]

pub mod ast;
pub mod borrow;
pub mod codegen;
pub mod diag;
pub mod driver;
pub mod hir;
pub mod lexer;
pub mod parser;
pub mod runtime;
pub mod span;
pub mod typeck;
pub mod types;

pub use diag::{Diagnostic, Diagnostics, Severity};
pub use span::{SourceFile, Span};
