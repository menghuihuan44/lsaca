# string: String

**Status in 2.0.0.** The `String` type and its 1.7 methods are migrated from the
builtin prelude into the `std` crate (`std::string`) without surface changes. The
borrowed `str` type is new in 2.0.0 (see [Slices and str](../language/26-slices-and-str.md)).
`String::from_str`, `as_str`, `as_mut_str`, `as_bytes` and the `str` methods in
the next section are implemented in `stdlib/src/string.ls`; the other items listed
under *Planned* are not implemented in 2.0.0 and are planned for a later release.

`String` is the owned, growable, valid-UTF-8 text type. A string literal already has
type `String`; `String::from` accepts a literal or another `String`.

## Migrated API

The method set is unchanged from 1.7:

| Area | Methods |
| --- | --- |
| size and content | `len`, `is_empty`, `clear` |
| building | `push_str`, `push(char)`, `pop`, `repeat`, `+` / `+=` |
| searching | `contains`, `starts_with`, `ends_with`, `find`, `parse_i64`, `parse_f64` |
| transforming | `replace`, `to_uppercase`, `to_lowercase`, `trim`, `split`, `substring` |
| character oriented | `char_count`, `chars`, `char_at`, `is_char_boundary`, `String::from_char` |

The full signatures, examples and panic conditions are in the
[Standard library chapter](../language/15-standard-library.md#string). A `String` is
always valid UTF-8; byte indices are checked at run time, and slicing text panics
unless both boundaries are character boundaries.

## Implemented additions

These items were added on top of the migrated 1.7 surface and are implemented in
`stdlib/src/string.ls`:

| Item | Signature | Notes |
| --- | --- | --- |
| `String::from_str` | `(&str) -> String` | copies the bytes of a borrowed text view; the only way to own borrowed text |
| `as_str` | `(&self) -> &str` | borrows the text without copying |
| `as_mut_str` | `(&mut self) -> &mut str` | mutable borrowed view; requires a unique borrow |
| `as_bytes` | `(&self) -> &[u8]` | the UTF-8 bytes as a slice |
| `str` methods | `len`, `is_empty`, `char_count`, `contains`, `starts_with`, `ends_with`, `find`, `split`, `parse_i64`, `parse_f64`, `to_uppercase`, `to_lowercase`, `trim`, `is_char_boundary`, `char_at`, `slice`, `as_bytes` | methods on `&str`; documented in [Slices and str](../language/26-slices-and-str.md) |

`String::from_str` is implemented over the `__str_from_str` intrinsic, which copies
the bytes into a fresh managed buffer. `as_str` and `as_mut_str` produce the fat
reference view described in [Slices and str](../language/26-slices-and-str.md); they
do not allocate.

## Planned in 2.0

| Item | Signature | Notes |
| --- | --- | --- |
| `push_str` with `&str` | `(&mut self, &str)` | borrowed text parameters are planned for a later release; in 2.0.0 the parameter is an owned `String` |
| text-taking methods with `&str` | needles, prefixes, patterns, replacements | deferred; in 2.0.0 these parameters are owned `String` |
| `String` as a builtin marker | - | `impl String` blocks may only be written in the `std` crate; user crates get `E0118` |

Public APIs in the library take owned `String` arguments for text so that they stay
usable from programs that only have string literals; passing a `&str` back into
owned text requires `String::from_str`.

The legacy methods `String::replace`, `String::trim`, `String::parse_i64` and
`String::parse_f64` are still served by the compiler's builtin method table in 2.0.0,
so their 1.7 signatures are unchanged even though the `str` methods are the
long-term home of trimming and parsing.

## Memory and ORC

A `String` owns a reference-counted UTF-8 buffer. `.clone()` copies the text; moves
transfer the handle. `String` participates in the ORC cycle collector like every
other managed value. See [Memory model](../language/05-memory-model.md).

## See also

* [Characters and Unicode](../language/22-characters-and-unicode.md) - character
  semantics, boundary panics and Debug escaping.
* [char](char.md) - character classification (ASCII methods implemented, Unicode
  classification not implemented in 2.0.0).
* [Slices and str](../language/26-slices-and-str.md) - borrowed text.
