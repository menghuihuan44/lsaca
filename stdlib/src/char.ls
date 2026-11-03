// Character classification helpers.
//
// The methods in `impl char` below are pure lsaca code (no extern
// intrinsics): they compare the scalar value against ASCII code point
// ranges, exactly like Rust's `char::is_ascii_*` family. They report
// ASCII results only; Unicode classification such as `is_alphabetic`
// over non-ASCII scalars needs the Unicode tables and is planned for a
// later version, so it is documented as a limitation instead of being
// silently wrong.

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

    /// Returns `true` for the ASCII whitespace characters that appear in
    /// text files: space, tab, line feed and carriage return.
    pub fn is_ascii_whitespace(&self) -> bool {
        return *self == ' ' || *self == '\t' || *self == '\n' || *self == '\r';
    }
}
