// markdown.ls -- a Markdown subset to HTML converter.
//
// Realistic example program with a byte-identical C counterpart (markdown.c).
//
//   lsaca run examples/programs/markdown.ls
//   lsaca run examples/programs/markdown.ls -- document.md
//
// Supported blocks: headings, paragraphs, unordered lists, block quotes and
// fenced code blocks. Supported inline markup: **bold**, *italic*, `code`,
// [links](url) and backslash escapes. Block elements are written one per
// line; the converter never emits blank lines between blocks.

struct Stats {
    headings: usize,
    paragraphs: usize,
    lists: usize,
    items: usize,
    quotes: usize,
    code_blocks: usize,
}

impl Stats {
    fn new() -> Stats {
        return Stats { headings: 0, paragraphs: 0, lists: 0, items: 0, quotes: 0, code_blocks: 0 };
    }
    fn report(&self) -> String {
        return format!(
            "blocks: headings={} paragraphs={} lists={} items={} quotes={} code={}",
            self.headings, self.paragraphs, self.lists, self.items, self.quotes, self.code_blocks
        );
    }
}

fn push_escaped(out: &mut String, ch: char) {
    if ch == '&' {
        out.push_str("&amp;");
    } else if ch == '<' {
        out.push_str("&lt;");
    } else if ch == '>' {
        out.push_str("&gt;");
    } else if ch == '"' {
        out.push_str("&quot;");
    } else {
        out.push(ch);
    }
}

fn escape_text(out: &mut String, text: &str) {
    let chars: Vec<char> = text.chars();
    let mut index: usize = 0;
    while index < chars.len() {
        push_escaped(out, chars[index]);
        index += 1;
    }
}

fn slice_text(chars: &Vec<char>, start: usize, end: usize) -> String {
    let mut text = String::new();
    let mut index: usize = start;
    while index < end && index < chars.len() {
        text.push(chars[index]);
        index += 1;
    }
    return text;
}

fn find_char(chars: &Vec<char>, from: usize, needle: char) -> usize {
    let mut index: usize = from;
    while index < chars.len() {
        if chars[index] == needle {
            return index;
        }
        index += 1;
    }
    return chars.len();
}

fn find_pair(chars: &Vec<char>, from: usize) -> usize {
    let mut index: usize = from;
    while index + 1 < chars.len() {
        if chars[index] == '*' && chars[index + 1] == '*' {
            return index;
        }
        index += 1;
    }
    return chars.len();
}

fn render_inline(out: &mut String, text: &str) {
    let chars: Vec<char> = text.chars();
    let mut index: usize = 0;
    while index < chars.len() {
        let ch = chars[index];
        if ch == '\\' {
            if index + 1 < chars.len() {
                push_escaped(out, chars[index + 1]);
                index += 2;
            } else {
                push_escaped(out, ch);
                index += 1;
            }
        } else if ch == '`' {
            let close = find_char(&chars, index + 1, '`');
            if close < chars.len() {
                let inner = slice_text(&chars, index + 1, close);
                out.push_str("<code>");
                escape_text(out, inner.as_str());
                out.push_str("</code>");
                index = close + 1;
            } else {
                push_escaped(out, ch);
                index += 1;
            }
        } else if ch == '*' {
            if index + 1 < chars.len() && chars[index + 1] == '*' {
                let close = find_pair(&chars, index + 2);
                if close < chars.len() {
                    let inner = slice_text(&chars, index + 2, close);
                    out.push_str("<strong>");
                    render_inline(out, inner.as_str());
                    out.push_str("</strong>");
                    index = close + 2;
                } else {
                    push_escaped(out, ch);
                    index += 1;
                }
            } else {
                let close = find_char(&chars, index + 1, '*');
                if close < chars.len() && close > index + 1 {
                    let inner = slice_text(&chars, index + 1, close);
                    out.push_str("<em>");
                    render_inline(out, inner.as_str());
                    out.push_str("</em>");
                    index = close + 1;
                } else {
                    push_escaped(out, ch);
                    index += 1;
                }
            }
        } else if ch == '[' {
            let close = find_char(&chars, index + 1, ']');
            if close < chars.len() && close + 1 < chars.len() && chars[close + 1] == '(' {
                let end = find_char(&chars, close + 2, ')');
                if end < chars.len() {
                    let label = slice_text(&chars, index + 1, close);
                    let href = slice_text(&chars, close + 2, end);
                    out.push_str("<a href=\"");
                    escape_text(out, href.as_str());
                    out.push_str("\">");
                    render_inline(out, label.as_str());
                    out.push_str("</a>");
                    index = end + 1;
                } else {
                    push_escaped(out, ch);
                    index += 1;
                }
            } else {
                push_escaped(out, ch);
                index += 1;
            }
        } else {
            push_escaped(out, ch);
            index += 1;
        }
    }
}

fn heading_level(text: &str) -> usize {
    let chars: Vec<char> = text.chars();
    let mut level: usize = 0;
    while level < chars.len() && chars[level] == '#' {
        level += 1;
    }
    if level >= 1 && level <= 6 && level < chars.len() && chars[level] == ' ' {
        return level;
    }
    return 0;
}

fn is_block_start(text: &str) -> bool {
    if heading_level(text) > 0 {
        return true;
    }
    if text.starts_with("- ") {
        return true;
    }
    if text.starts_with(">") {
        return true;
    }
    if text.starts_with("```") {
        return true;
    }
    return false;
}

fn render_document(document: &str, stats: &mut Stats) -> String {
    let mut out = String::new();
    let lines: Vec<String> = document.lines();
    let mut index: usize = 0;
    while index < lines.len() {
        let line = lines[index].clone();
        let trimmed = line.trim();
        if trimmed.chars().len() == 0 {
            index += 1;
        } else if trimmed.starts_with("```") {
            stats.code_blocks += 1;
            out.push_str("<pre><code>");
            index += 1;
            let mut first = true;
            while index < lines.len() {
                let code_line = lines[index].clone();
                let code_trimmed = code_line.trim();
                if code_trimmed.starts_with("```") {
                    index += 1;
                    break;
                }
                if !first {
                    out.push('\n');
                }
                escape_text(&mut out, code_line.as_str());
                first = false;
                index += 1;
            }
            out.push_str("</code></pre>\n");
        } else if heading_level(trimmed) > 0 {
            let level = heading_level(trimmed);
            stats.headings += 1;
            let chars: Vec<char> = trimmed.chars();
            let inner = slice_text(&chars, level + 1, chars.len());
            let display = inner.trim();
            let open = format!("<h{}>", level);
            out.push_str(&open);
            render_inline(&mut out, display);
            let close = format!("</h{}>", level);
            out.push_str(&close);
            out.push('\n');
            index += 1;
        } else if trimmed.starts_with("- ") {
            stats.lists += 1;
            out.push_str("<ul>\n");
            while index < lines.len() {
                let item_line = lines[index].clone();
                let item_trimmed = item_line.trim();
                if !item_trimmed.starts_with("- ") {
                    break;
                }
                let chars: Vec<char> = item_trimmed.chars();
                let inner = slice_text(&chars, 2, chars.len());
                let display = inner.trim();
                out.push_str("<li>");
                render_inline(&mut out, display);
                out.push_str("</li>\n");
                stats.items += 1;
                index += 1;
            }
            out.push_str("</ul>\n");
        } else if trimmed.starts_with(">") {
            stats.quotes += 1;
            let mut quoted = String::new();
            while index < lines.len() {
                let quote_line = lines[index].clone();
                let quote_trimmed = quote_line.trim();
                if !quote_trimmed.starts_with(">") {
                    break;
                }
                let chars: Vec<char> = quote_trimmed.chars();
                let mut start: usize = 1;
                if start < chars.len() && chars[start] == ' ' {
                    start += 1;
                }
                let inner = slice_text(&chars, start, chars.len());
                let text = inner.trim();
                if quoted.len() > 0 {
                    quoted.push(' ');
                }
                quoted.push_str(text);
                index += 1;
            }
            out.push_str("<blockquote>\n<p>");
            render_inline(&mut out, quoted.trim());
            out.push_str("</p>\n</blockquote>\n");
        } else {
            stats.paragraphs += 1;
            let mut paragraph = String::new();
            while index < lines.len() {
                let text_line = lines[index].clone();
                let text_trimmed = text_line.trim();
                if text_trimmed.chars().len() == 0 {
                    break;
                }
                if is_block_start(text_trimmed) {
                    break;
                }
                if paragraph.len() > 0 {
                    paragraph.push(' ');
                }
                paragraph.push_str(text_trimmed);
                index += 1;
            }
            out.push_str("<p>");
            render_inline(&mut out, paragraph.trim());
            out.push_str("</p>\n");
        }
    }
    return out;
}

fn embedded_document() -> String {
    let mut text = String::new();
    text += "# Markdown demo\n";
    text += "\n";
    text += "markdown.ls converts a small Markdown subset to HTML. It handles **bold**\n";
    text += "text, *italic* text, `inline code` and links such as\n";
    text += "[the lsaca repository](https://example.com/lsaca?lang=lsaca&tab=docs).\n";
    text += "\n";
    text += "Escapes keep markup literal: \\*not italic\\*, \\`not code\\`, \\# no heading.\n";
    text += "\n";
    text += "## Lists, quotes and code\n";
    text += "\n";
    text += "- first item\n";
    text += "- second item with **bold** and *emphasis*\n";
    text += "- third item with `x = 1` and a [link](https://example.com/list)\n";
    text += "\n";
    text += "> quoted line one\n";
    text += "> quoted line two with **bold** and `code`\n";
    text += "> and a [link](https://example.com/quote)\n";
    text += "\n";
    text += "```lsaca\n";
    text += "fn main() {\n";
    text += "    println!(\"hello from a fenced block <tag> & more\");\n";
    text += "}\n";
    text += "```\n";
    text += "\n";
    text += "Trailing paragraph with unicode: café, naïve, and 5 < 6 & 7 > 6.\n";
    return text;
}

fn main() {
    println!("markdown: convert a Markdown subset to HTML");
    let document = embedded_document();
    let mut stats = Stats::new();
    let html = render_document(document.as_str(), &mut stats);
    println!("input lines: {}", document.lines().len());
    println!("output bytes: {}", html.len());
    println!("{}", stats.report());
    println!("{}", html);
    let args = std::process::args();
    if args.len() > 0 {
        let path = args[0].clone();
        println!("file: {}", path);
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                let mut file_stats = Stats::new();
                let file_html = render_document(text.as_str(), &mut file_stats);
                println!("file input lines: {}", text.lines().len());
                println!("file output bytes: {}", file_html.len());
                println!("{}", file_stats.report());
                println!("{}", file_html);
            }
            Err(error) => {
                println!("file read error: {}", error.message());
            }
        }
    }
}
