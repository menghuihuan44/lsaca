// json.ls -- a JSON parser, validator and canonical serializer.
//
// Realistic example program with a byte-identical C counterpart (json.c).
//
//   lsaca run examples/programs/json.ls
//   lsaca run examples/programs/json.ls -- some_file.json
//
// Version 2.2 has no char::from_u32, so only \uXXXX escapes up to U+00FF
// are decoded; higher escapes are reported as an error.
enum Value {
    Null,
    Bool(bool),
    Number(f64),
    Text(String),
    Array(Vec<Value>),
    Object(Vec<(String, Value)>),
}

struct JsonError {
    message: String,
    line: usize,
    column: usize,
}

struct Parser {
    chars: Vec<char>,
    index: usize,
    line: usize,
    column: usize,
}

struct Stats {
    nodes: i64,
    objects: i64,
    arrays: i64,
    strings: i64,
    numbers: i64,
    bools: i64,
    nulls: i64,
    max_depth: i64,
}

impl JsonError {
    fn new(message: &str, line: usize, column: usize) -> JsonError {
        return JsonError { message: String::from_str(message), line: line, column: column };
    }
    fn describe(&self) -> String {
        return format!("{} at line {} column {}", self.message, self.line, self.column);
    }
}

impl Stats {
    fn new() -> Stats {
        return Stats {
            nodes: 0,
            objects: 0,
            arrays: 0,
            strings: 0,
            numbers: 0,
            bools: 0,
            nulls: 0,
            max_depth: 0,
        };
    }
    fn walk(&mut self, value: &Value, depth: i64) {
        self.nodes += 1;
        if depth > self.max_depth {
            self.max_depth = depth;
        }
        match value {
            Value::Null => {
                self.nulls += 1;
            }
            Value::Bool(_flag) => {
                self.bools += 1;
            }
            Value::Number(_number) => {
                self.numbers += 1;
            }
            Value::Text(_text) => {
                self.strings += 1;
            }
            Value::Array(items) => {
                self.arrays += 1;
                let mut index: usize = 0;
                while index < items.len() {
                    self.walk(&items[index], depth + 1);
                    index += 1;
                }
            }
            Value::Object(entries) => {
                self.objects += 1;
                let mut index: usize = 0;
                while index < entries.len() {
                    self.walk(&entries[index].1, depth + 1);
                    index += 1;
                }
            }
        }
    }
}

impl Parser {
    fn new(text: &str) -> Parser {
        return Parser { chars: text.chars(), index: 0, line: 1, column: 1 };
    }
    fn at_end(&self) -> bool {
        return self.index >= self.chars.len();
    }
    fn peek(&self) -> Option<char> {
        if self.index >= self.chars.len() {
            return None;
        }
        return Some(self.chars[self.index]);
    }
    fn advance(&mut self) -> Option<char> {
        if self.index >= self.chars.len() {
            return None;
        }
        let ch = self.chars[self.index];
        self.index += 1;
        if ch == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        return Some(ch);
    }
    fn error(&self, message: &str) -> JsonError {
        return JsonError::new(message, self.line, self.column);
    }
    fn next_is_digit(&self) -> bool {
        match self.peek() {
            Some(ch) => ch >= '0' && ch <= '9',
            None => false,
        }
    }
    fn skip_whitespace(&mut self) {
        loop {
            match self.peek() {
                Some(ch) => {
                    if ch == ' ' || ch == '\t' || ch == '\n' || ch == '\r' {
                        self.advance();
                    } else {
                        return;
                    }
                }
                None => {
                    return;
                }
            }
        }
    }
    fn parse_document(&mut self) -> Result<Value, JsonError> {
        self.skip_whitespace();
        let value = self.parse_value()?;
        self.skip_whitespace();
        if !self.at_end() {
            return Err(self.error("unexpected trailing characters"));
        }
        return Ok(value);
    }
    fn parse_value(&mut self) -> Result<Value, JsonError> {
        self.skip_whitespace();
        let ch = match self.peek() {
            Some(value) => value,
            None => {
                return Err(self.error("unexpected end of input"));
            }
        };
        if ch == '{' {
            return self.parse_object();
        }
        if ch == '[' {
            return self.parse_array();
        }
        if ch == '"' {
            let text = self.parse_string()?;
            return Ok(Value::Text(text));
        }
        if ch == 't' {
            self.expect_word("true")?;
            return Ok(Value::Bool(true));
        }
        if ch == 'f' {
            self.expect_word("false")?;
            return Ok(Value::Bool(false));
        }
        if ch == 'n' {
            self.expect_word("null")?;
            return Ok(Value::Null);
        }
        if ch == '-' || (ch >= '0' && ch <= '9') {
            return self.parse_number();
        }
        return Err(self.error("unexpected character"));
    }
    fn expect_word(&mut self, word: &str) -> Result<(), JsonError> {
        let expected = word.chars();
        let mut index: usize = 0;
        while index < expected.len() {
            match self.advance() {
                Some(ch) => {
                    if ch != expected[index] {
                        return Err(self.error("invalid literal"));
                    }
                }
                None => {
                    return Err(self.error("unexpected end of input"));
                }
            }
            index += 1;
        }
        return Ok(());
    }
    fn parse_object(&mut self) -> Result<Value, JsonError> {
        self.advance();
        let mut entries: Vec<(String, Value)> = Vec::new();
        self.skip_whitespace();
        match self.peek() {
            Some('}') => {
                self.advance();
                return Ok(Value::Object(entries));
            }
            None => {
                return Err(self.error("unterminated object"));
            }
            Some(_) => {}
        }
        loop {
            self.skip_whitespace();
            let key = match self.peek() {
                Some('"') => self.parse_string()?,
                Some(_) => {
                    return Err(self.error("expected a string object key"));
                }
                None => {
                    return Err(self.error("unterminated object"));
                }
            };
            self.skip_whitespace();
            match self.peek() {
                Some(':') => {
                    self.advance();
                }
                Some(_) => {
                    return Err(self.error("expected ':' after object key"));
                }
                None => {
                    return Err(self.error("unterminated object"));
                }
            }
            let value = self.parse_value()?;
            let mut duplicate = false;
            let mut index: usize = 0;
            while index < entries.len() {
                let existing = entries[index].0.clone();
                if existing == key {
                    duplicate = true;
                    break;
                }
                index += 1;
            }
            if duplicate {
                return Err(self.error("duplicate object key"));
            }
            entries.push((key, value));
            self.skip_whitespace();
            match self.peek() {
                Some(',') => {
                    self.advance();
                    self.skip_whitespace();
                    match self.peek() {
                        Some('}') => {
                            return Err(self.error("trailing comma in object"));
                        }
                        Some(_) => {}
                        None => {
                            return Err(self.error("unterminated object"));
                        }
                    }
                }
                Some('}') => {
                    self.advance();
                    break;
                }
                Some(_) => {
                    return Err(self.error("expected ',' or '}' after object value"));
                }
                None => {
                    return Err(self.error("unterminated object"));
                }
            }
        }
        return Ok(Value::Object(entries));
    }
    fn parse_array(&mut self) -> Result<Value, JsonError> {
        self.advance();
        let mut items: Vec<Value> = Vec::new();
        self.skip_whitespace();
        match self.peek() {
            Some(']') => {
                self.advance();
                return Ok(Value::Array(items));
            }
            None => {
                return Err(self.error("unterminated array"));
            }
            Some(_) => {}
        }
        loop {
            let value = self.parse_value()?;
            items.push(value);
            self.skip_whitespace();
            match self.peek() {
                Some(',') => {
                    self.advance();
                    self.skip_whitespace();
                    match self.peek() {
                        Some(']') => {
                            return Err(self.error("trailing comma in array"));
                        }
                        Some(_) => {}
                        None => {
                            return Err(self.error("unterminated array"));
                        }
                    }
                }
                Some(']') => {
                    self.advance();
                    break;
                }
                Some(_) => {
                    return Err(self.error("expected ',' or ']' after array element"));
                }
                None => {
                    return Err(self.error("unterminated array"));
                }
            }
        }
        return Ok(Value::Array(items));
    }
    fn parse_string(&mut self) -> Result<String, JsonError> {
        self.advance();
        let mut out = String::new();
        loop {
            let ch = match self.advance() {
                Some(value) => value,
                None => {
                    return Err(self.error("unterminated string"));
                }
            };
            if ch == '"' {
                return Ok(out);
            }
            if ch == '\\' {
                let escape = match self.advance() {
                    Some(value) => value,
                    None => {
                        return Err(self.error("unterminated escape sequence"));
                    }
                };
                if escape == '"' {
                    out.push('"');
                } else if escape == '\\' {
                    out.push('\\');
                } else if escape == '/' {
                    out.push('/');
                } else if escape == 'b' {
                    out.push(8 as u8 as char);
                } else if escape == 'f' {
                    out.push(12 as u8 as char);
                } else if escape == 'n' {
                    out.push('\n');
                } else if escape == 'r' {
                    out.push('\r');
                } else if escape == 't' {
                    out.push('\t');
                } else if escape == 'u' {
                    let code = self.parse_hex4()?;
                    if code > 255 {
                        return Err(self.error("unicode escapes above U+00FF are not supported in this build"));
                    }
                    out.push(code as u8 as char);
                } else {
                    return Err(self.error("invalid escape sequence"));
                }
            } else if (ch as u32) < 32 {
                return Err(self.error("unescaped control character in string"));
            } else {
                out.push(ch);
            }
        }
    }
    fn parse_hex4(&mut self) -> Result<u32, JsonError> {
        let mut value: u32 = 0;
        let mut index: usize = 0;
        while index < 4 {
            let ch = match self.advance() {
                Some(character) => character,
                None => {
                    return Err(self.error("unterminated unicode escape"));
                }
            };
            let code = ch as u32;
            let digit: u32 = if ch >= '0' && ch <= '9' {
                code - 48
            } else if ch >= 'a' && ch <= 'f' {
                code - 87
            } else if ch >= 'A' && ch <= 'F' {
                code - 55
            } else {
                return Err(self.error("invalid unicode escape digit"));
            };
            value = value * 16 + digit;
            index += 1;
        }
        return Ok(value);
    }
    fn collect_digits(&mut self, text: &mut String) -> usize {
        let mut count: usize = 0;
        loop {
            match self.peek() {
                Some(ch) => {
                    if ch >= '0' && ch <= '9' {
                        text.push(ch);
                        self.advance();
                        count += 1;
                    } else {
                        return count;
                    }
                }
                None => {
                    return count;
                }
            }
        }
    }
    fn parse_number(&mut self) -> Result<Value, JsonError> {
        let mut text = String::new();
        match self.peek() {
            Some('-') => {
                self.advance();
                text.push('-');
            }
            Some(_) => {}
            None => {
                return Err(self.error("unexpected end of input"));
            }
        }
        match self.peek() {
            Some('0') => {
                text.push('0');
                self.advance();
                if self.next_is_digit() {
                    return Err(self.error("leading zeros are not allowed"));
                }
            }
            Some(_) => {
                let digits = self.collect_digits(&mut text);
                if digits == 0 {
                    return Err(self.error("invalid number"));
                }
            }
            None => {
                return Err(self.error("unexpected end of input"));
            }
        }
        match self.peek() {
            Some('.') => {
                self.advance();
                text.push('.');
                let digits = self.collect_digits(&mut text);
                if digits == 0 {
                    return Err(self.error("missing fraction digits"));
                }
            }
            Some(_) => {}
            None => {}
        }
        match self.peek() {
            Some('e') => {
                self.advance();
                text.push('e');
                if !self.collect_exponent_sign(&mut text) {
                    return Err(self.error("missing exponent digits"));
                }
            }
            Some('E') => {
                self.advance();
                text.push('e');
                if !self.collect_exponent_sign(&mut text) {
                    return Err(self.error("missing exponent digits"));
                }
            }
            Some(_) => {}
            None => {}
        }
        match text.parse_f64() {
            Ok(value) => {
                return Ok(Value::Number(value));
            }
            Err(_error) => {
                return Err(self.error("invalid number"));
            }
        }
    }
    fn collect_exponent_sign(&mut self, text: &mut String) -> bool {
        match self.peek() {
            Some('+') => {
                self.advance();
                text.push('+');
            }
            Some('-') => {
                self.advance();
                text.push('-');
            }
            Some(_) => {}
            None => {}
        }
        let digits = self.collect_digits(text);
        return digits > 0;
    }
}
fn hex_digit(value: u32) -> char {
    let digit = value % 16;
    if digit < 10 {
        return (48 + digit) as u8 as char;
    }
    return (55 + digit) as u8 as char;
}
fn escape_char_into(out: &mut String, ch: char) {
    let code = ch as u32;
    if ch == '"' {
        out.push('\\');
        out.push('"');
    } else if ch == '\\' {
        out.push('\\');
        out.push('\\');
    } else if ch == '\n' {
        out.push('\\');
        out.push('n');
    } else if ch == '\r' {
        out.push('\\');
        out.push('r');
    } else if ch == '\t' {
        out.push('\\');
        out.push('t');
    } else if code == 8 {
        out.push('\\');
        out.push('b');
    } else if code == 12 {
        out.push('\\');
        out.push('f');
    } else if code < 32 || code == 127 {
        out.push('\\');
        out.push('u');
        out.push(hex_digit(code / 4096));
        out.push(hex_digit((code / 256) % 16));
        out.push(hex_digit((code / 16) % 16));
        out.push(hex_digit(code % 16));
    } else {
        out.push(ch);
    }
}
fn escape_into(out: &mut String, text: &str) {
    out.push('"');
    let chars = text.chars();
    let mut index: usize = 0;
    while index < chars.len() {
        escape_char_into(out, chars[index]);
        index += 1;
    }
    out.push('"');
}
fn format_number(value: f64) -> String {
    if value == 0.0 {
        return String::from("0");
    }
    if value <= 9000000000000000.0 && value >= -9000000000000000.0 {
        let truncated = value as i64;
        if (truncated as f64) == value {
            return format!("{}", truncated);
        }
    }
    let text = format!("{:.6}", value);
    let mut trimmed = text;
    while trimmed.ends_with("0") {
        trimmed.pop();
    }
    if trimmed.ends_with(".") {
        trimmed.pop();
    }
    return trimmed;
}
fn write_value(out: &mut String, value: &Value, pretty: bool, depth: usize) {
    match value {
        Value::Null => {
            out.push_str("null");
        }
        Value::Bool(flag) => {
            if *flag {
                out.push_str("true");
            } else {
                out.push_str("false");
            }
        }
        Value::Number(number) => {
            let text = format_number(*number);
            out.push_str(text.as_str());
        }
        Value::Text(text) => {
            escape_into(out, text.as_str());
        }
        Value::Array(items) => {
            if pretty && items.len() == 0 {
                out.push_str("[]");
                return;
            }
            out.push('[');
            if pretty {
                out.push('\n');
            }
            let mut index: usize = 0;
            while index < items.len() {
                if pretty {
                    indent_into(out, depth + 1);
                }
                write_value(out, &items[index], pretty, depth + 1);
                if index + 1 < items.len() {
                    out.push(',');
                }
                if pretty {
                    out.push('\n');
                }
                index += 1;
            }
            if pretty {
                indent_into(out, depth);
            }
            out.push(']');
        }
        Value::Object(entries) => {
            if pretty && entries.len() == 0 {
                out.push_str("{}");
                return;
            }
            let mut keys: Vec<String> = Vec::new();
            let mut index: usize = 0;
            while index < entries.len() {
                keys.push(entries[index].0.clone());
                index += 1;
            }
            keys.sort();
            out.push('{');
            if pretty {
                out.push('\n');
            }
            let mut key_index: usize = 0;
            while key_index < keys.len() {
                if pretty {
                    indent_into(out, depth + 1);
                }
                escape_into(out, keys[key_index].as_str());
                if pretty {
                    out.push_str(": ");
                } else {
                    out.push(':');
                }
                let wanted = keys[key_index].clone();
                let mut entry_index: usize = 0;
                while entry_index < entries.len() {
                    let existing = entries[entry_index].0.clone();
                    if existing == wanted {
                        write_value(out, &entries[entry_index].1, pretty, depth + 1);
                        break;
                    }
                    entry_index += 1;
                }
                if key_index + 1 < keys.len() {
                    out.push(',');
                }
                if pretty {
                    out.push('\n');
                }
                key_index += 1;
            }
            if pretty {
                indent_into(out, depth);
            }
            out.push('}');
        }
    }
}
fn indent_into(out: &mut String, depth: usize) {
    let mut count: usize = 0;
    while count < depth {
        out.push_str("  ");
        count += 1;
    }
}
fn canonical(value: &Value) -> String {
    let mut out = String::new();
    write_value(&mut out, value, false, 0);
    return out;
}
fn embedded_document() -> String {
    let mut text = String::new();
    text += "{\n";
    text += "  \"name\": \"lsaca\",\n";
    text += "  \"version\": 2.2,\n";
    text += "  \"active\": true,\n";
    text += "  \"scores\": [1, 2.5, 3.75],\n";
    text += "  \"meta\": {\n";
    text += "    \"tags\": [\"lsaca\", \"json\", \"v2.2\"],\n";
    text += "    \"empty\": [],\n";
    text += "    \"nothing\": {}\n";
    text += "  },\n";
    text += "  \"unicode\": \"caf\\u00e9\",\n";
    text += "  \"escaped\": \"line\\nbreak\\ttab \\\"quoted\\\" back\\\\slash\",\n";
    text += "  \"final\": null\n";
    text += "}\n";
    return text;
}
fn main() {
    println!("json: parse, validate and canonicalize JSON");
    let mut sources: Vec<String> = Vec::new();
    sources.push(embedded_document());
    sources.push(String::from("[1, 2, 3]"));
    sources.push(String::from("{\"z\": 1, \"a\": 2}"));
    sources.push(String::from("\"plain string\""));
    sources.push(String::from("{\"a\": 1, \"a\": 2}"));
    sources.push(String::from("[1, 2,]"));
    sources.push(String::from("{\"key\": \"\\u4e2d\"}"));
    sources.push(String::from("{\"broken\": \"text}"));
    println!("documents: {}", sources.len());
    let mut accepted: i64 = 0;
    let mut rejected: i64 = 0;
    let mut index: usize = 0;
    while index < sources.len() {
        let number = index + 1;
        let text = sources[index].clone();
        let mut parser = Parser::new(text.as_str());
        match parser.parse_document() {
            Ok(value) => {
                accepted += 1;
                println!("[{}] parsed", number);
                let canonical_text = canonical(&value);
                println!("  canonical: {}", canonical_text);
                let mut stats = Stats::new();
                stats.walk(&value, 1);
                println!("  stats: nodes={} objects={} arrays={} strings={} numbers={} bools={} nulls={} depth={}", stats.nodes, stats.objects, stats.arrays, stats.strings, stats.numbers, stats.bools, stats.nulls, stats.max_depth);
                if index == 0 {
                    let mut pretty_text = String::new();
                    write_value(&mut pretty_text, &value, true, 0);
                    println!("  pretty:");
                    println!("{}", pretty_text);
                }
            }
            Err(error) => {
                rejected += 1;
                println!("[{}] rejected: {}", number, error.describe());
            }
        }
        index += 1;
    }
    println!("summary: {} accepted, {} rejected", accepted, rejected);
    let args = std::process::args();
    if args.len() > 0 {
        let path = args[0].clone();
        println!("file: {}", path);
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                let mut parser = Parser::new(text.as_str());
                match parser.parse_document() {
                    Ok(value) => {
                        println!("file document: parsed");
                        let canonical_text = canonical(&value);
                        println!("{}", canonical_text);
                    }
                    Err(error) => {
                        println!("file document: rejected: {}", error.describe());
                    }
                }
            }
            Err(error) => {
                println!("file document: read error: {}", error.message());
            }
        }
    }
}