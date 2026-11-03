//! Package manifests and dependency resolution (v1.7).
//!
//! The compiler discovers the root `lsaca.toml`, parses its dependency table
//! (path dependencies only, no registry), and resolves the full package graph
//! before any source file is loaded. This module owns the manifest format and
//! the resolver; the driver turns each package into one crate.
//!
//! The public interface is frozen in the v1.7 design document:
//! [PackageInfo], [Dependency], [Manifest], [Package], [PackageGraph],
//! [discover_manifest], [parse_manifest], [resolve].
//!
//! Every manifest is registered in the [`SourceMap`], so manifest diagnostics render
//! with a `file:line:column` location, and every manifest problem is an
//! `E0001` error. The manifest format is the strict TOML subset from the design:
//!
//! ```toml
//! [package]
//! name = "example"
//! version = "0.1.0"
//!
//! [dependencies]
//! util = { path = "../util" }
//! ```

#![allow(dead_code)]

use std::collections::HashMap;
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::diag::{Diagnostic, Diagnostics};
use crate::span::{SourceMap, Span};

/// File name of a package manifest.
const MANIFEST_FILE: &str = "lsaca.toml";

/// Version used when the `[package]` table has no `version` key.
const DEFAULT_VERSION: &str = "0.1.0";

/// Diagnostic code used for every manifest problem.
const MANIFEST_ERROR_CODE: &str = "E0001";

/// Language keywords. A package or dependency name must not be one of these.
const KEYWORDS: &[&str] = &[
    "as", "break", "const", "continue", "else", "enum", "false", "fn", "for", "if", "impl", "in",
    "let", "loop", "match", "move", "mut", "pub", "return", "self", "struct", "trait", "true",
    "type", "while",
];

/// The `[package]` table of a manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageInfo {
    /// Package name: a valid, lowercase lsaca identifier.
    pub name: String,
    /// Semantic version string, for example `0.1.0`.
    pub version: String,
}

/// One entry of the `[dependencies]` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependency {
    /// Name the manifest uses to refer to the dependency.
    pub name: String,
    /// Canonicalized path of the dependency package directory.
    pub path: PathBuf,
    /// Span of the dependency declaration inside the manifest source.
    pub span: Span,
}

/// A parsed `lsaca.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    /// Path of the manifest file itself.
    pub path: PathBuf,
    /// Directory containing the manifest.
    pub dir: PathBuf,
    /// The `[package]` table.
    pub package: PackageInfo,
    /// Dependencies in declaration order.
    pub dependencies: Vec<Dependency>,
}

/// One package of the resolved graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    /// Package name.
    pub name: String,
    /// Package version.
    pub version: String,
    /// Canonical package directory.
    pub dir: PathBuf,
    /// The parsed manifest of the package.
    pub manifest: Manifest,
    /// True for packages other than the root package.
    pub is_dependency: bool,
    /// Indices of direct dependencies in [PackageGraph::packages].
    pub dependencies: Vec<usize>,
}

/// The resolved package graph: dependencies first, root package last.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageGraph {
    /// Packages in dependency-first order; the root package is last.
    pub packages: Vec<Package>,
    /// Index of the root package in [PackageGraph::packages].
    pub root: usize,
}

/// Find `lsaca.toml` in `start_dir` or its nearest ancestor.
///
/// The search starts in `start_dir` itself and walks up one directory at a
/// time. The returned path is canonicalized so it can be compared with the
/// paths stored in a [Manifest].
pub fn discover_manifest(start_dir: &Path) -> io::Result<Option<PathBuf>> {
    let mut directory = if start_dir.is_absolute() {
        start_dir.to_path_buf()
    } else {
        env::current_dir()?.join(start_dir)
    };
    loop {
        let candidate = directory.join(MANIFEST_FILE);
        match fs::metadata(&candidate) {
            Ok(metadata) => {
                if metadata.is_file() {
                    return Ok(Some(canonicalize_path(&candidate)));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        match directory.parent() {
            Some(parent) => directory = parent.to_path_buf(),
            None => return Ok(None),
        }
    }
}

/// Read and validate one manifest, reporting problems as diagnostics.
///
/// The manifest text is registered in `sources` so diagnostics can render with
/// line/column snippets; returns `None` when the manifest is invalid.
pub fn parse_manifest(
    path: &Path,
    sources: &mut SourceMap,
    diagnostics: &mut Diagnostics,
) -> Option<Manifest> {
    let errors_before = diagnostics.error_count();
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            let display = display_path(path);
            let file_index = sources.add_file(display.clone(), String::new());
            let base = sources.file(file_index).base();
            diagnostics.push(manifest_error(
                format!("could not read package manifest `{display}`: {error}"),
                Span::empty(base),
            ));
            return None;
        }
    };
    let canonical = canonicalize_path(path);
    let file_index = sources.add_file(display_path(&canonical), text.clone());
    let base = sources.file(file_index).base();
    let parsed = ManifestParser::new(&text, base, errors_before, diagnostics).run();
    if diagnostics.error_count() > errors_before {
        return None;
    }
    let (package, mut dependencies) = parsed?;
    let dir = canonical
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    for dependency in &mut dependencies {
        dependency.path = canonicalize_path(&dir.join(&dependency.path));
    }
    Some(Manifest {
        path: canonical,
        dir,
        package,
        dependencies,
    })
}

/// Resolve the whole graph from the root manifest.
///
/// Dependencies are parsed recursively; dependency paths are resolved relative
/// to each manifest directory and canonicalized. The graph order is a
/// deterministic depth-first post-order from the root with dependencies
/// visited in manifest declaration order, so the root package is last.
/// Returns `None` when any diagnostic error was produced.
pub fn resolve(
    root_manifest: &Path,
    sources: &mut SourceMap,
    diagnostics: &mut Diagnostics,
) -> Option<PackageGraph> {
    Resolver::new(sources, diagnostics).run(root_manifest)
}

/// Canonical path, falling back to the input when the file system cannot
/// canonicalize it (for example when the path does not exist).
fn canonicalize_path(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Display form of a path used for [SourceMap] file names: relative to the
/// current directory when possible and always with forward slashes.
fn display_path(path: &Path) -> String {
    let relative = relative_to_cwd(path).unwrap_or_else(|| path.to_path_buf());
    let text = relative.to_string_lossy().replace('\\', "/");
    if let Some(stripped) = text.strip_prefix("//?/") {
        return stripped.to_string();
    }
    text
}

/// Best-effort relative form of `path` against the current directory.
fn relative_to_cwd(path: &Path) -> Option<PathBuf> {
    let cwd = env::current_dir().ok()?;
    let canonical_cwd = fs::canonicalize(&cwd).ok();
    if let Some(relative) = canonical_cwd
        .as_deref()
        .and_then(|base| path.strip_prefix(base).ok())
    {
        return Some(relative.to_path_buf());
    }
    path.strip_prefix(&cwd).ok().map(Path::to_path_buf)
}

/// Build an `E0001` diagnostic.
fn manifest_error(message: impl Into<String>, span: Span) -> Diagnostic {
    Diagnostic::error(message, span).with_code(MANIFEST_ERROR_CODE)
}

/// Whether `name` is a valid package or dependency name: a lowercase ASCII
/// letter or `_` followed by lowercase ASCII letters, digits and `_`, not a
/// language keyword and not the bare `_` placeholder.
fn valid_package_name(name: &str) -> bool {
    let mut characters = name.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    if !(first.is_ascii_lowercase() || first == '_') {
        return false;
    }
    if name == "_" {
        return false;
    }
    characters.all(|character| {
        character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
    }) && !KEYWORDS.contains(&name)
}

/// Whether `version` is three dot-separated integers such as `0.1.0`.
fn valid_version(version: &str) -> bool {
    let mut parts = version.split('.');
    let major = parts.next();
    let minor = parts.next();
    let patch = parts.next();
    let extra = parts.next();
    let (Some(major), Some(minor), Some(patch), None) = (major, minor, patch, extra) else {
        return false;
    };
    [major, minor, patch]
        .iter()
        .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}

/// Which table the manifest scanner is currently reading.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum CurrentTable {
    /// No table header was read yet.
    #[default]
    None,
    /// Inside `[package]`.
    Package,
    /// Inside `[dependencies]`.
    Dependencies,
    /// Inside a table that was already rejected as unknown; its keys are
    /// skipped so one bad table produces one diagnostic.
    Other,
}

/// A decoded string value of the manifest.
struct StringValue {
    /// Decoded contents (escapes resolved).
    content: String,
    /// Local start offset of the raw contents inside the manifest text.
    start: usize,
    /// Local end offset of the raw contents inside the manifest text.
    end: usize,
    /// Local offset just past the closing quote.
    after: usize,
}

/// A decoded `{ path = "..." }` dependency value.
struct PathValue {
    /// Decoded path string.
    path: String,
    /// Local start offset of the raw path contents.
    start: usize,
    /// Local end offset of the raw path contents.
    end: usize,
    /// Local offset just past the closing `}`.
    after: usize,
}

/// Line-oriented parser for the strict manifest TOML subset.
struct ManifestParser<'a> {
    text: &'a str,
    base: u32,
    errors_before: usize,
    diagnostics: &'a mut Diagnostics,
    current: CurrentTable,
    seen_package: bool,
    seen_dependencies: bool,
    package_header: Option<Span>,
    name_seen: bool,
    name: Option<String>,
    version_seen: bool,
    version: Option<String>,
    dependencies: Vec<Dependency>,
}

impl<'a> ManifestParser<'a> {
    fn new(
        text: &'a str,
        base: u32,
        errors_before: usize,
        diagnostics: &'a mut Diagnostics,
    ) -> ManifestParser<'a> {
        ManifestParser {
            text,
            base,
            errors_before,
            diagnostics,
            current: CurrentTable::None,
            seen_package: false,
            seen_dependencies: false,
            package_header: None,
            name_seen: false,
            name: None,
            version_seen: false,
            version: None,
            dependencies: Vec::new(),
        }
    }

    /// Parse the whole manifest and build its package metadata.
    fn run(mut self) -> Option<(PackageInfo, Vec<Dependency>)> {
        self.parse();
        self.finish()
    }

    /// Scan every line of the manifest.
    fn parse(&mut self) {
        let mut line_start = 0usize;
        loop {
            let line_end = match self.text[line_start..].find('\n') {
                Some(offset) => line_start + offset,
                None => self.text.len(),
            };
            self.parse_line(line_start, line_end);
            if line_end >= self.text.len() {
                break;
            }
            line_start = line_end + 1;
        }
    }

    /// Parse one line (without its trailing newline).
    fn parse_line(&mut self, line_start: usize, line_end: usize) {
        let mut end = line_end;
        while end > line_start && self.text.as_bytes()[end - 1] == b'\r' {
            end -= 1;
        }
        let mut start = line_start;
        while start < end && matches!(self.text.as_bytes()[start], b' ' | b'\t') {
            start += 1;
        }
        if start >= end || self.text.as_bytes()[start] == b'#' {
            return;
        }
        if self.text.as_bytes()[start] == b'[' {
            self.parse_table_header(start, end);
        } else {
            self.parse_key_value(start, end);
        }
    }

    /// Parse a `[package]` or `[dependencies]` header.
    fn parse_table_header(&mut self, start: usize, end: usize) {
        let Some(close_offset) = self.text[start..end].find(']') else {
            self.error("malformed table header: missing `]`", start, end);
            return;
        };
        let close = start + close_offset;
        let inner = &self.text[start + 1..close];
        let name = inner.trim().to_string();
        let name_start = start + 1 + (inner.len() - inner.trim_start().len());
        let name_end = name_start + name.len();
        self.check_trailing_after(close + 1, end, "table header");
        if name.is_empty() {
            self.error("empty table header", start, close + 1);
            return;
        }
        if !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            let message = format!("malformed table header `[{name}]`");
            self.error(message, name_start, name_end);
            self.current = CurrentTable::Other;
            return;
        }
        match name.as_str() {
            "package" => {
                if self.seen_package {
                    self.error(
                        "the table `[package]` is declared more than once",
                        name_start,
                        name_end,
                    );
                }
                self.seen_package = true;
                if self.package_header.is_none() {
                    self.package_header = Some(self.span(start, close + 1));
                }
                self.current = CurrentTable::Package;
            }
            "dependencies" => {
                if self.seen_dependencies {
                    self.error(
                        "the table `[dependencies]` is declared more than once",
                        name_start,
                        name_end,
                    );
                }
                self.seen_dependencies = true;
                self.current = CurrentTable::Dependencies;
            }
            _ => {
                let message =
                    format!("unknown section `[{name}]`; expected `[package]` or `[dependencies]`");
                self.error(message, name_start, name_end);
                self.current = CurrentTable::Other;
            }
        }
    }

    /// Parse one `key = value` line.
    fn parse_key_value(&mut self, start: usize, end: usize) {
        let mut key_end = start;
        while key_end < end {
            let byte = self.text.as_bytes()[key_end];
            if byte.is_ascii_alphanumeric() || byte == b'_' {
                key_end += 1;
            } else {
                break;
            }
        }
        if key_end == start {
            self.error("expected a key such as `name`", start, (start + 1).min(end));
            return;
        }
        let key = self.text[start..key_end].to_string();
        if self.current == CurrentTable::None {
            self.error(
                "manifest settings must appear inside a `[package]` or `[dependencies]` table",
                start,
                key_end,
            );
            return;
        }
        if self.current == CurrentTable::Other {
            return;
        }
        let mut position = self.skip_ws(key_end, end);
        if position >= end || self.text.as_bytes()[position] != b'=' {
            let message = format!("expected `=` after the key `{key}`");
            self.error(message, position.min(end), (position + 1).min(end));
            return;
        }
        position = self.skip_ws(position + 1, end);
        if position >= end {
            let message = format!("missing value after `{key} =`");
            self.error(message, position, position);
            return;
        }
        if self.current == CurrentTable::Package {
            self.parse_package_entry(&key, (start, key_end), position, end);
        } else {
            self.parse_dependency_entry(&key, (start, key_end), position, end);
        }
    }

    /// Parse a key of the `[package]` table.
    fn parse_package_entry(
        &mut self,
        key: &str,
        key_span: (usize, usize),
        value_start: usize,
        end: usize,
    ) {
        if !matches!(key, "name" | "version") {
            let message =
                format!("unknown key `{key}` in `[package]`; expected `name` or `version`");
            self.error(message, key_span.0, key_span.1);
            return;
        }
        let Some(value) = self.parse_string(value_start, end) else {
            return;
        };
        self.check_trailing_after(value.after, end, "value");
        match key {
            "name" => {
                if self.name_seen {
                    self.error(
                        "duplicate key `name` in the `[package]` table",
                        key_span.0,
                        key_span.1,
                    );
                    return;
                }
                self.name_seen = true;
                if value.content == "std" {
                    self.error(
                        "`std` is reserved for the standard library and cannot be a package name",
                        value.start,
                        value.end,
                    );
                } else if !valid_package_name(&value.content) {
                    let message = if KEYWORDS.contains(&value.content.as_str()) {
                        format!(
                            "`{}` is a keyword and cannot be a package name",
                            value.content
                        )
                    } else {
                        format!(
                            "invalid package name `{}`: package names start with a lowercase \
                             letter or `_` and contain only lowercase ASCII letters, digits and `_`",
                            value.content
                        )
                    };
                    self.error(message, value.start, value.end);
                }
                self.name = Some(value.content);
            }
            "version" => {
                if self.version_seen {
                    self.error(
                        "duplicate key `version` in the `[package]` table",
                        key_span.0,
                        key_span.1,
                    );
                    return;
                }
                self.version_seen = true;
                if !valid_version(&value.content) {
                    let message = format!(
                        "invalid version `{}`: expected three dot-separated integers such as \
                         `0.1.0`",
                        value.content
                    );
                    self.error(message, value.start, value.end);
                }
                self.version = Some(value.content);
            }
            _ => {}
        }
    }

    /// Parse one entry of the `[dependencies]` table.
    fn parse_dependency_entry(
        &mut self,
        key: &str,
        key_span: (usize, usize),
        value_start: usize,
        end: usize,
    ) {
        let duplicate = self
            .dependencies
            .iter()
            .any(|dependency| dependency.name == key);
        if key == "std" {
            self.error(
                "`std` is reserved for the standard library and cannot be a dependency name",
                key_span.0,
                key_span.1,
            );
        } else if !valid_package_name(key) {
            let message = if KEYWORDS.contains(&key) {
                format!("`{key}` is a keyword and cannot be a dependency name")
            } else {
                format!(
                    "invalid dependency name `{key}`: dependency names start with a lowercase \
                     letter or `_` and contain only lowercase ASCII letters, digits and `_`"
                )
            };
            self.error(message, key_span.0, key_span.1);
        }
        if duplicate {
            let message = format!("duplicate dependency `{key}` in `[dependencies]`");
            self.error(message, key_span.0, key_span.1);
        }
        let Some(value) = self.parse_path_table(value_start, end) else {
            return;
        };
        self.check_trailing_after(value.after, end, "dependency table");
        if duplicate {
            return;
        }
        let span = self.span(value.start, value.end);
        self.dependencies.push(Dependency {
            name: key.to_string(),
            path: PathBuf::from(value.path),
            span,
        });
    }

    /// Parse a `{ path = "..." }` inline table.
    fn parse_path_table(&mut self, value_start: usize, end: usize) -> Option<PathValue> {
        let mut position = self.skip_ws(value_start, end);
        if position >= end || self.text.as_bytes()[position] != b'{' {
            self.error(
                "malformed dependency: expected the inline table `{ path = \"...\" }`",
                value_start,
                end,
            );
            return None;
        }
        position = self.skip_ws(position + 1, end);
        let key_start = position;
        while position < end {
            let byte = self.text.as_bytes()[position];
            if byte.is_ascii_alphanumeric() || byte == b'_' {
                position += 1;
            } else {
                break;
            }
        }
        let key = self.text[key_start..position].to_string();
        if key != "path" {
            let message = if key.is_empty() {
                "malformed dependency: expected `path` in the inline table".to_string()
            } else {
                format!(
                    "malformed dependency: unknown key `{key}` in the inline table; expected `path`"
                )
            };
            self.error(message, value_start, end);
            return None;
        }
        position = self.skip_ws(position, end);
        if position >= end || self.text.as_bytes()[position] != b'=' {
            self.error(
                "malformed dependency: expected `=` after `path`",
                value_start,
                end,
            );
            return None;
        }
        position = self.skip_ws(position + 1, end);
        let value = self.parse_string(position, end)?;
        position = self.skip_ws(value.after, end);
        if position >= end || self.text.as_bytes()[position] != b'}' {
            self.error(
                "malformed dependency: expected `}` to close the inline table",
                value_start,
                end,
            );
            return None;
        }
        Some(PathValue {
            path: value.content,
            start: value.start,
            end: value.end,
            after: position + 1,
        })
    }

    /// Parse a double-quoted string starting at `start`.
    fn parse_string(&mut self, start: usize, end: usize) -> Option<StringValue> {
        if start >= end {
            self.error("expected a double-quoted string value", start, end);
            return None;
        }
        if self.text.as_bytes()[start] == b'\'' {
            self.error(
                "single-quoted strings are not supported; use double quotes",
                start,
                (start + 1).min(end),
            );
            return None;
        }
        if self.text.as_bytes()[start] != b'"' {
            let token_end = self.token_end(start, end);
            self.error("expected a double-quoted string value", start, token_end);
            return None;
        }
        let content_start = start + 1;
        let mut position = content_start;
        let mut content = String::new();
        while position < end {
            let byte = self.text.as_bytes()[position];
            if byte == b'"' {
                return Some(StringValue {
                    content,
                    start: content_start,
                    end: position,
                    after: position + 1,
                });
            }
            if byte == b'\\' {
                if position + 1 >= end {
                    break;
                }
                match self.text.as_bytes()[position + 1] {
                    b'"' => {
                        content.push('"');
                        position += 2;
                    }
                    b'\\' => {
                        content.push('\\');
                        position += 2;
                    }
                    b'n' => {
                        content.push('\n');
                        position += 2;
                    }
                    b't' => {
                        content.push('\t');
                        position += 2;
                    }
                    _ => {
                        let escape_end = self.char_after(position + 1, end);
                        let escape = &self.text[position + 1..escape_end];
                        let message =
                            format!("unsupported escape sequence `\\{escape}` in a string");
                        self.error(message, position, escape_end);
                        position = escape_end;
                    }
                }
                continue;
            }
            let character_end = self.char_after(position, end);
            if let Some(character) = self.text[position..character_end].chars().next() {
                content.push(character);
            }
            position = character_end;
        }
        self.error("unterminated string: missing the closing `\"`", start, end);
        None
    }

    /// Reject non-comment text after a token.
    fn check_trailing_after(&mut self, start: usize, end: usize, context: &str) {
        let position = self.skip_ws(start, end);
        if position < end && self.text.as_bytes()[position] != b'#' {
            let message = format!("unexpected characters after the {context}");
            self.error(message, position, end);
        }
    }

    /// Local position just past the first character at `offset`.
    fn char_after(&self, offset: usize, end: usize) -> usize {
        if offset >= end {
            return end;
        }
        self.text[offset..end]
            .chars()
            .next()
            .map_or(offset + 1, |character| offset + character.len_utf8())
    }

    /// Local end of a bare token (used to underline unsupported values).
    fn token_end(&self, start: usize, end: usize) -> usize {
        let mut position = start;
        while position < end {
            let Some(character) = self.text[position..end].chars().next() else {
                break;
            };
            if character.is_whitespace() {
                break;
            }
            position += character.len_utf8();
        }
        if position == start {
            (start + 1).min(end)
        } else {
            position
        }
    }

    /// Skip spaces, tabs and carriage returns.
    fn skip_ws(&self, start: usize, end: usize) -> usize {
        let mut position = start;
        while position < end && matches!(self.text.as_bytes()[position], b' ' | b'\t' | b'\r') {
            position += 1;
        }
        position
    }

    /// Global span for a local byte range.
    fn span(&self, start: usize, end: usize) -> Span {
        let start =
            u32::try_from(start).map_or(u32::MAX, |offset| self.base.saturating_add(offset));
        let end = u32::try_from(end).map_or(u32::MAX, |offset| self.base.saturating_add(offset));
        Span::new(start, end.max(start))
    }

    /// Push an `E0001` diagnostic for a local byte range.
    fn error(&mut self, message: impl Into<String>, start: usize, end: usize) {
        let span = self.span(start, end);
        self.diagnostics.push(manifest_error(message, span));
    }

    /// Report a missing `[package]` table or a missing `name` key, then return
    /// the package metadata when the manifest is complete.
    fn finish(self) -> Option<(PackageInfo, Vec<Dependency>)> {
        if self.diagnostics.error_count() == self.errors_before {
            if !self.seen_package {
                self.diagnostics.push(manifest_error(
                    "missing `[package]` table in the manifest",
                    Span::empty(self.base),
                ));
            } else if self.name.is_none() {
                let span = self
                    .package_header
                    .unwrap_or_else(|| Span::empty(self.base));
                self.diagnostics.push(manifest_error(
                    "missing `name` in the `[package]` table",
                    span,
                ));
            }
        }
        let name = self.name?;
        let version = self.version.unwrap_or_else(|| DEFAULT_VERSION.to_string());
        Some((PackageInfo { name, version }, self.dependencies))
    }
}

/// What the resolver does next after looking at the current frame.
enum ResolveAction {
    /// Descend into the dependency with this index in the frame's manifest.
    Dependency(usize),
    /// Every dependency of the current frame is resolved; build its package.
    Pop,
}

/// One package being resolved; the stack of frames mirrors the DFS.
struct ResolveFrame {
    manifest: Manifest,
    next_dependency: usize,
    dependencies: Vec<usize>,
}

/// Graph-building state for [resolve].
struct Resolver<'a> {
    sources: &'a mut SourceMap,
    diagnostics: &'a mut Diagnostics,
    errors_before: usize,
    packages: Vec<Package>,
    stack: Vec<ResolveFrame>,
    /// Canonical directory -> position in `stack`, for cycle detection.
    in_progress: HashMap<PathBuf, usize>,
    /// Canonical directory -> finished package index.
    by_dir: HashMap<PathBuf, usize>,
    /// Package name -> canonical directory, for duplicate-name detection.
    by_name: HashMap<String, PathBuf>,
    root_index: Option<usize>,
}

impl<'a> Resolver<'a> {
    fn new(sources: &'a mut SourceMap, diagnostics: &'a mut Diagnostics) -> Resolver<'a> {
        let errors_before = diagnostics.error_count();
        Resolver {
            sources,
            diagnostics,
            errors_before,
            packages: Vec::new(),
            stack: Vec::new(),
            in_progress: HashMap::new(),
            by_dir: HashMap::new(),
            by_name: HashMap::new(),
            root_index: None,
        }
    }

    /// Resolve the graph, iteratively so deep dependency chains cannot
    /// overflow the stack.
    ///
    /// The loop deliberately reads the top frame through a short borrow, then
    /// drops it before the body pushes or pops frames, so clippy's
    /// while-let-loop rewrite would not borrow-check here.
    #[allow(clippy::while_let_loop)]
    fn run(mut self, root_manifest: &Path) -> Option<PackageGraph> {
        let root_path = canonicalize_path(root_manifest);
        let root = parse_manifest(&root_path, self.sources, self.diagnostics)?;
        self.by_name
            .insert(root.package.name.clone(), root.dir.clone());
        self.in_progress.insert(root.dir.clone(), 0);
        self.stack.push(ResolveFrame {
            manifest: root,
            next_dependency: 0,
            dependencies: Vec::new(),
        });
        loop {
            let action = match self.stack.last_mut() {
                Some(frame) => {
                    if frame.next_dependency < frame.manifest.dependencies.len() {
                        let index = frame.next_dependency;
                        frame.next_dependency += 1;
                        ResolveAction::Dependency(index)
                    } else {
                        ResolveAction::Pop
                    }
                }
                None => break,
            };
            match action {
                ResolveAction::Dependency(index) => {
                    let (parent_dir, dependency) = match self.stack.last() {
                        Some(frame) => (
                            frame.manifest.dir.clone(),
                            frame.manifest.dependencies[index].clone(),
                        ),
                        None => break,
                    };
                    if !self.resolve_dependency(&parent_dir, &dependency) {
                        return None;
                    }
                }
                ResolveAction::Pop => {
                    let Some(frame) = self.stack.pop() else {
                        break;
                    };
                    let is_dependency = !self.stack.is_empty();
                    let index = self.packages.len();
                    let dir = frame.manifest.dir.clone();
                    self.packages.push(Package {
                        name: frame.manifest.package.name.clone(),
                        version: frame.manifest.package.version.clone(),
                        dir: dir.clone(),
                        manifest: frame.manifest,
                        is_dependency,
                        dependencies: frame.dependencies,
                    });
                    self.by_dir.insert(dir.clone(), index);
                    self.in_progress.remove(&dir);
                    match self.stack.last_mut() {
                        Some(parent) => parent.dependencies.push(index),
                        None => {
                            self.root_index = Some(index);
                            break;
                        }
                    }
                }
            }
        }
        if self.diagnostics.error_count() > self.errors_before {
            return None;
        }
        let root = self.root_index?;
        Some(PackageGraph {
            packages: self.packages,
            root,
        })
    }

    /// Resolve one dependency edge of the frame whose directory is
    /// `parent_dir`. Returns `false` after reporting an error.
    fn resolve_dependency(&mut self, parent_dir: &Path, dependency: &Dependency) -> bool {
        let joined = parent_dir.join(&dependency.path);
        let Ok(dep_dir) = fs::canonicalize(&joined) else {
            self.error(
                format!(
                    "dependency `{}`: path `{}` does not exist",
                    dependency.name,
                    display_path(&dependency.path)
                ),
                dependency.span,
            );
            return false;
        };
        if dep_dir == parent_dir {
            let parent_name = self
                .stack
                .last()
                .map(|frame| frame.manifest.package.name.clone())
                .unwrap_or_default();
            self.error(
                format!("package `{parent_name}` cannot depend on itself"),
                dependency.span,
            );
            return false;
        }
        if let Some(&position) = self.in_progress.get(&dep_dir) {
            let mut chain: Vec<String> = self
                .stack
                .get(position..)
                .map(|frames| {
                    frames
                        .iter()
                        .map(|frame| frame.manifest.package.name.clone())
                        .collect()
                })
                .unwrap_or_default();
            let first = chain.first().cloned().unwrap_or_default();
            chain.push(first);
            self.error(
                format!("cyclic package dependency: {}", chain.join(" -> ")),
                dependency.span,
            );
            return false;
        }
        if let Some(&index) = self.by_dir.get(&dep_dir) {
            if let Some(frame) = self.stack.last_mut() {
                frame.dependencies.push(index);
            }
            return true;
        }
        let manifest_path = dep_dir.join(MANIFEST_FILE);
        if !manifest_path.is_file() {
            self.error(
                format!(
                    "dependency `{}`: `{}` does not contain a `lsaca.toml` manifest",
                    dependency.name,
                    display_path(&dep_dir)
                ),
                dependency.span,
            );
            return false;
        }
        if !dep_dir.join("src").join("lib.ls").is_file() {
            self.error(
                format!(
                    "dependency `{}` is not a library: `{}` does not contain `src/lib.ls`",
                    dependency.name,
                    display_path(&dep_dir)
                ),
                dependency.span,
            );
            return false;
        }
        let Some(manifest) = parse_manifest(&manifest_path, self.sources, self.diagnostics) else {
            return false;
        };
        let name = manifest.package.name.clone();
        if let Some(existing) = self.by_name.get(&name) {
            let existing = existing.clone();
            self.error(
                format!(
                    "the package name `{name}` is provided by two different paths: `{}` and `{}`",
                    display_path(&existing),
                    display_path(&dep_dir)
                ),
                dependency.span,
            );
            return false;
        }
        let position = self.stack.len();
        self.by_name.insert(name, dep_dir.clone());
        self.in_progress.insert(dep_dir.clone(), position);
        self.stack.push(ResolveFrame {
            manifest,
            next_dependency: 0,
            dependencies: Vec::new(),
        });
        true
    }

    /// Push an `E0001` diagnostic for a manifest span.
    fn error(&mut self, message: impl Into<String>, span: Span) {
        self.diagnostics.push(manifest_error(message, span));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Serial number that makes scratch directories unique within a process.
    static NEXT_SERIAL: AtomicU64 = AtomicU64::new(0);

    /// Self-cleaning scratch directory under target/package-test-tmp.
    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new(label: &str) -> TestDir {
            let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/package-test-tmp");
            let serial = NEXT_SERIAL.fetch_add(1, Ordering::Relaxed);
            let path = base.join(format!("{label}-{}-{serial}", std::process::id()));
            fs::create_dir_all(&path).expect("create test directory");
            TestDir { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }

        fn write(&self, relative: &str, contents: &str) -> PathBuf {
            let path = self.path.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create parent directory");
            }
            fs::write(&path, contents).expect("write test file");
            path
        }

        fn write_bytes(&self, relative: &str, contents: &[u8]) -> PathBuf {
            let path = self.path.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create parent directory");
            }
            fs::write(&path, contents).expect("write test file");
            path
        }

        /// Write a valid package: a manifest plus an empty library source.
        fn package(&self, relative: &str, name: &str) -> PathBuf {
            self.write(
                &format!("{relative}/lsaca.toml"),
                &format!("[package]\nname = \"{name}\"\n"),
            );
            self.write(&format!("{relative}/src/lib.ls"), "");
            self.path.join(relative)
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    /// Build a minimal manifest with optional version and dependencies.
    fn manifest_text(name: &str, version: Option<&str>, dependencies: &[(&str, &str)]) -> String {
        let mut text = format!("[package]\nname = \"{name}\"\n");
        if let Some(version) = version {
            text.push_str(&format!("version = \"{version}\"\n"));
        }
        if !dependencies.is_empty() {
            text.push_str("\n[dependencies]\n");
            for (dependency, path) in dependencies {
                text.push_str(&format!("{dependency} = {{ path = \"{path}\" }}\n"));
            }
        }
        text
    }

    fn parse_file(path: &Path) -> (SourceMap, Diagnostics, Option<Manifest>) {
        let mut sources = SourceMap::new();
        let mut diagnostics = Diagnostics::new();
        let manifest = parse_manifest(path, &mut sources, &mut diagnostics);
        (sources, diagnostics, manifest)
    }

    fn resolve_file(path: &Path) -> (SourceMap, Diagnostics, Option<PackageGraph>) {
        let mut sources = SourceMap::new();
        let mut diagnostics = Diagnostics::new();
        let graph = resolve(path, &mut sources, &mut diagnostics);
        (sources, diagnostics, graph)
    }

    /// 1-based line and column of a byte offset in a test manifest.
    fn location_at(text: &str, offset: usize) -> (usize, usize) {
        let before = &text[..offset];
        let line = before.matches('\n').count() + 1;
        let line_start = before.rfind('\n').map_or(0, |index| index + 1);
        let column = before[line_start..].chars().count() + 1;
        (line, column)
    }

    fn location_of(text: &str, needle: &str) -> (usize, usize) {
        let offset = text.find(needle).expect("needle not found in manifest");
        location_at(text, offset)
    }

    fn nth_location_of(text: &str, needle: &str, nth: usize) -> (usize, usize) {
        let mut search = 0;
        for occurrence in 0..=nth {
            let offset = text[search..]
                .find(needle)
                .expect("needle occurrence not found in manifest");
            if occurrence == nth {
                return location_at(text, search + offset);
            }
            search += offset + needle.len();
        }
        unreachable!("the loop always returns")
    }

    /// Location of the first character inside a quoted value.
    fn quoted_location(text: &str, quoted: &str) -> (usize, usize) {
        let offset = text
            .find(quoted)
            .expect("quoted value not found in manifest")
            + 1;
        location_at(text, offset)
    }

    /// Location just past a token, used for empty end-of-line diagnostics.
    fn end_location(text: &str, needle: &str) -> (usize, usize) {
        let offset = text.find(needle).expect("needle not found in manifest") + needle.len();
        location_at(text, offset)
    }

    fn messages(diagnostics: &Diagnostics) -> Vec<String> {
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.clone())
            .collect()
    }

    fn single_error(diagnostics: &Diagnostics) -> &Diagnostic {
        assert_eq!(
            diagnostics.len(),
            1,
            "expected exactly one diagnostic, got {:?}",
            messages(diagnostics)
        );
        &diagnostics.as_slice()[0]
    }

    fn assert_at(sources: &SourceMap, span: Span, expected: (usize, usize)) {
        let (file, line, column) = sources.location(span).expect("span resolves to a file");
        assert_eq!(
            (line, column),
            expected,
            "span {span:?} points at the wrong location in {}",
            file.name()
        );
    }

    #[test]
    fn parse_valid_manifest_with_dependency() {
        let dir = TestDir::new("parse-valid");
        fs::create_dir_all(dir.path().join("util")).expect("create dependency directory");
        let text = r#"[package]
name = "example"
version = "1.2.3"

[dependencies]
util = { path = "util" }
"#;
        dir.write("lsaca.toml", text);
        let (sources, diagnostics, manifest) = parse_file(&dir.path().join("lsaca.toml"));
        assert!(diagnostics.is_empty(), "{:?}", messages(&diagnostics));
        let manifest = manifest.expect("valid manifest should parse");
        assert_eq!(sources.len(), 1);
        assert_eq!(manifest.dir, canonicalize_path(dir.path()));
        assert_eq!(
            manifest.path,
            canonicalize_path(&dir.path().join("lsaca.toml"))
        );
        assert_eq!(
            manifest.package,
            PackageInfo {
                name: "example".to_string(),
                version: "1.2.3".to_string(),
            }
        );
        assert_eq!(manifest.dependencies.len(), 1);
        let dependency = &manifest.dependencies[0];
        assert_eq!(dependency.name, "util");
        assert_eq!(dependency.path, canonicalize_path(&dir.path().join("util")));
        assert_eq!(dependency.span.len(), 4);
        assert_at(&sources, dependency.span, quoted_location(text, "\"util\""));
    }

    #[test]
    fn parse_default_version() {
        let dir = TestDir::new("parse-default-version");
        let text = manifest_text("example", None, &[]);
        dir.write("lsaca.toml", &text);
        let (_, diagnostics, manifest) = parse_file(&dir.path().join("lsaca.toml"));
        assert!(diagnostics.is_empty(), "{:?}", messages(&diagnostics));
        let manifest = manifest.expect("valid manifest should parse");
        assert_eq!(manifest.package.version, "0.1.0");
    }

    #[test]
    fn parse_comments_blank_lines_and_crlf() {
        let dir = TestDir::new("parse-syntax");
        let text = "# leading comment\r\n\r\n[package]\r\nname\t=\t\"example\"\r\nversion = \"1.0.0\"  # trailing comment\r\n\r\n[dependencies]\r\n# none\r\n";
        dir.write("lsaca.toml", text);
        let (_, diagnostics, manifest) = parse_file(&dir.path().join("lsaca.toml"));
        assert!(diagnostics.is_empty(), "{:?}", messages(&diagnostics));
        let manifest = manifest.expect("valid manifest should parse");
        assert_eq!(manifest.package.name, "example");
        assert_eq!(manifest.package.version, "1.0.0");
        assert!(manifest.dependencies.is_empty());
    }

    #[test]
    fn parse_missing_package_table() {
        let dir = TestDir::new("parse-missing-package");
        let text = "[dependencies]\n";
        dir.write("lsaca.toml", text);
        let (sources, diagnostics, manifest) = parse_file(&dir.path().join("lsaca.toml"));
        assert!(manifest.is_none());
        let error = single_error(&diagnostics);
        assert_eq!(error.code, Some(MANIFEST_ERROR_CODE));
        assert!(error.message.contains("missing"), "{}", error.message);
        assert!(error.message.contains("[package]"), "{}", error.message);
        assert_at(&sources, error.span, (1, 1));
    }

    #[test]
    fn parse_missing_name() {
        let dir = TestDir::new("parse-missing-name");
        let text = "[package]\nversion = \"0.1.0\"\n";
        dir.write("lsaca.toml", text);
        let (sources, diagnostics, manifest) = parse_file(&dir.path().join("lsaca.toml"));
        assert!(manifest.is_none());
        let error = single_error(&diagnostics);
        assert!(error.message.contains("missing"), "{}", error.message);
        assert!(error.message.contains("name"), "{}", error.message);
        assert_at(&sources, error.span, (1, 1));
    }

    #[test]
    fn parse_duplicate_package_table() {
        let dir = TestDir::new("parse-duplicate-table");
        let text = r#"[package]
name = "example"
[package]
"#;
        dir.write("lsaca.toml", text);
        let (sources, diagnostics, _) = parse_file(&dir.path().join("lsaca.toml"));
        let error = single_error(&diagnostics);
        assert!(
            error.message.contains("declared more than once"),
            "{}",
            error.message
        );
        assert_at(&sources, error.span, nth_location_of(text, "package", 1));
    }

    #[test]
    fn parse_duplicate_name_key() {
        let dir = TestDir::new("parse-duplicate-name");
        let text = r#"[package]
name = "example"
name = "other"
"#;
        dir.write("lsaca.toml", text);
        let (sources, diagnostics, _) = parse_file(&dir.path().join("lsaca.toml"));
        let error = single_error(&diagnostics);
        assert!(error.message.contains("duplicate key"), "{}", error.message);
        assert_at(&sources, error.span, nth_location_of(text, "name", 1));
    }

    #[test]
    fn parse_duplicate_version_key() {
        let dir = TestDir::new("parse-duplicate-version");
        let text = r#"[package]
name = "example"
version = "1.0.0"
version = "2.0.0"
"#;
        dir.write("lsaca.toml", text);
        let (sources, diagnostics, _) = parse_file(&dir.path().join("lsaca.toml"));
        let error = single_error(&diagnostics);
        assert!(error.message.contains("duplicate key"), "{}", error.message);
        assert!(error.message.contains("version"), "{}", error.message);
        assert_at(&sources, error.span, nth_location_of(text, "version", 1));
    }

    #[test]
    fn parse_unknown_package_key() {
        let dir = TestDir::new("parse-unknown-key");
        let text = r#"[package]
name = "example"
author = "me"
"#;
        dir.write("lsaca.toml", text);
        let (sources, diagnostics, _) = parse_file(&dir.path().join("lsaca.toml"));
        let error = single_error(&diagnostics);
        assert!(error.message.contains("unknown key"), "{}", error.message);
        assert!(error.message.contains("author"), "{}", error.message);
        assert_at(&sources, error.span, location_of(text, "author"));
    }

    #[test]
    fn parse_unknown_section() {
        let dir = TestDir::new("parse-unknown-section");
        let text = r#"[package]
name = "example"

[features]
fast = true
"#;
        dir.write("lsaca.toml", text);
        let (sources, diagnostics, _) = parse_file(&dir.path().join("lsaca.toml"));
        let error = single_error(&diagnostics);
        assert!(
            error.message.contains("unknown section"),
            "{}",
            error.message
        );
        assert!(error.message.contains("features"), "{}", error.message);
        assert_at(&sources, error.span, location_of(text, "features"));
    }

    #[test]
    fn parse_malformed_header_missing_close() {
        let dir = TestDir::new("parse-header-missing-close");
        let text = "[package\n";
        dir.write("lsaca.toml", text);
        let (sources, diagnostics, _) = parse_file(&dir.path().join("lsaca.toml"));
        let error = single_error(&diagnostics);
        assert!(
            error.message.contains("malformed table header"),
            "{}",
            error.message
        );
        assert_at(&sources, error.span, (1, 1));
    }

    #[test]
    fn parse_empty_header() {
        let dir = TestDir::new("parse-empty-header");
        let text = "[]\n";
        dir.write("lsaca.toml", text);
        let (sources, diagnostics, _) = parse_file(&dir.path().join("lsaca.toml"));
        let error = single_error(&diagnostics);
        assert!(
            error.message.contains("empty table header"),
            "{}",
            error.message
        );
        assert_at(&sources, error.span, (1, 1));
    }

    #[test]
    fn parse_malformed_header_name() {
        let dir = TestDir::new("parse-header-name");
        let text = "[pack-age]\n";
        dir.write("lsaca.toml", text);
        let (sources, diagnostics, _) = parse_file(&dir.path().join("lsaca.toml"));
        let error = single_error(&diagnostics);
        assert!(
            error.message.contains("malformed table header"),
            "{}",
            error.message
        );
        assert_at(&sources, error.span, location_of(text, "pack-age"));
    }

    #[test]
    fn parse_trailing_after_header() {
        let dir = TestDir::new("parse-header-trailing");
        let text = r#"[package] extra
name = "example"
"#;
        dir.write("lsaca.toml", text);
        let (sources, diagnostics, _) = parse_file(&dir.path().join("lsaca.toml"));
        let error = single_error(&diagnostics);
        assert!(
            error
                .message
                .contains("unexpected characters after the table header"),
            "{}",
            error.message
        );
        assert_at(&sources, error.span, location_of(text, "extra"));
    }

    #[test]
    fn parse_unterminated_string() {
        let dir = TestDir::new("parse-unterminated");
        let text = "[package]\nname = \"example\n";
        dir.write("lsaca.toml", text);
        let (sources, diagnostics, manifest) = parse_file(&dir.path().join("lsaca.toml"));
        assert!(manifest.is_none());
        let error = single_error(&diagnostics);
        assert!(
            error.message.contains("unterminated string"),
            "{}",
            error.message
        );
        assert_at(&sources, error.span, location_of(text, "\"example"));
    }

    #[test]
    fn parse_single_quoted_string() {
        let dir = TestDir::new("parse-single-quoted");
        let text = r#"[package]
name = 'example'
"#;
        dir.write("lsaca.toml", text);
        let (sources, diagnostics, _) = parse_file(&dir.path().join("lsaca.toml"));
        let error = single_error(&diagnostics);
        assert!(
            error
                .message
                .contains("single-quoted strings are not supported"),
            "{}",
            error.message
        );
        assert_at(&sources, error.span, location_of(text, "'"));
    }

    #[test]
    fn parse_unsupported_escape() {
        let dir = TestDir::new("parse-escape");
        let text = r#"[package]
name = "a\qb"
"#;
        dir.write("lsaca.toml", text);
        let (sources, diagnostics, _) = parse_file(&dir.path().join("lsaca.toml"));
        let error = single_error(&diagnostics);
        assert!(
            error.message.contains("unsupported escape sequence"),
            "{}",
            error.message
        );
        assert!(error.message.contains("\\q"), "{}", error.message);
        assert_at(&sources, error.span, location_of(text, "\\q"));
    }

    #[test]
    fn parse_trailing_after_value() {
        let dir = TestDir::new("parse-value-trailing");
        let text = r#"[package]
name = "example" junk
"#;
        dir.write("lsaca.toml", text);
        let (sources, diagnostics, _) = parse_file(&dir.path().join("lsaca.toml"));
        let error = single_error(&diagnostics);
        assert!(
            error
                .message
                .contains("unexpected characters after the value"),
            "{}",
            error.message
        );
        assert_at(&sources, error.span, location_of(text, "junk"));
    }

    #[test]
    fn parse_reserved_std_package_name() {
        let dir = TestDir::new("parse-reserved-std-name");
        let text = r#"[package]
name = "std"
version = "0.1.0"
"#;
        dir.write("lsaca.toml", text);
        let (sources, diagnostics, _) = parse_file(&dir.path().join("lsaca.toml"));
        let error = single_error(&diagnostics);
        assert!(
            error.message.contains("reserved for the standard library"),
            "{}",
            error.message
        );
        assert!(
            error.message.contains("cannot be a package name"),
            "{}",
            error.message
        );
        assert_at(&sources, error.span, quoted_location(text, "\"std\""));
    }

    #[test]
    fn parse_reserved_std_dependency_name() {
        let dir = TestDir::new("parse-reserved-std-dep");
        let text = r#"[package]
name = "example"
version = "0.1.0"

[dependencies]
std = { path = "../std" }
"#;
        dir.write("lsaca.toml", text);
        let (sources, diagnostics, _) = parse_file(&dir.path().join("lsaca.toml"));
        let error = single_error(&diagnostics);
        assert!(
            error.message.contains("reserved for the standard library"),
            "{}",
            error.message
        );
        assert!(
            error.message.contains("cannot be a dependency name"),
            "{}",
            error.message
        );
        assert_at(&sources, error.span, location_of(text, "std ="));
    }

    #[test]
    fn parse_invalid_package_name() {
        let dir = TestDir::new("parse-invalid-name");
        let text = r#"[package]
name = "Bad"
"#;
        dir.write("lsaca.toml", text);
        let (sources, diagnostics, _) = parse_file(&dir.path().join("lsaca.toml"));
        let error = single_error(&diagnostics);
        assert!(
            error.message.contains("invalid package name"),
            "{}",
            error.message
        );
        assert!(error.message.contains("Bad"), "{}", error.message);
        assert_at(&sources, error.span, quoted_location(text, "\"Bad\""));
    }

    #[test]
    fn parse_keyword_package_name() {
        let dir = TestDir::new("parse-keyword-name");
        let text = r#"[package]
name = "fn"
"#;
        dir.write("lsaca.toml", text);
        let (sources, diagnostics, _) = parse_file(&dir.path().join("lsaca.toml"));
        let error = single_error(&diagnostics);
        assert!(
            error.message.contains("cannot be a package name"),
            "{}",
            error.message
        );
        assert_at(&sources, error.span, quoted_location(text, "\"fn\""));
    }

    #[test]
    fn parse_invalid_version() {
        let dir = TestDir::new("parse-invalid-version");
        let text = r#"[package]
name = "example"
version = "1.2"
"#;
        dir.write("lsaca.toml", text);
        let (sources, diagnostics, _) = parse_file(&dir.path().join("lsaca.toml"));
        let error = single_error(&diagnostics);
        assert!(
            error.message.contains("invalid version"),
            "{}",
            error.message
        );
        assert_at(&sources, error.span, quoted_location(text, "\"1.2\""));
    }

    #[test]
    fn parse_duplicate_dependency() {
        let dir = TestDir::new("parse-duplicate-dependency");
        let text = r#"[package]
name = "example"

[dependencies]
util = { path = "../util" }
util = { path = "../util" }
"#;
        dir.write("lsaca.toml", text);
        let (sources, diagnostics, _) = parse_file(&dir.path().join("lsaca.toml"));
        let error = single_error(&diagnostics);
        assert!(
            error.message.contains("duplicate dependency"),
            "{}",
            error.message
        );
        assert_at(&sources, error.span, nth_location_of(text, "util", 2));
    }

    #[test]
    fn parse_malformed_dependency_values() {
        let dir = TestDir::new("parse-malformed-dependency");
        let cases: [(&str, &str, &str); 5] = [
            (
                r#"[package]
name = "example"

[dependencies]
util = "../util"
"#,
                "expected the inline table",
                "\"../util\"",
            ),
            (
                r#"[package]
name = "example"

[dependencies]
util = { pth = "util" }
"#,
                "unknown key",
                "{",
            ),
            (
                r#"[package]
name = "example"

[dependencies]
util = { path "util" }
"#,
                "expected",
                "{",
            ),
            (
                r#"[package]
name = "example"

[dependencies]
util = { path = "util"
"#,
                "to close the inline table",
                "{",
            ),
            (
                r#"[package]
name = "example"

[dependencies]
util = { }
"#,
                "expected `path` in the inline table",
                "{",
            ),
        ];
        for (text, fragment, location_needle) in cases {
            dir.write("lsaca.toml", text);
            let (sources, diagnostics, _) = parse_file(&dir.path().join("lsaca.toml"));
            let error = single_error(&diagnostics);
            assert!(error.message.contains(fragment), "{}", error.message);
            assert_at(&sources, error.span, location_of(text, location_needle));
        }
    }

    #[test]
    fn parse_trailing_after_dependency() {
        let dir = TestDir::new("parse-dependency-trailing");
        let text = r#"[package]
name = "example"

[dependencies]
util = { path = "util" } junk
"#;
        dir.write("lsaca.toml", text);
        let (sources, diagnostics, _) = parse_file(&dir.path().join("lsaca.toml"));
        let error = single_error(&diagnostics);
        assert!(
            error
                .message
                .contains("unexpected characters after the dependency table"),
            "{}",
            error.message
        );
        assert_at(&sources, error.span, location_of(text, "junk"));
    }

    #[test]
    fn parse_missing_value() {
        let dir = TestDir::new("parse-missing-value");
        let text = "[package]\nname =\n";
        dir.write("lsaca.toml", text);
        let (sources, diagnostics, _) = parse_file(&dir.path().join("lsaca.toml"));
        let error = single_error(&diagnostics);
        assert!(
            error.message.contains("missing value after"),
            "{}",
            error.message
        );
        assert_at(&sources, error.span, end_location(text, "name ="));
    }

    #[test]
    fn parse_expected_key() {
        let dir = TestDir::new("parse-expected-key");
        let text = r#"[package]
name = "example"
= "junk"
"#;
        dir.write("lsaca.toml", text);
        let (sources, diagnostics, _) = parse_file(&dir.path().join("lsaca.toml"));
        let error = single_error(&diagnostics);
        assert!(
            error.message.contains("expected a key"),
            "{}",
            error.message
        );
        assert_at(&sources, error.span, nth_location_of(text, "=", 1));
    }

    #[test]
    fn parse_expected_equals() {
        let dir = TestDir::new("parse-expected-equals");
        let text = r#"[package]
name "example"
"#;
        dir.write("lsaca.toml", text);
        let (sources, diagnostics, _) = parse_file(&dir.path().join("lsaca.toml"));
        let error = single_error(&diagnostics);
        assert!(
            error.message.contains("expected `=` after the key"),
            "{}",
            error.message
        );
        assert_at(&sources, error.span, location_of(text, "\"example\""));
    }

    #[test]
    fn parse_settings_require_table() {
        let dir = TestDir::new("parse-settings-table");
        let text = "name = \"example\"\n";
        dir.write("lsaca.toml", text);
        let (sources, diagnostics, _) = parse_file(&dir.path().join("lsaca.toml"));
        let error = single_error(&diagnostics);
        assert!(
            error
                .message
                .contains("manifest settings must appear inside"),
            "{}",
            error.message
        );
        assert_at(&sources, error.span, (1, 1));
    }

    #[test]
    fn parse_expected_double_quoted_value() {
        let dir = TestDir::new("parse-expected-string");
        let text = r#"[package]
name = example
"#;
        dir.write("lsaca.toml", text);
        let (sources, diagnostics, _) = parse_file(&dir.path().join("lsaca.toml"));
        let error = single_error(&diagnostics);
        assert!(
            error
                .message
                .contains("expected a double-quoted string value"),
            "{}",
            error.message
        );
        assert_at(&sources, error.span, location_of(text, "example"));
    }

    #[test]
    fn parse_invalid_dependency_names() {
        let dir = TestDir::new("parse-invalid-dependency");
        let cases: [(&str, &str); 2] = [
            (
                r#"[package]
name = "example"

[dependencies]
Util = { path = "util" }
"#,
                "invalid dependency name",
            ),
            (
                r#"[package]
name = "example"

[dependencies]
fn = { path = "util" }
"#,
                "cannot be a dependency name",
            ),
        ];
        for (text, fragment) in cases {
            dir.write("lsaca.toml", text);
            let (_, diagnostics, _) = parse_file(&dir.path().join("lsaca.toml"));
            let error = single_error(&diagnostics);
            assert!(error.message.contains(fragment), "{}", error.message);
        }
    }

    #[test]
    fn parse_invalid_utf8_manifest() {
        let dir = TestDir::new("parse-invalid-utf8");
        dir.write_bytes("lsaca.toml", &[0xFF, 0xFE]);
        let (sources, diagnostics, manifest) = parse_file(&dir.path().join("lsaca.toml"));
        assert!(manifest.is_none());
        let error = single_error(&diagnostics);
        assert!(
            error.message.contains("could not read package manifest"),
            "{}",
            error.message
        );
        assert_eq!(sources.len(), 1);
        assert_at(&sources, error.span, (1, 1));
    }

    #[test]
    fn parse_missing_file() {
        let dir = TestDir::new("parse-missing-file");
        let (sources, diagnostics, manifest) = parse_file(&dir.path().join("nested/lsaca.toml"));
        assert!(manifest.is_none());
        let error = single_error(&diagnostics);
        assert!(
            error.message.contains("could not read package manifest"),
            "{}",
            error.message
        );
        assert!(error.message.contains("lsaca.toml"), "{}", error.message);
        assert_eq!(sources.len(), 1);
    }

    #[test]
    fn parse_preserves_existing_diagnostics() {
        let dir = TestDir::new("parse-existing-diagnostics");
        let text = manifest_text("example", None, &[]);
        let path = dir.write("lsaca.toml", &text);
        let mut sources = SourceMap::new();
        let mut diagnostics = Diagnostics::new();
        diagnostics.error("pre-existing", Span::empty(0));
        let manifest = parse_manifest(&path, &mut sources, &mut diagnostics);
        assert!(manifest.is_some());
        assert_eq!(diagnostics.error_count(), 1);
        assert_eq!(diagnostics.as_slice()[0].message, "pre-existing");
    }

    #[test]
    fn parse_dependency_spans_are_distinct() {
        let dir = TestDir::new("parse-dependency-spans");
        let text = r#"[package]
name = "example"

[dependencies]
a = { path = "../a" }
b = { path = "../b" }
"#;
        dir.write("lsaca.toml", text);
        let (sources, diagnostics, manifest) = parse_file(&dir.path().join("lsaca.toml"));
        assert!(diagnostics.is_empty(), "{:?}", messages(&diagnostics));
        let manifest = manifest.expect("manifest should parse");
        assert_eq!(manifest.dependencies.len(), 2);
        assert_at(
            &sources,
            manifest.dependencies[0].span,
            quoted_location(text, "\"../a\""),
        );
        assert_at(
            &sources,
            manifest.dependencies[1].span,
            quoted_location(text, "\"../b\""),
        );
        assert_ne!(manifest.dependencies[0].span, manifest.dependencies[1].span);
    }

    #[test]
    fn discover_manifest_finds_manifest_in_directory() {
        let dir = TestDir::new("discover-direct");
        dir.write("lsaca.toml", &manifest_text("example", None, &[]));
        let found = discover_manifest(dir.path()).expect("discovery should succeed");
        assert_eq!(
            found,
            Some(canonicalize_path(&dir.path().join("lsaca.toml")))
        );
    }

    #[test]
    fn discover_manifest_walks_up() {
        let dir = TestDir::new("discover-up");
        dir.write("lsaca.toml", &manifest_text("example", None, &[]));
        dir.write("a/b/keep.txt", "");
        let found = discover_manifest(&dir.path().join("a/b")).expect("discovery should succeed");
        assert_eq!(
            found,
            Some(canonicalize_path(&dir.path().join("lsaca.toml")))
        );
    }

    #[test]
    fn discover_manifest_stops_at_nearest() {
        let dir = TestDir::new("discover-nearest");
        dir.write("lsaca.toml", &manifest_text("outer", None, &[]));
        dir.write("inner/lsaca.toml", &manifest_text("inner", None, &[]));
        dir.write("inner/deeper/keep.txt", "");
        let found =
            discover_manifest(&dir.path().join("inner/deeper")).expect("discovery should succeed");
        assert_eq!(
            found,
            Some(canonicalize_path(&dir.path().join("inner/lsaca.toml")))
        );
    }

    #[test]
    fn discover_manifest_returns_none() {
        let dir = TestDir::new("discover-none");
        dir.write("nested/deeper/keep.txt", "");
        let found =
            discover_manifest(&dir.path().join("nested/deeper")).expect("discovery should succeed");
        assert_eq!(found, None);
    }

    #[test]
    fn resolve_root_only() {
        let dir = TestDir::new("resolve-root");
        let root = dir.write("app/lsaca.toml", &manifest_text("app", Some("1.0.0"), &[]));
        let (sources, diagnostics, graph) = resolve_file(&root);
        assert!(diagnostics.is_empty(), "{:?}", messages(&diagnostics));
        let graph = graph.expect("graph should resolve");
        assert_eq!(graph.packages.len(), 1);
        assert_eq!(graph.root, 0);
        assert_eq!(graph.packages[0].name, "app");
        assert_eq!(graph.packages[0].version, "1.0.0");
        assert!(!graph.packages[0].is_dependency);
        assert!(graph.packages[0].dependencies.is_empty());
        assert_eq!(sources.len(), 1);
    }

    #[test]
    fn resolve_simple_dependency() {
        let dir = TestDir::new("resolve-simple");
        dir.package("util", "util");
        let root = dir.write(
            "app/lsaca.toml",
            &manifest_text("app", None, &[("util", "../util")]),
        );
        let (sources, diagnostics, graph) = resolve_file(&root);
        assert!(diagnostics.is_empty(), "{:?}", messages(&diagnostics));
        let graph = graph.expect("graph should resolve");
        assert_eq!(
            graph
                .packages
                .iter()
                .map(|package| package.name.as_str())
                .collect::<Vec<_>>(),
            vec!["util", "app"]
        );
        assert_eq!(graph.root, 1);
        assert!(graph.packages[0].is_dependency);
        assert!(!graph.packages[1].is_dependency);
        assert_eq!(graph.packages[1].dependencies, vec![0]);
        assert_eq!(
            graph.packages[0].dir,
            canonicalize_path(&dir.path().join("util"))
        );
        assert_eq!(sources.len(), 2);
    }

    #[test]
    fn resolve_diamond() {
        let dir = TestDir::new("resolve-diamond");
        dir.package("a", "a");
        dir.write("b/lsaca.toml", &manifest_text("b", None, &[("a", "../a")]));
        dir.write("b/src/lib.ls", "");
        dir.write("c/lsaca.toml", &manifest_text("c", None, &[("a", "../a")]));
        dir.write("c/src/lib.ls", "");
        let root = dir.write(
            "app/lsaca.toml",
            &manifest_text("app", None, &[("b", "../b"), ("c", "../c")]),
        );
        let (sources, diagnostics, graph) = resolve_file(&root);
        assert!(diagnostics.is_empty(), "{:?}", messages(&diagnostics));
        let graph = graph.expect("graph should resolve");
        assert_eq!(
            graph
                .packages
                .iter()
                .map(|package| package.name.as_str())
                .collect::<Vec<_>>(),
            vec!["a", "b", "c", "app"]
        );
        assert_eq!(graph.root, 3);
        assert_eq!(graph.packages[1].dependencies, vec![0]);
        assert_eq!(graph.packages[2].dependencies, vec![0]);
        assert_eq!(graph.packages[3].dependencies, vec![1, 2]);
        assert_eq!(sources.len(), 4);
    }

    #[test]
    fn resolve_declaration_order_and_determinism() {
        let dir = TestDir::new("resolve-order");
        dir.package("a", "a");
        dir.package("b", "b");
        let root = dir.write(
            "app/lsaca.toml",
            &manifest_text("app", None, &[("b", "../b"), ("a", "../a")]),
        );
        let (_, diagnostics, graph) = resolve_file(&root);
        assert!(diagnostics.is_empty(), "{:?}", messages(&diagnostics));
        let graph = graph.expect("graph should resolve");
        assert_eq!(
            graph
                .packages
                .iter()
                .map(|package| package.name.as_str())
                .collect::<Vec<_>>(),
            vec!["b", "a", "app"]
        );
        assert_eq!(graph.root, 2);
        assert_eq!(graph.packages[2].dependencies, vec![0, 1]);
        let (_, second_diagnostics, second) = resolve_file(&root);
        assert!(second_diagnostics.is_empty());
        assert_eq!(graph, second.expect("graph should resolve again"));
    }

    #[test]
    fn resolve_cycle() {
        let dir = TestDir::new("resolve-cycle");
        dir.write("a/lsaca.toml", &manifest_text("a", None, &[("b", "../b")]));
        dir.write("a/src/lib.ls", "");
        let b_text = manifest_text("b", None, &[("a", "../a")]);
        dir.write("b/lsaca.toml", &b_text);
        dir.write("b/src/lib.ls", "");
        let root = dir.path().join("a/lsaca.toml");
        let (sources, diagnostics, graph) = resolve_file(&root);
        assert!(graph.is_none());
        let error = single_error(&diagnostics);
        assert_eq!(error.message, "cyclic package dependency: a -> b -> a");
        assert_eq!(sources.len(), 2);
        assert_at(&sources, error.span, quoted_location(&b_text, "\"../a\""));
    }

    #[test]
    fn resolve_self_dependency() {
        let dir = TestDir::new("resolve-self");
        let text = manifest_text("app", None, &[("app", ".")]);
        let root = dir.write("app/lsaca.toml", &text);
        let (sources, diagnostics, graph) = resolve_file(&root);
        assert!(graph.is_none());
        let error = single_error(&diagnostics);
        assert_eq!(error.message, "package `app` cannot depend on itself");
        assert_at(&sources, error.span, quoted_location(&text, "\".\""));
    }

    #[test]
    fn resolve_missing_dependency_path() {
        let dir = TestDir::new("resolve-missing-dependency");
        let text = manifest_text("app", None, &[("util", "../missing")]);
        let root = dir.write("app/lsaca.toml", &text);
        let (sources, diagnostics, graph) = resolve_file(&root);
        assert!(graph.is_none());
        let error = single_error(&diagnostics);
        assert!(error.message.contains("dependency"), "{}", error.message);
        assert!(error.message.contains("util"), "{}", error.message);
        assert!(
            error.message.contains("does not exist"),
            "{}",
            error.message
        );
        assert_at(
            &sources,
            error.span,
            quoted_location(&text, "\"../missing\""),
        );
    }

    #[test]
    fn resolve_dependency_without_manifest() {
        let dir = TestDir::new("resolve-dependency-no-manifest");
        fs::create_dir_all(dir.path().join("util")).expect("create dependency directory");
        let root = dir.write(
            "app/lsaca.toml",
            &manifest_text("app", None, &[("util", "../util")]),
        );
        let (_, diagnostics, graph) = resolve_file(&root);
        assert!(graph.is_none());
        let error = single_error(&diagnostics);
        assert!(
            error
                .message
                .contains("does not contain a `lsaca.toml` manifest"),
            "{}",
            error.message
        );
    }

    #[test]
    fn resolve_dependency_not_library() {
        let dir = TestDir::new("resolve-dependency-not-library");
        dir.write("util/lsaca.toml", &manifest_text("util", None, &[]));
        let root = dir.write(
            "app/lsaca.toml",
            &manifest_text("app", None, &[("util", "../util")]),
        );
        let (_, diagnostics, graph) = resolve_file(&root);
        assert!(graph.is_none());
        let error = single_error(&diagnostics);
        assert!(
            error.message.contains("is not a library"),
            "{}",
            error.message
        );
        assert!(error.message.contains("src/lib.ls"), "{}", error.message);
    }

    #[test]
    fn resolve_duplicate_package_name() {
        let dir = TestDir::new("resolve-duplicate-name");
        dir.package("one", "shared");
        dir.package("two", "shared");
        let root = dir.write(
            "app/lsaca.toml",
            &manifest_text("app", None, &[("one", "../one"), ("two", "../two")]),
        );
        let (_, diagnostics, graph) = resolve_file(&root);
        assert!(graph.is_none());
        let error = single_error(&diagnostics);
        assert!(
            error.message.contains("provided by two different paths"),
            "{}",
            error.message
        );
        assert!(error.message.contains("shared"), "{}", error.message);
    }

    #[test]
    fn resolve_dependency_manifest_span() {
        let dir = TestDir::new("resolve-dependency-span");
        dir.write("bad/lsaca.toml", "[package]\n");
        dir.write("bad/src/lib.ls", "");
        let root = dir.write(
            "app/lsaca.toml",
            &manifest_text("app", None, &[("bad", "../bad")]),
        );
        let (sources, diagnostics, graph) = resolve_file(&root);
        assert!(graph.is_none());
        let error = single_error(&diagnostics);
        assert!(error.message.contains("missing"), "{}", error.message);
        assert!(error.message.contains("name"), "{}", error.message);
        assert_eq!(sources.len(), 2);
        let (file, line, column) = sources.location(error.span).expect("span location");
        assert!(
            file.name().ends_with("bad/lsaca.toml"),
            "unexpected file: {}",
            file.name()
        );
        assert_eq!((line, column), (1, 1));
    }

    #[test]
    fn resolve_missing_root_manifest() {
        let dir = TestDir::new("resolve-missing-root");
        let root = dir.path().join("nested/lsaca.toml");
        let (sources, diagnostics, graph) = resolve_file(&root);
        assert!(graph.is_none());
        let error = single_error(&diagnostics);
        assert!(
            error.message.contains("could not read package manifest"),
            "{}",
            error.message
        );
        assert_eq!(sources.len(), 1);
        assert_at(&sources, error.span, (1, 1));
    }

    #[test]
    fn resolve_deep_chain() {
        let dir = TestDir::new("resolve-deep-chain");
        for index in 0..15 {
            let name = format!("p{index}");
            let dependency = if index + 1 < 15 {
                format!(
                    "[dependencies]\np{} = {{ path = \"../p{}\" }}\n",
                    index + 1,
                    index + 1
                )
            } else {
                String::new()
            };
            let text = format!("[package]\nname = \"{name}\"\n{dependency}");
            dir.write(&format!("p{index}/lsaca.toml"), &text);
            dir.write(&format!("p{index}/src/lib.ls"), "");
        }
        let root = dir.write(
            "app/lsaca.toml",
            &manifest_text("app", None, &[("p0", "../p0")]),
        );
        let (_, diagnostics, graph) = resolve_file(&root);
        assert!(diagnostics.is_empty(), "{:?}", messages(&diagnostics));
        let graph = graph.expect("deep chain should resolve");
        assert_eq!(graph.packages.len(), 16);
        assert_eq!(graph.root, 15);
        assert_eq!(graph.packages[15].name, "app");
        assert!(!graph.packages[15].is_dependency);
        assert!(graph.packages[0].dependencies.is_empty());
        for index in 0..15 {
            assert!(graph.packages[index].is_dependency);
            assert_eq!(graph.packages[index].name, format!("p{}", 14 - index));
        }
        for index in 1..15 {
            assert_eq!(graph.packages[index].dependencies, vec![index - 1]);
        }
    }
}
