// String and str: growable UTF-8 strings and borrowed string views.
//
// The compiler knows String and str as builtin types; the declarations here
// only claim the names for the standard library. Methods are inherent impls
// backed by extern intrinsics: codegen expands the intrinsic inline and the
// legacy compiler fallback table is shadowed by these methods.
//
// Since 2.1.0 read-only text queries take a &str receiver and &str arguments.
// String methods call them with &self, which coerces &String to &str. Methods
// that take a text argument declare it as &str: pass a string literal, a &str
// value, or &some_string (an owned String does not coerce implicitly).
//
// Lengths, indices and counts are usize at this public surface. The frozen
// intrinsic ABI keeps i64 for length-like results and indices; the wrappers
// below convert at the intrinsic boundary.
//
// Iterators over the text (chars, split_whitespace, lines) allocate and return
// Vec<String> values: borrowed item types such as Vec<&str> are not supported
// in 2.1.0 because references cannot be stored in Vec. This is a documented
// deviation from Rust that keeps the API usable today.

use crate::core::ParseFloatError;
use crate::core::ParseIntError;

pub struct String {
    internal: (),
}

// String-only mutations and constructors keep the original intrinsic names.
pub extern fn __str_push_str(s: &mut String, other: String);
pub extern fn __str_push_char(s: &mut String, ch: char);
pub extern fn __str_pop(s: &mut String) -> Option<char>;
pub extern fn __str_clear(s: &mut String);
pub extern fn __str_substring(s: &String, start: i64, length: i64) -> String;
pub extern fn __str_repeat(s: &String, count: i64) -> String;
pub extern fn __str_chars(s: &String) -> Vec<char>;
pub extern fn __str_new() -> String;
pub extern fn __str_from(value: String) -> String;
pub extern fn __str_from_char(ch: char) -> String;
pub extern fn __str_from_str(value: &str) -> String;

// Frozen Phase B read-only text query intrinsics (receiver and text arguments
// are &str). They are used by both impl String and impl str below.
pub extern fn __str_len(s: &str) -> i64;
pub extern fn __str_char_count(s: &str) -> i64;
pub extern fn __str_contains(s: &str, needle: &str) -> bool;
pub extern fn __str_starts_with(s: &str, prefix: &str) -> bool;
pub extern fn __str_ends_with(s: &str, suffix: &str) -> bool;
pub extern fn __str_find(s: &str, needle: &str) -> Option<i64>;
pub extern fn __str_split(s: &str, separator: &str) -> Vec<String>;
pub extern fn __str_parse_i64(s: &str) -> Option<i64>;
pub extern fn __str_parse_f64(s: &str) -> Option<f64>;
pub extern fn __str_to_uppercase(s: &str) -> String;
pub extern fn __str_to_lowercase(s: &str) -> String;
pub extern fn __str_trim(s: &str) -> &str;
pub extern fn __str_is_char_boundary(s: &str, index: i64) -> bool;
pub extern fn __str_char_at(s: &str, index: i64) -> Option<char>;
pub extern fn __str_slice(s: &str, start: i64, end: i64) -> &str;
pub extern fn __str_as_bytes(s: &str) -> &[u8];

// Conversions between String and borrowed string views.
pub extern fn __string_as_str(s: &String) -> &str;
pub extern fn __string_as_mut_str(s: &mut String) -> &mut str;

impl String {
    pub fn new() -> String {
        return __str_new();
    }

    pub fn from(value: String) -> String {
        return __str_from(value);
    }

    pub fn from_char(ch: char) -> String {
        return __str_from_char(ch);
    }

    pub fn from_str(value: &str) -> String {
        return __str_from_str(value);
    }

    pub fn as_str(&self) -> &str {
        return __string_as_str(self);
    }

    pub fn as_mut_str(&mut self) -> &mut str {
        return __string_as_mut_str(self);
    }

    pub fn as_bytes(&self) -> &[u8] {
        return __str_as_bytes(self);
    }

    /// Number of bytes in the string (UTF-8 length, not character count).
    pub fn len(&self) -> usize {
        return __str_len(self) as usize;
    }

    pub fn is_empty(&self) -> bool {
        return __str_len(self) == 0;
    }

    /// Appends a borrowed string slice, copying its bytes.
    pub fn push_str(&mut self, other: &str) {
        __str_push_str(self, String::from_str(other));
    }

    pub fn push(&mut self, ch: char) {
        __str_push_char(self, ch);
    }

    pub fn pop(&mut self) -> Option<char> {
        return __str_pop(self);
    }

    pub fn clear(&mut self) {
        __str_clear(self);
    }

    pub fn contains(&self, needle: &str) -> bool {
        return __str_contains(self, needle);
    }

    pub fn starts_with(&self, prefix: &str) -> bool {
        return __str_starts_with(self, prefix);
    }

    pub fn ends_with(&self, suffix: &str) -> bool {
        return __str_ends_with(self, suffix);
    }

    /// Byte index of the first occurrence of `needle`, like Rust's str::find.
    pub fn find(&self, needle: &str) -> Option<usize> {
        match __str_find(self, needle) {
            Some(index) => { return Some(index as usize); }
            None => { return None; }
        }
    }

    /// Splits on `separator` into owned pieces (Rust yields Vec<&str>).
    pub fn split(&self, separator: &str) -> Vec<String> {
        return __str_split(self, separator);
    }

    pub fn to_uppercase(&self) -> String {
        return __str_to_uppercase(self);
    }

    pub fn to_lowercase(&self) -> String {
        return __str_to_lowercase(self);
    }

    /// Returns the borrowed trimmed view (Rust String::trim does the same).
    pub fn trim(&self) -> &str {
        return __str_trim(self);
    }

    pub fn trim_start(&self) -> &str {
        return __lsaca_trim_start_text(self);
    }

    pub fn trim_end(&self) -> &str {
        return __lsaca_trim_end_text(self);
    }

    /// Byte range substring; invalid ranges panic like the intrinsic.
    pub fn substring(&self, start: usize, length: usize) -> String {
        return __str_substring(self, start as i64, length as i64);
    }

    pub fn repeat(&self, count: usize) -> String {
        return __str_repeat(self, count as i64);
    }

    pub fn char_count(&self) -> usize {
        return __str_char_count(self) as usize;
    }

    pub fn chars(&self) -> Vec<char> {
        return __str_chars(self);
    }

    pub fn char_at(&self, index: usize) -> Option<char> {
        return __str_char_at(self, index as i64);
    }

    pub fn is_char_boundary(&self, index: usize) -> bool {
        return __str_is_char_boundary(self, index as i64);
    }

    /// Borrowed byte range slice; both bounds must be char boundaries.
    pub fn slice(&self, start: usize, end: usize) -> &str {
        return __str_slice(self, start as i64, end as i64);
    }

    /// Splits on Unicode whitespace into owned pieces (Rust yields Vec<&str>).
    pub fn split_whitespace(&self) -> Vec<String> {
        return __lsaca_split_whitespace_text(self);
    }

    /// Splits on line feeds, stripping a trailing carriage return (Rust lines).
    pub fn lines(&self) -> Vec<String> {
        return __lsaca_split_lines_text(self);
    }

    /// Replaces every occurrence of `from` with `to` (Rust str::replace).
    pub fn replace(&self, from: &str, to: &str) -> String {
        return __lsaca_str_replace(self, from, to);
    }

    pub fn parse_i64(&self) -> Result<i64, ParseIntError> {
        match __str_parse_i64(self) {
            Some(parsed) => { return Ok(parsed); }
            None => { return Err(ParseIntError::__new()); }
        }
    }

    pub fn parse_f64(&self) -> Result<f64, ParseFloatError> {
        match __str_parse_f64(self) {
            Some(parsed) => { return Ok(parsed); }
            None => { return Err(ParseFloatError::__new()); }
        }
    }
}

impl str {
    /// Number of bytes in the slice (UTF-8 length, not character count).
    pub fn len(&self) -> usize {
        return __str_len(self) as usize;
    }

    pub fn is_empty(&self) -> bool {
        return __str_len(self) == 0;
    }

    pub fn char_count(&self) -> usize {
        return __str_char_count(self) as usize;
    }

    /// Repeats the slice `count` times (allocates an owned copy first:
    /// the __str_repeat intrinsic takes an owned String in this ABI).
    pub fn repeat(&self, count: usize) -> String {
        let owned: String = String::from_str(self);
        return __str_repeat(&owned, count as i64);
    }

    pub fn contains(&self, needle: &str) -> bool {
        return __str_contains(self, needle);
    }

    pub fn starts_with(&self, prefix: &str) -> bool {
        return __str_starts_with(self, prefix);
    }

    pub fn ends_with(&self, suffix: &str) -> bool {
        return __str_ends_with(self, suffix);
    }

    pub fn find(&self, needle: &str) -> Option<usize> {
        match __str_find(self, needle) {
            Some(index) => { return Some(index as usize); }
            None => { return None; }
        }
    }

    pub fn split(&self, separator: &str) -> Vec<String> {
        return __str_split(self, separator);
    }

    pub fn split_whitespace(&self) -> Vec<String> {
        return __lsaca_split_whitespace_text(self);
    }

    pub fn lines(&self) -> Vec<String> {
        return __lsaca_split_lines_text(self);
    }

    pub fn replace(&self, from: &str, to: &str) -> String {
        return __lsaca_str_replace(self, from, to);
    }

    pub fn parse_i64(&self) -> Result<i64, ParseIntError> {
        match __str_parse_i64(self) {
            Some(parsed) => { return Ok(parsed); }
            None => { return Err(ParseIntError::__new()); }
        }
    }

    pub fn parse_f64(&self) -> Result<f64, ParseFloatError> {
        match __str_parse_f64(self) {
            Some(parsed) => { return Ok(parsed); }
            None => { return Err(ParseFloatError::__new()); }
        }
    }

    pub fn to_uppercase(&self) -> String {
        return __str_to_uppercase(self);
    }

    pub fn to_lowercase(&self) -> String {
        return __str_to_lowercase(self);
    }

    pub fn trim(&self) -> &str {
        return __str_trim(self);
    }

    pub fn trim_start(&self) -> &str {
        return __lsaca_trim_start_text(self);
    }

    pub fn trim_end(&self) -> &str {
        return __lsaca_trim_end_text(self);
    }

    pub fn is_char_boundary(&self, index: usize) -> bool {
        return __str_is_char_boundary(self, index as i64);
    }

    pub fn char_at(&self, index: usize) -> Option<char> {
        return __str_char_at(self, index as i64);
    }

    pub fn slice(&self, start: usize, end: usize) -> &str {
        return __str_slice(self, start as i64, end as i64);
    }

    pub fn as_bytes(&self) -> &[u8] {
        return __str_as_bytes(self);
    }

    /// Allocates an owned copy of the text and returns its characters
    /// (borrowed item iterators are not supported, see the file header).
    pub fn chars(&self) -> Vec<char> {
        let owned: String = String::from_str(self);
        return __str_chars(&owned);
    }
}

// Free helpers implementing the parts of the str API that have no dedicated
// intrinsic. They scan UTF-8 bytes; slice indexing and usize lengths are part
// of the 2.1.0 numeric surface.

/// Byte width of the Unicode White_Space scalar at `index`, or 0 when the
/// bytes there do not start a whitespace character. Covers the White_Space
/// characters missing from the ASCII table (U+0085, U+00A0, U+1680, U+2000..
/// U+200A, U+2028, U+2029, U+202F, U+205F, U+3000).
pub fn __lsaca_utf8_whitespace_len(bytes: &[u8], index: usize) -> usize {
    let length: usize = bytes.len();
    if index >= length { return 0; }
    let first: u8 = bytes[index];
    if first == 0x20 { return 1; }
    if first >= 0x09 && first <= 0x0D { return 1; }
    if index + 1 >= length { return 0; }
    let second: u8 = bytes[index + 1];
    if first == 0xC2 {
        if second == 0x85 || second == 0xA0 { return 2; }
        return 0;
    }
    if index + 2 >= length { return 0; }
    let third: u8 = bytes[index + 2];
    if first == 0xE1 && second == 0x9A && third == 0x80 { return 3; }
    if first == 0xE2 && second == 0x80 {
        if third >= 0x80 && third <= 0x8A { return 3; }
        if third == 0xA8 || third == 0xA9 || third == 0xAF { return 3; }
    }
    if first == 0xE2 && second == 0x81 && third == 0x9F { return 3; }
    if first == 0xE3 && second == 0x80 && third == 0x80 { return 3; }
    return 0;
}

/// Borrowed trim_start over Unicode White_Space characters.
pub fn __lsaca_trim_start_text(value: &str) -> &str {
    let bytes: &[u8] = __str_as_bytes(value);
    let length: usize = bytes.len();
    let mut index: usize = 0;
    while index < length {
        let width: usize = __lsaca_utf8_whitespace_len(bytes, index);
        if width == 0 { break; }
        index = index + width;
    }
    return __str_slice(value, index as i64, length as i64);
}

/// Borrowed trim_end over Unicode White_Space characters.
pub fn __lsaca_trim_end_text(value: &str) -> &str {
    let bytes: &[u8] = __str_as_bytes(value);
    let length: usize = bytes.len();
    let mut index: usize = 0;
    let mut end: usize = 0;
    while index < length {
        let width: usize = __lsaca_utf8_whitespace_len(bytes, index);
        if width > 0 { index = index + width; continue; }
        let mut next: usize = index + 1;
        while next < length && !__str_is_char_boundary(value, next as i64) { next = next + 1; }
        index = next;
        end = index;
    }
    return __str_slice(value, 0, end as i64);
}

/// Byte index just past the UTF-8 scalar that starts at `index`.
fn __lsaca_next_char_end(value: &str, index: usize) -> usize {
    let length: usize = __str_len(value) as usize;
    let mut next: usize = index + 1;
    while next < length && !__str_is_char_boundary(value, next as i64) { next = next + 1; }
    return next;
}

/// split_whitespace for a borrowed slice, into owned pieces.
pub fn __lsaca_split_whitespace_text(value: &str) -> Vec<String> {
    let bytes: &[u8] = __str_as_bytes(value);
    let length: usize = bytes.len();
    let mut result: Vec<String> = Vec::new();
    let mut index: usize = 0;
    while index < length {
        let width: usize = __lsaca_utf8_whitespace_len(bytes, index);
        if width > 0 { index = index + width; continue; }
        let start: usize = index;
        while index < length {
            let inner: usize = __lsaca_utf8_whitespace_len(bytes, index);
            if inner > 0 { break; }
            index = __lsaca_next_char_end(value, index);
        }
        let piece: &str = __str_slice(value, start as i64, index as i64);
        result.push(String::from_str(piece));
    }
    return result;
}

/// lines for a borrowed slice, into owned pieces; strips CR of CRLF pairs.
pub fn __lsaca_split_lines_text(value: &str) -> Vec<String> {
    let bytes: &[u8] = __str_as_bytes(value);
    let length: usize = bytes.len();
    let mut result: Vec<String> = Vec::new();
    let mut index: usize = 0;
    let mut start: usize = 0;
    while index < length {
        if bytes[index] == 0x0A {
            let mut end: usize = index;
            if end > start && bytes[end - 1] == 0x0D { end = end - 1; }
            let piece: &str = __str_slice(value, start as i64, end as i64);
            result.push(String::from_str(piece));
            start = index + 1;
        }
        index = index + 1;
    }
    if start < length {
        let rest: &str = __str_slice(value, start as i64, length as i64);
        result.push(String::from_str(rest));
    }
    return result;
}

/// replace for borrowed slices; an empty `from` inserts `to` at both ends and
/// between every character, matching Rust's str::replace.
pub fn __lsaca_str_replace(value: &str, from: &str, to: &str) -> String {
    let mut result: String = String::new();
    if __str_len(from) == 0 {
        let owned: String = String::from_str(value);
        result.push_str(to);
        let chars: Vec<char> = __str_chars(&owned);
        for ch in chars {
            result.push(ch);
            result.push_str(to);
        }
        return result;
    }
    let parts: Vec<String> = __str_split(value, from);
    let mut first: bool = true;
    for part in parts {
        if first { first = false; } else { result.push_str(to); }
        result.push_str(&part);
    }
    return result;
}

// Parse helpers used by the compiler's generic .parse::<T>() sugar (the type
// checker picks one of these by the target type and re-exports them from the
// crate root). Ranges follow the integer limits of the language: there is no
// i128/u128, so those two forms are not offered.

pub fn __lsaca_parse_i8(value: &str) -> Result<i8, ParseIntError> {
    match __str_parse_i64(value) {
        Some(parsed) => {
            if parsed < -128 || parsed > 127 { return Err(ParseIntError::__new()); }
            return Ok(parsed as i8);
        }
        None => { return Err(ParseIntError::__new()); }
    }
}

pub fn __lsaca_parse_i16(value: &str) -> Result<i16, ParseIntError> {
    match __str_parse_i64(value) {
        Some(parsed) => {
            if parsed < -32768 || parsed > 32767 { return Err(ParseIntError::__new()); }
            return Ok(parsed as i16);
        }
        None => { return Err(ParseIntError::__new()); }
    }
}

pub fn __lsaca_parse_i32(value: &str) -> Result<i32, ParseIntError> {
    match __str_parse_i64(value) {
        Some(parsed) => {
            if parsed < -2147483648 || parsed > 2147483647 { return Err(ParseIntError::__new()); }
            return Ok(parsed as i32);
        }
        None => { return Err(ParseIntError::__new()); }
    }
}

pub fn __lsaca_parse_i64(value: &str) -> Result<i64, ParseIntError> {
    match __str_parse_i64(value) {
        Some(parsed) => { return Ok(parsed); }
        None => { return Err(ParseIntError::__new()); }
    }
}

pub fn __lsaca_parse_isize(value: &str) -> Result<isize, ParseIntError> {
    match __str_parse_i64(value) {
        Some(parsed) => { return Ok(parsed as isize); }
        None => { return Err(ParseIntError::__new()); }
    }
}

pub fn __lsaca_parse_u8(value: &str) -> Result<u8, ParseIntError> {
    match __str_parse_i64(value) {
        Some(parsed) => {
            if parsed < 0 || parsed > 255 { return Err(ParseIntError::__new()); }
            return Ok(parsed as u8);
        }
        None => { return Err(ParseIntError::__new()); }
    }
}

pub fn __lsaca_parse_u16(value: &str) -> Result<u16, ParseIntError> {
    match __str_parse_i64(value) {
        Some(parsed) => {
            if parsed < 0 || parsed > 65535 { return Err(ParseIntError::__new()); }
            return Ok(parsed as u16);
        }
        None => { return Err(ParseIntError::__new()); }
    }
}

pub fn __lsaca_parse_u32(value: &str) -> Result<u32, ParseIntError> {
    match __str_parse_i64(value) {
        Some(parsed) => {
            if parsed < 0 || parsed > 4294967295 { return Err(ParseIntError::__new()); }
            return Ok(parsed as u32);
        }
        None => { return Err(ParseIntError::__new()); }
    }
}

// u64/usize values above i64::MAX are rejected: the parse intrinsic stops at
// i64 (documented limitation for 2.1.0).
pub fn __lsaca_parse_u64(value: &str) -> Result<u64, ParseIntError> {
    match __str_parse_i64(value) {
        Some(parsed) => {
            if parsed < 0 { return Err(ParseIntError::__new()); }
            return Ok(parsed as u64);
        }
        None => { return Err(ParseIntError::__new()); }
    }
}

pub fn __lsaca_parse_usize(value: &str) -> Result<usize, ParseIntError> {
    match __str_parse_i64(value) {
        Some(parsed) => {
            if parsed < 0 { return Err(ParseIntError::__new()); }
            return Ok(parsed as usize);
        }
        None => { return Err(ParseIntError::__new()); }
    }
}

pub fn __lsaca_parse_f32(value: &str) -> Result<f32, ParseFloatError> {
    match __str_parse_f64(value) {
        Some(parsed) => { return Ok(parsed as f32); }
        None => { return Err(ParseFloatError::__new()); }
    }
}

pub fn __lsaca_parse_f64(value: &str) -> Result<f64, ParseFloatError> {
    match __str_parse_f64(value) {
        Some(parsed) => { return Ok(parsed); }
        None => { return Err(ParseFloatError::__new()); }
    }
}