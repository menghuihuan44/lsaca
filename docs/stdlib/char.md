# char: character classification

**Status in 2.0.0.** Partly implemented. The `char` type itself is available
(added in 1.6.0). The 2.0 `std` crate adds ASCII classification as inherent
methods on `char` (`impl char` in `stdlib/src/char.ls`); classification over the
full Unicode property tables is not implemented in 2.0.0 and is marked as such below.

## The type

`char` is a `Copy` Unicode scalar value written with single quotes (`'a'`, `'\n'`,
`'é'`). String literals and character literals share escape rules. A `char` is not a
grapheme cluster, and there is no normalization API. Characters compare by scalar
value, convert with `as` (only `u8 as char` is allowed into `char`), and can be used
as `HashMap` keys. The language reference is
[Characters and Unicode](../language/22-characters-and-unicode.md).

## Implemented API: ASCII classification

These are methods on `char`, not free functions: write `'a'.is_ascii_digit()`.
They are implemented in `stdlib/src/char.ls` and are available through the
prelude without an import.

| Method | Returns | Meaning |
| --- | --- | --- |
| `is_ascii_word(self)` | `bool` | ASCII letter, ASCII digit or `'_'` |
| `is_ascii_digit(self)` | `bool` | `'0'..='9'` |
| `is_ascii_uppercase(self)` | `bool` | `'A'..='Z'` |
| `is_ascii_lowercase(self)` | `bool` | `'a'..='z'` |
| `is_ascii_alphabetic(self)` | `bool` | `'A'..='Z'` or `'a'..='z'` |
| `is_ascii_alphanumeric(self)` | `bool` | alphabetic or digit |
| `is_ascii_hexdigit(self)` | `bool` | `0-9`, `a-f` or `A-F` |
| `is_ascii_whitespace(self)` | `bool` | space, `\t`, `\n`, `\r`, `\x0b`, `\x0c` |

The receiver is taken by value because `char` is `Copy`. Every method is a
strict ASCII test: a non-ASCII code point returns `false` for all of them, so
`'é'.is_ascii_alphabetic()` is `false`. Unicode classification needs the
generated property tables described in the next section.

```ls
fn main() {
    println!("{} {}", 'a'.is_ascii_alphabetic(), '1'.is_ascii_digit());
    println!("{} {}", 'F'.is_ascii_hexdigit(), ' '.is_ascii_whitespace());
}
```

Output:

```text
true true
true true
```

Both lines print `true true` (checked with the 2.0.0 compiler).

## Unicode classification (planned for a later release)

The 2.0 design also fixes a Unicode-property classification surface, implemented
over generated tables so that behaviour is consistent across platforms and
independent of locale. The exact names are not frozen until the module lands; the
intended functions are:

| Function (planned) | Returns | Meaning |
| --- | --- | --- |
| `is_alphabetic(c)` | `bool` | Unicode alphabetic property |
| `is_numeric(c)` | `bool` | Unicode numeric property |
| `is_alphanumeric(c)` | `bool` | alphabetic or numeric |
| `is_whitespace(c)` | `bool` | Unicode whitespace, the same set `trim` uses |
| `is_ascii(c)` | `bool` | code point below 128 |
| `to_digit(c, radix)` | `Option<u32>` | digit value in the given radix |
| `from_digit(value, radix)` | `Option<char>` | character for a digit value |
| `to_uppercase(c)` | `String` | Unicode default uppercasing (can be more than one character) |
| `to_lowercase(c)` | `String` | Unicode default lowercasing |

The classification tables are generated at build time from the Unicode data files
that 1.6.0 already uses for case mapping and trimming; the generator is checked
into the repository and its output is committed, so builds do not need network
access. Until the tables and the module-level functions land there are no
`std::char` free functions: only the methods above exist.

## See also

* [Characters and Unicode](../language/22-characters-and-unicode.md).
* [string](string.md) - `String` case mapping (`to_uppercase`,
  `to_lowercase`) and trimming.
