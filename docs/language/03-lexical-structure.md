# Lexical structure

This page describes the raw text of a program: how source files are encoded, the
character set of identifiers, the keywords, comments, and the exact forms of
integer, float, string, boolean and unit literals.

## Source files and encoding

A source file has the extension `.ls` and is interpreted as UTF-8 text. There is
no shebang line, no encoding declaration and no preprocessor, so the first two
bytes are ordinary program text. Outside string literals and comments, only
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
  --> example-02.ls:2:9
   |
 2 |     let 2fast = 2;
   |         ^^^^^

error[E0002]: invalid suffix fast on numeric literal
  --> example-02.ls:3:20
   |
 3 |     println!("{}", 2fast);
   |                    ^^^^^

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
fn    let   mut   const  struct  enum   impl   type
if    else  match while  loop    for    in     return
break continue true false as     self   Self   pub    move
```

Two of them are context-sensitive in a useful way: `self` and `Self` are
keywords only where the grammar expects a parameter or a path segment, so they
can still be used as, say, struct field names — although code is clearer if they
are not. `pub` is accepted on items and ignored (see
[Program structure](02-program-structure.md)).

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
* There are **no suffixes**: `42i64`, `1u8` and `0xFFu32` are not literals, they
  are a literal followed by an identifier and fail to parse. To give a literal a
  type, use a `let` annotation or an `as` cast.
* An unannotated integer literal has the type `i64`, unless its context forces
  another integer type (for example passing it to a function that expects `u8`,
  or assigning it to a `u8` variable).
* The value must fit the inferred type. A decimal literal that does not fit
  `i64` is a compile-time error; for narrower types the literal must fit the
  target type after inference.

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
  --> example-05.ls:2:23
   |
 2 |     let too_big: i8 = 999;
   |                       ^^^

lsaca: example-05.ls: could not compile example-05.ls due to previous error(s)
```

The range check applies to the literal token itself, **before** any surrounding
negation is folded, so the most negative value of a signed type cannot be written
as a plain negative literal: `let x: i8 = -128;` is rejected with
`error[E0080]: literal out of range for i8: 128`. Compute it instead, for
example `-127 - 1`, or produce it by arithmetic. This is a known simplification
of the 1.2.0 constant checker.

## Float literals

A float literal is a decimal number with a fractional part or an exponent.
Accepted forms include `1.5`, `2.0`, `1e10`, `1.5e-3` and `2.0E+4`:

* `.` must be followed by at least one digit: `1.` is not a float literal (`1`
  followed by a field-access dot), and `.5` is not a literal either; write `0.5`.
* The exponent marker is `e` or `E`, optionally followed by a sign and at least
  one digit.
* There are no float suffixes, no hexadecimal floats, and no `NaN`/`inf`
  literals. Infinities and NaN are produced at run time by ordinary arithmetic,
  for example `1.0 / 0.0`.
* An unannotated float literal has type `f64` unless context requires `f32`.

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
unescaped double quote or backslash. They denote values of the built-in type
`String`: owned, immutable, reference-counted strings with cheap copies (see
[Memory model](05-memory-model.md)).

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
| `\u{HEX}` | A Unicode scalar value written in hexadecimal, one or more digits: `\u{4F60}` is 你. |

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
1
ABC
你好
héllo 世界
```

The `\0` escape inserts a real NUL byte into the string; the string is still
`len()` bytes long and is not truncated (above, the escaped string has length 3).
Displaying or comparing such a string works on its bytes. Because lsaca strings
are UTF-8 and methods use byte offsets, the `\u{...}` escape is the way to build
non-ASCII text without writing it literally.

```ls,compile-fail
fn main() {
    let bad = "a\qb";
    println!("{}", bad);
}
```

Error:
```text
error[E0005]: unknown escape sequence \q
  --> example-08.ls:2:17
   |
 2 |     let bad = "a\qb";
   |                 ^^

lsaca: example-08.ls: could not compile example-08.ls due to previous error(s)
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

## There is no character literal

lsaca has no `char` type, so single quotes are not a literal delimiter: `'a'` is
a lexical error, and text of one character is a `String` of length 1, built with
double quotes. Likewise there are no byte-string literals (`b"..."`) and no raw
strings.

```ls,compile-fail
fn main() {
    let c = 'a';
    println!("{}", c);
}
```

Error:
```text
error[E0001]: unexpected character '
  --> example-10.ls:2:13
   |
 2 |     let c = 'a';
   |             ^

error[E0001]: unexpected character '
  --> example-10.ls:2:15
   |
 2 |     let c = 'a';
   |               ^

lsaca: example-10.ls: could not compile example-10.ls due to previous error(s)
```

To work with "characters", use one-character strings and the string methods in
[Standard library](13-standard-library.md); remember that most of them use byte
offsets, not character indices.

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
* All built-in string methods: [Standard library](13-standard-library.md).
