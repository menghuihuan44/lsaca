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

The current release is **1.1.0**, which adds the builtin `Box<T>` type and
format-specifier widths/precision on top of the 1.0 language, fixes or-patterns
and match guards, and makes a non-`bool` condition (in `if`, `while`, `&&`, `||`
and match guards) a compile-time error instead of silently accepting integers.

## Highlights

- **Rust-like language**: `fn`, `let`/`let mut`, `struct`, `enum`, `impl`,
  `match` with exhaustiveness checking, `Option`, `Result`, `Vec<T>`, `Rc<T>`,
  `Box<T>`, tuples, closures, `if let`/`while let`, `for` loops, compound
  assignment and `print!`/`println!` macros. Blocks, `if` and `match` are
  expressions.
- **Static typing with local inference**: annotations are optional where the type
  follows from the initializer; numeric literals default to `i64`/`f64`.
- **ORC memory management**: every owned value is reference counted (strings,
  vectors, closures, and structs/enums that own data). Assignment, argument
  passing, capture and return copy handles; no use-after-free is possible.
  Shared mutable state is written with the builtin `Rc<T>` cell, and reference
  cycles created through `Rc<T>` are reclaimed automatically by a Bacon-Rajan
  trial-deletion cycle collector. `Box<T>` provides owned indirection with
  value semantics for recursive types.
- **Formatting controls**: format strings accept the Rust-style
  `[[fill]align][sign][0][width][.precision][?]` specification, so
  `{:>8}`, `{:*^9}`, `{:+}`, `{:08}`, `{:08.2}`, `{:.3}` and `{:?}` all
  work; invalid specifications are reported at compile time.
- **C17 + CMake back end**: the compiler emits a small portable C project
  (`main.c`, the runtime and `CMakeLists.txt`) and builds it. `lsaca emit-c`
  keeps the generated project so it can be inspected or embedded in another
  build.
- **Pure Rust compiler, no third-party crates**: the compiler uses only the Rust
  standard library and forbids unsafe code. The runtime is plain C17.
- **Documented and measured**: a detailed multi-page language manual under
  [docs/](docs/README.md), published as a static documentation site, plus
  cross-language benchmarks (compile time, runtime and peak memory) against C,
  Rust, Python and Node.js with the raw data checked in.
- **Tested end to end**: 160 compiler unit tests, an end-to-end suite that
  compiles and runs real programs, 13 CLI tests, a 411-check C runtime suite and
  a 100-check runtime smoke script; CI runs the test matrix on Linux, macOS and
  Windows.

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

[docs/README.md](docs/README.md) is the entry point to the language manual: full
syntax, semantics, the standard library, memory rules and the deliberate
deviations from Rust. See [examples/](examples/) for runnable programs.

Two 1.1 features are worth showing here. `Box<T>` is owned heap indirection with
value semantics (`Box::new`, `.get()`, `.set()`); unlike `Rc<T>` it is not
shared, and it provides the indirection needed for recursive types. Format macros
accept widths, alignment, fill characters, signs, zero padding and precision:

```rust
fn main() {
    let name = "lsaca";
    println!("[{:*^9}]", name);       // [**lsaca**]
    println!("[{:08.2}]", 3.141592);  // [00003.14]
    let boxed = Box::new(7);
    println!("{}", boxed.get());      // 7
}
```

## Documentation

The manual is split into one detailed page per topic under [docs/](docs/):

- [docs/README.md](docs/README.md) - the manual hub: overview, reading order and
  a table linking every chapter.
- [docs/language/01-getting-started.md](docs/language/01-getting-started.md)
  through
  [docs/language/14-roadmap.md](docs/language/14-roadmap.md) - getting started,
  program structure, lexical structure, types, the ORC memory model (`Rc<T>`,
  `Box<T>` and cycle collection), variables and mutability, operators,
  statements and control flow, functions, structs/enums/`impl`, pattern
  matching, closures, the standard library and formatting, and the roadmap with
  the deviations from Rust.
- [docs/language/15-benchmarks.md](docs/language/15-benchmarks.md) - benchmark
  methodology, per-workload analysis and the measured numbers.

The same pages are published as a static site with GitHub Pages at
<https://menghuihuan44.github.io/lsaca/> (built by
[`.github/workflows/pages.yml`](.github/workflows/pages.yml) from the Markdown
sources with `scripts/docsite.py`).

## Benchmarks

`python scripts/bench.py` runs four workloads in lsaca and in C, Rust, Python and
Node.js, verifies each output checksum, and records compile time, runtime median
and peak working set. lsaca programs are built through the normal cached
`lsaca build` pipeline (official mode); where CMake is unavailable the harness
falls back to `emit-c` plus a direct `gcc` invocation and records that mode in
the results.

Measured on Windows 11 with a 12th Gen Intel Core i7-1260P (16 logical CPUs,
15.6 GiB RAM), gcc 15.2.0, rustc 1.99.0, Python 3.14.4 and Node.js 24.14.1;
median of 7 runs after one warm-up, 3 compile repetitions. Full methodology and
caveats are in [docs/language/15-benchmarks.md](docs/language/15-benchmarks.md);
raw data is in [benchmarks/results/](benchmarks/results/).

Runtime, median milliseconds (lower is better):

| program | lsaca | C (gcc -O2) | Rust (rustc -O) | Python 3 | Node.js |
| --- | ---: | ---: | ---: | ---: | ---: |
| fib (recursive, n=30, 10 calls) | 30.6 | 30.6 | 15.7 | 1082.3 | 241.4 |
| loop_sum (8,000,000 additions) | 11.4 | 10.9 | 12.0 | 987.1 | 165.3 |
| string_build (1,000,000 appends) | 212.9 | 14.8 | 76.3 | 431.1 | 231.5 |
| vec_memory (1,000,000 structs) | 86.1 | 18.4 | 24.9 | 535.7 | 241.2 |

Compile time, seconds (lower is better): lsaca cold build 2.8-3.3, lsaca cached
build 0.017-0.019, `gcc -O2` 0.25-0.31, `rustc -O` 0.26-0.36. The cold lsaca
number includes CMake configure plus the C build; the cached number is what a
program pays after it has been built once.

Peak working set, MiB (lower is better): lsaca 4.0 on fib and loop_sum, 10.6 on
string_build and 35.1 on vec_memory; C 4.0/4.0/7.3/18.8; Rust
4.1/4.1/7.3/18.9; Python 11.0/11.0/74.1/142.2; Node.js
43.0/45.7/89.3/121.4 (same program order). Numeric loops reach C parity in this
run, while the vector and string workloads pay for per-element reference
counting, copy-on-write appends and value-semantics copies of vectors; these are
the next code generation optimizations, documented on the benchmark page.

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
cargo test --workspace          # 160 unit + 5 end-to-end + 13 CLI tests
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

The end-to-end tests in `crates/lsaca/tests/` compile and run the programs under
`crates/lsaca/tests/programs/` and `examples/`, and are skipped when no C
compiler is available. The C runtime has its own 411-check suite
(`runtime/test/main.c`) plus a 100-check Windows smoke script
(`runtime/test/run_smoke.ps1`). Generated projects are expected to compile
without warnings under `-Wall -Wextra`; CI additionally builds them with
`-Werror`, runs the C runtime in release, debug and leak-tracking modes, and
checks the documentation site with `scripts/docsite.py --check`. The complete CI
definition is in [.github/workflows/ci.yml](.github/workflows/ci.yml).

## Repository layout

```text
crates/lsaca/            the compiler library (lexer, parser, type checker, C codegen, driver)
crates/lsaca/tests/      end-to-end tests and test programs
crates/lsaca-cli/        the lsaca command line tool
runtime/                 the C17 ORC runtime embedded into generated projects
runtime/test/            C runtime tests (411-check suite and Windows smoke script)
examples/                example programs
benchmarks/              benchmark programs in five languages, harness and recorded results
scripts/                 benchmark harness (bench.py) and documentation site generator
docs/                    the multi-page language manual (docs/README.md is the hub)
.github/workflows/       CI, benchmark smoke and GitHub Pages workflows
```

## License

Apache License 2.0 - see [LICENSE](LICENSE).
