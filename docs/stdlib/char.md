# char: character classification

**Status in 2.3.0.** Partly implemented. The `char` type itself is available
(added in 1.6.0) and the `std` crate adds ASCII classification and conversion as
inherent methods on `char` (`impl char` in `stdlib/src/char.ls`). Version 2.3
adds Unicode classification and Unicode case conversion over generated tables.

## The type

`char` is a `Copy` Unicode scalar value written with single quotes (`'a'`, `'\n'`,
`'é'`). String literals and character literals share escape rules. A `char` is not a
grapheme cluster, and there is no normalization API. Characters compare by scalar
value, convert with `as` (only `u8 as char` is allowed into `char`), and can be used
as `HashMap` keys. The language reference is
[Characters and Unicode](../language/22-characters-and-unicode.md).

## ASCII classification and conversion

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

Every ASCII classification method is a strict ASCII test: a non-ASCII code point
returns `false` for all of them, so `'é'.is_ascii_alphabetic()` is `false` and
`'é'.is_ascii()` is `false` too.

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

## Unicode classification and case conversion

Version 2.3 adds Unicode-aware predicates and case conversion on `char`. The
methods are locale-independent and read the same generated tables that
`String` case mapping and trimming use, from Unicode 16.0.0.

| Method | Returns | Meaning |
| --- | --- | --- |
| `is_alphabetic(&self)` | `bool` | Unicode `Alphabetic` property: general categories `L*` and `Nl`, plus `Other_Alphabetic` |
| `is_numeric(&self)` | `bool` | Unicode general categories `Nd`, `Nl` and `No` |
| `is_alphanumeric(&self)` | `bool` | `is_alphabetic() || is_numeric()` |
| `is_whitespace(&self)` | `bool` | Unicode `White_Space` property, the same set `trim` removes |
| `to_uppercase(&self)` | `char` | Unicode default uppercasing, as the closest single character |
| `to_lowercase(&self)` | `char` | Unicode default lowercasing, as the closest single character |

`is_alphabetic` includes letters outside the ASCII range and characters such as
`'\u{0345}'` (combining Greek ypogegrammeni) that carry `Other_Alphabetic`.
`is_numeric` accepts decimal digits from any script as well as numeric
characters such as `'Ⅷ'` and `'\u{00B2}'`. `is_whitespace` uses the same
`White_Space` set as `String::trim`, so it includes `'\u{00A0}'`,
`'\u{2028}'` and `'\u{3000}'` but not the zero-width space `'\u{200B}'`.

```ls
fn main() {
    println!("{} {}", 'é'.is_alphabetic(), '7'.is_alphabetic());
    println!("{} {}", '7'.is_numeric(), 'Ⅷ'.is_numeric());
    println!("{} {}", '\u{3000}'.is_whitespace(), '\u{200B}'.is_whitespace());
    println!("{} {} {}", 'ß'.to_uppercase(), 'İ'.to_lowercase(), '山'.to_uppercase());
    println!("{} {}", 'é'.is_alphanumeric(), '\u{2167}'.is_alphanumeric());
}
```

Output:

```text
true false
true true
true false
S i 山
true true
```

The case methods return one `char`. Some Unicode case mappings expand to more
than one character (the sharp s `'ß'` uppercases to `SS`, and the dotted I
`'İ'` lowercases to `i` plus U+0307); `char::to_uppercase` and
`char::to_lowercase` return the first scalar of the full mapping, the closest
single character. This differs from Rust, where `char::to_uppercase` returns an
iterator and `String::to_uppercase` produces the full expansion. The
`String` methods in lsaca keep the full mapping: `String::from("ß").to_uppercase()`
is `SS`. See [string](string.md).

The tables are generated by `scripts/gen_unicode_tables.py` from Python's
`unicodedata` plus a committed Unicode 16.0.0 `Other_Alphabetic` extract; the
generated C file is committed, so builds need no network access. A `char` is
still a scalar value, not a grapheme cluster, and there is no normalization API.

## See also

* [Characters and Unicode](../language/22-characters-and-unicode.md).
* [string](string.md) - `String` case mapping (`to_uppercase`,
  `to_lowercase`) and trimming.
