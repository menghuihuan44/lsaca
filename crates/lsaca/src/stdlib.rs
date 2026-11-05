//! The embedded standard library.
//!
//! The standard library (stdlib/src) is written in lsaca itself. Its sources
//! are embedded into the compiler by `crate::stdlib_embed` and loaded here as
//! one parsed crate: the crate root is stdlib/src/lib.ls and every external
//! module it declares is resolved against the embedded files and turned into
//! an inline module, exactly like the driver does for user crates.
//!
//! Standard library sources occupy their own global offset range starting at
//! [BASE], far above the offsets the driver assigns to user code, so a span can
//! never be ambiguous between the library and the program. Diagnostics that
//! point into the library name the file `std/<path>`. A failure while parsing the
//! library is a compiler bug, so it is reported as an internal error.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::ast;
use crate::diag::{Diagnostic, Diagnostics};
use crate::lexer;
use crate::parser;
use crate::span::{SourceMap, Span};

/// Global offset where the embedded standard-library sources start.
pub const BASE: u32 = 1 << 30;

/// Path of the standard-library crate root inside stdlib/src.
pub const ROOT: &str = "lib.ls";

/// One parsed, embedded standard library.
pub struct Stdlib {
    sources: SourceMap,
    program: ast::Program,
}

impl Stdlib {
    /// The whole library as one crate: the root module with its modules inlined.
    pub fn program(&self) -> &ast::Program {
        &self.program
    }

    /// Every standard-library source file, with its global offset range.
    pub fn sources(&self) -> &SourceMap {
        &self.sources
    }
}

/// The embedded files as (path, source) pairs; the crate root comes first.
pub fn files() -> &'static [(&'static str, &'static str)] {
    crate::stdlib_embed::FILES
}

/// The standard library, parsed once on first use and cached afterwards.
pub fn stdlib() -> Result<&'static Stdlib, Diagnostics> {
    static CACHE: OnceLock<Result<Stdlib, Diagnostics>> = OnceLock::new();
    match CACHE.get_or_init(parse_stdlib) {
        Ok(stdlib) => Ok(stdlib),
        Err(errors) => Err(errors.clone()),
    }
}

/// The parsed standard library program.
pub fn program() -> Result<&'static ast::Program, Diagnostics> {
    stdlib().map(Stdlib::program)
}

/// All standard-library source files, for diagnostics and panic locations.
pub fn sources() -> Result<&'static SourceMap, Diagnostics> {
    stdlib().map(Stdlib::sources)
}

/// Rewrite a diagnostic produced by standard-library code into an internal error.
pub fn internal_error(diagnostic: Diagnostic) -> Diagnostic {
    if diagnostic
        .message
        .starts_with(crate::diag::INTERNAL_STDLIB_PREFIX)
    {
        return diagnostic;
    }
    let mut message = String::from("internal standard library error: ");
    message.push_str(&diagnostic.message);
    let mut rewritten = Diagnostic::error(message, diagnostic.span);
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

fn internal_error_now(message: impl Into<String>) -> Diagnostics {
    let mut errors = Diagnostics::new();
    errors.push(Diagnostic::error(
        format!("internal standard library error: {}", message.into()),
        Span::empty(BASE),
    ));
    errors
}

fn parse_stdlib() -> Result<Stdlib, Diagnostics> {
    let files = files();
    if files.is_empty() {
        return Err(internal_error_now("the embedded standard library is empty"));
    }
    let mut sources = SourceMap::new();
    let mut programs: HashMap<&'static str, ast::Program> = HashMap::new();
    let mut diagnostics = Diagnostics::new();
    for (path, text) in files {
        sources.add_file_at(format!("std/{}", path), *text, BASE);
        let file = sources.file(sources.len() - 1);
        let (tokens, lex_diagnostics) = lexer::tokenize_file(file);
        diagnostics.extend(lex_diagnostics);
        let (program, parse_diagnostics) = parser::parse(&tokens, file);
        diagnostics.extend(parse_diagnostics);
        programs.insert(*path, program);
    }
    if diagnostics.has_errors() {
        return Err(internal_errors(diagnostics));
    }
    let root = match programs.remove(ROOT) {
        Some(root) => root,
        None => {
            return Err(internal_error_now(format!(
                "the standard library is missing its crate root {}",
                ROOT
            )));
        }
    };
    let mut items = root.items;
    expand_module_items(&mut items, "", &mut programs, &mut diagnostics);
    if diagnostics.has_errors() {
        return Err(internal_errors(diagnostics));
    }
    Ok(Stdlib {
        sources,
        program: ast::Program { items },
    })
}

fn expand_module_items(
    items: &mut [ast::Item],
    dir: &str,
    programs: &mut HashMap<&'static str, ast::Program>,
    diagnostics: &mut Diagnostics,
) {
    for item in items.iter_mut() {
        let ast::Item::Mod(module) = item else {
            continue;
        };
        let module_dir = join_dir(dir, &module.name);
        if module.external {
            let plain = format!("{}.ls", module_dir);
            let nested = format!("{}/mod.ls", module_dir);
            let key = if programs.contains_key(plain.as_str()) {
                Some(plain.clone())
            } else if programs.contains_key(nested.as_str()) {
                Some(nested.clone())
            } else {
                None
            };
            match key {
                Some(path) => {
                    let source =
                        std::mem::take(&mut programs.get_mut(path.as_str()).unwrap().items);
                    module.external = false;
                    module.items = source;
                    expand_module_items(&mut module.items, &module_dir, programs, diagnostics);
                }
                None => {
                    diagnostics.push(Diagnostic::error(
                        format!(
                            "internal standard library error: module {} is missing (looked for std/{} and std/{})",
                            module.name, plain, nested
                        ),
                        module.name_span,
                    ));
                }
            }
        } else {
            expand_module_items(&mut module.items, &module_dir, programs, diagnostics);
        }
    }
}

fn join_dir(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        name.to_string()
    } else {
        format!("{}/{}", dir, name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embeds_the_crate_root_first() {
        let files = files();
        assert!(!files.is_empty());
        assert_eq!(files[0].0, ROOT);
    }

    #[test]
    fn parses_and_expands_the_library() {
        let library = stdlib().expect("the embedded standard library must parse");
        assert!(!library.sources().is_empty());
        assert!(
            !library.program().items.is_empty(),
            "the standard library must declare items"
        );
    }

    #[test]
    fn library_files_use_the_high_offset_range() {
        let library = stdlib().expect("the embedded standard library must parse");
        for file in library.sources().files() {
            assert!(file.base() >= BASE, "{} starts below BASE", file.name());
        }
    }
}
