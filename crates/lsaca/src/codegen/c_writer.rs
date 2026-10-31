//! A tiny indentation-aware buffer for emitting C source text.
//!
//! Codegen never builds C with format! chains directly; every line goes through
//! this writer so the generated file keeps a readable, deterministic layout
//! (two spaces per nesting level, no trailing whitespace).

/// Buffered C source writer with two-space indentation.
#[derive(Clone)]
pub struct CWriter {
    out: String,
    indent: usize,
}

impl CWriter {
    pub fn new() -> CWriter {
        CWriter {
            out: String::new(),
            indent: 0,
        }
    }

    /// Append one line at the current indentation level.
    pub fn line(&mut self, text: impl AsRef<str>) {
        let text = text.as_ref();
        if text.is_empty() {
            self.out.push('\n');
            return;
        }
        for _ in 0..self.indent {
            self.out.push_str("  ");
        }
        self.out.push_str(text);
        self.out.push('\n');
    }

    /// Append an empty line.
    pub fn blank(&mut self) {
        self.out.push('\n');
    }

    pub fn indent(&mut self) {
        self.indent += 1;
    }

    pub fn dedent(&mut self) {
        self.indent = self.indent.saturating_sub(1);
    }

    /// Append the complete contents of another writer verbatim.
    ///
    /// The other writer's indentation is already baked into its text, so this
    /// is only used to stitch together sections that had to be generated in a
    /// different order than they appear in the final file.
    pub fn append(&mut self, other: CWriter) {
        self.out.push_str(&other.out);
    }

    /// Consume the writer and return the generated text.
    pub fn finish(self) -> String {
        self.out
    }
}

impl Default for CWriter {
    fn default() -> CWriter {
        CWriter::new()
    }
}

#[cfg(test)]
mod tests {
    use super::CWriter;

    #[test]
    fn indents_and_unindents() {
        let mut w = CWriter::new();
        w.line("int main(void) {");
        w.indent();
        w.line("return 0;");
        w.dedent();
        w.line("}");
        assert_eq!(w.finish(), "int main(void) {\n  return 0;\n}\n");
    }
}
