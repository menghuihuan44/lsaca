//! Pipeline orchestration: source text to a checked HIR, then to a C project
//! and (optionally) a native executable.
//!
//! The compiler emits a regular, self-contained C project (main.c, the runtime
//! sources and CMakeLists.txt). By default the project is built by invoking the
//! C compiler directly: every runtime translation unit is compiled once per
//! (runtime content, compiler, flags) combination into a shared object cache
//! and then linked with main.c. The generated CMake project remains available
//! through [`BuildOptions::use_cmake`] (`lsaca --cmake`) and is what `lsaca
//! emit-c` writes. Build trees are content-addressed and cached between runs.
//!
//! The cache root is chosen defensively, because toolchains dislike non-ASCII
//! paths and locked-down environments (sandboxes, restricted CI runners) only
//! allow writes inside the workspace: the system temporary directory is used
//! when it has an ASCII path and is actually writable, and
//! `.lsaca-build-cache` in the working directory is the fallback. The compiled
//! executable is copied out of the cache, so workspace paths may contain any
//! characters.

use std::collections::VecDeque;
use std::env;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::process::{self, Command};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use crate::ast;
use crate::codegen::{self, CodegenOptions, Generated};
use crate::diag::{Diagnostic, Diagnostics};
use crate::hir;
use crate::lexer;
use crate::parser;
use crate::runtime;
use crate::span::{SourceFile, SourceMap, Span};
use crate::typeck;

/// Stack size of the worker thread that runs the recursive compiler passes.
///
/// The parser rejects syntax nested deeper than 128 AST levels, and every later
/// pass walks the tree recursively. The type checker needs roughly 10 KiB of
/// stack per level in a debug build, so on the default 1 MiB main thread a legal
/// program with about 99 nested expressions used to abort with a stack overflow
/// (exit code 0xC00000FD) instead of compiling. A 64 MiB thread keeps every
/// accepted program safe (128 levels need well under 2 MiB); the main thread
/// stack is left untouched for the rest of the process.
const COMPILER_STACK_SIZE: usize = 64 * 1024 * 1024;

/// Run one recursive compiler pass on a worker thread with a large stack.
///
/// Returns the panic payload when the thread could not be spawned or when the
/// pass panicked. Both are internal compiler errors: the type checker reports
/// them as a diagnostic, while the code generator re-raises the panic.
fn run_on_compiler_stack<T, F>(work: F) -> Result<T, Box<dyn std::any::Any + Send>>
where
    T: Send,
    F: FnOnce() -> T + Send,
{
    std::thread::scope(|scope| {
        match std::thread::Builder::new()
            .name("lsaca-compiler".to_string())
            .stack_size(COMPILER_STACK_SIZE)
            .spawn_scoped(scope, work)
        {
            Ok(handle) => handle.join(),
            Err(error) => Err(
                Box::new(format!("could not start the compiler thread: {error}"))
                    as Box<dyn std::any::Any + Send>,
            ),
        }
    })
}

/// Describe a panic payload passed through run_on_compiler_stack.
fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<String>() {
        return message.clone();
    }
    if let Some(message) = payload.downcast_ref::<&'static str>() {
        return (*message).to_string();
    }
    "internal compiler error".to_string()
}

/// The maximum module nesting depth. Deeper trees are rejected instead of
/// risking a stack overflow in the recursive loader or the compiler passes.
const MAX_MODULE_DEPTH: usize = 256;

/// Append the embedded standard-library sources to a compilation's sources.
///
/// User files are registered first and therefore occupy the low global offset
/// range; the library is appended at [crate::stdlib::BASE] afterwards. The same
/// files are registered in the same order by [crate::stdlib], so a span from a
/// standard-library item resolves in this map to std/<path>:line:column.
fn add_stdlib_sources(sources: &mut SourceMap) {
    for (path, text) in crate::stdlib::files() {
        sources.add_file_at(format!("std/{}", path), *text, crate::stdlib::BASE);
    }
}

/// Front-end pipeline: load, lex, parse and type check a program.
///
/// A program is one or more crates. Standalone files ([Frontend::new] and
/// [Frontend::load]) form a single crate; package mode ([Frontend::load_package])
/// builds the crate graph of the root package plus its path dependencies (the
/// package manager lives in [crate::package]).
#[derive(Debug)]
pub struct Frontend {
    /// Every source file of the compilation, with disjoint global spans.
    sources: SourceMap,
    /// All crates of the compilation; single-file mode contains one crate.
    graph: ast::CrateGraph,
    /// Whether the root crate must define `fn main()` (true for binaries,
    /// false for library crates).
    require_main: bool,
    /// Whether the root crate's entry file is a binary root (src/main.ls or a
    /// standalone file). A binary root without `fn main()` gets a specific
    /// diagnostic instead of the library-package message.
    binary_root: bool,
    /// Diagnostics produced before semantic analysis: IO errors while loading
    /// modules, invalid UTF-8, lexer and parser errors.
    load_diagnostics: Diagnostics,
}

impl Frontend {
    /// Build a front-end from in-memory source text.
    ///
    /// In-memory programs cannot load module files; a `mod name;` declaration
    /// is reported as E0583 because there is no directory to resolve it against.
    pub fn new(file_name: impl Into<String>, source: impl Into<String>) -> Frontend {
        let file_name = file_name.into();
        let crate_name = crate_name_from_path(Path::new(&file_name));
        let mut sources = SourceMap::new();
        sources.add_file(file_name, source.into());
        let mut diagnostics = Diagnostics::new();
        let mut program = parse_file(&sources, 0, &mut diagnostics);
        {
            let mut loader = ModuleLoader {
                sources: &mut sources,
                diagnostics: &mut diagnostics,
                stack: Vec::new(),
            };
            loader.expand_items(&mut program.items, None, 0);
        }
        add_stdlib_sources(&mut sources);
        Frontend {
            sources,
            graph: ast::CrateGraph::single(crate_name, program),
            require_main: true,
            binary_root: true,
            load_diagnostics: diagnostics,
        }
    }

    /// Load a program from a file on disk and resolve all of its module files.
    ///
    /// Only IO errors on the root file are returned as errors; problems with
    /// module files become diagnostics that [Frontend::check] reports like any
    /// other error, so a single run can report several missing or ambiguous
    /// modules at once.
    pub fn load(path: &Path) -> io::Result<Frontend> {
        match run_on_compiler_stack(|| Self::load_inner(path)) {
            Ok(result) => result,
            Err(payload) => Err(io::Error::other(format!(
                "internal compiler error while loading modules: {}",
                panic_message(payload.as_ref())
            ))),
        }
    }

    /// Loading runs on the compiler stack because [ModuleLoader] recurses once
    /// per module file; 256 levels need far more than the default 1 MiB stack.
    fn load_inner(path: &Path) -> io::Result<Frontend> {
        let root = fs::canonicalize(path)?;
        let contents = read_source_text(&root)?;
        let crate_name = crate_name_from_path(&root);
        let mut sources = SourceMap::new();
        sources.add_file(display_source_name(&root), contents);
        let mut diagnostics = Diagnostics::new();
        let mut program = parse_file(&sources, 0, &mut diagnostics);
        let root_dir = root.parent().map(Path::to_path_buf);
        {
            let mut loader = ModuleLoader {
                sources: &mut sources,
                diagnostics: &mut diagnostics,
                stack: vec![root],
            };
            loader.expand_items(&mut program.items, root_dir.as_deref(), 0);
        }
        add_stdlib_sources(&mut sources);
        Ok(Frontend {
            sources,
            graph: ast::CrateGraph::single(crate_name, program),
            require_main: true,
            binary_root: true,
            load_diagnostics: diagnostics,
        })
    }

    /// Load a package (or a member of one) starting from a directory.
    ///
    /// The nearest `lsaca.toml` is discovered walking up from `start_dir`,
    /// its dependency graph is resolved, and every package contributes one
    /// crate (root package last, its modules resolved relative to `src/`).
    pub fn load_package(start_dir: &Path) -> Result<Frontend, LoadError> {
        match run_on_compiler_stack(|| Self::load_package_inner(start_dir)) {
            Ok(result) => result,
            Err(payload) => Err(LoadError::Io(io::Error::other(format!(
                "internal compiler error while loading the package: {}",
                panic_message(payload.as_ref())
            )))),
        }
    }

    fn load_package_inner(start_dir: &Path) -> Result<Frontend, LoadError> {
        let mut sources = SourceMap::new();
        let mut diagnostics = Diagnostics::new();
        let manifest = match crate::package::discover_manifest(start_dir) {
            Ok(Some(manifest)) => manifest,
            Ok(None) => {
                return Err(LoadError::Io(io::Error::new(
                    io::ErrorKind::NotFound,
                    "no lsaca.toml found in this directory or any parent directory",
                )));
            }
            Err(error) => return Err(LoadError::Io(error)),
        };
        // The resolver parses the manifest and everything it reaches, so the
        // root manifest is registered in the source map exactly once.
        let manifest_path = match fs::canonicalize(&manifest) {
            Ok(path) => path,
            Err(error) => return Err(LoadError::Io(error)),
        };
        let graph = match crate::package::resolve(&manifest_path, &mut sources, &mut diagnostics) {
            Some(graph) => graph,
            None => {
                return Err(LoadError::Diagnostics {
                    diagnostics,
                    sources,
                });
            }
        };
        let root_package = graph.root;
        let mut crates: Vec<ast::Crate> = Vec::new();
        let mut binary_root = false;
        for (index, package) in graph.packages.iter().enumerate() {
            let is_root = index == root_package;
            let binary = package.dir.join("src").join("main.ls");
            let library = package.dir.join("src").join("lib.ls");
            if is_root && binary.is_file() && library.is_file() {
                // A package is either a binary or a library; guessing one would
                // silently ignore the other, so ask the author to pick.
                diagnostics.push(
                    Diagnostic::error(
                        format!(
                            "package '{}' has both src/main.ls and src/lib.ls",
                            package.name
                        ),
                        Span::empty(0),
                    )
                    .with_code("E0001")
                    .with_note(
                        "a package must be a binary (src/main.ls) or a library (src/lib.ls), not both"
                            .to_string(),
                    ),
                );
                return Err(LoadError::Diagnostics {
                    diagnostics,
                    sources,
                });
            }
            if is_root {
                binary_root = binary.is_file();
            }
            let root_file = if is_root && binary.is_file() {
                binary
            } else {
                library
            };
            let canonical = match fs::canonicalize(&root_file) {
                Ok(canonical) => canonical,
                Err(error) => return Err(LoadError::Io(error)),
            };
            let contents = match read_source_text(&canonical) {
                Ok(contents) => contents,
                Err(error) => return Err(LoadError::Io(error)),
            };
            let index_in_sources = sources.add_file(display_source_name(&canonical), contents);
            let mut program = parse_file(&sources, index_in_sources, &mut diagnostics);
            let root_dir = canonical.parent().map(Path::to_path_buf);
            {
                let mut loader = ModuleLoader {
                    sources: &mut sources,
                    diagnostics: &mut diagnostics,
                    stack: vec![canonical],
                };
                loader.expand_items(&mut program.items, root_dir.as_deref(), 0);
            }
            crates.push(ast::Crate {
                name: package.name.clone(),
                program,
                is_dependency: package.is_dependency,
            });
        }
        add_stdlib_sources(&mut sources);
        // Crate order follows the package graph (dependencies first, root
        // last), so the root crate is the last one in the graph.
        let root_index = crates.len().saturating_sub(1);
        // A package whose crate root is src/main.ls must define fn main();
        // a library package has no entry point. Checking the binary shape
        // (not the presence of fn main) is what makes a missing entry point a
        // diagnostic instead of silently accepting the package.
        Ok(Frontend {
            sources,
            graph: ast::CrateGraph {
                crates,
                root: root_index,
            },
            require_main: binary_root,
            binary_root,
            load_diagnostics: diagnostics,
        })
    }

    /// Every source file of the compilation.
    pub fn sources(&self) -> &SourceMap {
        &self.sources
    }

    /// The root source file (the file a diagnostic without a known span is
    /// rendered against).
    pub fn file(&self) -> &SourceFile {
        self.sources.file(0)
    }

    /// Every crate taking part in the compilation.
    pub fn graph(&self) -> &ast::CrateGraph {
        &self.graph
    }

    /// Whether the root crate is a binary root: a standalone file or a package
    /// with src/main.ls, even when it does not define fn main() yet.
    pub fn has_binary_root(&self) -> bool {
        self.binary_root
    }

    /// Whether the root crate defines a fn main() function.
    pub fn has_main(&self) -> bool {
        self.graph
            .root_crate()
            .program
            .items
            .iter()
            .any(|item| matches!(item, ast::Item::Fn(function) if function.name == "main"))
    }

    /// Force the `fn main()` requirement on or off (only meaningful for
    /// library crates and tests).
    pub fn require_main(&mut self, require_main: bool) {
        self.require_main = require_main;
    }

    /// Run the front-end. On success the returned HIR is ready for code
    /// generation. On failure the diagnostics explain why the program was
    /// rejected (they may contain several independent errors).
    pub fn check(&self) -> Result<hir::Program, Diagnostics> {
        self.check_reporting().map(|(program, _warnings)| program)
    }

    /// Run the front-end, keeping non-fatal diagnostics from every stage.
    ///
    /// On success the first element holds the HIR and the second holds the
    /// warnings that were reported while checking (unreachable match arms and
    /// similar). On failure the errors are returned as the error value, and
    /// they already include any warnings emitted before the failure.
    pub fn check_reporting(&self) -> Result<(hir::Program, Diagnostics), Diagnostics> {
        let mut diagnostics = self.load_diagnostics.clone();
        if diagnostics.has_errors() {
            return Err(diagnostics);
        }
        // The standard library is embedded in the compiler and parsed once per
        // process. Its sources live in their own offset range above the user
        // files (see [add_stdlib_sources]), so a span in the library resolves
        // to std/<path> while user spans keep their own file names.
        let prelude = match run_on_compiler_stack(crate::stdlib::program) {
            Ok(Ok(program)) => Some(program),
            Ok(Err(errors)) => {
                diagnostics.extend(errors);
                return Err(diagnostics);
            }
            Err(payload) => {
                diagnostics.error(
                    format!(
                        "the compiler crashed while parsing the standard library ({})",
                        panic_message(&*payload)
                    ),
                    Span::default(),
                );
                return Err(diagnostics);
            }
        };
        // Type checking and HIR lowering recurse once per AST level, so they
        // run on the large-stack worker thread: the parser already guarantees
        // that accepted syntax is at most 128 levels deep, which overflows the
        // default main-thread stack in debug builds.
        let options = typeck::CheckOptions::new().require_main(self.require_main);
        let graph = &self.graph;
        let sources = self.sources();
        let checked = run_on_compiler_stack(|| {
            typeck::check_crate_reporting_with_options(graph, prelude, sources, &options)
        });
        let (result, type_diagnostics) = match checked {
            Ok(checked) => checked,
            Err(payload) => {
                diagnostics.error(
                    format!(
                        "the compiler crashed while checking this program ({})",
                        panic_message(&*payload)
                    ),
                    Span::default(),
                );
                return Err(diagnostics);
            }
        };
        match result {
            Ok(program) => {
                diagnostics.extend(type_diagnostics);
                Ok((program, diagnostics))
            }
            Err(errors) => {
                diagnostics.extend(errors);
                diagnostics.extend(type_diagnostics);
                Err(diagnostics)
            }
        }
    }
}

/// An error from loading a package: either a plain IO error (missing root
/// file) or accumulated diagnostics (bad manifest, missing modules, ...).
#[derive(Debug)]
pub enum LoadError {
    Io(io::Error),
    Diagnostics {
        diagnostics: Diagnostics,
        sources: SourceMap,
    },
}

impl LoadError {
    /// Render the error for the CLI. Diagnostics are rendered with their
    /// source snippets; an IO error is rendered as a single line.
    pub fn render(&self) -> String {
        match self {
            LoadError::Io(error) => format!("{}", error),
            LoadError::Diagnostics {
                diagnostics,
                sources,
            } => diagnostics.render(sources),
        }
    }
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::Io(error) => write!(formatter, "{}", error),
            LoadError::Diagnostics { .. } => write!(formatter, "{}", self.render().trim_end()),
        }
    }
}

impl std::error::Error for LoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            LoadError::Io(error) => Some(error),
            LoadError::Diagnostics { .. } => None,
        }
    }
}

impl From<io::Error> for LoadError {
    fn from(error: io::Error) -> LoadError {
        LoadError::Io(error)
    }
}

/// Reads and parses module files, rewriting external `mod name;` items into
/// inline modules so the rest of the compiler only ever sees one module tree
/// per crate.
struct ModuleLoader<'a> {
    sources: &'a mut SourceMap,
    diagnostics: &'a mut Diagnostics,
    /// Canonical paths of the module files currently being loaded, used to
    /// detect circular module declarations.
    stack: Vec<PathBuf>,
}

impl ModuleLoader<'_> {
    /// Expand every module in one module item list.
    ///
    /// `dir` is the directory module files are resolved against: the parent
    /// directory of the current file. It is `None` for in-memory programs,
    /// where external modules are impossible by definition.
    fn expand_items(&mut self, items: &mut Vec<ast::Item>, dir: Option<&Path>, depth: usize) {
        if items.is_empty() {
            return;
        }
        if depth > MAX_MODULE_DEPTH {
            let span = items[0].span();
            self.diagnostics.push(
                Diagnostic::error(
                    format!("module nesting is deeper than {} levels", MAX_MODULE_DEPTH),
                    span,
                )
                .with_code("E0001"),
            );
            items.clear();
            return;
        }
        let mut seen: Vec<(String, Span)> = Vec::new();
        for item in items.iter_mut() {
            let ast::Item::Mod(decl) = item else {
                continue;
            };
            if let Some((_, previous)) = seen.iter().find(|(name, _)| name == &decl.name) {
                self.diagnostics.push(
                    Diagnostic::error(
                        format!("the name `{}` is defined multiple times", decl.name),
                        decl.name_span,
                    )
                    .with_code("E0428")
                    .with_label(*previous, "previous definition of the module here"),
                );
                decl.external = false;
                decl.items.clear();
                continue;
            }
            seen.push((decl.name.clone(), decl.name_span));
            if decl.external {
                let Some(dir) = dir else {
                    self.diagnostics.push(
                        Diagnostic::error(
                            format!(
                                "cannot load module `{}` from an in-memory source",
                                decl.name
                            ),
                            decl.name_span,
                        )
                        .with_code("E0583")
                        .with_note(
                            "the program was not loaded from a file, so there is no directory to find module files in",
                        ),
                    );
                    decl.external = false;
                    continue;
                };
                match self.load_module(decl, dir, depth) {
                    Some((mut child_items, child_dir)) => {
                        decl.external = false;
                        decl.items.clear();
                        decl.items.append(&mut child_items);
                        self.expand_items(&mut decl.items, Some(&child_dir), depth + 1);
                    }
                    None => {
                        decl.external = false;
                        decl.items.clear();
                    }
                }
            } else {
                let child_dir = dir.map(|dir| dir.join(&decl.name));
                self.expand_items(&mut decl.items, child_dir.as_deref(), depth + 1);
            }
        }
    }

    /// Load one external module file and parse it.
    fn load_module(
        &mut self,
        decl: &ast::ModDecl,
        parent_dir: &Path,
        depth: usize,
    ) -> Option<(Vec<ast::Item>, PathBuf)> {
        let flat = parent_dir.join(format!("{}.ls", decl.name));
        let nested = parent_dir.join(&decl.name).join("mod.ls");
        let (path, child_dir) = match (flat.is_file(), nested.is_file()) {
            (true, true) => {
                self.diagnostics.push(
                    Diagnostic::error(
                        format!(
                            "file for module `{}` found at both {} and {}",
                            decl.name,
                            display_source_name(&flat),
                            display_source_name(&nested)
                        ),
                        decl.name_span,
                    )
                    .with_code("E0761")
                    .with_note(
                        "delete or rename one of the two module files so only one definition exists",
                    ),
                );
                return None;
            }
            (true, false) => (flat, parent_dir.join(&decl.name)),
            (false, true) => (nested, parent_dir.join(&decl.name)),
            (false, false) => {
                self.diagnostics.push(
                    Diagnostic::error(
                        format!("file not found for module `{}`", decl.name),
                        decl.name_span,
                    )
                    .with_code("E0583")
                    .with_note(format!(
                        "expected one of: {}, {}",
                        display_source_name(&flat),
                        display_source_name(&nested)
                    )),
                );
                return None;
            }
        };
        let canonical = fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
        if let Some(start) = self.stack.iter().position(|entry| entry == &canonical) {
            let mut chain: Vec<String> = self.stack[start..]
                .iter()
                .map(|entry| display_source_name(entry))
                .collect();
            chain.push(display_source_name(&canonical));
            self.diagnostics.push(
                Diagnostic::error(
                    format!("circular module files: {}", chain.join(" -> ")),
                    decl.name_span,
                )
                .with_code("E0001")
                .with_note(format!("module `{}` was already being loaded", decl.name)),
            );
            return None;
        }
        let contents = match read_source_text(&path) {
            Ok(contents) => contents,
            Err(error) => {
                self.diagnostics.push(
                    Diagnostic::error(
                        format!(
                            "could not read module `{}` ({}): {}",
                            decl.name,
                            display_source_name(&path),
                            error
                        ),
                        decl.name_span,
                    )
                    .with_code("E0583"),
                );
                return None;
            }
        };
        let index = self.sources.add_file(display_source_name(&path), contents);
        self.stack.push(canonical);
        let mut program = parse_file(self.sources, index, self.diagnostics);
        self.expand_items(&mut program.items, Some(&child_dir), depth + 1);
        self.stack.pop();
        Some((program.items, child_dir))
    }
}

/// Read a source file, rejecting invalid UTF-8 with a clear message.
fn read_source_text(path: &Path) -> io::Result<String> {
    let bytes = fs::read(path)?;
    String::from_utf8(bytes).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{} is not valid UTF-8", display_source_name(path)),
        )
    })
}

/// Tokenize one file with global spans.
fn tokenize_source(file: &SourceFile) -> (Vec<lexer::Token>, Diagnostics) {
    let (mut tokens, mut diagnostics) = lexer::tokenize(file.text());
    let base = file.base();
    if base != 0 {
        for token in &mut tokens {
            token.span = shift_span(token.span, base);
        }
        let mut shifted = Diagnostics::new();
        for mut diagnostic in diagnostics.into_vec() {
            diagnostic.span = shift_span(diagnostic.span, base);
            for label in &mut diagnostic.labels {
                label.span = shift_span(label.span, base);
            }
            shifted.push(diagnostic);
        }
        diagnostics = shifted;
    }
    (tokens, diagnostics)
}

fn shift_span(span: Span, base: u32) -> Span {
    Span::new(
        span.start.saturating_add(base),
        span.end.saturating_add(base),
    )
}

/// Lex and parse one already registered source file.
fn parse_file(sources: &SourceMap, index: usize, diagnostics: &mut Diagnostics) -> ast::Program {
    let file = sources.file(index);
    let (tokens, lexer_diagnostics) = tokenize_source(file);
    diagnostics.extend(lexer_diagnostics);
    let (program, parse_diagnostics) = parser::parse(&tokens, file);
    diagnostics.extend(parse_diagnostics);
    program
}

/// A display name for a source file: relative to the current directory when
/// possible, always with forward slashes.
///
/// Loaded paths are canonicalized, which on Windows turns them into verbatim
/// paths of the form \\?\C:\...; those are normalized back to plain paths
/// so diagnostics and panic locations show src/main.ls instead of //?/C:/...
fn display_source_name(path: &Path) -> String {
    let shown_path = normalize_path(path);
    let cwd = env::current_dir().ok().map(|dir| normalize_path(&dir));
    let shown = match cwd {
        Some(cwd) => shown_path
            .strip_prefix(&cwd)
            .map(Path::to_path_buf)
            .unwrap_or_else(|_| shown_path.clone()),
        None => shown_path.clone(),
    };
    let text = shown.to_string_lossy().replace('\\', "/");
    if text.is_empty() {
        shown_path.to_string_lossy().replace('\\', "/")
    } else {
        text
    }
}

/// Remove the Windows verbatim path prefix (\\?\ and \\?\UNC\) so paths
/// compare and display like ordinary paths.
fn normalize_path(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    if let Some(rest) = text.strip_prefix("\\\\?\\UNC\\") {
        PathBuf::from(format!("\\\\{}", rest))
    } else if let Some(rest) = text.strip_prefix("\\\\?\\") {
        PathBuf::from(rest)
    } else {
        path.to_path_buf()
    }
}

/// The crate name of a standalone file: its file stem, sanitized into a
/// lowercase identifier so it can be used in paths such as `crate_name::item`.
fn crate_name_from_path(path: &Path) -> String {
    let base = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("program");
    let stem = base.strip_suffix(".ls").unwrap_or(base);
    sanitize_crate_name(stem)
}

fn sanitize_crate_name(raw: &str) -> String {
    let mut name = String::with_capacity(raw.len());
    for (index, character) in raw.trim().chars().enumerate() {
        if character.is_ascii_alphanumeric() || character == '_' {
            if index == 0 && character.is_ascii_digit() {
                name.push('_');
            }
            name.push(character.to_ascii_lowercase());
        } else {
            name.push('_');
        }
    }
    if name.is_empty()
        || !name
            .chars()
            .any(|character| character.is_ascii_alphanumeric())
    {
        // A name made only of separators (for example "---.ls") is not a
        // useful crate name; fall back to the generic one.
        String::from("program")
    } else {
        name
    }
}

/// Generate the C project for a checked program.
///
/// Code generation walks the HIR recursively, so it runs on the large-stack
/// worker thread as well. A failure here is an internal compiler bug (the input
/// has already been type checked), so the panic is propagated unchanged.
pub fn generate_c(program: &hir::Program, options: &CodegenOptions) -> Generated {
    match run_on_compiler_stack(|| codegen::generate(program, options)) {
        Ok(generated) => generated,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

/// Write the generated C project (main.c, the extra translation units of a
/// split program, every runtime source published by [`runtime::sources`] and
/// CMakeLists.txt) into a directory, creating it if needed.
pub fn write_project(dir: &Path, generated: &Generated) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    write_atomic(&dir.join("main.c"), &generated.main_c)?;
    for unit in &generated.units {
        write_atomic(&dir.join(&unit.file_name), &unit.contents)?;
    }
    write_runtime_sources(dir)?;
    write_atomic(&dir.join("CMakeLists.txt"), &generated.cmake)?;
    Ok(())
}

/// Write the runtime sources (headers and translation units) next to main.c.
fn write_runtime_sources(dir: &Path) -> io::Result<()> {
    for (name, contents) in runtime::sources() {
        write_atomic(&dir.join(name), contents)?;
    }
    Ok(())
}

/// Write a file without ever exposing a partially written file to concurrent
/// readers: the contents go to a unique temporary file in the same directory
/// that is then renamed into place. Build trees are content-addressed, so
/// several threads may legitimately write the same file with the same bytes;
/// losing that race is not an error.
fn write_atomic(path: &Path, contents: &str) -> io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("file");
    let temp = temporary_path(parent, name);
    fs::write(&temp, contents)?;
    match fs::rename(&temp, path) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = fs::remove_file(&temp);
            if is_non_empty_file(path) {
                Ok(())
            } else {
                Err(io::Error::new(
                    error.kind(),
                    format!("could not write {}: {}", path.display(), error),
                ))
            }
        }
    }
}

/// A collision-free temporary path in the same directory as the final file.
fn temporary_path(dir: &Path, name: &str) -> PathBuf {
    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    dir.join(format!(".{}.{}.{}.tmp", name, process::id(), id))
}

/// Move a freshly built file to its final content-addressed location.
///
/// Concurrent builds may race to install the same file. Each build compiles to
/// its own temporary path, so a rename that loses the race can be discarded as
/// soon as the destination is a complete file.
///
/// A rename only works inside one file system. The direct backend compiles into
/// the build tree while cached runtime objects may live in the system temporary
/// directory, and on Windows those are often on different drives, so a failed
/// rename falls back to copying through a staging file in the destination
/// directory. The final rename keeps the install atomic for concurrent readers.
fn install_built_file(temp: &Path, destination: &Path, what: &str) -> io::Result<()> {
    let mut last_error = None;
    for attempt in 0..5 {
        match fs::rename(temp, destination) {
            Ok(()) => return Ok(()),
            Err(error) => {
                if is_non_empty_file(destination) {
                    let _ = fs::remove_file(temp);
                    return Ok(());
                }
                match copy_into_place(temp, destination) {
                    Ok(()) => {
                        let _ = fs::remove_file(temp);
                        return Ok(());
                    }
                    Err(copy_error) => {
                        if is_non_empty_file(destination) {
                            let _ = fs::remove_file(temp);
                            return Ok(());
                        }
                        last_error = Some(io::Error::new(
                            copy_error.kind(),
                            format!("{} (the rename failed first: {})", copy_error, error),
                        ));
                    }
                }
            }
        }
        if attempt < 4 {
            std::thread::sleep(std::time::Duration::from_millis(10 * (attempt + 1)));
        }
    }
    let _ = fs::remove_file(temp);
    let error = last_error.unwrap_or_else(|| io::Error::other("unknown error"));
    Err(io::Error::new(
        error.kind(),
        format!(
            "could not install the {} at {}: {}",
            what,
            destination.display(),
            error
        ),
    ))
}

/// Copy a finished file into place through a staging file in the destination
/// directory.
///
/// The copy exists for file systems a rename cannot cross. The staging file is
/// unique per process and installed with a rename, so concurrent readers only
/// ever observe a complete file.
fn copy_into_place(temp: &Path, destination: &Path) -> io::Result<()> {
    let parent = destination.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("file");
    let staging = temporary_path(parent, name);
    if let Err(error) = fs::copy(temp, &staging) {
        let _ = fs::remove_file(&staging);
        return Err(error);
    }
    match fs::rename(&staging, destination) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = fs::remove_file(&staging);
            Err(error)
        }
    }
}

/// Result of a successful native build.
#[derive(Debug, Clone)]
pub struct BuiltExecutable {
    /// Directory containing the build tree (the CMake build directory or the
    /// direct-build object and executable directory).
    pub build_dir: PathBuf,
    /// Path to the built executable.
    pub executable: PathBuf,
}

/// Options controlling one native build.
#[derive(Debug, Clone)]
pub struct BuildOptions {
    /// Build with optimizations and wrapping arithmetic instead of debug
    /// overflow checks.
    pub release: bool,
    /// Name of the generated executable (without a platform suffix).
    pub program_name: String,
    /// Print the commands that are executed.
    pub verbose: bool,
    /// Build through the generated CMake project instead of invoking the C
    /// compiler directly. The default is the direct backend, which avoids the
    /// CMake configure step.
    pub use_cmake: bool,
}

impl Default for BuildOptions {
    fn default() -> BuildOptions {
        BuildOptions {
            release: false,
            program_name: String::from("program"),
            verbose: false,
            use_cmake: false,
        }
    }
}

impl BuildOptions {
    pub fn new(program_name: impl Into<String>) -> BuildOptions {
        BuildOptions {
            program_name: program_name.into(),
            ..BuildOptions::default()
        }
    }

    pub fn release(mut self, release: bool) -> BuildOptions {
        self.release = release;
        self
    }

    pub fn verbose(mut self, verbose: bool) -> BuildOptions {
        self.verbose = verbose;
        self
    }

    /// Select the CMake back end instead of the default direct compiler back
    /// end. `lsaca --cmake` sets this.
    pub fn use_cmake(mut self, use_cmake: bool) -> BuildOptions {
        self.use_cmake = use_cmake;
        self
    }
}

/// Build a generated C project.
///
/// The project directory must already contain main.c, the runtime sources and
/// CMakeLists.txt (see [write_project]). By default the C compiler is invoked
/// directly; when [BuildOptions::use_cmake] is set the generated CMake project
/// is configured and built instead. The build tree is created in
/// project_dir/build.
pub fn build_project(project_dir: &Path, options: &BuildOptions) -> io::Result<BuiltExecutable> {
    // Resolve the project directory before deriving any child path from it.
    // Child toolchains run with the project directory as their working
    // directory and receive the scratch directory through TEMP/TMP/TMPDIR, so a
    // relative project path would be resolved twice (once by the parent, once
    // by the child) and point at a directory that does not exist.
    let project_dir = std::path::absolute(project_dir)?;
    let cache_root = project_dir.parent().unwrap_or(&project_dir).to_path_buf();
    if options.use_cmake {
        build_project_with_cmake(&project_dir, options)
    } else {
        build_project_direct(&project_dir, &cache_root, options)
    }
}

/// Configure and build a generated C project with CMake.
///
/// This is the optional back end selected by [BuildOptions::use_cmake]
/// ("lsaca --cmake"). The default back end invokes the compiler directly. The
/// build tree is created in project_dir/build.
fn build_project_with_cmake(
    project_dir: &Path,
    options: &BuildOptions,
) -> io::Result<BuiltExecutable> {
    let cmake = find_tool("cmake", &[".exe"])?.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "CMake was not found on PATH; install CMake 3.16 or newer to build lsaca programs",
        )
    })?;
    let compiler = find_c_compiler()?;
    let generator = find_generator()?;

    let build_dir = project_dir.join("build");
    fs::create_dir_all(&build_dir)?;
    // Point the child toolchain at a scratch directory inside the project, which
    // is known to be writable because the generated sources were just written
    // there. This keeps CMake, the compiler and the generator from depending on
    // the system temporary directory, which is missing in some sandboxes.
    let tmp_dir = project_dir.join("tmp");
    fs::create_dir_all(&tmp_dir)?;

    let mut configure = Command::new(&cmake);
    configure
        .arg("-S")
        .arg(project_dir)
        .arg("-B")
        .arg(&build_dir)
        .arg("-G")
        .arg(&generator)
        .arg(format!("-DCMAKE_C_COMPILER={}", compiler.display()))
        .arg(format!(
            "-DCMAKE_BUILD_TYPE={}",
            if options.release { "Release" } else { "Debug" }
        ));
    if debug_rc_requested() {
        // The generated project defines a CMake option for leak tracking in
        // every configuration; expose it through the environment so users do
        // not have to edit the generated build system.
        configure.arg("-DLSACA_DEBUG_RC=ON");
    }
    configure.current_dir(project_dir);
    prepare_child(&mut configure, compiler.parent(), &tmp_dir);
    run_tool(&mut configure, "cmake configure", options.verbose)?;

    let mut build = Command::new(&cmake);
    build
        .arg("--build")
        .arg(&build_dir)
        .arg("--parallel")
        .arg(jobs());
    build.current_dir(project_dir);
    prepare_child(&mut build, compiler.parent(), &tmp_dir);
    run_tool(&mut build, "cmake build", options.verbose)?;

    let executable = locate_executable(&build_dir, &options.program_name).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "the C compiler reported success but no executable named {} was found in {}",
                options.program_name,
                build_dir.display()
            ),
        )
    })?;

    Ok(BuiltExecutable {
        build_dir,
        executable,
    })
}

/// Compile and link a generated project by invoking the C compiler directly.
///
/// Runtime translation units are compiled once into a shared object cache that
/// lives outside the per-program build cache (see [runtime_cache_root]), so a
/// cold program build only pays for its own generated sources plus the link.
/// The runtime cost is paid only when the runtime sources, the compiler or the
/// flags change. Missing runtime objects are compiled in parallel and the
/// generated main.c is compiled at the same time; once both are ready
/// everything is linked into project_dir/build.
fn build_project_direct(
    project_dir: &Path,
    cache_root: &Path,
    options: &BuildOptions,
) -> io::Result<BuiltExecutable> {
    let compiler = find_c_compiler()?;
    let build_dir = project_dir.join("build");
    fs::create_dir_all(&build_dir)?;
    let tmp_dir = project_dir.join("tmp");
    fs::create_dir_all(&tmp_dir)?;
    let flags = compiler_flags(options);
    let started = Instant::now();

    write_runtime_sources(project_dir)?;

    let runtime_sources = runtime_c_sources();
    let object_dir = match runtime_object_dir(&compiler, &flags) {
        Ok(dir) => dir,
        // When no shared runtime cache is usable fall back to a per-build-tree
        // directory so the build still succeeds (just without reuse).
        Err(_) => cache_root
            .join("_runtime")
            .join(runtime_object_key(&compiler, &flags)),
    };
    fs::create_dir_all(&object_dir)?;

    let mut missing: Vec<(String, PathBuf)> = Vec::new();
    let mut reused = 0usize;
    for source in &runtime_sources {
        let object = object_dir.join(runtime_object_name(source));
        if is_non_empty_file(&object) {
            reused += 1;
        } else {
            missing.push(((*source).to_string(), object));
        }
    }
    // The generated sources to compile: main.c plus the extra translation
    // units of a split project (discovered on disk, because [write_project]
    // wrote them into the project directory).
    let generated_sources = generated_source_files(project_dir);
    let generated_objects: Vec<PathBuf> = generated_sources
        .iter()
        .map(|name| build_dir.join(object_name_for_source(name)))
        .collect();

    let (runtime_result, generated_result) = std::thread::scope(|scope| {
        let runtime_handle = scope.spawn(|| {
            compile_runtime_objects(&compiler, project_dir, &tmp_dir, &flags, options, &missing)
        });
        let generated_handle = scope.spawn(|| {
            compile_generated_objects(
                &compiler,
                project_dir,
                &tmp_dir,
                &flags,
                options,
                &generated_sources,
                &generated_objects,
            )
        });
        (
            join_compilation(runtime_handle),
            join_compilation(generated_handle),
        )
    });
    runtime_result?;
    generated_result?;

    let executable = build_dir.join(executable_name(&options.program_name));
    let temp_executable = temporary_path(&build_dir, &executable_name(&options.program_name));
    let mut link = Command::new(&compiler);
    for object in &generated_objects {
        link.arg(object);
    }
    for source in &runtime_sources {
        link.arg(object_dir.join(runtime_object_name(source)));
    }
    link.arg("-o").arg(&temp_executable);
    if cfg!(unix) {
        link.arg("-lm");
    }
    if cfg!(windows) {
        // The runtime uses Winsock for its networking primitives.
        link.arg("-lws2_32");
    }
    link.current_dir(project_dir);
    prepare_child(&mut link, compiler.parent(), &tmp_dir);
    run_tool(&mut link, "link executable", options.verbose)?;
    install_built_file(&temp_executable, &executable, "executable")?;
    for object in &generated_objects {
        let _ = fs::remove_file(object);
    }

    if options.verbose {
        eprintln!(
            "lsaca: build: direct compiler backend, {} runtime object(s) reused, {} compiled, {} generated translation unit(s), {:.3}s total",
            reused,
            missing.len(),
            generated_sources.len(),
            started.elapsed().as_secs_f64()
        );
    }

    if !executable.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "the C compiler reported success but no executable named {} was found in {}",
                options.program_name,
                build_dir.display()
            ),
        ));
    }
    Ok(BuiltExecutable {
        build_dir,
        executable,
    })
}

/// The generated translation units of a project: main.c followed by the extra
/// units of a split program, in deterministic lexical order.
fn generated_source_files(project_dir: &Path) -> Vec<String> {
    let mut sources = vec![String::from("main.c")];
    let Ok(entries) = fs::read_dir(project_dir) else {
        return sources;
    };
    let mut units: Vec<String> = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with("lsaca_unit_") && name.ends_with(".c") {
            units.push(name);
        }
    }
    units.sort();
    sources.extend(units);
    sources
}

/// Object file name for one generated C source (`main.c` -> `main.o`).
fn object_name_for_source(source: &str) -> String {
    let stem = source.strip_suffix(".c").unwrap_or(source);
    format!("{}.o", stem)
}

/// Compile every generated translation unit, using up to one thread per job.
fn compile_generated_objects(
    compiler: &Path,
    project_dir: &Path,
    tmp_dir: &Path,
    flags: &[String],
    options: &BuildOptions,
    sources: &[String],
    objects: &[PathBuf],
) -> io::Result<()> {
    if sources.is_empty() {
        return Ok(());
    }
    let threads = build_parallelism().min(sources.len()).max(1);
    let verbose = options.verbose;
    let mut results: Vec<io::Result<()>> = Vec::new();
    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for worker in 0..threads {
            handles.push(scope.spawn(move || -> io::Result<()> {
                for index in (worker..sources.len()).step_by(threads) {
                    compile_generated_object(
                        compiler,
                        project_dir,
                        tmp_dir,
                        flags,
                        &sources[index],
                        &objects[index],
                        verbose,
                    )?;
                }
                Ok(())
            }));
        }
        for handle in handles {
            results.push(
                handle
                    .join()
                    .unwrap_or_else(|_| Err(io::Error::other("a compilation thread panicked"))),
            );
        }
    });
    for result in results {
        result?;
    }
    Ok(())
}

/// Compile one generated translation unit into an object file.
fn compile_generated_object(
    compiler: &Path,
    project_dir: &Path,
    tmp_dir: &Path,
    flags: &[String],
    source: &str,
    object: &Path,
    verbose: bool,
) -> io::Result<()> {
    let mut command = Command::new(compiler);
    command
        .arg("-c")
        .arg(project_dir.join(source))
        .args(flags)
        .arg("-I")
        .arg(project_dir)
        .arg("-o")
        .arg(object);
    command.current_dir(project_dir);
    prepare_child(&mut command, compiler.parent(), tmp_dir);
    run_tool(&mut command, &format!("compile {}", source), verbose)
}

/// Wait for one compilation thread and turn a thread panic into an IO error.
fn join_compilation(handle: std::thread::ScopedJoinHandle<'_, io::Result<()>>) -> io::Result<()> {
    match handle.join() {
        Ok(result) => result,
        Err(_) => Err(io::Error::other("a compilation thread panicked")),
    }
}

/// Compile the runtime translation units that are not in the object cache yet,
/// using up to one thread per job for the missing files.
fn compile_runtime_objects(
    compiler: &Path,
    project_dir: &Path,
    tmp_dir: &Path,
    flags: &[String],
    options: &BuildOptions,
    missing: &[(String, PathBuf)],
) -> io::Result<()> {
    if missing.is_empty() {
        return Ok(());
    }
    let threads = build_parallelism().min(missing.len()).max(1);
    let verbose = options.verbose;
    let mut results: Vec<io::Result<()>> = Vec::new();
    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for worker in 0..threads {
            handles.push(scope.spawn(move || -> io::Result<()> {
                for index in (worker..missing.len()).step_by(threads) {
                    let (source, object) = &missing[index];
                    // Compile to a private temporary file and install it
                    // atomically: parallel test threads (and parallel
                    // compiler invocations) may miss the same object and
                    // race to build it.
                    let temp = temporary_path(tmp_dir, &runtime_object_name(source));
                    let mut command = Command::new(compiler);
                    command
                        .arg("-c")
                        .arg(project_dir.join(source))
                        .args(flags)
                        .arg("-I")
                        .arg(project_dir)
                        .arg("-o")
                        .arg(&temp);
                    command.current_dir(project_dir);
                    prepare_child(&mut command, compiler.parent(), tmp_dir);
                    run_tool(
                        &mut command,
                        &format!("compile runtime source {}", source),
                        verbose,
                    )?;
                    install_built_file(&temp, object, "runtime object")?;
                }
                Ok(())
            }));
        }
        for handle in handles {
            results.push(handle.join().unwrap_or_else(|_| {
                Err(io::Error::other("a runtime compilation thread panicked"))
            }));
        }
    });
    for result in results {
        result?;
    }
    Ok(())
}

/// Runtime translation units, in the order published by [runtime::sources].
fn runtime_c_sources() -> Vec<&'static str> {
    runtime::sources()
        .iter()
        .map(|(name, _)| *name)
        .filter(|name| name.ends_with(".c"))
        .collect()
}

/// Object file name for one runtime source file.
fn runtime_object_name(source: &str) -> String {
    let stem = Path::new(source)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(source);
    format!("{}.o", stem)
}

/// Cache key of the shared runtime object cache for one compiler and flag set.
fn runtime_object_key(compiler: &Path, flags: &[String]) -> String {
    let mut hash = FNV_OFFSET_BASIS;
    for (name, contents) in runtime::sources() {
        for bytes in [name.as_bytes(), contents.as_bytes()] {
            hash = fnv1a(hash, bytes);
            hash = fnv1a(hash, &[0xff]);
        }
    }
    hash = fnv1a(hash, compiler.as_os_str().to_string_lossy().as_bytes());
    hash = fnv1a(hash, &[0xff]);
    for flag in flags {
        hash = fnv1a(hash, flag.as_bytes());
        hash = fnv1a(hash, &[0xff]);
    }
    format!("{:016x}", hash)
}

/// Directory of the shared runtime object cache for one compiler and flag set.
///
/// The cache is deliberately independent from [cache_root]: it survives
/// clearing the per-program build cache (for example between benchmark runs),
/// so a cold program build does not recompile the standard runtime.
pub fn runtime_object_dir(compiler: &Path, flags: &[String]) -> io::Result<PathBuf> {
    let root = runtime_cache_root()?;
    Ok(root.join(runtime_object_key(compiler, flags)))
}

/// Directory for the shared compiled-runtime object cache.
///
/// Candidates, in order:
///
/// 1. LSACA_RUNTIME_CACHE_DIR, when set. An unusable value is an error
///    instead of a silent fallback, because the user asked for it explicitly;
/// 2. lsaca-runtime-objects under the system temporary directory, when that
///    path is ASCII-only and writable;
/// 3. .lsaca-runtime-cache in the current directory.
pub fn runtime_cache_root() -> io::Result<PathBuf> {
    if let Some(configured) = env::var_os("LSACA_RUNTIME_CACHE_DIR") {
        let dir = PathBuf::from(configured);
        return match probe_cache_dir(&dir) {
            Ok(()) => Ok(dir),
            Err(error) => Err(io::Error::new(
                error.kind(),
                format!(
                    "LSACA_RUNTIME_CACHE_DIR ({}) is not usable as a runtime object cache: {}",
                    dir.display(),
                    error
                ),
            )),
        };
    }
    let mut failures: Vec<String> = Vec::new();
    for candidate in runtime_cache_root_candidates() {
        match probe_cache_dir(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(error) => failures.push(format!("{}: {}", candidate.display(), error)),
        }
    }
    Err(io::Error::other(format!(
        "no writable runtime object cache found ({}); set LSACA_RUNTIME_CACHE_DIR to a writable directory",
        failures.join("; ")
    )))
}

fn runtime_cache_root_candidates() -> Vec<PathBuf> {
    let base = env::temp_dir();
    let mut candidates: Vec<PathBuf> = Vec::new();
    if is_ascii_path(&base) {
        candidates.push(base.join("lsaca-runtime-objects"));
    }
    if let Ok(cwd) = env::current_dir() {
        let local = cwd.join(".lsaca-runtime-cache");
        if !candidates.iter().any(|candidate| candidate == &local) {
            candidates.push(local);
        }
    }
    candidates
}

/// Flags shared by every translation unit of a build.
fn compiler_flags(options: &BuildOptions) -> Vec<String> {
    let mut flags: Vec<String> = ["-std=c17", "-Wall", "-Wextra"]
        .iter()
        .map(|flag| (*flag).to_string())
        .collect();
    if options.release {
        flags.push(String::from("-O2"));
        flags.push(String::from("-DNDEBUG"));
    } else {
        flags.push(String::from("-g"));
        flags.push(String::from("-O0"));
        flags.push(String::from("-DLSACA_DEBUG"));
    }
    if debug_rc_requested() {
        flags.push(String::from("-DLSACA_DEBUG_RC"));
    }
    flags
}

fn is_non_empty_file(path: &Path) -> bool {
    fs::metadata(path)
        .map(|metadata| metadata.is_file() && metadata.len() > 0)
        .unwrap_or(false)
}

/// Executable file name for the current platform.
fn executable_name(program_name: &str) -> String {
    if cfg!(windows) {
        format!("{}.exe", program_name)
    } else {
        program_name.to_string()
    }
}

/// Number of concurrent compiler invocations to use.
fn build_parallelism() -> usize {
    jobs().parse::<usize>().unwrap_or(4).clamp(1, 64)
}

/// Build a generated project inside a content-addressed cache.
///
/// The cache key covers every generated file, the runtime sources, the compiler
/// identity and the build profile, so a cached executable can be reused for an
/// identical compilation. The cache lives under cache_root and is safe to
/// delete at any time.
pub fn build_with_cache(
    generated: &Generated,
    options: &BuildOptions,
    cache_root: &Path,
) -> io::Result<BuiltExecutable> {
    // Accept relative cache roots (LSACA_CACHE_DIR may be relative) by
    // resolving them against the current directory once, up front. Every path
    // handed to a child toolchain must be absolute; see [build_project].
    let cache_root = std::path::absolute(cache_root)?;
    let compiler = find_c_compiler()?;
    let key = cache_key(generated, options, &compiler);
    let project_dir = cache_root.join(format!("{:016x}", key));
    let build_dir = project_dir.join("build");
    if let Some(executable) = locate_executable(&build_dir, &options.program_name) {
        return Ok(BuiltExecutable {
            build_dir,
            executable,
        });
    }
    write_project(&project_dir, generated).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!(
                "could not write the generated C project to {}: {}",
                project_dir.display(),
                error
            ),
        )
    })?;
    let result = if options.use_cmake {
        build_project_with_cmake(&project_dir, options)
    } else {
        build_project_direct(&project_dir, &cache_root, options)
    };
    result.map_err(|error| {
        io::Error::new(
            error.kind(),
            format!(
                "could not build the generated C project in {}: {}",
                project_dir.display(),
                error
            ),
        )
    })
}

/// Compute the content address of one generated build. Exposed for tests and
/// for the CLI, which caches builds across invocations.
pub fn cache_key(generated: &Generated, options: &BuildOptions, compiler: &Path) -> u64 {
    let mut hash = FNV_OFFSET_BASIS;
    for bytes in [
        generated.main_c.as_bytes(),
        generated.cmake.as_bytes(),
        compiler.as_os_str().to_string_lossy().as_bytes(),
        options.program_name.as_bytes(),
    ] {
        hash = fnv1a(hash, bytes);
        hash = fnv1a(hash, &[0xff]);
    }
    for unit in &generated.units {
        for bytes in [unit.file_name.as_bytes(), unit.contents.as_bytes()] {
            hash = fnv1a(hash, bytes);
            hash = fnv1a(hash, &[0xff]);
        }
    }
    for (name, contents) in runtime::sources() {
        for bytes in [name.as_bytes(), contents.as_bytes()] {
            hash = fnv1a(hash, bytes);
            hash = fnv1a(hash, &[0xff]);
        }
    }
    hash = fnv1a(
        hash,
        &[
            if options.release { 1 } else { 0 },
            if options.use_cmake { 1 } else { 0 },
            if debug_rc_requested() { 1 } else { 0 },
        ],
    );
    hash
}

/// Whether the environment asks for reference-count leak tracking in every
/// build configuration (`LSACA_DEBUG_RC`).
fn debug_rc_requested() -> bool {
    std::env::var("LSACA_DEBUG_RC")
        .map(|value| debug_rc_enabled(&value))
        .unwrap_or(false)
}

/// Parse the boolean spelling accepted by `LSACA_DEBUG_RC`.
fn debug_rc_enabled(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "on" | "yes"
    )
}

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

fn fnv1a(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Copy a built executable to destination, creating parent directories.
pub fn copy_executable(executable: &Path, destination: &Path) -> io::Result<()> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(executable, destination)?;
    Ok(())
}

/// Directory for cached build trees and intermediates.
///
/// The returned directory is created when necessary, is absolute (so child
/// toolchains can use paths derived from it) and is guaranteed to be writable.
/// The candidates are, in order:
///
/// 1. `LSACA_CACHE_DIR`, when set. An unusable value is an error instead of a
///    silent fallback, because the user asked for it explicitly;
/// 2. `lsaca-builds` under the system temporary directory, when that path is
///    ASCII-only and writable;
/// 3. `.lsaca-build-cache` in the current directory. This keeps builds working
///    in sandboxes that only allow writes inside the workspace, and when the
///    system temporary directory is not writable.
pub fn cache_root() -> io::Result<PathBuf> {
    if let Some(configured) = env::var_os("LSACA_CACHE_DIR") {
        let requested = PathBuf::from(configured);
        let dir = std::path::absolute(&requested)?;
        return match probe_cache_dir(&dir) {
            Ok(()) => Ok(dir),
            Err(error) => Err(io::Error::new(
                error.kind(),
                format!(
                    "LSACA_CACHE_DIR ({}) is not usable as a build cache: {}",
                    requested.display(),
                    error
                ),
            )),
        };
    }
    let mut failures: Vec<String> = Vec::new();
    for candidate in cache_root_candidates() {
        match probe_cache_dir(&candidate) {
            Ok(()) => return std::path::absolute(&candidate),
            Err(error) => failures.push(format!("{}: {}", candidate.display(), error)),
        }
    }
    Err(io::Error::other(format!(
        "no writable build cache directory found ({}); set LSACA_CACHE_DIR to a writable directory",
        failures.join("; ")
    )))
}

fn cache_root_candidates() -> Vec<PathBuf> {
    let base = env::temp_dir();
    let mut candidates: Vec<PathBuf> = Vec::new();
    if is_ascii_path(&base) {
        candidates.push(base.join("lsaca-builds"));
    }
    if let Ok(cwd) = env::current_dir() {
        let local = cwd.join(".lsaca-build-cache");
        if !candidates.iter().any(|candidate| candidate == &local) {
            candidates.push(local);
        }
    }
    candidates
}

/// Check that `dir` can be created and that new files can be written in it.
fn probe_cache_dir(dir: &Path) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    static PROBE_COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let probe = dir.join(format!(
        ".lsaca-write-probe-{}-{}-{}",
        process::id(),
        nanos,
        PROBE_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let result = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
        .map(|_| ());
    let _ = fs::remove_file(&probe);
    result
}

fn is_ascii_path(path: &Path) -> bool {
    path.to_string_lossy().is_ascii()
}

fn jobs() -> String {
    match env::var("LSACA_JOBS") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => env::var("NUMBER_OF_PROCESSORS").unwrap_or_else(|_| String::from("4")),
    }
}

fn run_tool(command: &mut Command, what: &str, verbose: bool) -> io::Result<()> {
    if verbose {
        eprintln!("lsaca: running {}: {:?}", what, command);
    }
    let output = command.output().map_err(|error| {
        io::Error::new(error.kind(), format!("failed to start {}: {}", what, error))
    })?;
    if verbose {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        for line in stdout.lines().chain(stderr.lines()) {
            eprintln!("lsaca:   {}", line);
        }
    }
    if output.status.success() {
        return Ok(());
    }
    let mut message = format!("{} failed with {}", what, output.status);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stdout.trim().is_empty() {
        message.push_str("\n--- stdout ---\n");
        message.push_str(stdout.trim_end());
    }
    if !stderr.trim().is_empty() {
        message.push_str("\n--- stderr ---\n");
        message.push_str(stderr.trim_end());
    }
    Err(io::Error::other(message))
}

/// Give child processes a writable temporary directory and a PATH that can find
/// the C compiler even when it was located through one of the well-known
/// fallback locations.
fn prepare_child(command: &mut Command, compiler_dir: Option<&Path>, tmp_dir: &Path) {
    command
        .env("TEMP", tmp_dir)
        .env("TMP", tmp_dir)
        .env("TMPDIR", tmp_dir);
    let Some(compiler_dir) = compiler_dir else {
        return;
    };
    let mut paths: Vec<PathBuf> = vec![compiler_dir.to_path_buf()];
    if let Some(existing) = env::var_os("PATH") {
        paths.extend(env::split_paths(&existing));
    }
    if let Ok(joined) = env::join_paths(paths) {
        command.env("PATH", joined);
    }
}

/// Find the C compiler used for native builds.
///
/// Order: LSACA_CC, gcc/cc/clang on PATH, then the usual MSYS2 installation
/// directories. The returned path is absolute whenever the compiler was found
/// through a fallback location.
pub fn find_c_compiler() -> io::Result<PathBuf> {
    if let Some(configured) = env::var_os("LSACA_CC") {
        let path = PathBuf::from(configured);
        if which_path(&path).is_some() {
            return Ok(path);
        }
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "LSACA_CC points at {}, which is not an executable file",
                path.display()
            ),
        ));
    }
    for name in ["gcc", "cc", "clang"] {
        if let Some(found) = find_tool(name, &[".exe"])? {
            return Ok(found);
        }
    }
    let mut fallbacks: Vec<PathBuf> = Vec::new();
    for root in [
        r"C:\msys64\ucrt64\bin",
        r"C:\msys64\mingw64\bin",
        r"C:\msys64\clang64\bin",
        r"C:\mingw64\bin",
        r"C:\ProgramData\chocolatey\bin",
    ] {
        fallbacks.push(PathBuf::from(root).join("gcc.exe"));
        fallbacks.push(PathBuf::from(root).join("clang.exe"));
    }
    for candidate in fallbacks {
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "no C compiler found: install GCC/Clang (for example with MSYS2) and put it on PATH, or set LSACA_CC to the compiler executable",
    ))
}

fn find_generator() -> io::Result<String> {
    if find_tool("ninja", &[".exe"])?.is_some() {
        return Ok(String::from("Ninja"));
    }
    if cfg!(windows) {
        if find_tool("mingw32-make", &[".exe"])?.is_some() {
            return Ok(String::from("MinGW Makefiles"));
        }
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "no build generator found: install Ninja (recommended) or mingw32-make and put it on PATH",
        ))
    } else {
        // POSIX systems ship a `make` compatible generator with CMake itself,
        // so Ninja is a performance choice rather than a requirement.
        Ok(String::from("Unix Makefiles"))
    }
}

/// Locate a tool by name on PATH. The returned path is absolute.
pub fn find_tool(name: &str, extensions: &[&str]) -> io::Result<Option<PathBuf>> {
    let mut names: Vec<String> = vec![name.to_string()];
    if Path::new(name).extension().is_none() {
        for extension in extensions {
            names.push(format!("{}{}", name, extension));
        }
    }
    let Some(path_var) = env::var_os("PATH") else {
        return Ok(None);
    };
    for dir in env::split_paths(&path_var) {
        for candidate in &names {
            let path = dir.join(candidate);
            if path.is_file() {
                return Ok(Some(path));
            }
        }
    }
    Ok(None)
}

fn which_path(path: &Path) -> Option<PathBuf> {
    if path.is_file() {
        return Some(path.to_path_buf());
    }
    None
}

/// Find the executable produced by the build, tolerating the directory layouts
/// used by single-config and multi-config CMake generators.
pub fn locate_executable(build_dir: &Path, program_name: &str) -> Option<PathBuf> {
    if !build_dir.is_dir() {
        return None;
    }
    let mut candidates: Vec<PathBuf> = Vec::new();
    let suffixes = if cfg!(windows) {
        [".exe", ""]
    } else {
        ["", ".exe"]
    };
    for suffix in suffixes {
        candidates.push(build_dir.join(format!("{}{}", program_name, suffix)));
        candidates.push(
            build_dir
                .join("bin")
                .join(format!("{}{}", program_name, suffix)),
        );
        candidates.push(
            build_dir
                .join("Debug")
                .join(format!("{}{}", program_name, suffix)),
        );
        candidates.push(
            build_dir
                .join("Release")
                .join(format!("{}{}", program_name, suffix)),
        );
    }
    for candidate in &candidates {
        if candidate.is_file() {
            return Some(candidate.clone());
        }
    }
    // Last resort: breadth-first search up to four levels deep. CMake sometimes
    // adds configuration subdirectories for multi-config generators.
    let mut queue: VecDeque<(PathBuf, usize)> = VecDeque::new();
    queue.push_back((build_dir.to_path_buf(), 0));
    while let Some((dir, depth)) = queue.pop_front() {
        if depth >= 4 {
            continue;
        }
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                queue.push_back((path, depth + 1));
            } else if path
                .file_name()
                .and_then(|name| name.to_str())
                .map(|name| name == program_name || name == format!("{}.exe", program_name))
                .unwrap_or(false)
            {
                return Some(path);
            }
        }
    }
    None
}

/// Make a path relative to the current directory when that is shorter and
/// unambiguous, purely for nicer diagnostics.
pub fn display_path(path: &Path) -> String {
    path.components()
        .filter(|component| !matches!(component, Component::CurDir))
        .collect::<PathBuf>()
        .display()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::GeneratedUnit;
    use crate::span::SourceLookup;

    /// Scratch directory under the workspace target directory.
    ///
    /// The tests must not depend on the system temporary directory: the
    /// compiler supports environments where child processes cannot write there,
    /// so its own tests are written to work in exactly those environments too.
    fn scratch_dir(name: &str) -> PathBuf {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/driver-test-tmp")
            .join(format!("{}-{}", process::id(), name));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("could not create the test directory");
        dir
    }

    #[test]
    fn probe_accepts_writable_directory() {
        let dir = scratch_dir("writable");
        probe_cache_dir(&dir).expect("a fresh temporary directory should be writable");
        assert!(dir.is_dir());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn probe_rejects_path_below_a_file() {
        let dir = scratch_dir("blocked");
        let file = dir.join("blocker");
        fs::write(&file, b"not a directory").expect("could not write the test file");
        assert!(probe_cache_dir(&file.join("nested")).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn cache_candidates_end_with_workspace_fallback() {
        let candidates = cache_root_candidates();
        let cwd = env::current_dir().expect("the current directory should be available");
        assert_eq!(candidates.last(), Some(&cwd.join(".lsaca-build-cache")));
    }

    #[test]
    fn runtime_cache_candidates_end_with_workspace_fallback() {
        let candidates = runtime_cache_root_candidates();
        let cwd = env::current_dir().expect("the current directory should be available");
        assert_eq!(candidates.last(), Some(&cwd.join(".lsaca-runtime-cache")));
    }

    #[test]
    fn copy_into_place_installs_complete_files() {
        let dir = scratch_dir("install-copy");
        let source_dir = dir.join("source");
        let destination_dir = dir.join("destination");
        fs::create_dir_all(&source_dir).expect("could not create the source directory");
        let source = source_dir.join("lsaca_rt.o");
        fs::write(&source, b"object bytes").expect("could not write the source file");
        let destination = destination_dir.join("nested").join("lsaca_rt.o");
        copy_into_place(&source, &destination).expect("the copy fallback should install the file");
        assert!(
            source.exists(),
            "the caller removes the source after installing"
        );
        assert_eq!(
            fs::read(&destination).expect("could not read the installed file"),
            b"object bytes"
        );
        let staging: Vec<PathBuf> =
            fs::read_dir(destination.parent().expect("the file has a parent"))
                .expect("could not list the destination directory")
                .filter_map(|entry| entry.ok())
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|extension| extension == "tmp"))
                .collect();
        assert!(staging.is_empty(), "staging files were left behind");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn runtime_object_key_covers_sources_compiler_and_flags() {
        let compiler = Path::new("gcc");
        let base = runtime_object_key(compiler, &[String::from("-O2")]);
        assert_eq!(base.len(), 16);
        assert_ne!(base, runtime_object_key(compiler, &[String::from("-O0")]));
        assert_ne!(
            base,
            runtime_object_key(Path::new("clang"), &[String::from("-O2")])
        );
    }

    fn sample_generated() -> Generated {
        Generated {
            main_c: String::from("int main(void) { return 0; }\n"),
            cmake: String::from("project(sample)\n"),
            units: Vec::new(),
        }
    }

    #[test]
    fn discovers_split_translation_units_in_order() {
        let dir = scratch_dir("units");
        for name in ["main.c", "lsaca_unit_2.c", "lsaca_unit_1.c", "notes.txt"] {
            fs::write(dir.join(name), b"/* generated */\n").expect("could not write a source file");
        }
        let sources = generated_source_files(&dir);
        assert_eq!(
            sources,
            vec![
                String::from("main.c"),
                String::from("lsaca_unit_1.c"),
                String::from("lsaca_unit_2.c"),
            ]
        );
        assert_eq!(object_name_for_source("main.c"), "main.o");
        assert_eq!(object_name_for_source("lsaca_unit_1.c"), "lsaca_unit_1.o");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn cache_key_covers_split_translation_units() {
        let compiler = PathBuf::from("cc");
        let options = BuildOptions::new("sample");
        let single = sample_generated();
        let plain_key = cache_key(&single, &options, &compiler);
        let mut split = single.clone();
        split.units.push(GeneratedUnit {
            file_name: String::from("lsaca_unit_1.c"),
            contents: String::from("void lsaca_fn_extra(void) {}\n"),
        });
        let split_key = cache_key(&split, &options, &compiler);
        assert_ne!(plain_key, split_key);
        split.units[0].contents.push_str("/* changed */\n");
        assert_ne!(split_key, cache_key(&split, &options, &compiler));
    }

    #[test]
    fn write_project_writes_every_translation_unit() {
        let dir = scratch_dir("project");
        let mut generated = sample_generated();
        generated.units.push(GeneratedUnit {
            file_name: String::from("lsaca_unit_1.c"),
            contents: String::from("void lsaca_fn_extra(void) {}\n"),
        });
        write_project(&dir, &generated).expect("the project should be written");
        assert!(dir.join("main.c").is_file());
        assert!(dir.join("lsaca_unit_1.c").is_file());
        assert_eq!(
            fs::read_to_string(dir.join("lsaca_unit_1.c")).expect("unit should be readable"),
            "void lsaca_fn_extra(void) {}\n"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn parses_the_debug_rc_switch() {
        for enabled in ["1", "true", "ON", " yes "] {
            assert!(
                debug_rc_enabled(enabled),
                "{enabled:?} should enable tracking"
            );
        }
        for disabled in ["", "0", "false", "off", "no", "2"] {
            assert!(
                !debug_rc_enabled(disabled),
                "{disabled:?} should disable tracking"
            );
        }
    }

    /// Write one file below a scratch directory, creating parents as needed.
    fn write_scratch(dir: &Path, relative: &str, contents: &str) {
        let path = dir.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("could not create a test directory");
        }
        fs::write(&path, contents).expect("could not write a test source file");
    }

    /// Number of source files contributed by the user program.
    ///
    /// Every compilation also registers the embedded standard library as a set
    /// of std/<path> files, so assertions about the user's own files have to
    /// subtract those.
    fn user_source_count(frontend: &Frontend) -> usize {
        frontend.sources().len() - crate::stdlib::files().len()
    }

    /// Load a program and return every diagnostic rendered for assertions.
    fn load_and_render(root: &Path) -> Result<Frontend, String> {
        match Frontend::load(root) {
            Ok(frontend) => match frontend.check() {
                Ok(_) => Ok(frontend),
                Err(diagnostics) => Err(diagnostics.render(frontend.sources())),
            },
            Err(error) => Err(error.to_string()),
        }
    }

    #[test]
    fn loads_external_module_files() {
        let dir = scratch_dir("modules-ok");
        write_scratch(&dir, "main.ls", "mod helper;\nfn main() { }\n");
        write_scratch(&dir, "helper.ls", "pub fn helper() -> i64 { 7 }\n");
        let frontend = Frontend::load(&dir.join("main.ls")).expect("the root file should load");
        assert_eq!(user_source_count(&frontend), 2, "both files are registered");
        let root = frontend.graph().root_crate();
        let module = root
            .program
            .items
            .iter()
            .find_map(|item| match item {
                ast::Item::Mod(decl) => Some(decl),
                _ => None,
            })
            .expect("the module declaration survives loading");
        assert_eq!(module.name, "helper");
        assert!(!module.external, "external modules are rewritten inline");
        assert_eq!(module.items.len(), 1, "the module body is loaded");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn loads_nested_modules_relative_to_the_module_directory() {
        let dir = scratch_dir("modules-nested");
        write_scratch(&dir, "main.ls", "mod util;\nfn main() { }\n");
        write_scratch(&dir, "util/mod.ls", "pub mod math;\n");
        write_scratch(&dir, "util/math.ls", "pub fn value() -> i64 { 3 }\n");
        let frontend =
            Frontend::load(&dir.join("main.ls")).expect("the nested module tree should load");
        assert_eq!(user_source_count(&frontend), 3, "every file is registered");
        let root = frontend.graph().root_crate();
        let util = root
            .program
            .items
            .iter()
            .find_map(|item| match item {
                ast::Item::Mod(decl) => Some(decl),
                _ => None,
            })
            .expect("the module declaration survives loading");
        assert_eq!(util.name, "util");
        let math = util
            .items
            .iter()
            .find_map(|item| match item {
                ast::Item::Mod(decl) => Some(decl),
                _ => None,
            })
            .expect("the nested module is loaded");
        assert_eq!(math.name, "math");
        assert_eq!(math.items.len(), 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_module_files_are_reported() {
        let dir = scratch_dir("modules-missing");
        write_scratch(&dir, "main.ls", "mod absent;\nfn main() { }\n");
        let rendered = load_and_render(&dir.join("main.ls")).expect_err("the module is missing");
        assert!(
            rendered.contains("file not found for module"),
            "{}",
            rendered
        );
        assert!(rendered.contains("not found"), "{}", rendered);
        assert!(rendered.contains("E0583"), "{}", rendered);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn ambiguous_module_files_are_reported() {
        let dir = scratch_dir("modules-ambiguous");
        write_scratch(&dir, "main.ls", "mod helper;\nfn main() { }\n");
        write_scratch(&dir, "helper.ls", "pub fn helper() {}\n");
        write_scratch(&dir, "helper/mod.ls", "pub fn helper() {}\n");
        let rendered = load_and_render(&dir.join("main.ls")).expect_err("both files exist");
        assert!(rendered.contains("E0761"), "{}", rendered);
        assert!(rendered.contains("found at both"), "{}", rendered);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn circular_module_files_are_reported() {
        let dir = scratch_dir("modules-circular");
        // The module directory of a crate root file is its own directory, so
        // "mod main;" inside main.ls asks for main.ls again: a cycle.
        write_scratch(&dir, "main.ls", "mod main;\nfn main() { }\n");
        let rendered = load_and_render(&dir.join("main.ls")).expect_err("the modules are circular");
        assert!(rendered.contains("circular module files"), "{}", rendered);
        assert!(rendered.contains("E0001"), "{}", rendered);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn duplicate_module_declarations_are_reported() {
        let dir = scratch_dir("modules-duplicate");
        write_scratch(&dir, "main.ls", "mod helper;\nmod helper;\nfn main() { }\n");
        write_scratch(&dir, "helper.ls", "pub fn helper() {}\n");
        let rendered =
            load_and_render(&dir.join("main.ls")).expect_err("the module is declared twice");
        assert!(rendered.contains("E0428"), "{}", rendered);
        assert!(rendered.contains("defined multiple times"), "{}", rendered);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn in_memory_sources_cannot_load_modules() {
        let frontend = Frontend::new("memory.ls", "mod helper;\nfn main() { }\n");
        let diagnostics = match frontend.check() {
            Ok(_) => panic!("an external module cannot be loaded from memory"),
            Err(diagnostics) => diagnostics,
        };
        let rendered = diagnostics.render(frontend.sources());
        assert!(rendered.contains("E0583"), "{}", rendered);
        assert!(rendered.contains("in-memory"), "{}", rendered);
    }

    #[test]
    fn rejects_invalid_utf8_sources() {
        let dir = scratch_dir("modules-utf8");
        write_scratch(&dir, "main.ls", "fn main() { }\n");
        let frontend = Frontend::load(&dir.join("main.ls")).expect("the root file is valid");
        assert_eq!(user_source_count(&frontend), 1);
        fs::write(dir.join("main2.ls"), b"mod bad;\n").expect("could not write a root file");
        fs::write(dir.join("bad.ls"), [0xffu8, 0xfe]).expect("could not write raw bytes");
        let rendered =
            load_and_render(&dir.join("main2.ls")).expect_err("the module file is not UTF-8");
        assert!(rendered.contains("could not read module"), "{}", rendered);
        fs::write(dir.join("invalid_root.ls"), [0xffu8, 0xfe]).expect("could not write raw bytes");
        let error = Frontend::load(&dir.join("invalid_root.ls"))
            .expect_err("an invalid UTF-8 root file is an IO error");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn crate_names_are_sanitized() {
        let frontend = Frontend::new("Weird-Name 9.ls", "fn main() { }\n");
        assert_eq!(frontend.graph().root_crate().name, "weird_name_9");
        let fallback = Frontend::new("---.ls", "fn main() { }\n");
        assert_eq!(fallback.graph().root_crate().name, "program");
    }
    #[test]
    fn source_names_are_relative_and_have_no_windows_prefix() {
        let dir = scratch_dir("display-names");
        write_scratch(&dir, "main.ls", "mod helper;\nfn main() { }\n");
        write_scratch(&dir, "helper.ls", "pub fn helper() {}\n");
        let frontend = Frontend::load(&dir.join("main.ls")).expect("the program should load");
        assert_eq!(user_source_count(&frontend), 2);
        for index in 0..frontend.sources().len() {
            let name = frontend.sources().file(index).name();
            assert!(!name.contains("//?/"), "verbatim prefix in {name}");
            assert!(!name.contains('\\'), "backslashes in {name}");
            assert!(name.ends_with(".ls"), "unexpected name {name}");
        }
        assert!(
            frontend
                .sources()
                .file(0)
                .name()
                .contains("driver-test-tmp"),
            "{} should be relative",
            frontend.sources().file(0).name()
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn standard_library_spans_resolve_in_the_user_source_map() {
        let frontend = Frontend::new("main.ls", "fn main() { }\n");
        let library = crate::stdlib::program().expect("the standard library must parse");
        let mut resolved = 0;
        for item in &library.items {
            let file = frontend
                .sources()
                .file_of_span(item.span())
                .expect("every standard-library item has a file");
            assert!(
                file.name().starts_with("std/"),
                "unexpected standard library file name {}",
                file.name()
            );
            resolved += 1;
        }
        assert!(resolved > 0, "the standard library has items");
        let root = frontend.graph().root_crate();
        let item = root.program.items.first().expect("main is an item");
        let file = frontend
            .sources()
            .file_of_span(item.span())
            .expect("user items resolve in the merged map");
        assert_eq!(file.name(), "main.ls");
    }

    fn package_case_dir(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/packages")
            .join(name)
    }

    #[test]
    fn loads_a_package_with_a_path_dependency() {
        let frontend =
            Frontend::load_package(&package_case_dir("with_dep")).expect("the package should load");
        let names: Vec<&str> = frontend
            .graph()
            .crates
            .iter()
            .map(|krate| krate.name.as_str())
            .collect();
        assert_eq!(names, vec!["greeting", "with_dep"]);
        assert!(frontend.graph().crates[0].is_dependency);
        assert!(!frontend.graph().crates[1].is_dependency);
        assert_eq!(frontend.graph().root, 1);
        assert!(frontend.has_main());
        // Two crate roots plus the two manifests the resolver registers so
        // manifest diagnostics can render.
        assert_eq!(
            user_source_count(&frontend),
            4,
            "two manifests and two crate roots"
        );
        let roots: Vec<&str> = frontend
            .graph()
            .crates
            .iter()
            .map(|krate| {
                frontend
                    .sources()
                    .file_of_span(krate.program.items[0].span())
                    .map_or("", crate::span::SourceFile::name)
            })
            .collect();
        assert!(
            roots
                .iter()
                .any(|name| name.ends_with("greeting/src/lib.ls")),
            "{roots:?}"
        );
    }

    #[test]
    fn loads_a_library_package_without_main() {
        let frontend = Frontend::load_package(&package_case_dir("library_only"))
            .expect("a library package should load");
        assert!(!frontend.has_main());
        assert_eq!(frontend.graph().crates.len(), 1);
        assert_eq!(frontend.graph().root_crate().name, "library_only");
    }

    #[test]
    fn missing_manifests_are_not_found() {
        let dir = scratch_dir("no-manifest");
        let error = Frontend::load_package(&dir).expect_err("a package needs a manifest");
        match error {
            LoadError::Io(io_error) => {
                assert_eq!(io_error.kind(), io::ErrorKind::NotFound);
                assert!(io_error.to_string().contains("no lsaca.toml"), "{io_error}");
            }
            other => panic!("expected an IO error, got {other:?}"),
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn malformed_manifests_are_reported() {
        let error = Frontend::load_package(&package_case_dir("bad_manifest"))
            .expect_err("the manifest is malformed");
        let rendered = error.render();
        assert!(rendered.contains("E0001"), "{rendered}");
        assert!(rendered.contains("name"), "{rendered}");
    }

    #[test]
    fn dependency_errors_are_reported() {
        let error = Frontend::load_package(&package_case_dir("missing_dep"))
            .expect_err("the dependency is missing");
        let rendered = error.render();
        assert!(rendered.contains("E0001"), "{rendered}");
        assert!(rendered.contains("does_not_exist"), "{rendered}");
        let error = Frontend::load_package(&package_case_dir("cycle_a"))
            .expect_err("the dependency cycle is rejected");
        let rendered = error.render();
        assert!(rendered.contains("cyclic package dependency"), "{rendered}");
    }

    #[test]
    fn packages_cannot_be_both_a_binary_and_a_library() {
        let error = Frontend::load_package(&package_case_dir("both_roots"))
            .expect_err("a package with two roots must be rejected");
        let rendered = error.render();
        assert!(rendered.contains("E0001"), "{rendered}");
        assert!(rendered.contains("src/main.ls"), "{rendered}");
        assert!(rendered.contains("src/lib.ls"), "{rendered}");
    }
}
