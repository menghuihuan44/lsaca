# lsaca documentation

lsaca is a small, statically typed programming language with Rust-like syntax.
You write `fn`s, `struct`s, `enum`s, `impl` blocks, generic declarations,
traits, pattern matches and closures; the compiler checks the program and
translates it to portable C17. A
tiny C runtime provides the memory management machinery, and the compiler builds
the generated C itself with the system C compiler, so the result is a normal
native executable. CMake is optional: pass `--cmake` to build through the emitted
CMake project instead. A program can be a single file, a crate split across
modules and files, or a package with path dependencies; see
[Modules](language/23-modules.md) and [Packages](language/24-packages.md).

Version 2.0.0 (the previous major release) moved the standard library out of the
compiler into a normal crate named `std` and added the pointer-sized integer types `isize` and `usize`, trait
objects (`dyn Trait`), slices and `str`, `extern fn` declarations for compiler
intrinsics and default trait method bodies. Version 2.1.0 is the Rust-alignment
release: a bare integer literal is `i32`, lengths, indices, counts and capacities
are `usize`, standard-library text parameters are `&str` (a literal in a `&str`
position is a static slice and allocates nothing), tuple and unit structs and
`mut` parameters are supported, `parse` is typed with `ParseIntError` and
`ParseFloatError`, and the standard library fills in the Rust-shaped surface
(iterator adapters, string and character methods, `time` types, `env::VarError`,
`net::Shutdown`, `math` constants, integer-sized file APIs). The
[standard library reference](stdlib/README.md) documents the library module by
module, and marks each module and API as implemented in 2.1.0 or planned for a
later release.

This documentation set is the reference for the language as of version 2.1.0.
It is split into short pages, one per topic, so you can read it linearly
from [Getting started](language/01-getting-started.md) or jump straight to the
part you need. Every runnable example on these pages has been executed with the
compiler of the release it documents and shows its real output, including the
multi-file and package examples; the 2.1.0 examples, including the runnable
examples in the standard-library reference, are part of the 2.1.0 verification
gate and are executed against the 2.1.0 compiler.

## Why lsaca?

* **Rust's ergonomics, none of Rust's ceremony.** The syntax is deliberately
  familiar: `fn`, `let`, `struct`, `enum`, `impl`, `match`, closures, the same
  operator set and the same expression-oriented style. Version 1.4 added
  ownership and move semantics, automatically derived `Copy` and `Clone`, the
  `Drop` trait and mutable references to owned heap types; 1.5 added
  user-defined generics and traits, lazy iterators and the builtin
  `HashMap<K, V>`; 1.6 added the `char` scalar type and made every `String` a
  guaranteed-valid UTF-8 string with character-aware methods; 1.7 added modules,
  multi-file crates and a package manager with path dependencies; 2.0 added
  `isize`/`usize`, trait objects, slices and `str`, `extern fn` intrinsics and
  trait methods with default bodies; 2.1.0 makes `i32` the default integer,
  `usize` the length/index type and `&str` the standard text parameter, adds
  tuple/unit structs and `mut` parameters, and completes the Rust-shaped
  standard library. `Copy` and `Clone` are derived for you and
  there are still no lifetime annotations to write. Version 2.1.0 follows
  Rust's defaults for integers (`i32`), lengths and indices (`usize`) and
  borrowed text (`&str`) more closely than any earlier release.
* **Memory safety without a tracing garbage collector.** Values are managed by
  **ORC**: automatic reference counting plus a trial-deletion (Bacon-Rajan)
  cycle collector. Ownership decides when a value can be moved and when
  `Drop` runs; acyclic data is freed deterministically when its last owner is
  released, cycles are collected automatically, and there is no stop-the-world
  tracing GC. Trait objects extend the model without changing it: a `Box<dyn
  Trait>` still moves a concrete value and a `Rc<dyn Trait>` still refcounts one.
* **A small, complete language.** A program is one crate: a root file plus the
  modules it declares, compiled as one unit, or a package of crates. The items
  are `fn`, `struct`, `enum`, `impl`, `trait`, `const`, `type`, `mod` and
  `use`; the standard library is a crate whose root is the prelude: `String`,
  `Vec<T>`, `HashMap<K, V>`, `Option<T>`, `Result<T, E>`, `Rc<T>`, `Box<T>`
  and the lazy iterator adapters, plus the module APIs documented in the
  [standard library reference](stdlib/README.md). Version 1.2 added the postfix
  `?` operator and the Option/Result combinators; 1.3 added non-owning
  references and borrowing; 1.4 added ownership, moves, `.clone()` and `Drop`;
  1.5 added user-defined generics, traits, iterators and `HashMap`; 1.6 added the
  `char` type, Unicode-aware `String` methods and a Swiss-Table `HashMap`; 1.7
  added modules, multi-file crates and packages with path dependencies; 2.0 adds
  `isize`/`usize`, trait objects, slices and `str`, `extern fn` intrinsics and
  default trait method bodies.
* **Predictable native output, built directly.** The compiler emits readable
  C17 plus a small vendored runtime; by default it compiles and links that C
  itself, with no CMake, no Ninja and no extra configuration step. It also
  writes a complete `CMakeLists.txt`, so `lsaca emit-c` output can be inspected
  or embedded in another build, and `--cmake` builds through it when you want
  the platform's CMake pipeline.
* **Fast builds.** On the reference machine a cold build of an ordinary benchmark
  program takes about 0.46-0.82 s (the two largest single-file programs,
  `iterator_pipeline` and `fn_pipeline`, take 1.67 s and 1.57 s; 1.96 s for the
  deliberately large `compile_large` program, which records 301,361 bytes - about
  0.30 MB - of generated program C in `main.c`; 2.96 s for the multi-file
  `modules_compile` crate and 1.33 s for the three-package `package_compile`
  project), and a cached rebuild of an unchanged program takes 53-108 ms (549 ms for
  `compile_large`, 510 ms for `modules_compile`, 116 ms for `package_compile`).
  Runtime objects are compiled once per profile and reused. These figures are the
  official 2.1.0 numbers measured on 2026-11-06 and published on the
  [Benchmarks](language/21-benchmarks.md) page.
* **A useful subset, honestly documented.** Version 1.1.0 added `Box<T>` and
  format widths/precision; 1.2.0 added `?` and the combinators; 1.3.0 added
  references and the borrow checker; 1.4.0 added ownership, moves, `.clone()` and
  `Drop`; 1.5.0 added user-defined generics and traits, lazy iterators,
  `HashMap<K, V>` and a 16-program benchmark suite; 1.6.0 added the `char` type,
  UTF-8-validated strings with Unicode case and whitespace handling and a
  Swiss-Table `HashMap` (19-program suite); 1.7.0 added modules, multi-file
  crates, a package manager with path dependencies and `lsaca new`, and extended
  the benchmark suite to 21 programs with compile-time workloads for multi-file
  crates and packages; 2.0.0 added `isize`/`usize`, trait objects, slices and
  `str`, `extern fn` intrinsics, default trait method bodies and the standard
  library as a crate; 2.1.0 aligns the numeric and text defaults with Rust
  (`i32` literals, `usize` lengths and indices, `&str` parameters), adds tuple and
  unit structs and `mut` parameters, and rounds out the standard library. Known
  implementation limitations are listed in
  [Roadmap and differences from Rust](language/16-roadmap.md) rather than
  hidden.

A first taste:

```rust
fn half(n: i64) -> Option<i64> {
    if n % 2 == 0 { Some(n / 2) } else { None }
}

fn quarter(n: i64) -> Option<i64> {
    Some(half(half(n)?)?)
}

fn main() {
    println!("{:?} {:?}", quarter(8), quarter(6));
    println!("hello, {}", String::from("world"));
}
```

That program prints `Some(2) None` then `hello, world`, and is explained step by
step in [Getting started](language/01-getting-started.md). Because ownership is
new since 1.4.0, a program that reuses a heap value after giving it away needs an
explicit `.clone()`; see [Ownership and moves](language/11-ownership-and-moves.md).
The feature releases have their own pages:
[Generics](language/17-generics.md), [Traits](language/18-traits.md),
[Iterators](language/19-iterators.md), [HashMap](language/20-hashmap.md),
[Characters and Unicode](language/22-characters-and-unicode.md),
[Modules](language/23-modules.md), [Packages](language/24-packages.md),
[Trait objects](language/25-trait-objects.md) and
[Slices and str](language/26-slices-and-str.md).
More complete programs live in the repository's `examples/` directory, including
[`examples/ownership.ls`](../examples/ownership.ls), which demonstrates moves,
`.clone()` and a `Drop` hook end to end,
[`examples/iterators.ls`](../examples/iterators.ls), which implements a user
iterator and runs it through the adapters, and
[`examples/unicode.ls`](../examples/unicode.ls), which walks strings by
character and shows Unicode case mapping.

## Documentation map

Start with **Getting started** if lsaca is new to you. The rest is grouped by
topic: the language chapters first, then the standard library reference, then the
reference material and the example programs.

### Getting started

| Page | What it covers |
| --- | --- |
| [01 Getting started](language/01-getting-started.md) | Requirements, building the compiler, the first program, the direct and CMake backends, the CLI and exit codes. |

### Language

| Page | What it covers |
| --- | --- |
| [02 Program structure](language/02-program-structure.md) | Crates and compilation units, the item kinds including `extern fn`, modules, `main`, nesting and the complete EBNF grammar. |
| [03 Lexical structure](language/03-lexical-structure.md) | Identifiers, keywords, comments, integer/float/string/character literals, escapes and booleans. |
| [04 Types](language/04-types.md) | Primitive and builtin types including `char`, `isize`/`usize`, slices and `str`, trait objects, tuples, function types, `Self`, type inference and `as` casts. |
| [05 Memory model](language/05-memory-model.md) | Ownership, moves, `Drop`, ORC reference counting, copy-on-write, iterator and `HashMap` ownership, `Box<T>`, `Rc<T>` cycles, the cycle collector and leak tracking. |
| [06 Variables and mutability](language/06-variables-and-mutability.md) | `let`, `let mut`, shadowing, assignment (which moves), compound assignment and `const`. |
| [07 Operators](language/07-operators.md) | The full precedence table, arithmetic (including overflow), bitwise and boolean operators, comparisons, casts and `?`. |
| [08 Statements and control flow](language/08-statements-and-control-flow.md) | Blocks, `if`/`else`, `loop`, `while`, `for` (which moves a `Vec`), ranges, `if let`, `while let` and `return`. |
| [09 Functions](language/09-functions.md) | Declarations, parameters (by value, which moves, and `mut` parameters), recursion, function values, early returns, `extern fn` and the restrictions on `main`. |
| [10 Structs, enums and impl](language/10-structs-enums-and-impl.md) | Data declarations including tuple and unit structs, literals, methods and associated functions, receivers, `Drop`, `.clone()` and value semantics. |
| [11 Ownership and moves](language/11-ownership-and-moves.md) | `Copy` versus move-only types, consuming contexts, move errors, `Drop`, drop order, `.clone()` semantics and how moves interact with references, ORC and trait objects. |
| [12 References and borrowing](language/12-references-and-borrowing.md) | `&T`/`&mut T`, borrow expressions and dereferencing, the aliasing rules, reference parameters and returns, match ergonomics, mutable heap references, `&[T]`/`&str` and how references interact with ORC. |
| [13 Pattern matching](language/13-pattern-matching.md) | All pattern forms including or-patterns and `match` guards, exhaustiveness, refutability, unreachable arms and moves in patterns. |
| [14 Closures](language/14-closures.md) | Closure syntax and types, capture by value (copy or move), mutation, `move`, storing closures in aggregates, and using `Rc<T>` for shared state. |
| [15 Standard library](language/15-standard-library.md) | Language-level library behaviour: formatting, UTF-8 `String`, `Vec`, `Option`/`Result`, `?`, `Rc`, `Box`, iterators and `HashMap`; links to the per-module reference. |
| [16 Roadmap and differences](language/16-roadmap.md) | How lsaca deliberately differs from Rust, verified limitations of 2.1.0 and what comes next. |
| [17 Generics](language/17-generics.md) | Generic structs, enums, functions and impls, bounds, turbofish syntax, monomorphization and its late-checking rules. |
| [18 Traits](language/18-traits.md) | Trait declarations, default method bodies, implementing a trait for a struct or enum, static dispatch, coherence errors and `Fn`/`Drop` bounds. |
| [19 Iterators](language/19-iterators.md) | The `Iterator<T>` trait, `.iter()`, the 2.1.0 adapters (`zip`, `chain`, `filter_map`, `take_while`, `skip_while`, `step_by`), the terminal operations (with `usize` counts and positions), `for` loops over iterators and writing your own iterator. |
| [20 HashMap](language/20-hashmap.md) | The builtin `HashMap<K, V>`: construction, allowed key types, ownership of keys and values, lookup and mutation, the Swiss-Table layout, iteration order and equality. |
| [21 Benchmarks](language/21-benchmarks.md) | Measured compile times, runtime and memory against C, Rust, Python and Node: the official 2.1.0 run. |
| [22 Characters and Unicode](language/22-characters-and-unicode.md) | The `char` type, character literals and escapes, Unicode scalar values, UTF-8 `String` APIs, byte indices versus character indices, Unicode case mapping and whitespace, `Debug` escaping and validated text input. |
| [23 Modules](language/23-modules.md) | Inline and external modules, the module file layout, `crate`/`self`/`super` paths, `use` trees, visibility, multi-file diagnostics and how modules are compiled. |
| [24 Packages](language/24-packages.md) | The `lsaca.toml` manifest, path dependencies and the dependency graph, `src/main.ls` versus `src/lib.ls`, `lsaca new`, package mode for `run`/`build`/`check`/`emit-c` and the resulting C project. |
| [25 Trait objects](language/25-trait-objects.md) | `dyn Trait`, coercion from `Box`/`Rc`/references, object safety, sharing and copies, vtables and the object-safety limitations of 2.1.0. |
| [26 Slices and str](language/26-slices-and-str.md) | The unsized types `[T]` and `str`, slice expressions with `usize` bounds, implicit coercions from `Vec`/`String`, static `&str` literals, slice and `str` methods, UTF-8 and bounds panics and the representation. |

### Standard library

The library is documented module by module in the
[standard library reference](stdlib/README.md), which is the API reference; the
[Standard library chapter](language/15-standard-library.md) covers the
language-level behaviour that is not a module (formatting macros, panics, exit
codes).

| Module | Page | Status in 2.1.0 |
| --- | --- | --- |
| `std::core` (Option, Result, Iterator) | [core](stdlib/core.md) | migrated from 1.7; the 2.1.0 Option/Result combinators implemented |
| `std::string` | [string](stdlib/string.md) | `&str` text API, `String` forwarding, `parse` with typed errors and the new `str` methods implemented |
| `std::char` | [char](stdlib/char.md) | ASCII classification methods implemented; Unicode classification planned |
| `std::vec` | [vec](stdlib/vec.md) | migrated; `sort`, `truncate`/`swap`/`contains`, `as_slice` and `as_mut_slice` implemented |
| `std::iter` | [iter](stdlib/iter.md) | `zip`, `chain`, `filter_map`, `take_while`, `skip_while`, `step_by` and the new terminals implemented; `rev`/`flat_map` planned |
| `std::fmt` | [fmt](stdlib/fmt.md) | compiler-implemented |
| `std::math` | [math](stdlib/math.md) | migrated plus the 16 `f64` constants; `abs`/`min`/`max` remain compiler builtins |
| `std::collections` | [collections](stdlib/collections/README.md) | `HashMap` migrated; `HashSet`, `BTreeSet`, `BTreeMap`, `VecDeque` and `BinaryHeap` implemented |
| `std::io` | [io](stdlib/io.md) | error types, `io::Result`, file and stream handles with integer `read`/`write`/`seek` implemented; the stream traits are planned |
| `std::fs` | [fs](stdlib/fs.md) | `&str` path API, directory and metadata API, `FileType` and integer-sized metadata implemented |
| `std::net` | [net](stdlib/net.md) | implemented (plain TCP, `Shutdown` added) |
| `std::time` | [time](stdlib/time.md) | `Duration`, `Instant`, `SystemTime`, `unix_epoch()` and `sleep` added |
| `std::env` | [env](stdlib/env.md) | implemented; `var` returns `Result<String, VarError>` |
| `std::process` | [process](stdlib/process.md) | `args` and `exit` implemented |
| `std::error` | [error](stdlib/error.md) | the `Error` trait implemented, with `io::Error` and the parse errors as implementors |

The status column records what version 2.1.0 provides; where a module is only
partly implemented, its page separates the implemented API from the API that is
not implemented in 2.1.0 and planned for a later release. Entries marked *planned*
are documented from the design rather than from runnable behaviour.

### Reference

| Page | What it covers |
| --- | --- |
| [Program structure](language/02-program-structure.md#the-complete-grammar) | The complete EBNF grammar of the language. |
| [Getting started](language/01-getting-started.md) | The complete command-line interface, the build backends and exit codes. |
| [Types](language/04-types.md) | The complete type system summary and the type table. |
| [Memory model](language/05-memory-model.md) | The ORC memory model, drop order and leak tracking. |
| [Roadmap and differences from Rust](language/16-roadmap.md) | The exact list of deviations from Rust and the known limitations of 2.1.0. |

### Guides and examples

| Page | What it covers |
| --- | --- |
| [Getting started](language/01-getting-started.md) | The first program and the two build backends step by step. |
| [Ownership and moves](language/11-ownership-and-moves.md) | A guide to `Copy`, moves, `Drop` and `.clone()`. |
| [`examples/ownership.ls`](../examples/ownership.ls) | A complete program demonstrating moves, `.clone()` and a `Drop` hook. |
| [`examples/iterators.ls`](../examples/iterators.ls) | A user iterator run through the adapters. |
| [`examples/unicode.ls`](../examples/unicode.ls) | Walking strings by character and Unicode case mapping. |
| [Benchmarks](language/21-benchmarks.md) | The measured numbers and the benchmark programs. |

## Building the compiler

These pages document the *language*. For build requirements, the repository
layout, the C toolchain details, environment variables and troubleshooting, see
the [project README](../README.md). The short version:

```console
cargo build --release
./target/release/lsaca run examples/hello.ls
```

The binary is a single executable. It needs a C17 compiler at run time and
nothing else; CMake 3.16 or newer is only needed if you pass `--cmake`. The
compiler itself has no dependencies beyond the Rust standard library. Building
the compiler also embeds the standard library, so the compiler binary contains
the `std` crate's source.

## Conventions used on these pages

* Runnable examples appear in `ls` code blocks. Unless marked otherwise the
  block is a complete program; the `text` block below it is the exact output
  observed when the program was compiled and run.
* Blocks marked `compile-fail` show a program the front end rejects, followed
  by the exact diagnostic text. Blocks marked `panic` show a program that
  compiles but panics at run time.
* A multi-file program is shown in one block marked `tree`: the block body is a
  file tree, each file introduced by a `--- relative/path` line, and `main.ls`
  at the root is the entry file. A block marked `tree,package=DIR` is a package
  tree whose manifest is `DIR/lsaca.toml`; it is compiled and run in package
  mode from inside `DIR`. Both forms follow the same `Output:`/`Error:`
  convention as single-file examples.
* A block marked `check` runs `lsaca check` instead of `lsaca run`: it shows
  the front end output for a program that is expected to be accepted.
* Syntax-only fragments and the grammar use `rust` or `ebnf` blocks; shell
  sessions use `console` blocks. These are not executable examples.
* "Debug build" means the default profile used when `-r`/`--release` is absent:
  the C is compiled with `-g -O0 -DLSACA_DEBUG`, which enables checked integer
  arithmetic and extra run-time checks. "Release build" means `-O2 -DNDEBUG`,
  with wrapping integer arithmetic. The two backends (direct and CMake) use the
  same flags.

## License

lsaca is distributed under the Apache-2.0 license; see the repository for the
full text.
