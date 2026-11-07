# string: String and str

**Status in 2.2.0.** Implemented. The `String` type and its methods live in
`stdlib/src/string.ls` (`std::string`); the borrowed `str` type is a builtin
unsized type (see [Slices and str](../language/26-slices-and-str.md)) and every
`str` method is also implemented there. `impl String` forwards each `str`
method with the same signature, so the two surfaces are kept in step.

`String` is the owned, growable, valid-UTF-8 text type. A string literal has type
`String` by default; in a position that expects `&str` (a function or method
argument, an annotated `let`, a return slot) it is compiled as a static slice and
allocates nothing. `String::from` accepts a literal or another `String`;
`String::from_str` copies a borrowed view into owned text.

## String methods

| Method | Signature | Meaning |
| --- | --- | --- |
| `String::new` | `() -> String` | empty string |
| `String::from` | `(String) -> String` | identity on an owned string; a literal is already `String` |
| `String::from_char` | `(char) -> String` | one-character string |
| `String::from_str` | `(&str) -> String` | copies borrowed text |
| `as_str` / `as_mut_str` | `(&self) -> &str` / `(&mut self) -> &mut str` | borrowed view, no copy |
| `as_bytes` | `(&self) -> &[u8]` | the UTF-8 bytes as a slice |
| `len` / `is_empty` | `(&self) -> usize` / `(&self) -> bool` | byte length / emptiness |
| `push_str` | `(&mut self, &str)` | appends borrowed text |
| `push` | `(&mut self, char)` | appends one character |
| `pop` | `(&mut self) -> Option<char>` | removes and returns the last character |
| `clear` | `(&mut self)` | removes all text |
| `contains` / `starts_with` / `ends_with` | `(&self, &str) -> bool` | substring tests |
| `find` | `(&self, &str) -> Option<usize>` | byte index of the first occurrence |
| `split` | `(&self, &str) -> Vec<String>` | splits on a separator |
| `split_whitespace` / `lines` | `(&self) -> Vec<String>` | splits on whitespace / on line breaks |
| `replace` | `(&self, &str, &str) -> String` | replaces every occurrence |
| `to_uppercase` / `to_lowercase` | `(&self) -> String` | case mapping |
| `trim` / `trim_start` / `trim_end` | `(&self) -> &str` | removes whitespace at the ends |
| `substring` | `(&self, start: usize, length: usize) -> String` | copies a byte range |
| `slice` | `(&self, start: usize, end: usize) -> &str` | borrows a byte range |
| `repeat` | `(&self, usize) -> String` | repeats the text |
| `char_count` | `(&self) -> usize` | number of characters |
| `chars` | `(&self) -> Vec<char>` | characters |
| `char_at` | `(&self, usize) -> Option<char>` | character by character index |
| `is_char_boundary` | `(&self, usize) -> bool` | whether a byte offset is a character boundary |
| `parse_i64` | `(&self) -> Result<i64, ParseIntError>` | decimal integer |
| `parse_f64` | `(&self) -> Result<f64, ParseFloatError>` | decimal float |

## str methods

The `str` methods mirror the `String` ones with a borrowed receiver:
`len`, `is_empty`, `char_count`, `repeat`, `contains`, `starts_with`,
`ends_with`, `find`, `split`, `split_whitespace`, `lines`, `replace`,
`parse_i64`, `parse_f64`, `to_uppercase`, `to_lowercase`, `trim`,
`trim_start`, `trim_end`, `is_char_boundary`, `char_at`, `slice`,
`as_bytes` and `chars`. Their signatures use `usize` for indices and counts and
`&self` for the receiver, and `&str` for text parameters. See
[Slices and str](../language/26-slices-and-str.md) for the complete table.

## Concatenation and parse

`String + &str` produces a new `String`, and `+=` accepts `&str`
(`s += &t`). Two owned `String` values cannot be added, and `&str + &str` is not
supported. Integer parsing is typed: `parse_i64` returns
`Result<i64, ParseIntError>`, `parse_f64` returns
`Result<f64, ParseFloatError>`. The generic `.parse::<T>()` method accepts `i64`
and `f64` in 2.2.0 and reports the same typed errors (`ParseIntError` for
integers, `ParseFloatError` for floats); other type arguments are rejected with
`E0107` and the broader set is planned for a later release. The error types
implement `std::error::Error` (see [error](error.md)).

## Planned for a later release

* No further `String` API is planned: the remaining Rust surface (for example
  `from_utf8`, `into_bytes`, `splitn`) needs types or traits that lsaca does not
  have.
* `String` is a builtin marker: `impl String` blocks may only be written in the
  `std` crate; user crates get `E0118`.

## Memory and ORC

A `String` owns a reference-counted UTF-8 buffer. `.clone()` copies the text; moves
transfer the handle. `String` participates in the ORC cycle collector like every
other managed value. See [Memory model](../language/05-memory-model.md).

## See also

* [Characters and Unicode](../language/22-characters-and-unicode.md) - character
  semantics, boundary panics and Debug escaping.
* [char](char.md) - character classification (ASCII methods implemented, Unicode
  classification planned).
* [Slices and str](../language/26-slices-and-str.md) - borrowed text.
