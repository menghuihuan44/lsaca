# Characters and Unicode

Version 1.6.0 adds the `char` primitive type and makes `String` a real UTF-8
string: every `String` value in a running program always contains valid UTF-8,
and the standard library grows character-oriented APIs next to the existing
byte-oriented ones. This page covers character literals and values, the
character APIs of `String`, the boundary rules of `substring`, Unicode case
mapping and trimming, Rust-style `Debug` escaping, UTF-8 validation of external
input, and `char` as a `HashMap` key.

The lexical rules for literals are in [Lexical structure](03-lexical-structure.md);
the full method tables are in [Standard library](15-standard-library.md); pattern
syntax is in [Pattern matching](13-pattern-matching.md).

## The `char` type

A `char` is a single Unicode scalar value: an integer in the range
`0..=0x10FFFF` excluding the surrogate range `0xD800..=0xDFFF`. That is the
same set of values Rust's `char` covers. A scalar value may be encoded as one,
two, three or four UTF-8 bytes; the `char` itself is a single value, not a
string.

`char` is a `Copy` scalar, like the integer and floating-point types and
`bool`. It is stored as an unsigned 32-bit value, does not participate in ORC
reference counting, and is never moved. Any number of copies of a `char` can be
used after it is read.

`char` can appear wherever a scalar type can appear: in `let` bindings,
`const` items, function parameters and return types, struct fields, enum
payloads, tuples, and inside builtin generic types such as `Vec<char>`,
`Option<char>`, `Result<char, E>`, `Box<char>`, `Rc<char>` and as a
`HashMap<char, V>` key.

```ls
struct Glyph {
    symbol: char,
    weight: i64,
}

enum Token {
    Letter(char),
    Digit(char),
}

fn first(text: &String) -> Option<char> {
    text.char_at(0)
}

fn main() {
    let letter: char = 'x';
    let smiley: char = '😀';
    let glyph = Glyph { symbol: letter, weight: 3 };
    let token = Token::Digit('7');
    let letters: Vec<char> = vec!['a', 'b', 'c'];
    let hello = String::from("hello");
    let maybe: Option<char> = first(&hello);
    let result: Result<char, i64> = Ok('z');
    let boxed: Box<char> = Box::new('q');
    let shared: Rc<char> = Rc::new('r');

    println!("{} {} {} {}", letter, smiley, glyph.symbol, glyph.weight);
    println!("{:?} {:?}", token, letters);
    println!("{:?} {:?} {:?} {:?}", maybe, result, boxed, shared);
}
```

Output:

```text
x 😀 x 3
Digit('7') ['a', 'b', 'c']
Some('h') Ok('z') 'q' Rc('r')
```

A `char` is not a one-character `String`: the two types are not
interchangeable, and a `String` holding a single character still costs a
heap allocation and reference count. Use `char` for single characters and
`String` for text.

## Character literals

A character literal is written in single quotes and contains exactly one
Unicode scalar value: `'a'`, `'é'`, `'你'`, `'😀'`. The character literal
syntax reuses the escape rules of string literals, so the same escape sequences
are available, with two additions that matter in this context:

| Escape | Meaning |
| --- | --- |
| `\n` | newline (U+000A) |
| `\r` | carriage return (U+000D) |
| `\t` | tab (U+0009) |
| `\\` | backslash |
| `\'` | single quote |
| `\"` | double quote (also usable unescaped) |
| `\0` | NUL (U+0000) |
| `\xNN` | one byte, `0x00`-`0x7F` only; the scalar value is that byte |
| `\u{HEX}` | one Unicode scalar value, one to six hexadecimal digits |

The single quote must be escaped because it would otherwise terminate the
literal: write `'\''` for the quote character. A double quote needs no
escape, so `'"'` is fine.

```ls
fn main() {
    let quote = '\'';
    let double = '"';
    let tab = '\t';
    let nul = '\0';
    let accent = '\u{e9}';
    let emoji = '\u{1F600}';

    println!("{} {} {} {}", quote, double, tab == '\t', nul == '\0');
    println!("{} {}", accent, emoji);
}
```

Output:

```text
' " true true
é 😀
```

The literal must contain exactly one scalar value; it cannot be empty and
cannot contain two characters. The escape `\xNN` accepts ASCII bytes only,
so it cannot be used to write a non-ASCII scalar; use `\u{...}` for that. The
surrogate range and values above `0x10FFFF` are not valid scalars and are
rejected in `\u{...}` escapes.

```ls,compile-fail
fn main() {
    let empty = '';
    println!("{:?}", empty);
}
```

Error:

```text
error[E0007]: empty character literal
  --> example-03.ls:2:17
   |
 2 |     let empty = '';
   |                 ^^

lsaca: example-03.ls: could not compile example-03.ls due to previous error(s)
```

```ls,compile-fail
fn main() {
    let too_many = 'ab';
    println!("{:?}", too_many);
}
```

Error:

```text
error[E0007]: character literal may only contain one codepoint
  --> example-04.ls:2:20
   |
 2 |     let too_many = 'ab';
   |                    ^^^^

lsaca: example-04.ls: could not compile example-04.ls due to previous error(s)
```

```ls,compile-fail
fn main() {
    let bad = '\q';
    println!("{:?}", bad);
}
```

Error:

```text
error[E0005]: unknown escape sequence \q
  --> example-05.ls:2:16
   |
 2 |     let bad = '\q';
   |                ^^

lsaca: example-05.ls: could not compile example-05.ls due to previous error(s)
```

The general escape table, including how these escapes work inside string
literals, is in [Lexical structure](03-lexical-structure.md#string-literals).

## Comparing characters

The six comparison operators work on `char` values and compare their scalar
values (Unicode code points): `==`, `!=`, `<`, `<=`, `>`, `>=`. This is
code point order, not a locale-aware collation, so `'a' < 'b'` is true and
`'Z' < 'a'` is true. Use `to_uppercase` or `to_lowercase` (described below)
before comparing if you want case-insensitive matching of ASCII text.

```ls
fn main() {
    let a = 'a';
    let b = 'b';
    println!("{} {} {} {}", a == 'a', a != b, a < b, b >= 'b');
    println!("{} {}", 'Z' < 'a', 'é' > 'e');
}
```

Output:

```text
true true true true
true true
```

Unlike integers, characters are not numbers in the type system: arithmetic,
bitwise operations and unary minus do not apply to `char`. They are rejected
with a type error. Casting is the explicit way to get the numeric value.

```ls,compile-fail
fn main() {
    let next = 'a' + 1;
    println!("{}", next);
}
```

Error:

```text
error[E0308]: mismatched types
  --> example-07.ls:2:16
   |
 2 |     let next = 'a' + 1;
   |                ^^^^^^^
   = note: expected char, found {integer}

lsaca: example-07.ls: could not compile example-07.ls due to previous error(s)
```

## Casting between `char` and numbers

`as` is the only conversion between a `char` and a numeric type:

| Cast | Result |
| --- | --- |
| `char as i8`, `i16`, `i32`, `i64` | the scalar value, truncated to the target type |
| `char as u8`, `u16`, `u32`, `u64` | the scalar value, truncated to the target type |
| `char as f32`, `f64` | the scalar value as a floating-point number |
| `u8 as char` | the byte value as a scalar |
| other integer `as char` | rejected: only `u8` can be cast as `char` |

Casting a non-ASCII character to a narrow integer truncates it, so do not rely
on `as u8` as an "encoding" step; the numeric value of `'é'` is
`0xE9 = 233`, which happens to fit a `u8` here, but `'😀'` does not fit and is
truncated. A `char` cannot be cast to `bool`, and `bool` cannot be cast to
`char`.

```ls
fn main() {
    println!("{}", 'A' as i64);
    println!("{}", 'é' as i64);
    println!("{}", 'A' as u8 + 1);
    println!("{:?}", 65 as u8 as char);
    println!("{}", 'a' as f64);
}
```

Output:

```text
65
233
66
'A'
97
```

```ls,compile-fail
fn main() {
    let value: char = 1000 as char;
    println!("{}", value);
}
```

Error:

```text
error[E0308]: only u8 can be cast as char
  --> example-09.ls:2:23
   |
 2 |     let value: char = 1000 as char;
   |                       ^^^^^^^^^^^^

lsaca: example-09.ls: could not compile example-09.ls due to previous error(s)
```

## Character patterns

A character literal is a valid pattern and can be combined with or-patterns
and a fallback arm, just like integer literals. Because there are far more
characters than a `match` can list, the compiler always requires a fallback
arm unless the arm list is exhaustive for another reason; use `_` for the
remaining characters.

```ls
fn kind(c: char) -> String {
    match c {
        'a' | 'e' | 'i' | 'o' | 'u' => String::from("vowel"),
        '0' | '1' | '2' | '3' | '4' | '5' | '6' | '7' | '8' | '9' => String::from("digit"),
        _ => String::from("other"),
    }
}

fn main() {
    for c in vec!['a', '7', 'Z', 'é'] {
        println!("{} {}", c, kind(c));
    }
}
```

Output:

```text
a vowel
7 digit
Z other
é other
```

Moves in patterns do not apply to `char` because it is `Copy`; a character
pattern copies the scalar value. The full pattern grammar is in
[Pattern matching](13-pattern-matching.md).

## Formatting characters

`{}` writes the character itself, encoded as UTF-8. `{:?}` writes a quoted,
Rust-style character literal: the quote, the backslash and the control escapes
are used where they exist, and anything else that is not printable is written
as `\u{...}`.

```ls
fn main() {
    println!("{}", 'A');
    println!("{}", '😀');
    println!("{:?}", 'A');
    println!("{:?}", '\n');
    println!("{:?}", '\'');
    println!("{:?}", '😀');
}
```

Output:

```text
A
😀
'A'
'\n'
'\''
'😀'
```

Width, alignment and fill are supported for characters and count one column
per character. The precision, `+` and `0` format flags do not apply to
characters and are rejected at compile time.

```ls
fn main() {
    println!("[{:5}]", 'x');
    println!("[{:<5}]", 'x');
    println!("[{:*^5}]", 'é');
}
```

Output:

```text
[x    ]
[x    ]
[**é**]
```

```ls,compile-fail
fn main() {
    println!("[{:.2}]", 'x');
}
```

Error:

```text
error[E0001]: the precision of a format specifier is not supported for characters
  --> example-13.ls:2:25
   |
 2 |     println!("[{:.2}]", 'x');
   |                         ^^^

lsaca: example-13.ls: could not compile example-13.ls due to previous error(s)
```

The formatting rules for the other types are in
[Standard library](15-standard-library.md#formatting-and-printing).

## `String` is always valid UTF-8

Since 1.6.0, the bytes of every `String` are valid UTF-8 at every point a
program can observe the string. This is a type invariant, not something the
program has to maintain: every constructor and every method preserves it.
String literals, concatenation, `push_str`, `replace`, `repeat`, `split`,
case conversion, trimming, `substring`, `String::from_char`, `push`,
`read_line`, `read_file` and `args` all either produce valid UTF-8 or reject
the input. There is no way to build an invalid `String` from valid language
values; invalid data can only arrive from the operating system, and the I/O
functions check it (see below).

The byte-oriented API is unchanged and intentionally Rust-compatible:
`len()` is the length in bytes, `find()` returns a byte index, and
`substring(start, len)` takes byte offsets and byte lengths. The new
character-oriented methods are added next to them. Use the byte API when you
care about storage size or when you are working with ASCII, and the character
API when you care about characters.

```ls
fn main() {
    let text = String::from("héllo");
    println!("{} {}", text.len(), text.char_count());
    println!("{}", text.find(String::from("llo")));
    println!("{}", text.chars().len());
}
```

Output:

```text
6 5
Some(3)
5
```

## Character-oriented `String` methods

| Method | Returns | Meaning |
| --- | --- | --- |
| `char_count()` | `i64` | number of Unicode scalar values |
| `chars()` | `Vec<char>` | every scalar value, in order |
| `char_at(i)` | `Option<char>` | the `i`-th scalar value; `None` when out of range |
| `is_char_boundary(i)` | `bool` | whether byte index `i` is a character boundary |
| `push(c)` | nothing | append one character (the string must be `mut`) |
| `pop()` | `Option<char>` | remove the last character (the string must be `mut`) |
| `String::from_char(c)` | `String` | a new one-character string |

`char_at` counts characters, not bytes: `char_at(1)` of `"héllo"` is
`Some('é')`, and `char_at(5)` is `None`. `is_char_boundary(i)` returns
`true` for `0`, for `len()`, and for every byte index where a scalar value
starts; it returns `false` for byte positions inside a multi-byte character
and for indices greater than `len()`.

```ls
fn main() {
    let text = String::from("héllo");
    let characters: Vec<char> = text.chars();
    println!("{:?}", characters);
    println!("{:?} {:?}", text.char_at(0), text.char_at(1));
    println!("{:?} {:?}", text.char_at(4), text.char_at(5));
    println!("{} {} {}", text.is_char_boundary(0), text.is_char_boundary(1), text.is_char_boundary(2));
    println!("{} {}", text.is_char_boundary(3), text.is_char_boundary(99));
}
```

Output:

```text
['h', 'é', 'l', 'l', 'o']
Some('h') Some('é')
Some('o') None
true true false
true false
```

`push` and `pop` change the string in place, so the binding must be
`mut`; they keep the UTF-8 invariant. `String::from_char` builds a new
string from one character.

```ls
fn main() {
    let mut text = String::from("h");
    text.push('é');
    text.push('!');
    println!("{} {}", text, text.char_count());

    let last: Option<char> = text.pop();
    println!("{:?} {}", last, text);

    let mut empty: String = String::from("");
    println!("{:?}", empty.pop());
    let single = String::from_char('ñ');
    println!("{} {}", single, single.len());
}
```

Output:

```text
hé! 3
Some('!') hé
None
ñ 2
```

`chars()` returns an eager `Vec<char>`, not a lazy iterator. That makes it
easy to pass to the iterator adapters (a `Vec<char>` can be iterated with
`for`, `map`, `filter` and the other adapters), at the cost of one
allocation per call. See [Iterators](19-iterators.md).

## `substring` and character boundaries

`substring(start, len)` keeps its byte-offset, byte-length signature: it
returns the string formed by the `len` bytes starting at byte offset
`start`. Because those offsets are not necessarily character boundaries,
`substring` checks its arguments at run time and panics when the result would
split a character or run past the end of the string. A panic exits the process
with code 101.

The three failure cases, matching Rust's `str` slicing rules, are:

1. `start + len` is greater than the byte length: the message reads
   `byte index N is out of bounds of` and then the string itself in
   backticks, where `N` is `start + len`; a string longer than 256 bytes
   is truncated with `...`.
2. `start` is not a character boundary: the message is
   `byte index N is not a char boundary; it is inside C (bytes A..B) of S`,
   where `N` is `start`, `C` is the character at that position in Rust
   `Debug` form (`'é'`), `A..B` is that character's byte range, and `S` is
   the string.
3. `start + len` is not a character boundary (and is not the end of the
   string): the same non-boundary message with `N = start + len`.

For example, on `"héllo"` the `é` occupies bytes 1 and 2, so
`substring(1, 2)` is safe and returns `"é"`, while `substring(2, 1)`
panics because byte 2 is inside the `é`.

```ls
fn main() {
    let text = String::from("héllo");
    let accent = text.substring(1, 2);
    println!("{} {} {}", accent, accent.len(), accent.char_count());
    let tail = text.substring(3, 3);
    println!("{}", tail);
}
```

Output:

```text
é 2 1
llo
```

```ls,panic
fn main() {
    let text = String::from("héllo");
    let broken = text.substring(2, 1);
    println!("{}", broken);
}
```

Panic:

```text
thread 'main' panicked at example-18.ls:3:18:
byte index 2 is not a char boundary; it is inside 'é' (bytes 1..3) of `héllo`
```

```ls,panic
fn main() {
    let text = String::from("héllo");
    let past = text.substring(5, 10);
    println!("{}", past);
}
```

Panic:

```text
thread 'main' panicked at example-19.ls:3:16:
byte index 15 is out of bounds of `héllo`
```

## Unicode case mapping and trimming

`to_uppercase()` and `to_lowercase()` implement the Unicode default case
mappings, not ASCII-only ones. They can change the number of characters, because
some mappings are one-to-many: `"ß".to_uppercase()` is `"SS"`,
`"İ".to_lowercase()` is `i` followed by U+0307, and
`"ﬁ".to_uppercase()` is `"FI"`. The methods return new `String` values and
do not modify the receiver.

`trim()` removes leading and trailing Unicode whitespace, that is, characters
for which Rust's `char::is_whitespace` is true: ASCII spaces and controls,
but also U+00A0 no-break space, U+3000 ideographic space and the other Unicode
`White_Space` characters. It does not remove internal whitespace.

```ls
fn main() {
    let german = String::from("straße");
    println!("{} {}", german.to_uppercase(), german.to_uppercase().char_count());

    let accent = String::from("école");
    println!("{}", accent.to_uppercase());

    let dotted = String::from("İ");
    println!("{} {}", dotted.to_lowercase(), dotted.to_lowercase().char_count());

    let padded = String::from("\u{a0}  hello  \u{3000}");
    println!("[{}] {}", padded.trim(), padded.trim().char_count());
}
```

Output:

```text
STRASSE 7
ÉCOLE
i̇ 2
[hello] 5
```

## `Debug` escaping

`{:?}` of a `String` uses Rust's `str` `Debug` rules. The quote and
backslash are escaped, `\n`, `\r` and `\t` use their short forms, and
characters that are not printable ASCII/Unicode text are escaped as
`\u{...}`: control characters, format characters, private-use characters,
unassigned code points, the line and paragraph separators, and combining marks.
Printable characters, including non-ASCII text, are written as UTF-8 without
escaping. `{:?}` of a `char` uses the same rules but wraps the result in
single quotes.

```ls
fn main() {
    println!("{:?}", "hi");
    println!("{:?}", "a\tb\nc");
    println!("{:?}", "say \"hi\" \\ bye");
    println!("{:?}", "café");
    println!("{:?}", '\n');
    println!("{:?}", 'é');
    println!("{:?}", '\u{1}');
}
```

Output:

```text
"hi"
"a\tb\nc"
"say \"hi\" \\ bye"
"café"
'\n'
'é'
'\u{1}'
```

This is the same escaping Rust produces for strings and characters; the exact
set of "printable" characters follows Rust's `char::escape_debug` behaviour.

## Invalid UTF-8 from the outside world

Because the language itself cannot construct an invalid `String`, invalid
UTF-8 can only arrive from the operating system: a file, standard input, or a
command-line argument. The standard library validates those inputs instead of
replacing bad bytes with U+FFFD.

| API | Invalid UTF-8 behaviour |
| --- | --- |
| `read_line()` | panics with `stream did not contain valid UTF-8` (exit code 101) |
| `read_file(path)` | returns `Err("stream did not contain valid UTF-8")`; it does not panic |
| `args()` | panics with `arguments contain invalid UTF-8` (exit code 101) |

`read_line()` returns `Option<String>`: `None` at end of input, and
`Some(line)` for a line with the line terminator removed. If the bytes read
are not valid UTF-8, the whole program panics at the call site; there is no
lossy variant in 1.6.0. `read_file` is the non-panicking form: it returns
`Result<String, String>`, and a file whose bytes are not valid UTF-8 becomes
`Err("stream did not contain valid UTF-8")`. `args()` panics if any argument
is not valid UTF-8.

`write_file` is unchanged: it writes the bytes of a `String`, which are
always valid UTF-8, so it cannot create an invalid file.

These behaviours cannot be shown as a runnable example on this page, because a
lsaca program cannot produce invalid bytes: the check exists precisely to stop
them. The end-to-end test harness covers the three cases with raw byte streams.

## `char` in a `HashMap`

`char` is a valid `HashMap` key type alongside `i64`, `u64`, `bool` and
`String`. Keys are copied into the map; the usual rule that `insert` moves
its arguments applies to the `char` key trivially (it is `Copy`). The hash of
a character is the hash of its scalar value.

```ls
fn main() {
    let mut counts: HashMap<char, i64> = HashMap::new();
    let text = String::from("hello");
    for c in text.chars() {
        let previous: Option<i64> = counts.get(c);
        match previous {
            Some(n) => { counts.insert(c, n + 1); }
            None => { counts.insert(c, 1); }
        }
    }
    let h: Option<i64> = counts.get('h');
    let z: Option<i64> = counts.get('z');
    println!("{:?} {:?} {}", h, z, counts.len());

    let mut one: HashMap<char, i64> = HashMap::new();
    one.insert('a', 1);
    let mut two: HashMap<char, i64> = HashMap::new();
    two.insert('a', 1);
    println!("{}", one == two);
}
```

Output:

```text
Some(1) None 4
true
```

The printed order of a `HashMap` is not specified; the map prints its slots
in the order the implementation stores them, which is deterministic for a given
sequence of operations but is not an insertion order and is not a language
guarantee. Content equality (`==`) ignores insertion order. The map is not
iterable with `for` in 1.6.0; use `keys()` or `values()` first. See
[HashMap](20-hashmap.md) for the implementation and hashing notes.

## Differences from Rust

The character support follows Rust closely but stays inside lsaca's design:

* `char` has no methods. Rust's `char::is_alphabetic`, `is_whitespace`,
  `to_digit`, `is_ascii` and friends do not exist; case mapping and trimming
  are `String` methods.
* There are no character ranges or slices: `'a'..='z'` is not a range, and
  strings cannot be indexed with characters.
* `chars()` returns an eager `Vec<char>`, not a lazy iterator. A `Vec<char>`
  can be used with the iterator adapters, but every call allocates.
* A `char` is a Unicode scalar value, not a grapheme cluster: an `"é"` typed
  as `e` plus a combining accent is two `char` values. There is no Unicode
  normalization (NFC/NFD) API.
* Arithmetic on characters is rejected; in Rust the same operations are also
  rejected, but lsaca's error message is its own.
* Numeric conversion is explicit through `as`; there is no
  `char::from_u32` or `u32::from(char)` trait machinery, and only `u8` may
  be cast to `char`.
* The byte-oriented `String` API is unchanged from 1.5.0: `len`, `find` and
  `substring` use bytes, and `s[0]` is still rejected with E0608.

## Known limitations in 1.6.0

* No character classification (alphabetic, numeric, whitespace) methods on
  `char`; Unicode property tables are only used internally by case mapping and
  trimming.
* No grapheme cluster or normalization support; operators work on scalar
  values.
* No locale-aware collation: comparisons and sorting use code point order.
* `substring` panics instead of returning an `Option`; a panic cannot be
  caught in 1.6.0.
* Invalid UTF-8 on stdin or in arguments panics rather than returning an error.
* `chars()` allocates a `Vec`; there is no borrowing character iterator.
* `HashMap` iteration is still not available; keys with either `String` or
  `char` hash through a deterministic, non-cryptographic hash function, which
  is not hardened against adversarial input.
* Formatting a character ignores East Asian display width; a character counts
  as one column for width and alignment.

See [Roadmap and differences from Rust](16-roadmap.md#known-limitations-in-160)
for the complete list.

## Summary

* `char` is a `Copy` Unicode scalar value; character literals use single
  quotes and the string escape rules.
* Characters compare by scalar value; arithmetic is rejected and conversions
  go through `as`, with only `u8 as char` allowed into `char`.
* `String` is guaranteed valid UTF-8; `len`, `find` and `substring` remain
  byte-based, while `char_count`, `chars`, `char_at`, `push`, `pop`,
  `is_char_boundary` and `String::from_char` work with characters.
* `substring` panics unless both boundaries are character boundaries and the
  range fits; the panic messages follow Rust.
* Case mapping and trimming are Unicode-aware, and `Debug` escaping follows
  Rust's `escape_debug`.
* `read_line` and `args` reject invalid UTF-8 by panicking, `read_file`
  returns an error, and `char` works as a `HashMap` key.

## What next

* [Lexical structure](03-lexical-structure.md) - the full escape and literal
  grammar.
* [Types](04-types.md) - where `char` sits among the scalar types.
* [Standard library](15-standard-library.md) - complete `String` method tables
  and formatting rules.
* [HashMap](20-hashmap.md) - the builtin map, including `char` keys.
* [Roadmap and differences from Rust](16-roadmap.md) - the full set of
  limitations in 1.6.0.
