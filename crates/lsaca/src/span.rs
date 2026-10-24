//! Source locations.
//!
//! Every token, AST node, HIR node and diagnostic carries a [Span]: a pair of
//! byte offsets into the original source text. Spans are deliberately small
//! (two u32 values) so they can be copied freely through the compiler.

use std::fmt;

/// A half-open byte range into a source file. The start is inclusive, the end
/// is exclusive. An empty span (start == end) points at a single position and
/// is used for expected-token / end-of-file diagnostics.
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
    /// Byte offset of the start of every line; line_starts[0] is always 0.
    line_starts: Vec<u32>,
}

impl SourceFile {
    pub fn new(name: impl Into<String>, text: impl Into<String>) -> SourceFile {
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
            line_starts,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn len(&self) -> usize {
        self.text.len()
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
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

    /// Convert a byte offset into a 1-based (line, column) pair. The column is
    /// counted in Unicode scalar values so diagnostics line up with the text
    /// displayed in a terminal.
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

    /// The (zero-based) line index containing a byte offset.
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
}
