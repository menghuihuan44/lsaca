//! A conservative source formatter for lsaca files.
//!
//! The formatter works on the token structure of each line instead of on the
//! AST so that comments, blank lines and the exact spelling of expressions
//! survive a formatting run. It normalises:
//!
//! - line endings to LF
//! - indentation to four spaces per nesting level of parentheses, brackets and
//!   braces
//! - continuation lines that start with a method call dot get one extra level
//! - trailing whitespace is removed
//! - runs of blank lines collapse to a single blank line
//! - the file ends with exactly one newline
//!
//! The interior spacing of a line is never rewritten, so formatting is
//! conservative and formatting twice produces the same text. Comments are
//! preserved verbatim, including multi line block comments.
//!
//! The command line driver uses this module after a successful parse, so a file
//! with syntax errors is never modified.

/// Format a whole source file.
///
/// The result always uses LF line endings and ends with exactly one newline.
/// Running this function on its own output returns the same text.
pub fn format_source(source: &str) -> String {
    let normalized = source.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = normalized.split('\n').collect();
    let mut formatted: Vec<String> = Vec::new();
    let mut depth: i32 = 0;
    let mut block_depth: usize = 0;
    let mut last_was_blank = false;

    for (index, raw) in lines.iter().enumerate() {
        let is_last_segment = index + 1 == lines.len();
        let content = raw.trim_end();
        if is_last_segment && content.is_empty() {
            break;
        }
        if block_depth > 0 {
            // A continuation line of a block comment is preserved as written;
            // only trailing whitespace is removed.
            formatted.push(content.to_string());
            last_was_blank = false;
            let mut state = block_depth;
            scan_line(content, &mut state);
            block_depth = state;
            continue;
        }
        let mut state = block_depth;
        let scan = scan_line(content, &mut state);
        block_depth = state;
        if content.trim().is_empty() {
            if !last_was_blank {
                formatted.push(String::new());
                last_was_blank = true;
            }
            continue;
        }
        last_was_blank = false;
        let trimmed = content.trim_start();
        let mut indent = depth - scan.leading_closers;
        if indent < 0 {
            indent = 0;
        }
        if scan.starts_with_dot && scan.leading_closers == 0 && indent == depth {
            indent += 1;
        }
        let mut line = String::with_capacity(trimmed.len() + indent as usize * 4);
        for _ in 0..indent {
            line.push_str("    ");
        }
        line.push_str(trimmed);
        formatted.push(line);
        depth += scan.delta;
        if depth < 0 {
            depth = 0;
        }
    }
    while formatted.first().is_some_and(|line| line.is_empty()) {
        formatted.remove(0);
    }
    while formatted.last().is_some_and(|line| line.is_empty()) {
        formatted.pop();
    }
    let mut text = formatted.join("\n");
    text.push('\n');
    text
}

/// What a single line contributes to the surrounding structure.
struct LineScan {
    /// How many closing delimiters the line starts with. Each one dedents the
    /// line itself by one level.
    leading_closers: i32,
    /// Net delimiter change caused by the code on the line.
    delta: i32,
    /// Whether the first significant character is a dot (a method chain
    /// continuation).
    starts_with_dot: bool,
    /// Whether the line contained anything except whitespace and comments.
    has_code: bool,
}

fn scan_line(line: &str, block_depth: &mut usize) -> LineScan {
    let chars: Vec<char> = line.chars().collect();
    let mut scan = LineScan {
        leading_closers: 0,
        delta: 0,
        starts_with_dot: false,
        has_code: false,
    };
    let mut index = 0;
    while index < chars.len() {
        let ch = chars[index];
        if *block_depth > 0 {
            if ch == '/' && chars.get(index + 1) == Some(&'*') {
                *block_depth += 1;
                index += 2;
                continue;
            }
            if ch == '*' && chars.get(index + 1) == Some(&'/') {
                *block_depth -= 1;
                index += 2;
                continue;
            }
            index += 1;
            continue;
        }
        match ch {
            '/' if chars.get(index + 1) == Some(&'/') => break,
            '/' if chars.get(index + 1) == Some(&'*') => {
                *block_depth += 1;
                index += 2;
                continue;
            }
            '"' => {
                begin_code(&mut scan, ch);
                index += 1;
                while index < chars.len() {
                    if chars[index] == '\\' {
                        index += 2;
                        continue;
                    }
                    if chars[index] == '"' {
                        index += 1;
                        break;
                    }
                    index += 1;
                }
                continue;
            }
            '\'' => {
                begin_code(&mut scan, ch);
                let next = chars.get(index + 1).copied();
                let after = chars.get(index + 2).copied();
                if next == Some('\\') {
                    // An escaped character literal such as '\\n' or '{'.
                    index += 2;
                    while index < chars.len() && chars[index] != '\'' {
                        if chars[index] == '\\' {
                            index += 1;
                        }
                        index += 1;
                    }
                    if index < chars.len() {
                        index += 1;
                    }
                } else if after == Some('\'') {
                    // A plain character literal such as 'a' or '{'.
                    index += 3;
                } else {
                    // A lifetime such as 'a or 'static; no terminator.
                    index += 1;
                }
                continue;
            }
            '{' | '(' | '[' => {
                begin_code(&mut scan, ch);
                scan.delta += 1;
                index += 1;
                continue;
            }
            '}' | ')' | ']' => {
                if !scan.has_code {
                    scan.leading_closers += 1;
                }
                scan.has_code = true;
                scan.delta -= 1;
                index += 1;
                continue;
            }
            ch if ch.is_whitespace() => {
                index += 1;
                continue;
            }
            ch => {
                begin_code(&mut scan, ch);
                index += 1;
                continue;
            }
        }
    }
    scan
}

fn begin_code(scan: &mut LineScan, ch: char) {
    if !scan.has_code {
        scan.has_code = true;
        scan.starts_with_dot = ch == '.';
    }
}

#[cfg(test)]
mod tests {
    use super::format_source;

    fn formatted(source: &str) -> String {
        format_source(source)
    }

    #[test]
    fn indents_nested_blocks() {
        let source = "fn main() {\nlet x = 1;\nif x > 0 {\nprintln!(\"positive\");\n}\n}\n";
        let expected = "fn main() {\n    let x = 1;\n    if x > 0 {\n        println!(\"positive\");\n    }\n}\n";
        assert_eq!(formatted(source), expected);
    }

    #[test]
    fn dedents_closing_delimiters() {
        let source = "fn main() {\n    let value = (\n1\n+ 2\n);\n}\n";
        let expected = "fn main() {\n    let value = (\n        1\n        + 2\n    );\n}\n";
        assert_eq!(formatted(source), expected);
    }

    #[test]
    fn keeps_strings_and_characters_intact() {
        let source = "fn main() {\nlet brace = \"{\";\nlet closing: char = '}';\nlet opening: char = '{';\nprintln!(\"{} {}\", brace, closing);\n}\n";
        let expected = "fn main() {\n    let brace = \"{\";\n    let closing: char = '}';\n    let opening: char = '{';\n    println!(\"{} {}\", brace, closing);\n}\n";
        assert_eq!(formatted(source), expected);
    }

    #[test]
    fn ignores_lifetimes() {
        let source = "fn first<'a>(values: &'a str) -> &'a str {\nvalues\n}\n";
        let expected = "fn first<'a>(values: &'a str) -> &'a str {\n    values\n}\n";
        assert_eq!(formatted(source), expected);
    }

    #[test]
    fn preserves_block_comments() {
        let source = "fn main() {\n/* a comment with { and } */\nlet x = 1;\n}\n";
        let expected = "fn main() {\n    /* a comment with { and } */\n    let x = 1;\n}\n";
        assert_eq!(formatted(source), expected);
    }

    #[test]
    fn indents_method_chain_continuations() {
        let source = "fn main() {\nlet values = vec![1, 2, 3]\n.iter()\n.map(|value| value + 1)\n.collect();\n}\n";
        let expected = "fn main() {\n    let values = vec![1, 2, 3]\n        .iter()\n        .map(|value| value + 1)\n        .collect();\n}\n";
        assert_eq!(formatted(source), expected);
    }

    #[test]
    fn collapses_blank_lines_and_ends_with_one_newline() {
        let source = "\n\nfn main() {\n\n\n\nlet x = 1;\n}\n\n\n";
        let expected = "fn main() {\n\n    let x = 1;\n}\n";
        assert_eq!(formatted(source), expected);
    }

    #[test]
    fn normalises_line_endings() {
        let source = "fn main() {\r\nlet x = 1;\r\n}\r\n";
        let expected = "fn main() {\n    let x = 1;\n}\n";
        assert_eq!(formatted(source), expected);
    }

    #[test]
    fn formatting_twice_is_idempotent() {
        let source = "fn main() {\nlet values = vec![1, 2, 3];\n/* block {\ncomment */\nfor value in values {\nprintln!(\"{} {}\", value, \"{\");\n}\n}\n";
        let once = formatted(source);
        let twice = formatted(&once);
        assert_eq!(once, twice);
        assert!(once.ends_with("}\n"));
        assert!(!once.contains("\r"));
    }
}
