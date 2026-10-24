# lsaca

lsaca is a small, statically typed programming language with Rust-like syntax and
semantics, implemented in Rust. Source code is compiled to C17, then built into a
native executable with CMake and the system C compiler. Memory is managed with
**ORC** (Ownership and Reference Counting): automatic reference counting (ARC,
in the spirit of Nim's ARC) plus a trial-deletion cycle collector. There is no
tracing garbage collector, no borrow checker, and no manual `free` calls.

This repository is a complete rewrite of an earlier C17 implementation. The
pipeline, the runtime, the memory model, the grammar and the back end were
redesigned rather than ported.

## Highlights

- **Rust-like language**: `fn`, `let`/`let mut`, `struct`, `enum`, `impl`,
  `match` with exhaustiveness checking, `Option`, `Result`, `Vec<T>`, `Rc<T>`,
  tuples, closures, `if let`/`while let`, `for` loops, compound assignment and
  `print!`/`println!` macros. Blocks, `if` and `match` are expressions.
- **Static typing with local inference**: annotations are optional where the type
  follows from the initializer; numeric literals default to `i64`/`f64`.
- **ORC memory management**: every owned value is reference counted (strings,
  vectors, closures, and structs/enums that own data). Assignment, argument
  passing, capture and return copy handles; no use-after-free is possible.
  Shared mutable state is written with the builtin `Rc<T>` cell, and reference
  cycles created through `Rc<T>` are reclaimed automatically by a Bacon-Rajan
  trial-deletion cycle collector.
- **C17 + CMake back end**: the compiler emits a small portable C project
  (`main.c`, the runtime and `CMakeLists.txt`) and builds it. `lsaca emit-c`
  keeps the generated project so it can be inspected or embedded in another
  build.
- **Pure Rust compiler, no third-party crates**: the compiler uses only the Rust
  standard library and forbids unsafe code. The runtime is plain C17.
- **Tested end to end**: unit tests for every compiler stage, an end-to-end suite
  that compiles and runs real programs, a C runtime test suite and CLI tests.

## Requirements

- Rust 1.99 or newer (`cargo`, 2024 edition).
- A C17 compiler: GCC or Clang. On Windows, MSYS2's `gcc` (UCRT64 environment)
  is detected automatically.
- CMake 3.16 or newer and a build generator: Ninja is used when available,
  otherwise CMake falls back to MinGW Makefiles on Windows and Unix Makefiles
  elsewhere.

## Quick start

```sh
cargo build --release
./target/release/lsaca run examples/hello.ls
```

On Windows the binary is `target\release\lsaca.exe`.

A tiny program:

```rust
fn main() {
    let mut total = 0;
    for i in 1..=10 {
        total = total + i;
    }
    println!("sum = {}", total);
}
```

## Command line

```text
lsaca run <file.ls> [-- <program args...>]   compile, build (cached) and run
lsaca build <file.ls> [-o <dir>]             build a native executable
lsaca emit-c <file.ls> [-o <dir>]            write the generated C project only
lsaca check <file.ls>                        run the front end and report diagnostics
lsaca version | help
```

Options: `-r, --release` (optimized build; arithmetic overflow wraps instead of
panicking in release mode), `-o, --out-dir <dir>` (output directory for
`build` and `emit-c`), `-v, --verbose` (show external commands) and `--keep`
(print the location of the temporary build tree).

Exit codes: `0` success, `1` compile or build error, `2` command line usage
error, `101` a panic inside the compiled program.

## How compilation works

```text
 source (.ls)
    |  lexer            tokens
    |  parser           AST
    |  type checker     typed HIR: name resolution, inference,
    |                   exhaustiveness and constant evaluation
    |  C code generator main.c + CMakeLists.txt
    v
 C17 project  +  runtime (lsaca_rt.h/.c, embedded in the compiler)
    |
    v  CMake + system C compiler
 native executable
```

The generated project is self contained:

```text
main.c          the user program plus generated copy/release/format helpers
lsaca_rt.h      runtime ABI: reference counting and cycle collection, String,
                Vec, Option, Result
lsaca_rt.c      runtime implementation: formatting, checked arithmetic, panics
CMakeLists.txt  portable build description (Debug: -g -O0 -DLSACA_DEBUG,
                Release: -O2 -DNDEBUG)
```

Builds are cached by content hash (source, generated C, runtime, compiler path,
build mode and leak-tracking mode), so repeated runs of unchanged programs are
fast. The cache
location is chosen defensively:

1. `LSACA_CACHE_DIR`, when set;
2. `lsaca-builds` under the system temporary directory, when that path is
   ASCII-only and actually writable;
3. `.lsaca-build-cache` in the current directory.

The fallback keeps builds working in sandboxes that only allow writes inside the
workspace, when the temporary directory is not writable, and when its path is
not ASCII-only (some C toolchains handle non-ASCII paths poorly). The chosen
cache root is probed for writability before it is used, and `-v` prints it.
Every build also runs CMake and the compiler with `TEMP`, `TMP` and `TMPDIR`
pointing inside the build tree, so the toolchain does not depend on the system
temporary directory either. The cache is safe to delete at any time. Built
executables are copied to the requested output directory, so source paths may
contain any characters.

## Memory management (ORC)

Every heap value (String, Vec, closure environment, an `Rc<T>` cell, or a
struct/enum value that contains one) is owned by its holder through a reference
count. Copying a value - assignment, argument passing, capture or return -
retains it; the holder releases it when the variable or temporary goes out of
scope. Strings and vectors are copy-on-write, so mutating one holder never
affects another. Function arguments are borrowed rather than moved: programs
never fail to compile because of a move. Unlike Rust, ORC keeps memory safety
but not the move checker.

Ordinary values cannot form reference cycles: copying a value never creates a
second path back to the same object. The builtin `Rc<T>` type is the exception:
it is a shared, mutable heap cell whose handles all alias the same payload, so a
program can build cyclic structures - for example a node whose `next` field
points back at the cell that contains it. To reclaim those, ORC has two parts:

1. automatic reference counting (ARC), which frees acyclic data as soon as its
   last handle is released, deterministically and without pauses;
2. a trial-deletion cycle collector (the Bacon-Rajan mark-gray / scan /
   collect-white algorithm), which reclaims cyclic garbage once no handles
   outside the cycle remain. Reachable cycles are never collected.

The collector runs automatically once enough candidate objects accumulate; the
threshold is internal and not user-visible, and there is no `collect` function
in the language. A final collection runs during process shutdown, before the
leak report. A shared counter is written like this:

```rust
let counter = Rc::new(0);
let copy = counter;
copy.set(copy.get() + 1);
println!("{}", counter.get());   // prints 1
```

Generated programs built in the CMake Debug configuration define
`LSACA_DEBUG`, which makes the runtime track reference-counted allocations and
report leaks at shutdown. Set `LSACA_DEBUG_RC=1` when running `lsaca` (or pass
`-DLSACA_DEBUG_RC=ON` when building an emitted project yourself) to enable the
same tracking in a release build as well. Cyclic garbage is collected before the
report, so the report only counts genuinely leaked allocations (for example
acyclic data that the compiler failed to release); it indicates a
compiler/runtime bug, not a user bug, unless the program panicked or called
`exit`. With either define the report is:

```text
lsaca: warning: N reference-counted allocation(s) leaked
lsaca:   leaked allocation of S bytes
```

Runtime panics (division by zero, out-of-bounds indexing, overflow in debug
builds, `panic!`/`unwrap` on `None`/`Err`) print
`thread 'main' panicked at <file>:<line>:<col>:` followed by the message and
exit with code 101.

## Language

```rust
struct Point { x: f64, y: f64 }

impl Point {
    fn origin() -> Point { Point { x: 0.0, y: 0.0 } }
    fn len_squared(&self) -> f64 { self.x * self.x + self.y * self.y }
}

fn main() {
    let point = Point { x: 3.0, y: 4.0 };
    let distance = sqrt(point.len_squared());
    println!("distance = {}", distance);
}
```

[docs/LANGUAGE.md](docs/LANGUAGE.md) is the language manual: full syntax,
semantics, the standard library, memory rules and the deliberate deviations from
Rust. See [examples/](examples/) for runnable programs.

## Environment variables

- `LSACA_CC` - C compiler used for generated projects (default: `gcc`, `cc` or
  `clang` from `PATH`, then common MSYS2 locations on Windows).
- `LSACA_JOBS` - parallel build jobs passed to the C build.
- `LSACA_CACHE_DIR` - build cache location (default: tried under the system
  temporary directory first, then `.lsaca-build-cache` in the current
  directory).
- `LSACA_DEBUG_RC` - set to `1` to track reference-counted allocations and
  report leaks in release builds too (generated projects expose the same switch
  as the CMake option `LSACA_DEBUG_RC`).

## Troubleshooting

- **`failed to build <file>: ... (os error 5)` / "Access is denied"**: the
  compiler could not write to its build cache. Run with `-v` to see the chosen
  cache directory, or point `LSACA_CACHE_DIR` at a writable directory. The
  compiler probes the cache location and falls back to `.lsaca-build-cache` in
  the current directory instead of failing on an unwritable `%TEMP%`.
- **`no C compiler found`**: install GCC or Clang (MSYS2 UCRT64 on Windows) and
  put it on `PATH`, or set `LSACA_CC` to the compiler executable.
- **`CMake was not found on PATH`**: install CMake 3.16 or newer.
- **`no build generator found`**: install Ninja (recommended) or
  `mingw32-make`.

## Testing

```sh
cargo test --workspace          # unit, end-to-end and CLI tests
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

The end-to-end tests in `crates/lsaca/tests/` compile and run the programs under
`crates/lsaca/tests/programs/` and `examples/`, and are skipped when no C
compiler is available. The C runtime has its own test suite under
`runtime/test/` (`runtime/test/main.c` plus the `runtime/test/run_smoke.ps1`
smoke script for Windows). Generated projects are expected to compile without
warnings under `-Wall -Wextra`.

## Repository layout

```text
crates/lsaca/            the compiler library (lexer, parser, type checker, C codegen, driver)
crates/lsaca/tests/      end-to-end tests and test programs
crates/lsaca-cli/        the lsaca command line tool
runtime/                 the C17 ORC runtime embedded into generated projects
runtime/test/            C runtime tests
examples/                example programs
docs/LANGUAGE.md         the language manual
```

## License

Apache License 2.0 - see [LICENSE](LICENSE).
