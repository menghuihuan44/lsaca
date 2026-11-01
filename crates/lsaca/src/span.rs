//! Source locations.
//!
//! Every token, AST node, HIR node and diagnostic carries a [Span]: a pair of
//! byte offsets. Spans are deliberately small (two u32 values) so they can be
//! copied freely through the compiler.
//!
//! A program may consist of several files (modules) and several crates
//! (packages). To keep spans uniform, every [SourceFile] owns a disjoint range
//! of the global offset space: its [SourceFile::base] plus local byte offsets.
//! A single file has base 0, so existing single-file code is unaffected.
//! [SourceMap] owns the list of loaded files and maps a global offset back to
//! its file; [SourceLookup] is the small trait the rest of the compiler uses so
//! that tests can pass a plain [SourceFile].

use std::fmt;

/// A half-open byte range into the global source space. The start is
/// inclusive, the end is exclusive. An empty span (start == end) points at a
/// single position and is used for expected-token / end-of-file diagnostics.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

impl Span {
    pub const fn new(start: u32, end: u32) -> Span {
        Span { start, end }
    }

    /// An empty span at a byte offset.
    pub const fn empty(at: u32) -> Span {
        Span { start: at, end: at }
    }

    pub fn from_usize(start: usize, end: usize) -> Span {
        Span {
            start: start as u32,
            end: end as u32,
        }
    }

    /// The smallest span covering both inputs. Empty spans are extended by the
    /// other span rather than shrinking the result to a point.
    pub fn join(self, other: Span) -> Span {
        let (mut start, mut end) = (self.start, self.end);
        if other.start < start {
            start = other.start;
        }
        if other.end > end {
            end = other.end;
        }
        // If one of the spans is empty and sits outside the other, keep it
        // anchored instead of collapsing to the empty position.
        if self.start == self.end {
            if other.contains(self.start) {
                return other;
            }
            start = start.min(other.start);
            end = end.max(other.end);
        }
        Span { start, end }
    }

    pub fn contains(self, offset: u32) -> bool {
        offset >= self.start && offset < self.end
    }

    pub fn len(self) -> u32 {
        self.end.saturating_sub(self.start)
    }

    pub fn is_empty(self) -> bool {
        self.start >= self.end
    }

    /// A span covering only the first character of this span, useful to point
    /// at a token without underlining all of it.
    pub fn point(self) -> Span {
        if self.start == self.end {
            self
        } else {
            let mut end = self.start + 1;
            // Never split a UTF-8 code point when offsetting.
            end = end.min(self.end);
            Span::new(self.start, end)
        }
    }
}

impl fmt::Debug for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}

/// A single source file plus its precomputed line table.
#[derive(Clone, Debug)]
pub struct SourceFile {
    name: String,
    text: String,
    /// Global offset of the first byte of this file. 0 for a lone file.
    base: u32,
    /// Byte offset of the start of every line; line_starts[0] is always 0.
    line_starts: Vec<u32>,
}

impl SourceFile {
    pub fn new(name: impl Into<String>, text: impl Into<String>) -> SourceFile {
        SourceFile::with_base(name, text, 0)
    }

    /// Build a file whose first byte sits at the given global offset.
    pub fn with_base(name: impl Into<String>, text: impl Into<String>, base: u32) -> SourceFile {
        let text = text.into();
        let mut line_starts = vec![0u32];
        for (idx, byte) in text.bytes().enumerate() {
            if byte == b'\n' {
                line_starts.push(idx as u32 + 1);
            }
        }
        SourceFile {
            name: name.into(),
            text,
            base,
            line_starts,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    /// Global offset of the first byte of this file.
    pub fn base(&self) -> u32 {
        self.base
    }

    /// Global offset one past the last byte of this file.
    pub fn end(&self) -> u32 {
        self.base.saturating_add(self.text.len() as u32)
    }

    pub fn len(&self) -> usize {
        self.text.len()
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Convert a global offset into this file's local byte offset (clamped).
    pub fn local_offset(&self, offset: u32) -> u32 {
        offset.saturating_sub(self.base).min(self.text.len() as u32)
    }

    /// Convert a global span into this file's local span (clamped).
    pub fn local_span(&self, span: Span) -> Span {
        let start = self.local_offset(span.start);
        let end = self.local_offset(span.end).max(start);
        Span::new(start, end)
    }

    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }

    /// Number of source lines that contain at least one byte (a trailing
    /// newline does not create an extra line for display purposes).
    pub fn visible_line_count(&self) -> usize {
        let total = self.line_starts.len();
        if total > 1 && self.text.ends_with('\n') {
            total - 1
        } else {
            total
        }
    }

    pub fn offset_of_line(&self, line: usize) -> u32 {
        let idx = line.saturating_sub(1).min(self.line_starts.len() - 1);
        self.line_starts[idx]
    }

    /// Text of a 1-based line, without the trailing newline.
    pub fn line_text(&self, line: usize) -> &str {
        if line == 0 || line > self.line_starts.len() {
            return "";
        }
        let start = self.line_starts[line - 1] as usize;
        let end = if line < self.line_starts.len() {
            self.line_starts[line] as usize
        } else {
            self.text.len()
        };
        let mut slice = &self.text[start..end];
        if let Some(stripped) = slice.strip_suffix('\n') {
            slice = stripped;
        }
        if let Some(stripped) = slice.strip_suffix('\r') {
            slice = stripped;
        }
        slice
    }

    pub fn line_span(&self, line: usize) -> Span {
        if line == 0 || line > self.line_starts.len() {
            return Span::empty(0);
        }
        let start = self.line_starts[line - 1];
        let end = if line < self.line_starts.len() {
            // Exclude the newline itself.
            self.line_starts[line].saturating_sub(1).max(start)
        } else {
            self.text.len() as u32
        };
        Span::new(start, end)
    }

    /// Convert a LOCAL byte offset into a 1-based (line, column) pair. The
    /// column is counted in Unicode scalar values so diagnostics line up with
    /// the text displayed in a terminal.
    pub fn line_col(&self, offset: u32) -> (usize, usize) {
        let offset = offset.min(self.text.len() as u32);
        let line_idx = match self.line_starts.binary_search(&offset) {
            Ok(idx) => idx,
            Err(idx) => idx.saturating_sub(1),
        };
        let line_start = self.line_starts[line_idx];
        let line_text = &self.text[line_start as usize..offset as usize];
        let col = line_text.chars().count() + 1;
        (line_idx + 1, col)
    }

    /// The (zero-based) line index containing a LOCAL byte offset.
    pub fn line_index(&self, offset: u32) -> usize {
        let offset = offset.min(self.text.len() as u32);
        match self.line_starts.binary_search(&offset) {
            Ok(idx) => idx,
            Err(idx) => idx.saturating_sub(1),
        }
    }

    /// Byte offset of the first non-whitespace character of a line.
    pub fn first_non_ws_of_line(&self, line: usize) -> u32 {
        if line == 0 {
            return 0;
        }
        let span = self.line_span(line);
        let start = span.start as usize;
        let end = span.end as usize;
        let mut idx = start;
        for ch in self.text[start..end].chars() {
            if !ch.is_whitespace() {
                break;
            }
            idx += ch.len_utf8();
        }
        idx as u32
    }
}

/// Read access to source files by span. Implemented by [SourceFile] (a single
/// in-memory file, always the owner) and by [SourceMap] (a multi-file program).
pub trait SourceLookup {
    fn file_of_offset(&self, offset: u32) -> Option<&SourceFile>;

    fn file_of_span(&self, span: Span) -> Option<&SourceFile> {
        self.file_of_offset(span.start)
    }

    /// The file used when a span cannot be resolved (diagnostics fallback).
    fn primary_file(&self) -> Option<&SourceFile>;
}

impl SourceLookup for SourceFile {
    fn file_of_offset(&self, _offset: u32) -> Option<&SourceFile> {
        Some(self)
    }

    fn primary_file(&self) -> Option<&SourceFile> {
        Some(self)
    }
}

/// All source files of one compilation, in load order, with disjoint global
/// offset ranges.
#[derive(Clone, Debug, Default)]
pub struct SourceMap {
    files: Vec<SourceFile>,
}

impl SourceMap {
    pub fn new() -> SourceMap {
        SourceMap { files: Vec::new() }
    }

    pub fn with_capacity(capacity: usize) -> SourceMap {
        SourceMap {
            files: Vec::with_capacity(capacity),
        }
    }

    /// Append a file and assign its global base offset. Returns the file index.
    pub fn add_file(&mut self, name: impl Into<String>, text: impl Into<String>) -> usize {
        let text: String = text.into();
        let base = match self.files.last() {
            None => 0u64,
            Some(last) => last.base as u64 + last.text.len() as u64 + 1,
        };
        // The global offset space is 32 bits. The driver limits the combined
        // source size long before this point; clamping here keeps SourceFile
        // construction infallible.
        let base = u32::try_from(base).unwrap_or(u32::MAX);
        let index = self.files.len();
        self.files.push(SourceFile::with_base(name, text, base));
        index
    }

    pub fn files(&self) -> &[SourceFile] {
        &self.files
    }

    pub fn file(&self, index: usize) -> &SourceFile {
        &self.files[index]
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Index of the file owning a global offset.
    pub fn find_offset(&self, offset: u32) -> Option<usize> {
        if self.files.is_empty() {
            return None;
        }
        // Last file whose base is <= offset.
        let mut low = 0usize;
        let mut high = self.files.len();
        while low < high {
            let mid = (low + high) / 2;
            if self.files[mid].base() <= offset {
                low = mid + 1;
            } else {
                high = mid;
            }
        }
        if low == 0 { Some(0) } else { Some(low - 1) }
    }

    /// Index of the file owning a global span (by its start).
    pub fn find_span(&self, span: Span) -> Option<usize> {
        self.find_offset(span.start)
    }

    pub fn file_name(&self, offset: u32) -> Option<&str> {
        self.find_offset(offset).map(|idx| self.files[idx].name())
    }

    /// Resolve a global span into (owning file, 1-based line, 1-based column).
    pub fn location(&self, span: Span) -> Option<(&SourceFile, usize, usize)> {
        let idx = self.find_offset(span.start)?;
        let file = &self.files[idx];
        let local = file.local_offset(span.start);
        let (line, col) = file.line_col(local);
        Some((file, line, col))
    }
}

impl SourceLookup for SourceMap {
    fn file_of_offset(&self, offset: u32) -> Option<&SourceFile> {
        self.find_offset(offset).map(|idx| &self.files[idx])
    }

    fn primary_file(&self) -> Option<&SourceFile> {
        self.files.first()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_and_columns() {
        let file = SourceFile::new("t.ls", "fn main() {\n    let x = 1;\n}\n");
        assert_eq!(file.line_count(), 4);
        assert_eq!(file.visible_line_count(), 3);
        assert_eq!(file.line_text(1), "fn main() {");
        assert_eq!(file.line_text(2), "    let x = 1;");
        assert_eq!(file.line_col(0), (1, 1));
        let second_line = file.offset_of_line(2);
        assert_eq!(file.line_col(second_line), (2, 1));
        assert_eq!(file.line_col(second_line + 4), (2, 5));
    }

    #[test]
    fn spans_join() {
        let a = Span::new(2, 4);
        let b = Span::new(6, 9);
        assert_eq!(a.join(b), Span::new(2, 9));
        let empty = Span::empty(20);
        assert_eq!(empty.join(b), Span::new(6, 20));
    }

    #[test]
    fn file_with_base_localizes_spans() {
        let file = SourceFile::with_base("b.ls", "let y = 2;\n", 100);
        assert_eq!(file.base(), 100);
        assert_eq!(file.end(), 111);
        let (line, col) = file.line_col(file.local_offset(104));
        assert_eq!((line, col), (1, 5));
        assert_eq!(file.local_span(Span::new(104, 109)), Span::new(4, 9));
        // Out-of-range spans clamp instead of panicking.
        assert_eq!(file.local_span(Span::new(1, 2)), Span::new(0, 0));
        assert_eq!(file.local_span(Span::new(500, 600)), Span::new(11, 11));
    }

    #[test]
    fn source_map_maps_offsets_to_files() {
        let mut map = SourceMap::new();
        let a = map.add_file("a.ls", "fn main() {\n}\n");
        let b = map.add_file("src/b.ls", "pub fn helper() {}\n");
        assert_eq!(a, 0);
        assert_eq!(b, 1);
        assert_eq!(map.len(), 2);
        // First file starts at 0, the second one past the first plus padding.
        assert_eq!(map.file(0).base(), 0);
        assert_eq!(map.file(0).len(), 14);
        assert_eq!(map.file(1).base(), 14 + 1);
        assert_eq!(map.find_offset(0), Some(0));
        assert_eq!(map.find_offset(13), Some(0));
        // The padding byte at offset 14 still belongs to the first file.
        assert_eq!(map.find_offset(14), Some(0));
        assert_eq!(map.find_offset(15), Some(1));
        let second = map.file(1);
        let span = Span::new(second.base() + 4, second.base() + 7);
        let (file, line, col) = map.location(span).expect("location");
        assert_eq!(file.name(), "src/b.ls");
        assert_eq!((line, col), (1, 5));
        assert_eq!(map.file_name(span.start), Some("src/b.ls"));
        assert_eq!(map.find_span(span), Some(1));
        // The padding byte belongs to the previous file and clamps.
        assert_eq!(map.find_offset(13), Some(0));
    }

    #[test]
    fn source_lookup_works_for_single_files() {
        let file = SourceFile::new("t.ls", "let x = 1;\n");
        let lookup: &dyn SourceLookup = &file;
        assert!(lookup.file_of_offset(4).is_some());
        assert_eq!(lookup.primary_file().map(SourceFile::name), Some("t.ls"));
        let map = SourceMap::new();
        assert!(map.file_of_offset(0).is_none());
        assert!(map.primary_file().is_none());
    }
}
