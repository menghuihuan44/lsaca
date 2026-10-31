//! Diagnostics: errors and warnings with source spans, rendered in a
//! rustc-like format.

use std::fmt::Write as _;

use crate::span::SourceFile;

pub use crate::span::Span;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Severity {
    Error,
    Warning,
    Note,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Note => "note",
        }
    }
}

/// An extra source annotation attached to a diagnostic.
#[derive(Clone, Debug)]
pub struct Label {
    pub span: Span,
    pub message: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Diagnostic {
    pub severity: Severity,
    /// Optional stable machine-readable code, for example "E0308".
    pub code: Option<&'static str>,
    pub message: String,
    pub span: Span,
    pub labels: Vec<Label>,
    pub notes: Vec<String>,
}

impl Diagnostic {
    pub fn error(message: impl Into<String>, span: Span) -> Diagnostic {
        Diagnostic {
            severity: Severity::Error,
            code: None,
            message: message.into(),
            span,
            labels: Vec::new(),
            notes: Vec::new(),
        }
    }

    pub fn warning(message: impl Into<String>, span: Span) -> Diagnostic {
        Diagnostic {
            severity: Severity::Warning,
            ..Diagnostic::error(message, span)
        }
    }

    pub fn with_code(mut self, code: &'static str) -> Diagnostic {
        self.code = Some(code);
        self
    }

    pub fn with_label(mut self, span: Span, message: impl Into<String>) -> Diagnostic {
        self.labels.push(Label {
            span,
            message: Some(message.into()),
        });
        self
    }

    pub fn with_span_label(mut self, span: Span) -> Diagnostic {
        self.labels.push(Label {
            span,
            message: None,
        });
        self
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Diagnostic {
        self.notes.push(note.into());
        self
    }

    pub fn span(&self) -> Span {
        self.span
    }
}

/// An ordered collection of diagnostics. Front-end phases append to the same
/// buffer so a file can report multiple independent problems in one run.
#[derive(Clone, Debug, Default)]
pub struct Diagnostics {
    items: Vec<Diagnostic>,
}

impl Diagnostics {
    pub fn new() -> Diagnostics {
        Diagnostics { items: Vec::new() }
    }

    pub fn with_capacity(capacity: usize) -> Diagnostics {
        Diagnostics {
            items: Vec::with_capacity(capacity),
        }
    }

    pub fn push(&mut self, diagnostic: Diagnostic) {
        self.items.push(diagnostic);
    }

    pub fn error(&mut self, message: impl Into<String>, span: Span) {
        self.push(Diagnostic::error(message, span));
    }

    pub fn warning(&mut self, message: impl Into<String>, span: Span) {
        self.push(Diagnostic::warning(message, span));
    }

    pub fn extend(&mut self, other: Diagnostics) {
        self.items.extend(other.items);
    }

    pub fn append(&mut self, other: &mut Diagnostics) {
        self.items.append(&mut other.items);
    }

    pub fn has_errors(&self) -> bool {
        self.items.iter().any(|d| d.severity == Severity::Error)
    }

    pub fn has_warnings(&self) -> bool {
        self.items.iter().any(|d| d.severity == Severity::Warning)
    }

    pub fn error_count(&self) -> usize {
        self.items
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .count()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Diagnostic> {
        self.items.iter()
    }

    pub fn into_vec(self) -> Vec<Diagnostic> {
        self.items
    }

    pub fn as_slice(&self) -> &[Diagnostic] {
        &self.items
    }

    /// Render every diagnostic against a source file.
    pub fn render(&self, file: &SourceFile) -> String {
        let mut out = String::new();
        for diagnostic in &self.items {
            render_diagnostic(&mut out, file, diagnostic);
        }
        out
    }

    /// Render and print every diagnostic to stderr.
    pub fn print(&self, file: &SourceFile, mut writer: impl std::io::Write) {
        let text = self.render(file);
        let _ = writer.write_all(text.as_bytes());
    }
}

/// Sort diagnostics by source position and de-duplicate exact duplicates.
pub fn sort_and_dedup(diagnostics: &mut Diagnostics) {
    diagnostics
        .items
        .sort_by_key(|d| (d.span.start, d.span.end));
    diagnostics
        .items
        .dedup_by(|a, b| a.message == b.message && a.span == b.span);
}

/// Render one diagnostic, including a caret underline for the primary span.
pub fn render_diagnostic(out: &mut String, file: &SourceFile, diagnostic: &Diagnostic) {
    let (line, column) = file.line_col(diagnostic.span.start);
    match diagnostic.code {
        Some(code) => {
            let _ = writeln!(
                out,
                "{}[{}]: {}",
                diagnostic.severity.as_str(),
                code,
                diagnostic.message
            );
        }
        None => {
            let _ = writeln!(
                out,
                "{}: {}",
                diagnostic.severity.as_str(),
                diagnostic.message
            );
        }
    }

    let line_text = file.line_text(line);
    let gutter_width = format!("{}", line).len();
    let _ = writeln!(
        out,
        "{:width$}--> {}:{}:{}",
        "",
        file.name(),
        line,
        column,
        width = gutter_width + 1
    );
    let _ = writeln!(out, "{:width$} |", "", width = gutter_width + 1);

    // Caret underline for the primary span, clipped to the displayed line.
    let line_span = file.line_span(line);
    let span_start = diagnostic.span.start.max(line_span.start);
    let span_end = diagnostic.span.end.min(line_span.end).max(span_start);
    let prefix_chars = file
        .text()
        .get(line_span.start as usize..span_start as usize)
        .map(|s| s.chars().count())
        .unwrap_or(0);
    let caret_count = file
        .text()
        .get(span_start as usize..span_end as usize)
        .map(|s| s.chars().count().max(1))
        .unwrap_or(1);

    // Machine-generated sources can have megabyte-long lines, and a broken
    // program can produce thousands of diagnostics; each one quoting the full
    // line would multiply the output by the file length. Only a window around
    // the primary span is displayed, with "..." marking the omitted text.
    const MAX_RENDERED_LINE_CHARS: usize = 160;
    let line_chars: Vec<char> = line_text.chars().collect();
    let (shown_text, shown_prefix, shown_caret) = if line_chars.len() <= MAX_RENDERED_LINE_CHARS {
        let shown_caret = caret_count.min(line_chars.len().saturating_sub(prefix_chars));
        (line_text.to_string(), prefix_chars, shown_caret.max(1))
    } else {
        let window_len = MAX_RENDERED_LINE_CHARS;
        let mut start = prefix_chars.saturating_sub(window_len / 3);
        if start + window_len > line_chars.len() {
            start = line_chars.len() - window_len;
        }
        let end = start + window_len;
        let lead = if start > 0 { "..." } else { "" };
        let tail = if end < line_chars.len() { "..." } else { "" };
        let window: String = line_chars[start..end].iter().collect();
        let relative_prefix = prefix_chars.saturating_sub(start);
        let shown_caret = caret_count
            .min(window_len.saturating_sub(relative_prefix))
            .max(1);
        (
            format!("{lead}{window}{tail}"),
            lead.len() + relative_prefix,
            shown_caret,
        )
    };
    let _ = writeln!(out, " {} | {}", line, shown_text);
    let _ = writeln!(
        out,
        "{:width$} | {}{}",
        "",
        " ".repeat(shown_prefix),
        "^".repeat(shown_caret),
        width = gutter_width + 1
    );

    for label in &diagnostic.labels {
        if let Some(message) = &label.message {
            let (l_line, l_col) = file.line_col(label.span.start);
            let _ = writeln!(
                out,
                "{:width$} = note: {}:{}:{}: {}",
                "",
                file.name(),
                l_line,
                l_col,
                message,
                width = gutter_width + 1
            );
        }
    }
    for note in &diagnostic.notes {
        let _ = writeln!(
            out,
            "{:width$} = note: {}",
            "",
            note,
            width = gutter_width + 1
        );
    }
    let _ = writeln!(out);
}

/// Convenience: render a batch of raw diagnostics for tests and the CLI.
pub fn render_all(file: &SourceFile, diagnostics: &[Diagnostic]) -> String {
    let mut out = String::new();
    for diagnostic in diagnostics {
        render_diagnostic(&mut out, file, diagnostic);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_caret() {
        let file = SourceFile::new("t.ls", "let x = 1;\n");
        let mut diagnostics = Diagnostics::new();
        diagnostics.push(Diagnostic::error("unexpected token", Span::new(4, 5)).with_code("E0001"));
        let text = diagnostics.render(&file);
        assert!(text.contains("error[E0001]: unexpected token"));
        assert!(text.contains("--> t.ls:1:5"));
        assert!(text.contains("    ^"));
    }

    #[test]
    fn clips_very_long_source_lines() {
        let mut source = String::new();
        source.push_str(&"a".repeat(500));
        source.push_str("bad");
        source.push_str(&"b".repeat(500));
        source.push('\n');
        let file = SourceFile::new("t.ls", &source);
        let mut diagnostics = Diagnostics::new();
        diagnostics
            .push(Diagnostic::error("unexpected token", Span::new(500, 503)).with_code("E0001"));
        let rendered = diagnostics.render(&file);
        assert!(rendered.contains("..."));
        assert!(rendered.contains("error[E0001]"));
        assert!(rendered.contains('^'));
        for line in rendered.lines() {
            assert!(
                line.chars().count() <= 200,
                "rendered line was too long: {} chars",
                line.chars().count()
            );
        }
    }

    #[test]
    fn counts_errors() {
        let mut diagnostics = Diagnostics::new();
        diagnostics.error("a", Span::empty(0));
        diagnostics.warning("b", Span::empty(1));
        assert!(diagnostics.has_errors());
        assert_eq!(diagnostics.error_count(), 1);
    }
}
