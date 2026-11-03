// String and str: growable UTF-8 strings and borrowed string views.
//
// The compiler knows String and str as builtin types; the declarations here
// only claim the names for the standard library. Methods are inherent impls
// backed by extern intrinsics: codegen expands the intrinsic inline and the
// legacy compiler fallback table is shadowed by these methods.
//
// Read-only text queries use the frozen Phase B intrinsic ABI: they take a
// &str receiver and &str arguments. String methods call them with &self, which
// coerces &String to &str implicitly. Methods that take a text argument declare
// that parameter as an owned String (string literals are String values) and
// pass it as &argument, which also coerces.
//
// These methods intentionally stay on the legacy compiler fallback table and
// are therefore NOT declared here: String::replace (not in the Phase B
// intrinsic set), String::trim (the legacy method returns an owned String while
// __str_trim returns the borrowed slice; a later version will align them),
// String::parse_i64/String::parse_f64 (legacy return type is Result, the str
// intrinsic returns Option).

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

    pub fn len(&self) -> i64 {
        return __str_len(self);
    }

    pub fn is_empty(&self) -> bool {
        return __str_len(self) == 0;
    }

    pub fn push_str(&mut self, other: String) {
        __str_push_str(self, other);
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

    pub fn contains(&self, needle: String) -> bool {
        return __str_contains(self, &needle);
    }

    pub fn starts_with(&self, prefix: String) -> bool {
        return __str_starts_with(self, &prefix);
    }

    pub fn ends_with(&self, suffix: String) -> bool {
        return __str_ends_with(self, &suffix);
    }

    pub fn find(&self, needle: String) -> Option<i64> {
        return __str_find(self, &needle);
    }

    pub fn split(&self, separator: String) -> Vec<String> {
        return __str_split(self, &separator);
    }

    pub fn to_uppercase(&self) -> String {
        return __str_to_uppercase(self);
    }

    pub fn to_lowercase(&self) -> String {
        return __str_to_lowercase(self);
    }

    pub fn substring(&self, start: i64, length: i64) -> String {
        return __str_substring(self, start, length);
    }

    pub fn repeat(&self, count: i64) -> String {
        return __str_repeat(self, count);
    }

    pub fn char_count(&self) -> i64 {
        return __str_char_count(self);
    }

    pub fn chars(&self) -> Vec<char> {
        return __str_chars(self);
    }

    pub fn char_at(&self, index: i64) -> Option<char> {
        return __str_char_at(self, index);
    }

    pub fn is_char_boundary(&self, index: i64) -> bool {
        return __str_is_char_boundary(self, index);
    }
}

impl str {
    pub fn len(&self) -> i64 {
        return __str_len(self);
    }

    pub fn is_empty(&self) -> bool {
        return __str_len(self) == 0;
    }

    pub fn char_count(&self) -> i64 {
        return __str_char_count(self);
    }

    pub fn contains(&self, needle: String) -> bool {
        return __str_contains(self, &needle);
    }

    pub fn starts_with(&self, prefix: String) -> bool {
        return __str_starts_with(self, &prefix);
    }

    pub fn ends_with(&self, suffix: String) -> bool {
        return __str_ends_with(self, &suffix);
    }

    pub fn find(&self, needle: String) -> Option<i64> {
        return __str_find(self, &needle);
    }

    pub fn split(&self, separator: String) -> Vec<String> {
        return __str_split(self, &separator);
    }

    pub fn parse_i64(&self) -> Option<i64> {
        return __str_parse_i64(self);
    }

    pub fn parse_f64(&self) -> Option<f64> {
        return __str_parse_f64(self);
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

    pub fn is_char_boundary(&self, index: i64) -> bool {
        return __str_is_char_boundary(self, index);
    }

    pub fn char_at(&self, index: i64) -> Option<char> {
        return __str_char_at(self, index);
    }

    pub fn slice(&self, start: i64, end: i64) -> &str {
        return __str_slice(self, start, end);
    }

    pub fn as_bytes(&self) -> &[u8] {
        return __str_as_bytes(self);
    }
}
