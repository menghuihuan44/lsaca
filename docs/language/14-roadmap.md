# Roadmap and deviations from Rust

lsaca borrows Rust's surface syntax and some of its semantics, but it is a
smaller language with an explicitly different memory model. This page lists the
intentional differences, the known gaps in the current release, and what is
planned next. It describes lsaca **1.2.0**.

## Intentional deviations from Rust

These are design decisions, not bugs. Code written for Rust will need edits in
these areas.

1. **ORC instead of ownership and borrowing.** There is no borrow checker, no
   move semantics and no lifetimes. Every value is freely copyable; assigning,
   passing, returning and capturing copy handles and bump reference counts.
   Mutable data structures are copied on write (`String`, `Vec`) or shared through
   `Rc<T>`. See [Memory model](05-memory-model.md).
2. **No references or pointers.** There is no `&T`/`&mut T` type, no raw
   pointers and no deref syntax. `&self` and `&mut self` exist only as method
   receiver syntax. `Box<T>` is an *owned* heap indirection, not a pointer:
   copies deep-copy the payload, and there is no way to observe an address.
   Shared mutable state is expressed with `Rc<T>`.
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
   tuples are the fixed-arity product type (flat tuples only in 1.2.0).
10. **Restricted patterns.** No `@` bindings, no `ref`, no slice, no range and no
    string-literal patterns. Or-patterns (`p | q`) and match guards are fully
    supported; see [Pattern matching](11-pattern-matching.md).
11. **No values out of `break`/`continue`.** Loops have type `()`; `break value`
    is a compile error. Use a mutable accumulator variable instead.
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
    [Standard library](13-standard-library.md).
14. **`Rc<T>` equality is identity.** Two `Rc` handles compare equal only when
    they point at the same cell; `Box<T>`, by contrast, compares structurally.
    User structs and enums have no `==` at all in 1.2.0.
15. **No unsafety.** There is no `unsafe`, no FFI and no threads. The generated C
    is not exposed to the language.

## Known limitations in 1.2.0

The following are implementation gaps in the current release, verified against
the shipped compiler. They are expected to be fixed in later releases; avoid them
in code you intend to keep.

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
| Equality | No `==` for `Vec`, `Option`, `Result`, tuples, structs, enums or `()` | Match/compare fields element by element |
| String indexing | `text[i]` is rejected (`E0608`) | Use `substring` / `find` / `split` |
| String API | No `String::clone` (not needed: copies are handles) and no `push(char)` | Use `push_str` |
| Formatting | Width is ignored for user structs/enums; no named or indexed arguments | Format fields individually |
| Numbers | `pow` is float-only; no integer exponentiation builtin | Convert to `f64` or write a loop |
| Patterns | No `@`/`ref`/slice/range/string-literal patterns | Bind with a wildcard and compare inside the arm |
| `?` syntax | `?` directly after a block expression (`{ … }?`) is a parse error | Bind the block to a variable or call a helper first |

## Not in version 1.2.0

The roadmap items that are still open:

* modules, imports and multi-file programs;
* user-defined generics and traits;
* references and borrow checking;
* arrays and slices;
* user-defined macros and attributes;
* iterators and iterator adapters;
* threads;
* a package manager;
* incremental compilation;
* a language server (LSP).

Three items have been delivered on the way to 1.2.0 and are intentionally no
longer on the list:

* an owned `Box<T>` type — **delivered in 1.1.0**;
* string formatting widths and precision — **delivered in 1.1.0**;
* the `?` error-propagation operator and the `Option`/`Result` combinators —
  **delivered in 1.2.0**.

## Compiler and runtime in 1.2.0

Version 1.2.0 is a compiler release as much as a language release. The
execution semantics of existing programs are unchanged; what changed is how the
compiler drives the C toolchain and how the runtime is organised:

* **Direct C compiler backend by default.** `lsaca run` and `lsaca build` no
  longer configure a CMake project: the compiler writes the generated C into the
  build cache, invokes the C compiler for `main.c` and the runtime units, and
  links the executable itself. CMake is only needed when you pass the global
  `--cmake` flag; `emit-c` still writes a complete CMake project.
* **Split runtime sources.** The single runtime file became four translation
  units (`lsaca_rt.c`, `lsaca_rt_string.c`, `lsaca_rt_vec.c`,
  `lsaca_rt_fmt.c`) plus the public `lsaca_rt.h` and internal
  `lsaca_rt_internal.h`. The generated `CMakeLists.txt` lists all five C files.
* **Cached runtime objects.** Compiled runtime objects are stored per profile
  under `<cache>/_runtime/<hash>/` and reused by later programs, so a new
  program only recompiles its own `main.c`. A warm build is dominated by the
  program's own compilation and link.
* **Faster generated code and runtime.** Representative measurements on the
  project's development machine (approximate; see
  [Benchmarks](15-benchmarks.md) for the full comparison with C, Rust, Python
  and Node):

  | Workload / metric | 1.1.0 | 1.2.0 |
  | --- | --- | --- |
  | Cold `lsaca build -r examples/hello.ls` | ~2.9 s | ~1.2 s |
  | Cached rebuild | ~0.03 s | ~0.03 s |
  | Cold `lsaca run examples/hello.ls` | ~2.5 s | ~0.9 s |
  | `vec_memory` runtime | ~86 ms | ~27 ms |
  | `vec_memory` peak RSS | ~35.1 MiB | ~18.8 MiB |
  | `string_build` runtime | ~213 ms | ~35 ms |
  | Runtime microbench: `Vec::push` | ~70 ms | ~11 ms |
  | Runtime microbench: string append | ~115 ms | ~22 ms |
  | Runtime microbench: integer formatting | ~102 ms | ~17 ms |

  The `fib` and `loop_sum` benchmarks were already dominated by the generated
  arithmetic and are essentially unchanged.

## Compatibility

lsaca 1.2.0 is **source-compatible** with 1.1.0. Everything that compiled
before still compiles, and the only language additions are the `?` operator and
the `Option`/`Result` combinators:

* `?` — postfix propagation for `Option`/`Result`-returning functions, with
  evaluation-once semantics, exact error-type matching and a const
  restriction. See
  [Error propagation with `?`](13-standard-library.md#error-propagation-with-).
* `Option::map/map_or/and_then/filter/ok_or` and
  `Result::map/map_err/and_then` — type-checker desugarings (no C runtime
  support needed) with the usual semantics. Added methods can never change the
  meaning of an existing program because these names did not exist on
  `Option`/`Result` before.

There is one **build-level** change: `run` and `build` use the direct C
compiler backend by default, and CMake is optional via `--cmake`. The generated
C project now has four runtime `.c` files and two headers; emitted projects from
1.1.0 must be regenerated with the 1.2.0 compiler to pick up the new runtime.
The build cache also stores compiled runtime objects, so the first build after
upgrading recompiles them once.

Compatibility with 1.0.0 is unchanged, and the leniencies kept in 1.1.0 are
still present: `as` from integers to `bool` works and `mut` on parameters is
accepted without effect (both are listed as known limitations above).
Conditions are strict — `if`, `while`, the operands of `&&` and `||`, match
guards and `assert!` all require `bool`.

Programs that relied on the 1.1.0 CMake-only pipeline can restore it with
`--cmake`; the compiler and runtime behaviour are identical either way.

## Benchmarks

Numbers for the current compiler — compile time, runtime and peak memory on four
workloads, compared with C, Rust, Python and Node — are in
[Benchmarks](15-benchmarks.md).

## What next

* The language tour starts at [Getting started](01-getting-started.md).
* Every builtin in detail: [Standard library](13-standard-library.md).
* The memory model that explains the copy semantics:
  [Memory model](05-memory-model.md).
