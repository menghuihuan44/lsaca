# Roadmap and deviations from Rust

lsaca borrows Rust's surface syntax and some of its semantics, but it is a
smaller language with an explicitly different memory model. This page lists the
intentional differences, the known gaps in the current release, and what is
planned next. It describes lsaca **1.1.0**.

## Intentional deviations from Rust

These are design decisions, not bugs. Code written for Rust will need edits in
these areas.

1. **ORC instead of ownership and borrowing.** There is no borrow checker, no
   move semantics and no lifetimes. Every value is freely copyable; assigning,
   passing, returning and capturing copy handles and bump reference counts.
   Mutable data structures are copied on write (`String`, `Vec`) or shared through
   `Rc<T>`. See [Memory model](05-memory-model.md).
2. **No references or pointers.** There is no `&T`/`&mut T` type, no raw
   pointers and no `Box`-like deref syntax. `&self` and `&mut self` exist only as
   method receiver syntax. Shared mutable state is expressed with `Rc<T>`.
3. **No modules, crates or imports.** A program is one `.ls` file; all items are
   in one namespace. See [Program structure](02-program-structure.md).
4. **No user-defined generics or traits.** The only generic types are the builtin
   `Vec<T>`, `Option<T>`, `Result<T, E>`, `Rc<T>` and `Box<T>`; user structs and
   enums are monomorphic. There is no trait system, no operator overloading and
   no custom `Display`.
5. **No user macros.** The only macros are the builtin formatting/assertion
   macros and `vec!`. There is no `macro_rules!`, no derive and no attributes.
6. **Closures capture by value.** A closure sees a snapshot of the variables it
   mentions; `move` is accepted but changes nothing. Shared mutable state inside
   a closure is done with a captured `Rc<T>`. See [Closures](12-closures.md).
7. **No `usize`, no `isize`.** Lengths, indices and counts are `i64`. Cast
   explicitly when a builtin needs another integer type.
8. **No `char` type.** Character literals (`'a'`) are a parse error. String
   methods work on bytes or on characters depending on the method; see
   [Standard library](13-standard-library.md#string).
9. **No arrays and no slices.** `Vec<T>` is the growable sequence type, and
   tuples are the fixed-arity product type (flat tuples only in 1.1.0).
10. **Restricted patterns.** No `@` bindings, no `ref`, no slice, no range and no
    string-literal patterns. Or-patterns (`p | q`) and match guards are fully
    supported; see [Pattern matching](11-pattern-matching.md).
11. **No values out of `break`/`continue`.** Loops have type `()`; `break value`
    is a compile error. Use a mutable accumulator variable instead.
12. **No numeric literal suffixes.** There is no `1i32`, `1u8` or `2.0f32`;
    the literal type comes from context or defaults to `i64`/`f64`, and `as`
    converts. NaN and infinity have no literals.
13. **Minimal standard library.** There are no iterators, no collections other
    than `Vec`, no formatting of integers in other bases, no named format
    arguments, and no `?` error propagation. What exists is documented in
    [Standard library](13-standard-library.md).
14. **`Rc<T>` equality is identity.** Two `Rc` handles compare equal only when
    they point at the same cell; `Box<T>`, by contrast, compares structurally.
    User structs and enums have no `==` at all in 1.1.0.
15. **No unsafety.** There is no `unsafe`, no FFI and no threads. The generated C
    is not exposed to the language.

## Known limitations in 1.1.0

The following are implementation gaps in the current release, verified against
the shipped compiler. They are expected to be fixed in later releases; avoid them
in code you intend to keep.

| Area | Limitation | Workaround |
| --- | --- | --- |
| Tuples | Nested tuple values and nested tuple patterns produce invalid C | Keep tuples flat |
| Struct fields | Fields whose type is `Option<scalar>` or a tuple produce invalid C | Store the payload in `Option<Box<…>>` or wrap in a struct |
| `Vec::with_capacity` | Generates invalid C, so it cannot be used | Use `Vec::new()` |
| `Box<Option<i64>>` | Generates invalid C (same tuple/Option-scalar representation gap) | Box the whole `Option<String>`/struct instead |
| Function values | Function values and closures in struct fields or `Vec` produce invalid C | Keep them in locals, parameters and return values |
| `assert_eq!` | Rejects some types that support `==`, e.g. `Box` | Use `assert!(a == b)` |
| Casts | `int as bool` is accepted (`0` false, nonzero true) although Rust rejects it | Avoid it; compare explicitly |
| Loop variables | `for mut i in …` parses and lets you reassign `i` | Keep loop variables immutable |
| Literals | The most negative value of a type cannot be written as a negative literal (`E0080` applies to the token before the sign) | Write `-127 - 1`, `-32_767 - 1` and so on |
| Constants | Constant types are restricted to scalars and `String`; constant expressions allow arithmetic but not comparisons or function calls | Use a `let` or compute at runtime |
| Parameters | `mut` on a parameter parses but has no effect; parameters are immutable | Copy the parameter into a local `let mut` |
| `Self` | `Self` works in type position but `Self::new(…)` and `Self { … }` are rejected | Write the concrete type name |
| Fields | `pub` on a struct field is a parse error; `pub` on top-level items is accepted and ignored | Omit `pub` everywhere except top-level items |
| Equality | No `==` for `Vec`, `Option`, `Result`, tuples, structs, enums or `()` | Match/compare fields element by element |
| String indexing | `text[i]` is rejected (`E0608`) | Use `substring` / `find` / `split` |
| Result API | No `ok_or`, `map`, `and_then`, `map_err` or `?` operator | Write `match` or `if let` |
| String API | No `String::clone` (`not needed: copies are handles) and no `push(char)` | Use `push_str` |
| Formatting | Width is ignored for user structs/enums; no named or indexed arguments | Format fields individually |
| Numbers | `pow` is float-only; no integer exponentiation builtin | Convert to `f64` or write a loop |
| Patterns | No `@`/`ref`/slice/range/string-literal patterns | Bind with a wildcard and compare inside the arm |

## Not in version 1.1.0

The roadmap items that are still open:

* modules, imports and multi-file programs;
* user-defined generics and traits;
* an owned `Box<T>` type — **delivered in 1.1.0**;
* references and borrow checking;
* arrays and slices;
* string formatting widths and precision — **delivered in 1.1.0**;
* user-defined macros and attributes;
* iterators and iterator adapters;
* threads;
* a package manager;
* incremental compilation;
* a language server (LSP).

The two delivered entries are kept struck through above for readers upgrading
from 1.0. The remainder of the list is unchanged from the 1.0 roadmap.

## Compatibility

lsaca 1.1.0 is source-compatible with 1.0.0: existing programs compile unchanged,
because the release only **adds** capabilities.

* `Box<T>` — a new builtin generic type with value semantics (deep-copied on
  assignment), `Box::new/get/set`, structural `==` and payload formatting.
* Format specifiers — `[[fill]align][sign][0][width][.precision][?]` in all
  formatting macros, with character-counted widths, float/string precision,
  zero padding and Debug formatting. Previously any width or precision was a
  compile error.
* Pattern matching — or-patterns and match guards are now implemented correctly:
  every or-pattern alternative is tested and a false guard continues with the
  remaining arms. Programs that avoided these forms are unaffected.

The generated C runtime `lsaca_rt.c`/`lsaca_rt.h` changed in 1.1.0. If you keep
emitted C projects around, regenerate them with the 1.1.0 compiler or recompile
them against the new runtime sources.

Programs that used the `as` cast from integers to `bool` or `mut` on parameters
still compile: those leniencies were kept for compatibility and are listed as
known limitations above. Conditions, however, are now strict: `if`, `while`, the
operands of `&&` and `||`, `match` guards and `assert!` all require `bool`, and
integer conditions that an earlier 1.1 build accepted are rejected with
`E0308` (Rust behaves the same way).

## Benchmarks

Numbers for the current compiler — compile time, runtime and peak memory on four
workloads, compared with C, Rust, Python and Node — are in
[Benchmarks](15-benchmarks.md).

## What next

* The language tour starts at [Getting started](01-getting-started.md).
* Every builtin in detail: [Standard library](13-standard-library.md).
* The memory model that explains the copy semantics:
  [Memory model](05-memory-model.md).
