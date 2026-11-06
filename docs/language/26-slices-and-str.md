# Slices and str

Version 2.0 added two unsized borrowed types: `[T]`, a view into a contiguous
sequence, and `str`, a view into UTF-8 text. Before 2.0 a `Vec` could only be
used as a whole, and `String` had no borrowed text form. Both types are
available as references only - `&[T]`, `&mut [T]`, `&str` and `&mut str` - and
both are accepted at function boundaries through implicit coercions from `Vec`
and `String`.

Version 2.1 made the borrowed types the normal parameter form for text: the
standard-library methods that only read text take `&str`, slice and text
lengths, indices and counts are `usize`, and a string literal used where `&str`
is expected is a statically allocated slice with no allocation. The examples on
this page use the 2.1 syntax and are executed by the documentation verifier
against the 2.1 compiler.

## Unsized types

`[T]` and `str` are unsized: they have no fixed size, so they cannot be stored
directly. The only values of these types are references:

| Type | Meaning | Stored as |
| --- | --- | --- |
| `&[T]` | borrowed view of a sequence of `T` | two words: data pointer and length |
| `&mut [T]` | mutable borrowed view | two words |
| `&str` | borrowed UTF-8 text | two words: byte pointer and byte length |
| `&mut str` | mutable borrowed text | two words |

All four views are `Copy`: copying one copies its two words, and nothing is cloned -
neither elements nor text bytes. None of these values participates in reference
counting - they are plain borrows of a `Vec` or `String` that owns the storage.

Using `[T]` or `str` anywhere except behind `&` or `&mut` is error `E0001`:

> slices and str are unsized; use `&[T]`, `&mut [T]`, `&str` or `&mut str`

As with all references in 2.1.0, slice and text values cannot be stored inside
structs, enums or `Vec` elements.

## Slice expressions

A slice expression selects a range of a `Vec` or a `String`. The base is the value
being sliced; the result must be behind `&` or `&mut`:

| Written as | Selects |
| --- | --- |
| `&values[start..end]` | elements `start` up to but not including `end` |
| `&values[start..=end]` | elements `start` through `end` inclusive |
| `&values[..end]` | elements from the beginning to `end` |
| `&values[start..]` | elements from `start` to the end |
| `&values[..]` | the whole sequence |

The rules are the same for `String`, where a slice expression produces `&str` and
the indices are byte offsets. Range bounds are `usize` values, like every other
length and index in 2.1; an unterminated literal bound is inferred as `usize` in
this position. A slice expression borrows its base for as long as the resulting
reference is used, so the `Vec` or `String` cannot be mutated or moved while the
slice is alive.

```ls
fn main() {
    let values: Vec<i64> = vec![10, 20, 30, 40, 50];
    let middle: &[i64] = &values[1..4];
    let head: &[i64] = &values[..2];
    let tail: &[i64] = &values[3..];
    let all: &[i64] = &values[..];
    println!("{} {} {} {}", middle.len(), head.len(), tail.len(), all.len());
}
```

Output:
```text
3 2 2 5
```

A `String` slice produces text, and its bounds must fall on character
boundaries (see below):

```ls
fn main() {
    let text: String = String::from("hello world");
    let hello: &str = &text[..5];
    let world: &str = &text[6..];
    let all: &str = &text[..];
    println!("{} {} {}", hello.len(), world.len(), all.len());
}
```

Output:
```text
5 5 11
```

The existing index expression `values[index]` keeps its behaviour: it reads one
element by value and panics when the index is out of bounds, and the index is a
`usize`, so an `i32`/`i64` value must be converted with `as`. A literal index is
inferred as `usize`. A slice expression is the borrowed form: it does not copy
elements, it refers to the storage of the base.

## Implicit coercions

A reference to `Vec` or `String` coerces to the corresponding borrowed view at
the same places as other coercions - let bindings, assignments, arguments, returns,
struct fields and `Vec::push` arguments:

| From | To |
| --- | --- |
| `&Vec<T>` | `&[T]` |
| `&mut Vec<T>` | `&mut [T]` |
| `&String` | `&str` |
| `&mut String` | `&mut str` |

```ls
fn total(values: &[i64]) -> i64 {
    let mut sum: i64 = 0;
    let mut index: usize = 0;
    while index < values.len() {
        sum += values[index];
        index += 1;
    }
    return sum;
}

fn main() {
    let values: Vec<i64> = vec![1, 2, 3, 4];
    println!("{}", total(&values));
    let more: &[i64] = &values;
    println!("{}", more.len());
}
```

Output:
```text
10
4
```

An owned `String` value does not coerce to `&str` on its own: the coercion starts
from a reference. Write `&text` to pass a `String` variable to a `&str`
parameter. A string literal, on the other hand, needs no conversion at all:
where `&str` is expected the literal becomes a statically allocated slice, with
no `String` allocation and no `String::from` call.

```ls
fn greet(name: &str) -> usize {
    name.len()
}

fn main() {
    let text: String = String::from("hello");
    let borrowed: &str = &text;
    let copy: String = String::from_str(borrowed);
    println!("{} {} {}", greet("hello"), greet(&text), text.len() + copy.len());
}
```

Output:
```text
5 5 10
```

Functions can take `&str` parameters: an argument of type `&String` coerces to
`&str` at the call, and `String::as_str()` produces a `&str` directly. Standard
library functions that return text return an owned `String` (`String::from`,
`String::from_str` and `replace`), while the methods that only read text take
`&str`.

## Methods on slices

The `[T]` and `str` implementations live in the standard library's `std` crate, like
the methods of `Vec` and `String`. User crates cannot write `impl [T]` or
`impl str` blocks: doing so is error `E0118` (`impl target must be a struct or
enum type defined in this program, found [i64]`). The slice methods are:

| Method | Signature | Notes |
| --- | --- | --- |
| `len` | `&self -> usize` | number of elements (bytes for `str`) |
| `is_empty` | `&self -> bool` | true when the length is 0 |
| `get` | `&self, usize -> Option<T>` | element by index; `None` when out of range; `T` must be `Copy` |
| `first` | `&self -> Option<T>` | first element; `T` must be `Copy` |
| `last` | `&self -> Option<T>` | last element; `T` must be `Copy` |

The 2.1.0 compiler implements `len` and `is_empty` on `[T]`;
`get`, `first` and `last` are fixed by the design but are not implemented in
2.1.0 and are planned for a later release.

`&mut [T]` is accepted as a type and coerces from `&mut Vec<T>`, and an element
can be written through indexing (`values[0] = 9`). Mutating slice *methods* and
`for` loops over a slice are not implemented in 2.1.0 and are planned for a later
release; write through the `Vec` itself for those.

## Methods on str

A `str` is always valid UTF-8, exactly like `String`. The byte length and the
character count therefore differ for non-ASCII text, and byte indices may point
inside a multi-byte character. The `str` methods are:

| Method | Returns | Notes |
| --- | --- | --- |
| `len` | `usize` | length in bytes, not characters |
| `is_empty` | `bool` | true when the byte length is 0 |
| `char_count` | `usize` | number of Unicode scalar values |
| `chars` | `Vec<char>` | every scalar value, in order |
| `as_bytes` | `&[u8]` | the UTF-8 bytes as a byte slice |
| `contains` | `bool` | substring test; the needle is `&str` |
| `starts_with` | `bool` | prefix test; the prefix is `&str` |
| `ends_with` | `bool` | suffix test; the suffix is `&str` |
| `find` | `Option<usize>` | byte index of the first occurrence; the needle is `&str` |
| `trim` | `&str` | strips leading and trailing Unicode whitespace |
| `split` | `Vec<String>` | splits on every occurrence; the separator is `&str` |
| `replace` | `String` | replaces every occurrence; both arguments are `&str` |
| `parse_i64` | `Result<i64, ParseIntError>` | parses the whole text as a decimal integer |
| `parse_f64` | `Result<f64, ParseFloatError>` | parses the whole text as a floating-point number |
| `parse` | `Result<T, E>` | generic parse; in 2.1.0 `T` must be `i64` (`E = ParseIntError`) or `f64` (`E = ParseFloatError`) |
| `to_uppercase` | `String` | Unicode default uppercasing |
| `to_lowercase` | `String` | Unicode default lowercasing |

The 2.1.0 compiler implements every method in this table, `chars` and `replace`
included. The read-only text arguments (`contains`, `starts_with`, `ends_with`,
`find`, `split` and `replace`) take `&str`, so a literal is passed directly and a
`String` variable as `&owned`.

```ls
fn main() {
    let text: String = String::from("  Héllo wörld  ");
    let trimmed: &str = text.as_str().trim();
    println!("{} {}", trimmed.len(), trimmed.char_count());
    println!("{}", trimmed.starts_with("Hé"));
    println!("{:?}", trimmed.find("wörld"));
    println!("{}", trimmed.to_uppercase());
}
```

Output:
```text
13 11
true
Some(7)
HÉLLO WÖRLD
```

The borrowed `trim` is the `str` method, so it is reached through `as_str()`.
`String::trim`, the legacy method, still returns an owned `String` (see
[Characters and Unicode](22-characters-and-unicode.md)); use `text.as_str().trim()`
when a borrowed view is wanted.

## Bounds and UTF-8 panics

Slice expressions are checked at run time, and a failed check panics with Rust's
wording. The process exits with code 101 and prints the location of the slice
expression. These are the messages the compiler emits:

```text
range end index 9 out of range for slice of length 3
slice index starts at 9 but ends at 3
byte index 99 is out of bounds of `hello`
byte index 2 is not a char boundary; it is inside 'é' (bytes 1..3) of `héllo`
```

* when a range end is past the length, the message is `range end index {end} out of
  range for slice of length {len}`;
* when a range start is past the end, the message is `slice index starts at {start}
  but ends at {len}` (there is no separate `range start index` message);
* when a `start..end` range has `start > end`, the message is `slice index starts at
  {start} but ends at {end}`;
* a text bound past the end reports `byte index {N} is out of bounds of` followed by
  the text in backticks;
* a text bound inside a multi-byte character reports `byte index {N} is not a char
  boundary; it is inside '{char}' (bytes {A}..{B}) of` followed by the text in
  backticks.

The same rules are used by `String::substring` (see
[Characters and Unicode](22-characters-and-unicode.md#substring-and-character-boundaries)),
and by the `str` methods that take text arguments. There is no slice-expression
form that returns `None` for out-of-bounds bounds; check the length first when the
range is not known to be valid.

## How slices and str are represented

An `&[T]` lowers to a by-value fat struct with a data pointer and a length:

```text
struct LsSlice_i64 { int64_t* data; size_t len; }
```

`&str` uses the same two words under a distinct `LsStr` C type, so it cannot be
confused with `&[u8]`. Both are passed by value and are never reference counted;
they borrow storage owned by a `Vec` or `String`. Slice expressions and the
implicit coercions lower to the same internal slice node with a base place and a
range, so `&values[1..3]` and a coercion of `&values` to `&[i64]` both compile to
the same representation.

All slice and text lengths and indices are `usize`: `len` returns `usize` and the
length word of the fat struct is the C `size_t`. The generated C project needs no
new runtime functions for slices and `str`; the small helpers are emitted inline
by the compiler.

## Known limitations in 2.1.0

* No arrays `[T; N]`: fixed-size arrays are not part of 2.1. Use a `Vec` and
  slice it.
* Slices and text views cannot be stored in structs, enums or `Vec` elements, and
  cannot be returned from functions as owned values - only borrowed for the duration
  of the owning `Vec` or `String`.
* No mutating slice methods or slice iterators yet: `&mut [T]` coerces, can be
  passed, and supports element assignment through indexing (`values[0] = 9`), but
  mutating methods and `for` loops over a slice are not implemented in 2.1.0 and
  are planned for a later release.
* No slice patterns and no character ranges in patterns; see
  [Pattern matching](13-pattern-matching.md).
* A `&str` is passed as `&text` (or `text.as_str()`) when it comes from an owned
  `String`; only literals convert implicitly.
* `str` comparisons with `==` are not available (`E0369`); compare via
  `String::from_str` for now.

## Summary

* `[T]` and `str` are unsized; the only values are `&[T]`, `&mut [T]`, `&str` and
  `&mut str`; using them elsewhere is `E0001`.
* Slice expressions `base[start..end]`, `[..end]`, `[start..]` and `[..]` work on
  `Vec` and `String` and must be behind `&` or `&mut`.
* `&Vec<T>` coerces to `&[T]` and `&String` to `&str` at let bindings,
  assignments, arguments, returns, fields and `Vec::push` arguments; a string
  literal in a `&str` position is a statically allocated slice.
* Lengths, indices and counts are `usize`; `len` counts bytes for `str`,
  `char_count` counts scalar values, and slicing text panics unless both bounds
  are character boundaries.
* Slice and text values are fat pointers passed by value; they are not
  reference counted and may not be stored in aggregates.
* The `[T]` and `str` method sets live in the standard library's `std` crate; user
  `impl` blocks for them are `E0118`.

## What next

* [Trait objects](25-trait-objects.md) - run-time dispatch for heterogeneous values.
* [Characters and Unicode](22-characters-and-unicode.md) - why byte and character
  indices differ.
* [Memory model](05-memory-model.md) - ownership and ORC behind `Vec` and `String`.
* [Standard library](15-standard-library.md) - the `std` crate and its module map.
