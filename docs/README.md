# lsaca documentation

lsaca is a small, statically typed programming language with Rust-like syntax.
You write `fn`s, `struct`s, `enum`s, `impl` blocks, generic declarations,
traits, pattern matches and closures; the compiler checks the program and
translates it to portable C17. A
tiny C runtime provides a standard library and the memory management machinery,
and the compiler builds the generated C itself with the system C compiler, so
the result is a normal native executable. CMake is optional: pass `--cmake` to
build through the emitted CMake project instead.

This documentation set is the reference for the language as of **version
1.5.0**. It is split into short pages, one per topic, so you can read it
linearly from [Getting started](language/01-getting-started.md) or jump straight
to the part you need. Every runnable example on these pages has been executed
with the 1.5.0 compiler and shows its real output.

## Why lsaca?

* **Rust's ergonomics, none of Rust's ceremony.** The syntax is deliberately
  familiar: `fn`, `let`, `struct`, `enum`, `impl`, `match`, closures, the same
  operator set and the same expression-oriented style. Version 1.4 added
  ownership and move semantics, automatically derived `Copy` and `Clone`, the
  `Drop` trait and mutable references to owned heap types; version 1.5 adds
  user-defined generics and traits, lazy iterators and the builtin
  `HashMap<K, V>`. `Copy` and `Clone` are derived for you and there are still no
  lifetime annotations to write.
* **Memory safety without a tracing garbage collector.** Values are managed by
  **ORC**: automatic reference counting plus a trial-deletion (Bacon-Rajan)
  cycle collector. Ownership decides when a value can be moved and when
  `Drop` runs; acyclic data is freed deterministically when its last owner is
  released, cycles are collected automatically, and there is no stop-the-world
  tracing GC.
* **A small, complete language.** One source file is one compilation unit. The
  items are `fn`, `struct`, `enum`, `impl`, `trait`, `const` and `type`; the
  standard library is built in: `String`, `Vec<T>`, `HashMap<K, V>`,
  `Option<T>`, `Result<T, E>`, `Rc<T>`, `Box<T>` and the lazy iterator adapters.
  Version 1.2 added the postfix `?` operator and the Option/Result combinators;
  1.3 added non-owning references and borrowing; 1.4 added ownership, moves,
  `.clone()` and `Drop`; 1.5 adds user-defined generics, traits, iterators and
  `HashMap`.
* **Predictable native output, built directly.** The compiler emits readable
  C17 plus a small vendored runtime; by default it compiles and links that C
  itself, with no CMake, no Ninja and no extra configuration step. It also
  writes a complete `CMakeLists.txt`, so `lsaca emit-c` output can be inspected
  or embedded in another build, and `--cmake` builds through it when you want
  the platform's CMake pipeline.
* **Fast builds.** On the reference machine a cold build of a benchmark program
  takes about 0.74-1.05 s (3.1 s for the deliberately large `compile_large`
  program, which emits about 1.05 MB of C), and a cached rebuild of an unchanged
  program takes 13-20 ms (0.23 s for `compile_large`). Runtime objects are
  compiled once per profile and reused. These figures were measured on 1.5.0;
  full details are on the [Benchmarks](language/21-benchmarks.md) page.
* **A useful subset, honestly documented.** Version 1.1.0 added `Box<T>` and
  format widths/precision; 1.2.0 added `?` and the combinators; 1.3.0 added
  references and the borrow checker; 1.4.0 added ownership, moves, `.clone()` and
  `Drop`; 1.5.0 adds user-defined generics and traits, lazy iterators,
  `HashMap<K, V>` and a 16-program benchmark suite. Known implementation
  limitations are listed in
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
The 1.5 features have their own pages:
[Generics](language/17-generics.md), [Traits](language/18-traits.md),
[Iterators](language/19-iterators.md) and [HashMap](language/20-hashmap.md).
More complete programs live in the repository's `examples/` directory, including
[`examples/ownership.ls`](../examples/ownership.ls), which demonstrates moves,
`.clone()` and a `Drop` hook end to end, and
[`examples/iterators.ls`](../examples/iterators.ls), which implements a user
iterator and runs it through the adapters.

## Documentation map

Start with **Getting started** if lsaca is new to you. The remaining pages are
roughly ordered from the surface of the language down to its memory model and
reference material.

| Page | What it covers |
| --- | --- |
| [01 Getting started](language/01-getting-started.md) | Requirements, building the compiler, the first program, the direct and CMake backends, the CLI and exit codes. |
| [02 Program structure](language/02-program-structure.md) | Compilation units, the seven item kinds, `main`, nesting and the complete EBNF grammar. |
| [03 Lexical structure](language/03-lexical-structure.md) | Identifiers, keywords, comments, integer/float/string literals, escapes and booleans. |
| [04 Types](language/04-types.md) | Primitive and builtin types, `Copy` and move-only types, tuples, function types, `Self`, type inference and `as` casts. |
| [05 Memory model](language/05-memory-model.md) | Ownership, moves, `Drop`, ORC reference counting, copy-on-write, iterator and `HashMap` ownership, `Box<T>`, `Rc<T>` cycles, the cycle collector and leak tracking. |
| [06 Variables and mutability](language/06-variables-and-mutability.md) | `let`, `let mut`, shadowing, assignment (which moves), compound assignment and `const`. |
| [07 Operators](language/07-operators.md) | The full precedence table, arithmetic (including overflow), bitwise and boolean operators, comparisons, casts and `?`. |
| [08 Statements and control flow](language/08-statements-and-control-flow.md) | Blocks, `if`/`else`, `loop`, `while`, `for` (which moves a `Vec`), ranges, `if let`, `while let` and `return`. |
| [09 Functions](language/09-functions.md) | Declarations, parameters (by value, which moves), recursion, function values, early returns and the restrictions on `main`. |
| [10 Structs, enums and impl](language/10-structs-enums-and-impl.md) | Data declarations, literals, methods and associated functions, receivers, `Drop`, `.clone()` and value semantics. |
| [11 Ownership and moves](language/11-ownership-and-moves.md) | `Copy` versus move-only types, consuming contexts, move errors, `Drop`, drop order, `.clone()` semantics and how moves interact with references and ORC. |
| [12 References and borrowing](language/12-references-and-borrowing.md) | `&T`/`&mut T`, borrow expressions and dereferencing, the aliasing rules, reference parameters and returns, match ergonomics, mutable heap references and how references interact with ORC. |
| [13 Pattern matching](language/13-pattern-matching.md) | All pattern forms including or-patterns and `match` guards, exhaustiveness, refutability, unreachable arms and moves in patterns. |
| [14 Closures](language/14-closures.md) | Closure syntax and types, capture by value (copy or move), mutation, `move`, storing closures in aggregates, and using `Rc<T>` for shared state. |
| [15 Standard library](language/15-standard-library.md) | Formatting (widths/precision), `String`, `Vec`, `Option` and `Result` with their combinators, `?`, `Rc`, `Box`, `.clone()`, iterators and `HashMap` at a glance, free functions and panics. |
| [16 Roadmap and differences](language/16-roadmap.md) | How lsaca deliberately differs from Rust, verified limitations of the current release and what comes next. |
| [17 Generics](language/17-generics.md) | Generic structs, enums, functions and impls, bounds, turbofish syntax, monomorphization and its late-checking rules. |
| [18 Traits](language/18-traits.md) | Trait declarations, implementing a trait for a struct or enum, method dispatch, coherence errors, `Fn`/`Drop` bounds and why there are no trait objects. |
| [19 Iterators](language/19-iterators.md) | The `Iterator<T>` trait, `.iter()`, lazy adapters, terminal operations, `for` loops over iterators and writing your own iterator. |
| [20 HashMap](language/20-hashmap.md) | The builtin `HashMap<K, V>`: construction, allowed key types, ownership of keys and values, lookup and mutation, iteration order and equality. |
| [21 Benchmarks](language/21-benchmarks.md) | Measured compile times, runtime and memory against C, Rust, Python and Node. |

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
compiler itself has no dependencies beyond the Rust standard library.

## Conventions used on these pages

* Runnable examples appear in `ls` code blocks. Unless marked otherwise the
  block is a complete program; the `text` block below it is the exact output
  observed when the program was compiled and run.
* Blocks marked `compile-fail` show a program the front end rejects, followed
  by the exact diagnostic text. Blocks marked `panic` show a program that
  compiles but panics at run time.
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
