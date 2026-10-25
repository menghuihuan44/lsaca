# lsaca documentation

lsaca is a small, statically typed programming language with Rust-like syntax.
You write `fn`s, `struct`s, `enum`s, `impl` blocks, pattern matches and
closures; the compiler checks the program and translates it to portable C17. A
tiny C runtime provides a standard library and the memory management machinery,
and the compiler builds the generated C itself with the system C compiler, so
the result is a normal native executable. CMake is optional: pass `--cmake` to
build through the emitted CMake project instead.

This documentation set is the reference for the language as of **version
1.2.0**. It is split into short pages, one per topic, so you can read it
linearly from [Getting started](language/01-getting-started.md) or jump straight
to the part you need. Every runnable example on these pages has been executed
with the 1.2.0 compiler and shows its real output.

## Why lsaca?

* **Rust's ergonomics, none of Rust's ceremony.** The syntax is deliberately
  familiar: `fn`, `let`, `struct`, `enum`, `impl`, `match`, closures, the same
  operator set and the same expression-oriented style. But there is no borrow
  checker, no lifetimes and no ownership annotations to fight.
* **Memory safety without a tracing garbage collector.** Values are managed by
  **ORC**: automatic reference counting plus a trial-deletion (Bacon-Rajan)
  cycle collector. Acyclic data is freed deterministically when its last handle
  goes away, cycles are collected automatically, and there is no stop-the-world
  tracing GC.
* **A small, complete language.** One source file is one compilation unit. The
  items are `fn`, `struct`, `enum`, `impl`, `const` and `type`; the standard
  library is built in: `String`, `Vec<T>`, `Option<T>`, `Result<T, E>`,
  `Rc<T>` and `Box<T>`. Version 1.2 adds the postfix `?` operator and the
  Option/Result combinators, so fallible code reads like Rust without any
  imports.
* **Predictable native output, built directly.** The compiler emits readable
  C17 plus a small vendored runtime; by default it compiles and links that C
  itself, with no CMake, no Ninja and no extra configuration step. It also
  writes a complete `CMakeLists.txt`, so `lsaca emit-c` output can be inspected
  or embedded in another build, and `--cmake` builds through it when you want
  the platform's CMake pipeline.
* **Fast builds.** The direct backend cut a cold build of the benchmark corpus
  from roughly 2.9 s to about 1.2 s on the reference machine; a cached rebuild
  takes about 0.03 s. Runtime objects are compiled once per profile and reused.
* **A useful subset, honestly documented.** Version 1.1.0 added `Box<T>` and
  format widths/precision; version 1.2.0 adds `?` and the combinators. Known
  implementation limitations are listed in
  [Roadmap and differences from Rust](language/14-roadmap.md) rather than
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
step in [Getting started](language/01-getting-started.md).

## Documentation map

Start with **Getting started** if lsaca is new to you. The remaining pages are
roughly ordered from the surface of the language down to its memory model and
reference material.

| Page | What it covers |
| --- | --- |
| [01 Getting started](language/01-getting-started.md) | Requirements, building the compiler, the first program, the direct and CMake backends, the CLI and exit codes. |
| [02 Program structure](language/02-program-structure.md) | Compilation units, the six item kinds, `main`, nesting and the complete EBNF grammar. |
| [03 Lexical structure](language/03-lexical-structure.md) | Identifiers, keywords, comments, integer/float/string literals, escapes and booleans. |
| [04 Types](language/04-types.md) | Primitive and builtin types, tuples, function types, `Self`, type inference and `as` casts. |
| [05 Memory model](language/05-memory-model.md) | ORC: reference counting, copy-on-write, value semantics, `Box<T>`, `Rc<T>` cycles, the cycle collector and leak tracking. |
| [06 Variables and mutability](language/06-variables-and-mutability.md) | `let`, `let mut`, shadowing, assignment, compound assignment and `const`. |
| [07 Operators](language/07-operators.md) | The full precedence table, arithmetic (including overflow), bitwise and boolean operators, comparisons, casts and `?`. |
| [08 Statements and control flow](language/08-statements-and-control-flow.md) | Blocks, `if`/`else`, `loop`, `while`, `for`, ranges, `if let`, `while let` and `return`. |
| [09 Functions](language/09-functions.md) | Declarations, parameters, recursion, function values, early returns and the restrictions on `main`. |
| [10 Structs, enums and impl](language/10-structs-enums-and-impl.md) | Data declarations, literals, methods and associated functions, receivers and value semantics. |
| [11 Pattern matching](language/11-pattern-matching.md) | All pattern forms including or-patterns and `match` guards, exhaustiveness, refutability and unreachable arms. |
| [12 Closures](language/12-closures.md) | Closure syntax and types, capture semantics, mutation, `move`, and using `Rc<T>` for shared state. |
| [13 Standard library](language/13-standard-library.md) | Formatting (widths/precision), `String`, `Vec`, `Option` and `Result` with their combinators, `?`, `Rc`, `Box`, free functions and panics. |
| [14 Roadmap and differences](language/14-roadmap.md) | How lsaca deliberately differs from Rust, verified limitations of the current release and what comes next. |
| [15 Benchmarks](language/15-benchmarks.md) | Measured compile times, runtime and memory against C, Rust, Python and Node. |

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
