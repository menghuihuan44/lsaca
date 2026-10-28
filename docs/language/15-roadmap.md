# Roadmap and deviations from Rust

lsaca borrows Rust's surface syntax and some of its semantics, but it is a
smaller language with an explicitly different memory model. This page lists the
intentional differences, the known gaps in the current release, and what is
planned next. It describes lsaca **1.3.0**.

## Intentional deviations from Rust

These are design decisions, not bugs. Code written for Rust will need edits in
these areas.

1. **ORC instead of ownership.** There is no move semantics, no lifetimes and no
   ownership annotations. Every value is freely copyable; assigning, passing,
   returning and capturing copy handles and bump reference counts. Mutable data
   structures are copied on write (`String`, `Vec`) or shared through `Rc<T>`.
   Version 1.3 adds a *lightweight borrow checker*, but it only checks
   **aliasing** for references: it never decides when a value is freed, and ORC
   remains the owner of every value. See [Memory model](05-memory-model.md).
2. **References are non-owning and restricted.** Version 1.3 adds `&T` and
   `&mut T`, with borrow expressions (`&place`, `&mut place`), dereference
   (`*r`) and match ergonomics — but there are no raw pointers, no address
   arithmetic and no way to observe an address. References cannot be stored in
   structs, enums, tuples, `Vec`, `Option`, `Result` or `Box`, cannot be captured
   by closures, cannot be compared with `==` and cannot be cast with `as`.
   Mutable references are limited to scalars, tuples, structs and enums; `&mut
   String`, `&mut Vec<T>`, `&mut Rc<T>` and `&mut Box<T>` are rejected. There is
   no lifetime syntax: elision follows fixed rules. See [References and
   borrowing](11-references-and-borrowing.md).
3. **No modules, crates or imports.** A program is one `.ls` file; all items are
   in one namespace. See [Program structure](02-program-structure.md).
4. **No user-defined generics or traits.** The only generic types are the builtin
   `Vec<T>`, `Option<T>`, `Result<T, E>`, `Rc<T>` and `Box<T>`; user structs and
   enums are monomorphic. There is no trait system, no operator overloading and
   no custom `Display`.
5. **No user macros.** The only macros are the builtin formatting/assertion
   macros and `vec!`. There is no `macro_rules!`, no derive and no attributes.
6. **Closures capture by value.** A closure sees a snapshot of the variables it
   mentions; `move` is accepted but changes nothing, and capturing a reference is
   rejected (`E0498`). Shared mutable state inside a closure is done with a
   captured `Rc<T>`. See [Closures](13-closures.md).
7. **No `usize`, no `isize`.** Lengths, indices and counts are `i64`. Cast
   explicitly when a builtin needs another integer type.
8. **No `char` type.** Character literals (`'a'`) are a parse error. String
   methods work on bytes or on characters depending on the method; see
   [Standard library](14-standard-library.md#string).
9. **No arrays and no slices.** `Vec<T>` is the growable sequence type, and
   tuples are the fixed-arity product type (flat tuples only in 1.3.0).
10. **Restricted patterns.** No `@` bindings, no `ref` patterns, no slice and no
    range patterns. String-literal patterns are supported, or-patterns (`p | q`)
    and match guards are fully supported, and a reference scrutinee is matched
    with match ergonomics rather than a `&` pattern. See [Pattern
    matching](12-pattern-matching.md).
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
    [Standard library](14-standard-library.md).
14. **`Rc<T>` equality is identity.** Two `Rc` handles compare equal only when
    they point at the same cell; `Box<T>`, by contrast, compares structurally.
    User structs and enums have no `==` at all in 1.3.0, and references have no
    `==` either — compare `*a == *b` instead.
15. **No unsafety.** There is no `unsafe`, no FFI and no threads. The generated C
    is not exposed to the language.

## Known limitations in 1.3.0

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
| Equality | No `==` for `Vec`, `Option`, `Result`, tuples, structs, enums, `()` or references | Match/compare fields element by element; compare through `*a == *b` for references |
| String indexing | `text[i]` is rejected (`E0608`) | Use `substring` / `find` / `split` |
| String API | No `String::clone` (not needed: copies are handles) and no `push(char)` | Use `push_str` |
| Formatting | Width, fill and alignment apply to a user struct/enum as a whole, but the `+`/`0` flags and precision are rejected for them (`E0001`); no named or indexed arguments | Format fields individually, or apply width/fill/alignment to the whole value |
| Numbers | `pow` is float-only; no integer exponentiation builtin | Convert to `f64` or write a loop |
| Patterns | No `@`/`ref`/slice/range patterns, and a reference pattern like `let &x = r` is rejected | Match a reference scrutinee directly: the ergonomic bindings are references, so read and write through `*binding` |
| `?` syntax | `?` directly after a block expression (`{ … }?`) is a parse error | Bind the block to a variable or call a helper first |
| Reference storage | References cannot be fields, tuple elements, `Vec` elements, `Option`/`Result` payloads, `Box` payloads or closure captures (`E0498`) | Keep references in locals, parameters and return values |
| Mutable heap references | `&mut String`, `&mut Vec<T>`, `&mut Rc<T>` and `&mut Box<T>` are not supported (`E0498`) | Take `&T` and restructure the code |
| Method receivers | A by-value `self` method cannot return a reference (`E0106`), and a `&self` method can only return shared references (`E0596`) | Return shared references from `&self` methods, mutable references from `&mut self` methods, and owned values from by-value methods |
| Return elision | No lifetime syntax; a free function returning a reference needs exactly one reference parameter, otherwise `E0106` | Pass exactly one reference parameter or return an owned value |
| Borrow granularity | `Vec` borrows are whole-container: two `&mut` borrows of different elements conflict (`E0499`) | Mutate elements one at a time or rebuild the vector |
| `for` over references | `for x in &vec` is rejected (`E0277`); loops iterate over a `Vec<T>` value or an integer range | Iterate by value, or use an index loop over a `&Vec` |
| Reference equality/casts | References have no `==` and no `as` conversion | Compare or convert the pointed-to values with `*r` |

## Not in version 1.3.0

The roadmap items that are still open:

* move semantics, `Clone`/`Drop` traits and destructors;
* mutable references to owned heap types (`&mut String`, `&mut Vec<T>`,
  `&mut Rc<T>`, `&mut Box<T>`);
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

* **v1.4** — move semantics (with `Clone`/`Drop`) and mutable references to
  owned heap types (`&mut String`, `&mut Vec<T>`).
* **v1.5** — user-defined generics and traits, iterators and `HashMap`.
* **v1.6** — modules, a package manager and a language server.

Items already delivered, intentionally no longer on the list:

* an owned `Box<T>` type — **delivered in 1.1.0**;
* string formatting widths and precision — **delivered in 1.1.0**;
* the `?` error-propagation operator and the `Option`/`Result` combinators —
  **delivered in 1.2.0**;
* non-owning references, borrow checking and match ergonomics — **delivered in
  1.3.0**.

## Language and runtime in 1.3.0

Version 1.3.0 is a **language release**: it adds `&T`/`&mut T` types, borrow
expressions, dereferencing, a lightweight borrow checker, reference parameters
and returns (with elision), and match ergonomics. It changes no execution
semantics for existing programs and adds **no runtime cost**: a reference is a
plain C pointer (`const T*` for `&T`, `T*` for `&mut T`), never retained or
released, so borrowing does not touch any reference count. ORC remains the only
owner of values, and since references cannot be stored, they cannot create cycles
or leaks. See [Memory model](05-memory-model.md) and [References and
borrowing](11-references-and-borrowing.md).

The compiler and runtime organization from 1.2.0 is unchanged:

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

The 1.3.0 toolchain measurements (approximate; see
[Benchmarks](16-benchmarks.md) for the full comparison with C, Rust, Python and
Node):

| Workload / metric | 1.3.0 |
| --- | --- |
| Cold `lsaca build -r`, per benchmark program | ~1.15-1.72 s |
| Cold build, `compile_large` (4,511 lines) | ~3.97 s |
| Cached rebuild | ~22-45 ms |
| Cached rebuild, `compile_large` | ~308 ms |
| `fib` runtime | 44.2 ms |
| `loop_sum` runtime | 13.9 ms |
| `vec_memory` runtime / peak RSS | 30.7 ms / 18.8 MiB |
| `string_build` runtime | 45.2 ms |
| `sort_i64` runtime | 63.1 ms |
| `binary_trees` runtime / peak RSS | 1216.7 ms / 150.1 MiB |
| `binary_trees_refs` runtime / peak RSS | 223.2 ms / 35.8 MiB |

The `fib` and `loop_sum` benchmarks are dominated by the generated arithmetic,
and references compile to plain pointers, so programs that use them measure the
same as equivalent programs that pass values.

## Compatibility

lsaca 1.3.0 is **source-compatible** with 1.2.0. Everything that compiled before
still compiles and behaves the same — all examples in the 1.2.0 manual still
build and run — and the language additions are purely additive:

* `&T` and `&mut T` types, `&place`/`&mut place` borrow expressions,
  `*r` dereference and automatic dereference in field access, method calls and
  indexing. New syntax, so no existing program changes meaning.
* A lightweight borrow checker that enforces aliasing for references. It only
  applies to code that uses references; programs without them are unaffected.
* Reference parameters and returns with fixed elision rules; `&self` and
  `&mut self` methods may now return references to their fields (a new
  capability, not a change to existing code).
* Match ergonomics for reference scrutinees.

The `mut`-on-parameters and `int as bool` leniencies kept since 1.1.0 are still
present. Conditions are strict — `if`, `while`, the operands of `&&` and `||`,
match guards and `assert!` all require `bool`. References are new in 1.3.0:
code that uses them needs a 1.3.0 compiler.

Generated C projects from earlier releases must be regenerated with the 1.3.0
compiler as usual; the emitted C for programs that do not use references keeps
the same shape as in 1.2.0.

## Benchmarks

Numbers for the current compiler — compile time, runtime and peak memory on
eleven runtime workloads (plus a compile-only scaling workload), compared with C,
Rust, Python and Node — are in [Benchmarks](16-benchmarks.md).

## What next

* The language tour starts at [Getting started](01-getting-started.md).
* Non-owning views and the borrow checker: [References and
  borrowing](11-references-and-borrowing.md).
* Every builtin in detail: [Standard library](14-standard-library.md).
* The memory model that explains the copy semantics:
  [Memory model](05-memory-model.md).
