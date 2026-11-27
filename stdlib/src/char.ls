// Character classification helpers.
//
// The `is_ascii_*` family is pure lsaca code (no extern intrinsics):
// it compares the scalar value against ASCII code point ranges, exactly
// like Rust's `char::is_ascii_*` family. The Unicode classification and
// case conversion methods call extern intrinsics backed by the generated
// tables in runtime/lsaca_rt_unicode.c, the same tables String case
// mapping and trimming use.
//
// char::to_uppercase and char::to_lowercase return the closest single
// scalar. When the Unicode mapping expands to several scalars (U+00DF
// uppercases to "SS", U+0130 lowercases to "i" + U+0307), only the first
// scalar is returned; String::to_uppercase/to_lowercase keep the full
// mapping. This deviation is documented in docs/stdlib/char.md.

pub extern fn __char_is_alphabetic(value: char) -> bool;
pub extern fn __char_is_numeric(value: char) -> bool;
pub extern fn __char_is_whitespace(value: char) -> bool;
pub extern fn __char_to_uppercase(value: char) -> char;
pub extern fn __char_to_lowercase(value: char) -> char;

impl char {
    /// Returns `true` when the scalar is an ASCII letter, digit or
    /// underscore (the characters used by identifiers).
    pub fn is_ascii_word(&self) -> bool {
        return (*self >= 'a' && *self <= 'z')
            || (*self >= 'A' && *self <= 'Z')
            || (*self >= '0' && *self <= '9')
            || *self == '_';
    }

    /// Returns `true` for the ASCII digits `0` through `9`.
    pub fn is_ascii_digit(&self) -> bool {
        return *self >= '0' && *self <= '9';
    }

    /// Returns `true` for the ASCII uppercase letters `A` through `Z`.
    pub fn is_ascii_uppercase(&self) -> bool {
        return *self >= 'A' && *self <= 'Z';
    }

    /// Returns `true` for the ASCII lowercase letters `a` through `z`.
    pub fn is_ascii_lowercase(&self) -> bool {
        return *self >= 'a' && *self <= 'z';
    }

    /// Returns `true` for ASCII letters in either case.
    pub fn is_ascii_alphabetic(&self) -> bool {
        return (*self >= 'a' && *self <= 'z') || (*self >= 'A' && *self <= 'Z');
    }

    /// Returns `true` for ASCII letters and digits.
    pub fn is_ascii_alphanumeric(&self) -> bool {
        return (*self >= 'a' && *self <= 'z')
            || (*self >= 'A' && *self <= 'Z')
            || (*self >= '0' && *self <= '9');
    }

    /// Returns `true` for the ASCII hexadecimal digits `0-9`, `a-f`, `A-F`.
    pub fn is_ascii_hexdigit(&self) -> bool {
        return (*self >= '0' && *self <= '9')
            || (*self >= 'a' && *self <= 'f')
            || (*self >= 'A' && *self <= 'F');
    }

    /// Returns `true` for the ASCII whitespace characters, matching Rust's
    /// `char::is_ascii_whitespace`: space, tab, line feed, vertical tab, form
    /// feed and carriage return.
    pub fn is_ascii_whitespace(&self) -> bool {
        let code: u32 = *self as u32;
        return code == 32 || (code >= 9 && code <= 13);
    }

    /// Returns `true` when the scalar is an ASCII character (0..=0x7F).
    pub fn is_ascii(&self) -> bool {
        let code: u32 = *self as u32;
        return code <= 127;
    }

    /// Returns `true` for ASCII control characters (0..=0x1F and 0x7F).
    pub fn is_ascii_control(&self) -> bool {
        let code: u32 = *self as u32;
        return code <= 31 || code == 127;
    }

    /// Returns `true` for printable ASCII characters, `!` through `~`.
    pub fn is_ascii_graphic(&self) -> bool {
        let code: u32 = *self as u32;
        return code >= 33 && code <= 126;
    }

    /// Returns `true` for ASCII punctuation.
    pub fn is_ascii_punctuation(&self) -> bool {
        let code: u32 = *self as u32;
        return (code >= 33 && code <= 47)
            || (code >= 58 && code <= 64)
            || (code >= 91 && code <= 96)
            || (code >= 123 && code <= 126);
    }

    /// Returns `true` when the scalar has the Unicode Alphabetic property:
    /// general categories L* and Nl plus Other_Alphabetic. Locale independent.
    pub fn is_alphabetic(&self) -> bool {
        return __char_is_alphabetic(*self);
    }

    /// Returns `true` when the scalar has the Unicode numeric property:
    /// general categories Nd (decimal), Nl (letter) and No (other).
    pub fn is_numeric(&self) -> bool {
        return __char_is_numeric(*self);
    }

    /// Returns `true` for Unicode alphabetic or numeric scalars.
    pub fn is_alphanumeric(&self) -> bool {
        return self.is_alphabetic() || self.is_numeric();
    }

    /// Returns `true` when the scalar has the Unicode White_Space property,
    /// the same set `String::trim` removes.
    pub fn is_whitespace(&self) -> bool {
        return __char_is_whitespace(*self);
    }

    /// Returns the closest single-scalar Unicode uppercase mapping.
    /// Multi-scalar mappings are truncated to their first scalar: `'ß'`
    /// returns `'S'` and `'ﬁ'` returns `'F'`.
    pub fn to_uppercase(&self) -> char {
        return __char_to_uppercase(*self);
    }

    /// Returns the closest single-scalar Unicode lowercase mapping.
    /// Multi-scalar mappings are truncated to their first scalar: `'İ'`
    /// returns `'i'`.
    pub fn to_lowercase(&self) -> char {
        return __char_to_lowercase(*self);
    }

    /// Maps an ASCII lowercase letter to uppercase; other scalars unchanged.
    pub fn to_ascii_uppercase(&self) -> char {
        let code: u32 = *self as u32;
        if code >= 97 && code <= 122 { return ((code - 32) as u8) as char; }
        return *self;
    }

    /// Maps an ASCII uppercase letter to lowercase; other scalars unchanged.
    pub fn to_ascii_lowercase(&self) -> char {
        let code: u32 = *self as u32;
        if code >= 65 && code <= 90 { return ((code + 32) as u8) as char; }
        return *self;
    }

    /// Returns the numeric value of the character in the given base
    /// (2..=36), or `None` when it is not a digit in that base.
    pub fn to_digit(&self, radix: u32) -> Option<u32> {
        if radix < 2 || radix > 36 { return None; }
        let code: u32 = *self as u32;
        let mut digit: u32 = 0;
        let mut valid: bool = false;
        if code >= 48 && code <= 57 { digit = code - 48; valid = true; }
        if code >= 97 && code <= 122 { digit = code - 97 + 10; valid = true; }
        if code >= 65 && code <= 90 { digit = code - 65 + 10; valid = true; }
        if !valid { return None; }
        if digit >= radix { return None; }
        return Some(digit);
    }
}
