# Roadmap and deviations from Rust

lsaca borrows Rust's surface syntax and some of its semantics, but it is a
smaller language with an explicitly different memory model. This page lists the
intentional differences, the known gaps in the current release, and what is
planned next. It describes lsaca **1.4.0**.

## Intentional deviations from Rust

These are design decisions, not bugs. Code written for Rust will need edits in
these areas.

1. **Ownership with automatic `Copy` and `Clone`.** Version 1.4 adds ownership
   and move semantics close to Rust's, but the marker traits are derived
   automatically instead of implemented by hand. A type is `Copy` when all of
   its parts are: integers, floats, `bool`, `()`, tuples, `Option<T>` and
   `Result<T, E>` of `Copy` types, references, and user structs/enums whose
   fields are all `Copy` and which do not implement `Drop`. Everything else
   (`String`, `Vec<T>`, `Rc<T>`, `Box<T>`, function values, and user types
   containing them) is move-only. `Clone` is automatic too: every type has
   `.clone()`, and `impl Copy`/`impl Clone` blocks are rejected. `Drop` is the
   only trait a program can implement. There are still no lifetime annotations;
   ORC remains the memory manager — heap allocations are reference-counted and
   freed when the last owner is released, so there is no `free`, no
   `drop(value)` builtin and no `std::mem::drop`. See
   [Ownership and moves](11-ownership-and-moves.md) and
   [Memory model](05-memory-model.md).
2. **References are non-owning and restricted.** Version 1.3 added `&T` and
   `&mut T`, with borrow expressions (`&place`, `&mut place`), dereference
   (`*r`) and match ergonomics — but there are no raw pointers, no address
   arithmetic and no way to observe an address. References cannot be stored in
   structs, enums, tuples, `Vec`, `Option`, `Result` or `Box`, cannot be captured
   by closures, cannot be compared with `==` and cannot be cast with `as`.
   Mutable references work for scalars, tuples, structs and enums, and — new in
   1.4 — for the owned heap types `&mut String`, `&mut Vec<T>`, `&mut Rc<T>` and
   `&mut Box<T>`; `&mut` to a user struct or enum that contains a heap field is
   still rejected (`E0498`). There is no lifetime syntax: elision follows fixed
   rules. See [References and borrowing](12-references-and-borrowing.md).
3. **No modules, crates or imports.** A program is one `.ls` file; all items are
   in one namespace. See [Program structure](02-program-structure.md).
4. **No user-defined generics or traits.** The only generic types are the builtin
   `Vec<T>`, `Option<T>`, `Result<T, E>`, `Rc<T>` and `Box<T>`; user structs and
   enums are monomorphic. There is no trait system, no operator overloading and
   no custom `Display`. `Drop` is the only implementable trait, and it cannot
   change an operator.
5. **No user macros.** The only macros are the builtin formatting/assertion
   macros and `vec!`. There is no `macro_rules!`, no derive and no attributes.
6. **Closures capture by value.** A closure owns its captured environment: it
   copies `Copy` values and moves move-only values into the closure, so the
   original binding becomes unusable unless its value is `Copy`. `move` is
   accepted but changes nothing, and capturing a reference is rejected
   (`E0498`). Shared mutable state inside a closure is done with a captured
   `Rc<T>`. See [Closures](14-closures.md).
7. **No `usize`, no `isize`.** Lengths, indices and counts are `i64`. Cast
   explicitly when a builtin needs another integer type.
8. **No `char` type.** Character literals (`'a'`) are a parse error. String
   methods work on bytes or on characters depending on the method; see
   [Standard library](15-standard-library.md#string).
9. **No arrays and no slices.** `Vec<T>` is the growable sequence type, and
   tuples are the fixed-arity product type (flat tuples only in 1.4.0).
10. **Restricted patterns.** No `@` bindings, no `ref` patterns, no slice and no
    range patterns. String-literal patterns are supported, or-patterns (`p | q`)
    and match guards are fully supported, and a reference scrutinee is matched
    with match ergonomics rather than a `&` pattern. See [Pattern
    matching](13-pattern-matching.md).
11. **No values out of `break`/`continue`.** Loops have type `()`; `break
    value` is a compile error. Use a mutable accumulator variable instead.
12. **No numeric literal suffixes.** There is no `1i32`, `1u8` or `2.0f32`;
    the literal type comes from context or defaults to `i64`/`f64`, and `as`
    converts. NaN and infinity have no literals.
13. **Minimal standard library.** There are no iterators, no collections other
    than `Vec`, no formatting of integers in other bases, and no named format
    arguments. The `?` operator is supported for `Option` and `Result`; the
    combinators are limited to `Option::map`, `Option::map_or`,
    `Option::and_then`, `Option::filter`, `Option::ok_or` and
    `Result::map`, `Result::map_err`, `Result::and_then`. There is no
    `Result::map_or` (matching Rust). What exists is documented in
    [Standard library](15-standard-library.md).
14. **`Rc<T>` equality is identity.** Two `Rc` handles compare equal only when
    they point at the same cell; `Box<T>`, by contrast, compares structurally.
    User structs and enums have no `==` at all in 1.4.0, and references have no
    `==` either — compare `*a == *b` instead.
15. **No unsafety.** There is no `unsafe`, no FFI and no threads. The generated C
    is not exposed to the language.

## Known limitations in 1.4.0

The following are implementation gaps or deliberate restrictions in the current
release, verified against the shipped compiler. They are expected to be fixed
in later releases; avoid them in code you intend to keep.

| Area | Limitation | Workaround |
| --- | --- | --- |
| Tuples | Nested tuple values and nested tuple patterns produce invalid C | Keep tuples flat |
| Struct fields | Fields whose type is `Option<scalar>` or a tuple produce invalid C | Store the payload in `Option<Box<…>>` or wrap in a struct |
| `Box<Option<i64>>` | Generates invalid C (same tuple/Option-scalar representation gap) | Box the whole `Option<String>` or struct instead |
| Function values | Function values and closures in struct fields or `Vec` produce invalid C | Keep them in locals, parameters and return values |
| `assert_eq!` | Rejects some types that support `==`, e.g. `Box` | Use `assert!(a == b)` |
| Casts | `int as bool` is accepted (`0` false, nonzero true) although Rust rejects it | Avoid it; compare explicitly |
| Loop variables | `for mut i in …` parses and lets you reassign `i` | Keep loop variables immutable |
| Literals | The most negative value of a type cannot be written as a negative literal (`E0080` applies to the token before the sign) | Write `-127 - 1`, `-32_767 - 1` and so on |
| Constants | Constant types are restricted to scalars and `String`; constant expressions allow arithmetic but not comparisons or function calls | Use a `let` or compute at runtime |
| Parameters | `mut` on a parameter parses but has no effect; parameters are immutable | Copy the parameter into a local `let mut` |
| `Self` | `Self` works in type position but `Self::new(…)` and `Self { … }` are rejected | Write the concrete type name |
| Fields | `pub` on a struct field is a parse error; `pub` on top-level items is accepted and ignored | Omit `pub` everywhere except top-level items |
| Equality | No `==` for `Vec`, `Option`, `Result`, tuples, structs, enums, `()` or references | Match/compare fields element by element; compare through `*a == *b` for references |
| String indexing | `text[i]` is rejected (`E0608`) | Use `substring` / `find` / `split` |
| String API | No `push(char)`; character literals do not exist (`.clone()` is available since 1.4) | Use `push_str` |
| Formatting | Width, fill and alignment apply to a user struct/enum as a whole, but the `+`/`0` flags and precision are rejected for them (`E0001`); no named or indexed arguments | Format fields individually, or apply width/fill/alignment to the whole value |
| Numbers | `pow` is float-only; no integer exponentiation builtin | Convert to `f64` or write a loop |
| Patterns | No `@`/`ref`/slice/range patterns, and a reference pattern like `let &x = r` is rejected | Match a reference scrutinee directly: the ergonomic bindings are references, so read and write through `*binding` |
| `?` syntax | `?` directly after a block expression (`{ … }?`) is a parse error | Bind the block to a variable or call a helper first |
| Reference storage | References cannot be fields, tuple elements, `Vec` elements, `Option`/`Result` payloads, `Box` payloads or closure captures (`E0498`) | Keep references in locals, parameters and return values |
| Mutable references | `&mut` to a user struct/enum containing a heap field is rejected (`E0498`); mutable references to scalars, `Copy` aggregates, `String`, `Vec<T>`, `Rc<T>` and `Box<T>` work | Take `&T` or restructure the code |
| Method receivers | A by-value `self` method cannot return a reference (`E0106`), and a `&self` method can only return shared references (`E0596`) | Return shared references from `&self` methods, mutable references from `&mut self` methods, and owned values from by-value methods |
| Return elision | No lifetime syntax; a free function returning a reference needs exactly one reference parameter, otherwise `E0106` | Pass exactly one reference parameter or return an owned value |
| Moves | No partial moves: reading a `Copy` field is fine, but moving a non-`Copy` field, element or dereferenced value out of a bigger value is rejected (`E0507`) | `.clone()` the field, or restructure the value |
| Move checking | Moving a value while a live borrow exists is rejected (`E0505`); use the value after a move is rejected (`E0382`) | Order borrows and moves explicitly, or `.clone()` before the move |
| Drop in patterns | `if let Some(value) = maybe` is rejected (`E0509`) when the payload type has drop glue | Match a reference instead: `if let Some(value) = &maybe` |
| Cloning | A type that implements `Drop` cannot be cloned (`E0277`) | Use references, or restructure so the value is not cloned |
| Custom traits | `impl Copy` and `impl Clone` are rejected because both are derived automatically | Rely on the automatic derivation |
| Explicit drop | There is no `drop(value)` function (`E0425`) and no `.drop()` method (`E0599`); a value is dropped when its last owner is released | End the scope early, or move the value into a helper that consumes it |
| Drop at exit | `Drop` hooks do not run when the process exits via `exit()` or a panic; they do run when `main` returns normally | Do not rely on hooks for cleanup at abnormal exits |
| Borrow granularity | `Vec` borrows are whole-container: two `&mut` borrows of different elements conflict (`E0499`) | Mutate elements one at a time or rebuild the vector |
| Borrow checker in one expression | Two mutable borrows written in a single expression are rejected (`E0499`) even when they run in sequence | Bind each call to a local, then combine the locals |
| `for` over references | `for x in &vec` is rejected (`E0277`); loops iterate over a `Vec<T>` value or an integer range | Iterate by value, or use an index loop over a `&Vec` |
| Reference equality/casts | References have no `==` and no `as` conversion | Compare or convert the pointed-to values with `*r` |

## Not in version 1.4.0

The roadmap items that are still open:

* references stored inside aggregates or captured by closures;
* lifetime annotations and generic lifetimes;
* arrays and slices;
* user-defined generics and traits;
* iterators and iterator adapters, `HashMap` and other collections;
* modules, imports and multi-file programs;
* user-defined macros and attributes;
* threads;
* a package manager;
* incremental compilation;
* a language server (LSP).

A plausible order for closing these gaps:

* **v1.5** — user-defined generics and traits, iterators and `HashMap`.
* **v1.6** — modules, a package manager and a language server.

Items already delivered, intentionally no longer on the list:

* an owned `Box<T>` type — **delivered in 1.1.0**;
* string formatting widths and precision — **delivered in 1.1.0**;
* the `?` error-propagation operator and the `Option`/`Result` combinators —
  **delivered in 1.2.0**;
* non-owning references, borrow checking and match ergonomics — **delivered in
  1.3.0**;
* ownership and move semantics, automatic `Copy`/`Clone` and the `Drop` trait —
  **delivered in 1.4.0**;
* mutable references to owned heap types (`&mut String`, `&mut Vec<T>`,
  `&mut Rc<T>`, `&mut Box<T>`) — **delivered in 1.4.0**.

## Language and runtime in 1.4.0

Version 1.4.0 is a **language release**: it adds ownership and move semantics,
automatically derived `Copy` and `Clone`, the `Drop` trait and mutable
references to owned heap types. Every value is now either `Copy` (implicitly
copied on use) or move-only; assigning, passing, returning, matching and
capturing a move-only value transfers it, and the move checker reports the
former owner as unusable (`E0382`) or a move out of a place that cannot be
given up (`E0507`). `.clone()` is the general way to keep a usable original.
`Drop` hooks run when the last owner of a value is released, before the value's
fields are released. ORC remains the memory manager underneath: heap payloads
are reference-counted, `.clone()` of a `String` shares its buffer copy-on-write,
`.clone()` of `Rc<T>` shares the cell, and `.clone()` of `Vec<T>`/`Box<T>`
deep-copies. See [Memory model](05-memory-model.md) and [Ownership and
moves](11-ownership-and-moves.md).

The compiler and runtime organization from 1.3.0 is unchanged:

* **Direct C compiler backend by default.** `lsaca run` and `lsaca build` write
  the generated C into the build cache, invoke the C compiler for `main.c` and
  the runtime units, and link the executable themselves. CMake is only needed
  with the global `--cmake` flag; `emit-c` still writes a complete CMake project.
* **Split runtime sources.** Four translation units (`lsaca_rt.c`,
  `lsaca_rt_string.c`, `lsaca_rt_vec.c`, `lsaca_rt_fmt.c`) plus the public
  `lsaca_rt.h` and internal `lsaca_rt_internal.h`; the generated `CMakeLists.txt`
  lists all five C files.
* **Cached runtime objects.** Compiled runtime objects live per profile under
  `<cache>/_runtime/<hash>/` and are reused, so a new program only recompiles
  its own `main.c` and links.

Compile-time, runtime and memory measurements for the 1.4.0 toolchain are in
[Benchmarks](17-benchmarks.md), alongside the comparison with C, Rust, Python and
Node. Headline numbers from that run: a cold build of a small program takes
1.03-1.32 s (4.03 s and about 1.05 MB of generated C for `compile_large`), a
cached build takes 20-36 ms, and the workload times range from 12.7 ms
(`loop_sum`) to 677.3 ms (`binary_trees`), with the borrowed tree traversal at
35.8 ms. Almost every workload is within about 1 MiB of C's peak memory; the
by-value tree traversal and the string workloads are the exceptions.

## Compatibility

lsaca 1.4.0 is **not a drop-in replacement** for 1.3.0 source. It is a language
release whose one breaking change is ownership: values are no longer implicitly
copied when they are assigned, passed, returned, matched or captured. Code that
relied on that copying now reports a moved value (`E0382`) or a move out of a
place (`E0507`) and must add `.clone()` or restructure. In exchange, moves make
the release cheaper for heap values and make `Drop` predictable.

The rest of the language is unchanged or additive:

* references from 1.3.0 keep the same restrictions, elision rules and borrow
  checking, and cannot be stored in aggregates or captured by closures;
* mutable references to `String`, `Vec<T>`, `Rc<T>` and `Box<T>` are new, so
  code that took a shared reference because a mutable one was rejected can be
  simplified, but existing code still compiles;
* automatically derived `Copy` and `Clone` and the `Drop` trait are new
  capabilities; `impl Copy` and `impl Clone` blocks are rejected with a message
  that says so;
* `Drop` hooks add observable behavior: reviewing drop order is worthwhile when
  migrating code that allocates things it used to leave to the collector.

The `mut`-on-parameters and `int as bool` leniencies kept since 1.1.0 are still
present. Conditions are strict — `if`, `while`, the operands of `&&` and `||`,
match guards and `assert!` all require `bool`. Generated C projects from earlier
releases must be regenerated with the 1.4.0 compiler; the emitted C for programs
that use ownership and `Drop` has a different shape than in 1.3.0.

## Benchmarks

Numbers for the current compiler — compile time, runtime and peak memory on
eleven runtime workloads (plus a compile-only scaling workload), compared with C,
Rust, Python and Node — are in [Benchmarks](17-benchmarks.md).

## What next

* The language tour starts at [Getting started](01-getting-started.md).
* Ownership, moves and destruction: [Ownership and
  moves](11-ownership-and-moves.md).
* Non-owning views and the borrow checker: [References and
  borrowing](12-references-and-borrowing.md).
* Every builtin in detail: [Standard library](15-standard-library.md).
* The memory model that ties ownership and ORC together:
  [Memory model](05-memory-model.md).
