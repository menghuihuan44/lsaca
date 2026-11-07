# Lexical structure

This page describes the raw text of a program: how source files are encoded, the
character set of identifiers, the keywords, comments, and the exact forms of
integer, float, string, character, boolean and unit literals. It describes
lsaca 2.2.0, whose integer literals default to `i32` and may carry a type
suffix, and whose apostrophe introduces either a character literal or a
lifetime/loop-label token.

## Source files and encoding

A source file has the extension `.ls` and is interpreted as UTF-8 text. There is
no shebang line, no encoding declaration and no preprocessor, so the first two
bytes are ordinary program text. Outside string and character literals and comments, only
ASCII characters are meaningful: every operator, delimiter and keyword is ASCII.
Non-ASCII characters may appear in comments and string literals freely, and
(somewhat unusually) in identifiers too.

Only whitespace separates tokens otherwise; indentation and line breaks are not
significant, and there are no automatic semicolon insertion rules. Statements
end with an explicit `;` (expressions used as tail values do not).

## Identifiers

An identifier starts with a letter or an underscore and continues with letters,
digits or underscores. lsaca does not restrict identifiers to ASCII: any Unicode
letter is accepted in a name, both as its first character and inside it.
Identifiers are case-sensitive and cannot start with a digit.

```ls
fn main() {
    let _unused = 1;
    let x2 = 2;
    let café = 3;
    let Übergröße = 4;
    println!("{} {} {} {}", _unused, x2, café, Übergröße);
}
```

Output:
```text
1 2 3 4
```

A leading underscore is an ordinary character, not a convention with special
meaning, and names do not need to be ASCII to be usable. What is not allowed is
a digit at the start of a name, because that would be ambiguous with a numeric
literal:

```ls,compile-fail
fn main() {
    let 2fast = 2;
    println!("{}", 2fast);
}
```

Error:
```text
error[E0002]: invalid suffix fast on numeric literal
  --> example-02.ls:2:10
   |
 2 |     let 2fast = 2;
   |          ^^^^

error[E0002]: invalid suffix fast on numeric literal
  --> example-02.ls:3:21
   |
 3 |     println!("{}", 2fast);
   |                     ^^^^

error: expected a pattern, found =
  --> example-02.ls:2:15
   |
 2 |     let 2fast = 2;
   |               ^

error: expected ';' after let statement, found integer literal 2
  --> example-02.ls:2:17
   |
 2 |     let 2fast = 2;
   |                 ^

lsaca: example-02.ls: could not compile example-02.ls due to previous error(s)
```

There is also a set of reserved words (next section) that cannot be used as
identifiers. Field names, method names and enum variant names follow the same
identifier rules.

## Keywords

The following words are reserved and cannot be used as identifiers:

```text
fn     let    mut    const   struct  enum    impl    type    trait
if     else   match  while   loop    for     in      return
break  continue true   false   as      self    Self    pub     move
mod    use    crate  super   dyn     extern
```

Two of them are context-sensitive in a useful way: `self` and `Self` are
keywords only where the grammar expects a parameter or a path segment, so they
can still be used as, say, struct field names — although code is clearer if they
are not. `crate` and `super` are keywords only in paths (expressions, types,
patterns and `use` trees). `mod` and `use` start item declarations since 1.7.0,
and `pub` is a visibility modifier (see [Program structure](02-program-structure.md)
and [Modules](23-modules.md)). Two keywords were added in 2.0.0: `dyn` introduces
a trait object type (`dyn Greet`), and `extern` introduces an extern function
declaration (`extern fn …;`). `str` is a primitive type name, not a keyword; it
is only meaningful as a type, and `[T]` is a type form built from square
brackets (see [Slices and str](26-slices-and-str.md)).

## Comments

Two comment forms exist:

* `//` starts a line comment that runs to the end of the line.
* `/* ... */` starts a block comment. Block comments **nest**, so `/* a /* b */
  c */` is one comment that ends at the final `*/`. An unterminated block
  comment is reported as an error at the end of the file.

There are no documentation comments: `///` is a line comment like any other, and
`/** ... */` is an ordinary block comment. Comments are whitespace and can
appear between any two tokens, including inside an expression.

```ls
/* This is a block comment.
   /* Comments nest, so the next marker does not end it: */
   The comment ends only at the final marker.
*/
fn main() {
    let x = 1 /* between tokens */ + 2;
    // A line comment can contain anything: /* and */ are just text.
    println!("x = {}", x); // trailing comment
}
```

Output:
```text
x = 3
```

## Integer literals

Integer literals can be written in four bases, with single underscores allowed
between digits for readability:

| Form | Example | Value |
| --- | --- | --- |
| Decimal | `42`, `1_000_000` | 42, 1000000 |
| Hexadecimal | `0xFF`, `0xdead_beef` | 255, 3735928559 |
| Octal | `0o77` | 63 |
| Binary | `0b1010_1010` | 170 |

Rules and restrictions:

* Digits are ASCII; the prefix is lowercase (`0x`, `0o`, `0b`) and hexadecimal
  digits may be either case (`0xFF` and `0xff` are the same literal).
* Underscores may appear between digits, but `_` cannot be the first or last
  character of the literal's digit sequence. Underscores do not change the value.
* A literal may end with a type suffix: `i8`, `i16`, `i32`, `i64`, `isize`,
  `u8`, `u16`, `u32`, `u64`, `usize` for integers, `f32`/`f64` for floats —
  for example `42i64`, `1u8`, `0xFFu32`, `1_000_000usize` and `1.5f32`. The
  suffix fixes the literal's type exactly. It is written immediately after the
  last digit, with no space in between, and must be one of those names or
  absent: `1.5u8`, `1i32f` and the unsupported `i128`/`u128` names are
  rejected while lexing, and the span points at the suffix itself. A bad
  suffix on an integer literal (including a non-decimal one such as `0xFFu8x`)
  is reported as `error[E0002]: invalid suffix … on numeric literal`; the same
  mistake on a float literal is `E0003`. A whole-number literal written with a
  float suffix — `1f64`, `42f32` — is a float literal, not an integer one.
* An unannotated integer literal has the type `i32`, unless its context forces
  another integer type (for example assigning it to a `u8` variable, passing it
  to a function that expects `u64`, using it as a length or index, or comparing
  it with a `usize` value).
* The value must fit the inferred type. A decimal literal that does not fit
  `i32` is a compile-time error unless a suffix or the context gives it a wider
  type; in every case the literal must fit the type it finally has. A
  well-formed but out-of-range literal is `E0080`.

```ls
fn main() {
    let decimal = 1_000_000;
    let hex = 0xFF;
    let octal = 0o77;
    let binary = 0b1010_1010;
    let negative = -42;
    println!("{} {} {} {} {}", decimal, hex, octal, binary, negative);

    let big: u64 = 18_446_744_073_709_551_615;
    let small: u8 = 255;
    let signed: i8 = -100;
    println!("{} {} {}", big, small, signed);
}
```

Output:
```text
1000000 255 63 170 -42
18446744073709551615 255 -100
```

A suffix is an alternative to an annotation, and both work in every base:

```ls
fn main() {
    let small = 1u8;
    let big = 42i64;
    let wide = 0xFFusize;
    let narrow: u16 = 65_535;
    println!("{} {} {}", small, big, wide);
    println!("{} {}", narrow, 10u32 + 32u32);
}
```

Output:
```text
1 42 255
65535 42
```

A literal out of range for its type is rejected before any C is generated:

```ls,compile-fail
fn main() {
    let too_big: i8 = 999;
    println!("{}", too_big);
}
```

Error:
```text
error[E0080]: literal out of range for i8: 999
  --> example-06.ls:2:23
   |
 2 |     let too_big: i8 = 999;
   |                       ^^^

lsaca: example-06.ls: could not compile example-06.ls due to previous error(s)
```

The range check understands a leading minus, so the most negative value of a
signed type can be written directly: `let x: i8 = -128;` and `-128i8` are
both accepted. The check applies to the signed value, not to the unsigned token
alone.

## Float literals

A float literal is a decimal number with a fractional part or an exponent.
Accepted forms include `1.5`, `2.0`, `1e10`, `1.5e-3` and `2.0E+4`:

* `.` must be followed by at least one digit: `1.` is not a float literal (`1`
  followed by a field-access dot), and `.5` is not a literal either; write `0.5`.
* The exponent marker is `e` or `E`, optionally followed by a sign and at least
  one digit.
* A float literal may carry the `f32` or `f64` suffix (`1.5f32`, `2.0f64`,
  and a whole number written `2f64` is a float literal too). There are no
  hexadecimal floats and no `NaN`/`inf` literals. Infinities and NaN are
  produced at run time by ordinary arithmetic, for example `1.0 / 0.0`.
* An unannotated float literal has type `f64` unless context requires `f32`; a
  suffix fixes the type even when context would infer otherwise.

```ls
fn main() {
    let a = 1.5;
    let b = 2.0;
    let c = 1e10;
    let d = 1.5e-3;
    let e = 2.0E+4;
    println!("{} {} {} {} {}", a, b, c, d, e);
    println!("{}", 1.0 / 0.0);
    println!("{}", -1.0 / 0.0);
    println!("{}", 0.0 / 0.0);
}
```

Output:
```text
1.5 2 10000000000 0.0015 20000
inf
-inf
NaN
```

The display of floats is the shortest representation that round-trips, without a
trailing `.0`: `2.0` prints as `2`, `1e10` prints as `10000000000`, and very
small values use decimal notation down to a point and then fall back to
scientific form as C's `printf` does. Section [Operators](07-operators.md)
covers float arithmetic in detail.

## String literals

String literals are double-quoted and may contain any UTF-8 text except an
unescaped double quote or backslash. By default they denote values of the
built-in type `String`: owned, immutable, reference-counted strings with cheap
copies (see [Memory model](05-memory-model.md)). Where a `&str` is expected —
a function or method argument, a `let` annotation, a return position — a
literal is compiled as a static UTF-8 slice instead, with no allocation (see
[Slices and str](26-slices-and-str.md)).

The following escape sequences are recognized:

| Escape | Meaning |
| --- | --- |
| `\n` | Newline (LF). |
| `\t` | Horizontal tab. |
| `\r` | Carriage return. |
| `\\` | Backslash. |
| `\"` | Double quote. |
| `\'` | Single quote. |
| `\0` | NUL character. |
| `\xNN` | A byte with the given two hexadecimal digits; only ASCII values (`\x00` to `\x7F`) are allowed. |
| `\u{HEX}` | A Unicode scalar value written in hexadecimal, one to six digits: `\u{4F60}` is 你. Surrogates and values above `0x10FFFF` are rejected. |

An unknown escape sequence is an error, and the diagnostic names the offending
sequence. Raw strings (`r"..."`) do not exist, so a backslash always starts an
escape.

```ls
fn main() {
    let newline = "a\nb";
    let tab = "a\tb";
    let quote = "he said \"hi\"";
    let backslash = "C:\\tmp";
    let nul_len = "a\0b".len();
    let ascii = "\x41\x42\x43";
    let unicode = "\u{4F60}\u{597D}";
    let literal = "héllo 世界";
    println!("{}", newline);
    println!("{}", tab);
    println!("{}", quote);
    println!("{}", backslash);
    println!("{}", nul_len);
    println!("{}", ascii);
    println!("{}", unicode);
    println!("{}", literal);
}
```

Output:

```text
a
b
a	b
he said "hi"
C:\tmp
3
ABC
你好
héllo 世界
```

The `\0` escape inserts a real NUL byte into the string; the string is still
`len()` bytes long and is not truncated (above, the escaped string has length 3).
Displaying or comparing such a string works on its bytes. The `\u{...}` escape is
the way to build non-ASCII text without writing it literally. Since 1.6.0 every
`String` is guaranteed valid UTF-8 and the library adds character-oriented
methods such as `char_count`, `chars` and `push(char)`; the byte-oriented
methods keep their byte semantics. See
[Characters and Unicode](22-characters-and-unicode.md).

```ls,compile-fail
fn main() {
    let bad = "a\qb";
    println!("{}", bad);
}
```

Error:
```text
error[E0005]: unknown escape sequence \q
  --> example-09.ls:2:17
   |
 2 |     let bad = "a\qb";
   |                 ^^

lsaca: example-09.ls: could not compile example-09.ls due to previous error(s)
```

## Boolean and unit literals

There are exactly two boolean literals, `true` and `false`, and they have type
`bool`. The only value of the unit type is written `()`; it is both a literal
expression and a type name, and it is what a function with no return type
produces.

```ls
fn main() {
    let yes: bool = true;
    let no = false;
    println!("{} {}", yes, no);
    println!("{} {}", true == true, true == false);
    let nothing: () = ();
    println!("{:?}", nothing);
}
```

Output:
```text
true false
true false
()
```

## Character literals

A character literal is written in single quotes and contains exactly one
Unicode scalar value: `'a'`, `'é'`, `'你'`, `'😀'`. It has the primitive type
`char`, a `Copy` scalar that is not a string; the type and its operations are
covered in [Characters and Unicode](22-characters-and-unicode.md).

Character literals accept the same escape sequences as string literals, with
the quoting rules adjusted: a single quote must be escaped as `\'`, while a
double quote may appear directly, so `'"'` is the double-quote character. The
`\xNN` escape is limited to ASCII bytes, and `\u{...}` accepts one to six
hexadecimal digits for any Unicode scalar value; surrogate code points and
values above `0x10FFFF` are rejected.

```ls
fn main() {
    let plain = 'a';
    let accent = 'é';
    let quote = '\'';
    let double = '"';
    let newline = '\n';
    let nul = '\0';
    let unicode = '\u{4F60}';
    println!("{} {} {} {}", plain, accent, quote, double);
    println!("{:?} {:?} {:?} {:?}", newline, nul, unicode, '😀');
}
```

Output:

```text
a é ' "
'\n' '\u{0}' '你' '😀'
```

The literal must not be empty and must not contain more than one scalar value:

```ls,compile-fail
fn main() {
    let empty = '';
    println!("{:?}", empty);
}
```

Error:

```text
error[E0007]: empty character literal
  --> example-12.ls:2:17
   |
 2 |     let empty = '';
   |                 ^^

lsaca: example-12.ls: could not compile example-12.ls due to previous error(s)
```

```ls,compile-fail
fn main() {
    let pair = 'ab';
    println!("{:?}", pair);
}
```

Error:

```text
error[E0007]: character literal may only contain one codepoint
  --> example-13.ls:2:16
   |
 2 |     let pair = 'ab';
   |                ^^^^

lsaca: example-13.ls: could not compile example-13.ls due to previous error(s)
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
  --> example-14.ls:2:16
   |
 2 |     let bad = '\q';
   |                ^^

lsaca: example-14.ls: could not compile example-14.ls due to previous error(s)
```

There are still no byte-string literals (`b"..."`) and no raw strings.

Character literals are also valid patterns: the match arms in
[Pattern matching](13-pattern-matching.md) can use them directly.

### Lifetimes and loop labels

The apostrophe does double duty. A `'` followed by an identifier run is a
lifetime token — `'a`, `'outer`, `'static` — except when the identifier run is
immediately followed by the closing apostrophe or by an escape, in which case it
is a character literal (`'a'`, and the malformed multi-codepoint forms such as
`'ab'` that the character-literal lexer reports). So `'a'` is a character and
`'a` is a lifetime. The stored lifetime name does not include the apostrophe.

Lifetime tokens appear in three places:

* generic parameter and argument lists: `struct View<'a> { … }`, `impl<'a>
  View<'a>`, `View<'a>`;
* reference types: `&'a T` and `&'a mut T`;
* loop labels: a lifetime followed by a colon at the start of a loop
  (`'outer: loop { … }`) and directly after `break` or `continue`
  (`break 'outer;`). A lifetime token in expression position that does not
  start a loop is a syntax error.

`'static` is a normal lifetime token with a fixed meaning: a `&'static T`
reference borrows for the whole program (string literals have this type behind
`&str`). See [Lifetimes](29-lifetimes.md), [Loop labels](28-loop-labels.md) and
[Arrays](27-arrays.md).

## Whitespace and semicolons

Whitespace (spaces, tabs, newlines, carriage returns and form feeds) separates
tokens and is otherwise ignored. Newlines are not terminators: a statement can
span several lines, and several statements can share a line. The only
line-oriented construct is the `//` comment.

```ls
fn main() {
    let a = 1; let b = 2;
    let c =
        a
        +
        b
    ;
    println!("{}", c);
}
```

Output:
```text
3
```

## What next

* The types these literals inhabit: [Types](04-types.md).
* Operators and their precedence: [Operators](07-operators.md).
* All built-in string methods: [Standard library](15-standard-library.md).
* Reference fields and lifetimes: [Lifetimes](29-lifetimes.md).
* Labelled loops: [Loop labels](28-loop-labels.md).
* Fixed-length arrays: [Arrays](27-arrays.md).
