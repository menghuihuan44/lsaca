# char: character classification

**Status in 2.1.0.** Partly implemented. The `char` type itself is available
(added in 1.6.0). The `std` crate adds ASCII classification and conversion as
inherent methods on `char` (`impl char` in `stdlib/src/char.ls`); classification
over the full Unicode property tables is not implemented and is marked as such
below.

## The type

`char` is a `Copy` Unicode scalar value written with single quotes (`'a'`, `'\n'`,
`'é'`). String literals and character literals share escape rules. A `char` is not a
grapheme cluster, and there is no normalization API. Characters compare by scalar
value, convert with `as` (only `u8 as char` is allowed into `char`), and can be used
as `HashMap` keys. The language reference is
[Characters and Unicode](../language/22-characters-and-unicode.md).

## Implemented API: ASCII classification and conversion

These are methods on `char`, not free functions: write `'a'.is_ascii_digit()`.
They are implemented in `stdlib/src/char.ls` and are available through the
prelude without an import. Every method takes the receiver by reference
(`&self`).

| Method | Returns | Meaning |
| --- | --- | --- |
| `is_ascii(&self)` | `bool` | code point below 128 |
| `is_ascii_digit(&self)` | `bool` | `'0'..='9'` |
| `is_ascii_uppercase(&self)` | `bool` | `'A'..='Z'` |
| `is_ascii_lowercase(&self)` | `bool` | `'a'..='z'` |
| `is_ascii_alphabetic(&self)` | `bool` | `'A'..='Z'` or `'a'..='z'` |
| `is_ascii_alphanumeric(&self)` | `bool` | alphabetic or digit |
| `is_ascii_hexdigit(&self)` | `bool` | `0-9`, `a-f` or `A-F` |
| `is_ascii_whitespace(&self)` | `bool` | space, `\t`, `\n`, `\r`, `\x0b`, `\x0c` |
| `is_ascii_control(&self)` | `bool` | a control character |
| `is_ascii_graphic(&self)` | `bool` | a printable character other than space |
| `is_ascii_punctuation(&self)` | `bool` | ASCII punctuation |
| `is_ascii_word(&self)` | `bool` | ASCII letter, ASCII digit or `'_'` (an lsaca extension used by the lexer) |
| `to_ascii_uppercase(&self)` | `char` | ASCII uppercase mapping; other characters unchanged |
| `to_ascii_lowercase(&self)` | `char` | ASCII lowercase mapping; other characters unchanged |
| `to_digit(&self, radix: u32)` | `Option<u32>` | digit value in the given radix (`2..=36`); `None` when the character is not a digit of that radix |

Every classification method is a strict ASCII test: a non-ASCII code point
returns `false` for all of them, so `'é'.is_ascii_alphabetic()` is `false` and
`'é'.is_ascii()` is `false` too. Unicode classification needs the generated
property tables described in the next section.

```ls
fn main() {
    println!("{} {}", 'a'.is_ascii_alphabetic(), '1'.is_ascii_digit());
    println!("{} {}", 'F'.is_ascii_hexdigit(), ' '.is_ascii_whitespace());
    println!("{} {}", '7'.to_digit(10).unwrap(), '8'.to_digit(8).is_some());
    println!("{}", 'é'.is_ascii());
}
```

Output:

```text
true true
true true
7 false
false
```

`'7'.to_digit(10)` is `Some(7)` and printing it through `{}` prints `7`;
`'8'` is not an octal digit, so `'8'.to_digit(8)` is `None` and
`is_some()` prints `false`. The last line shows that `é` is not ASCII.

## Unicode classification (planned for a later release)

The design also fixes a Unicode-property classification surface, implemented
over generated tables so that behaviour is consistent across platforms and
independent of locale. The exact names are not frozen until the module lands; the
intended functions are:

| Function (planned) | Returns | Meaning |
| --- | --- | --- |
| `is_alphabetic` | `bool` | Unicode alphabetic property |
| `is_numeric` | `bool` | Unicode numeric property |
| `is_alphanumeric` | `bool` | alphabetic or numeric |
| `is_whitespace` | `bool` | Unicode whitespace, the same set `trim` uses |
| `to_uppercase` | `String` | Unicode default uppercasing (can be more than one character) |
| `to_lowercase` | `String` | Unicode default lowercasing |

The classification tables are generated at build time from the Unicode data files
that 1.6.0 already uses for case mapping and trimming; the generator is checked
into the repository and its output is committed, so builds do not need network
access. Until the tables land there are no Unicode classification methods.

## See also

* [Characters and Unicode](../language/22-characters-and-unicode.md).
* [string](string.md) - `String` case mapping (`to_uppercase`,
  `to_lowercase`) and trimming.
