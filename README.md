# lsaca

lsaca is a small, statically typed programming language with Rust-like syntax and
semantics, implemented in Rust. Source code is compiled to C17 and built into a
native executable with the system C compiler. Memory is managed with **ORC**
(Ownership and Reference Counting): automatic reference counting (ARC, in the
spirit of Nim's ARC) plus a trial-deletion cycle collector. There is no tracing
garbage collector and no manual `free` calls; references (`&T`/`&mut T`) are
checked by a small borrow checker, but ownership and move rules are not part of
the language.

This repository is a complete rewrite of an earlier C17 implementation. The
pipeline, the runtime, the memory model, the grammar and the back end were
redesigned rather than ported.

The current release is **1.3.0**. It adds **references**: the shared `&T` and
mutable `&mut T` types, the `&place` / `&mut place` borrow expressions, `*x`
dereference, reference parameters, reference return values with lifetime
elision, auto-deref through references for field/index/method access, and match
ergonomics. A conservative NLL-lite borrow checker rejects aliasing violations,
and all of it runs on top of the unchanged ORC runtime: references are
non-owning, never touch reference counts, and there is still no move semantics.
The 1.3.0 language is source compatible with 1.2.0.

## Highlights

- **Rust-like language**: `fn`, `let`/`let mut`, `struct`, `enum`, `impl`,
  `match` with exhaustiveness checking, `Option`, `Result`, `Vec<T>`, `Rc<T>`,
  `Box<T>`, tuples, closures, `if let`/`while let`, `for` loops, compound
  assignment and `print!`/`println!` macros. Blocks, `if` and `match` are
  expressions.
- **Fallible code without ceremony**: postfix `?` works on `Option` and
  `Result`, returns from the enclosing function with the same return kind, and
  composes with the standard combinators `Option::map`/`map_or`/`and_then`/
  `filter`/`ok_or` and `Result::map`/`map_err`/`and_then`.
- **Static typing with local inference**: annotations are optional where the type
  follows from the initializer; numeric literals default to `i64`/`f64`.
- **ORC memory management**: every owned value is reference counted (strings,
  vectors, closures, and structs/enums that own data). Assignment, argument
  passing, capture and return copy handles; no use-after-free is possible.
  Shared mutable state is written with the builtin `Rc<T>` cell, and reference
  cycles created through `Rc<T>` are reclaimed automatically by a Bacon-Rajan
  trial-deletion cycle collector. `Box<T>` provides owned indirection with
  value semantics for recursive types.
- **References without ownership**: `&T` and `&mut T` borrow a place without
  taking ownership or changing a reference count. A conservative borrow checker
  enforces the usual aliasing rule (many shared borrows or one mutable borrow at
  a time) with non-lexical, last-use lifetimes; parameters and return values
  work with lifetime elision, and match ergonomics binds `&T` fields when
  matching a reference.
- **Formatting controls**: format strings accept the Rust-style
  `[[fill]align][sign][0][width][.precision][?]` specification, so
  `{:>8}`, `{:*^9}`, `{:+}`, `{:08}`, `{:08.2}`, `{:.3}` and `{:?}` all
  work; invalid specifications are reported at compile time.
- **C17 back end, built directly**: the compiler emits a small portable C
  project (the program plus a vendored runtime) and compiles and links it with
  the system C compiler. `lsaca emit-c` writes the whole project, including a
  complete `CMakeLists.txt`, so it can be inspected or embedded in another
  build; `--cmake` makes `run`/`build` use that CMake project instead.
- **Fast, cached builds**: a cold build of a benchmark program takes 1.15-1.72 s
  on the reference machine and a cached rebuild takes 22-45 ms, because the
  direct C backend skips CMake and runtime objects are compiled once per profile
  and shared by every program.
- **Pure Rust compiler, no third-party crates**: the compiler uses only the Rust
  standard library and forbids unsafe code. The runtime is plain C17.
- **Documented and measured**: a detailed multi-page language manual under
  [docs/](docs/README.md), published as a static documentation site, plus
  cross-language benchmarks (compile time, runtime and peak memory) against C,
  Rust, Python and Node.js with the raw data checked in.
- **Tested end to end**: 292 compiler unit tests, 7 end-to-end tests that compile
  and run real programs, 17 CLI tests, a 411-check C runtime suite and a
  100-check runtime smoke script; CI runs the test matrix on Linux, macOS and
  Windows, plus AddressSanitizer and UndefinedBehaviorSanitizer jobs.

## Requirements

- Rust 1.99 or newer (`cargo`, 2024 edition).
- A C17 compiler: GCC or Clang. On Windows, MSYS2's `gcc` (UCRT64 environment)
  is detected automatically, and `LSACA_CC` can point at any other compiler.
- Optional: CMake 3.16 or newer plus a build generator (Ninja is used when
  available, otherwise CMake falls back to MinGW Makefiles on Windows and Unix
  Makefiles elsewhere). CMake is only needed when you build with `--cmake` or
  build an emitted project yourself.

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

Global option `--cmake` makes `run` and `build` go through the generated CMake
project instead of the built-in direct C backend; without it the compiler needs
no CMake. It is rejected (exit code `2`) by `check`, `emit-c`, `version` and
`help`. Other options: `-r, --release` (optimized build; arithmetic overflow
wraps instead of panicking in release mode), `-o, --out-dir <dir>` (output
directory for `build` and `emit-c`), `-v, --verbose` (show external commands and
the cache location) and `--keep` (print the location of the temporary build
tree).

Exit codes: `0` success, `1` compile or build error, `2` command line usage
error, `101` a panic inside the compiled program.

## How compilation works

```text
 source (.ls)
    |  lexer            tokens
    |  parser           AST
    |  type checker     typed HIR: name resolution, inference,
    |                   exhaustiveness, constant evaluation, ? desugaring
    |  C code generator main.c
    v
 C17 project  +  runtime (4 C files, 2 headers, embedded in the compiler)
    |
    +--> system C compiler (default, no CMake) ----> native executable
    |
    +--> CMake + C compiler (--cmake) ------------> native executable
```

The generated project is self contained:

```text
main.c               the user program plus generated copy/release/format helpers
lsaca_rt.h           public runtime ABI
lsaca_rt_internal.h  internal runtime declarations shared by the C files
lsaca_rt.c           reference counting, cycle collection, panics, checked math
lsaca_rt_string.c    String implementation and formatting support
lsaca_rt_vec.c       Vec implementation
lsaca_rt_fmt.c       format-string machinery
CMakeLists.txt       portable build description (Debug: -g -O0 -DLSACA_DEBUG,
                     Release: -O2 -DNDEBUG)
```

Builds are cached by content hash (source, generated C, runtime sources,
compiler version and path, build mode and leak-tracking mode), so repeated runs
of unchanged programs are fast. The compiler object files for the runtime are
compiled once per profile and cached under `_runtime/<hash>/` in the cache root;
only `main.c` is recompiled for each program. The cache location is chosen
defensively:

1. `LSACA_CACHE_DIR`, when set;
2. `lsaca-builds` under the system temporary directory, when that path is
   ASCII-only and actually writable;
3. `.lsaca-build-cache` in the current directory.

The fallback keeps builds working in sandboxes that only allow writes inside the
workspace, when the temporary directory is not writable, and when its path is
not ASCII-only (some C toolchains handle non-ASCII paths poorly). The chosen
cache root is probed for writability before it is used, and `-v` prints it and
the C compiler commands. The cache is safe to delete at any time. Built
executables are copied to the requested output directory, so source paths may
contain any characters.

## Memory management (ORC)

Every heap value (String, Vec, closure environment, an `Rc<T>` cell, a
`Box<T>` payload, or a struct/enum value that contains one) is owned by its
holder through a reference count. Copying a value - assignment, argument
passing, capture or return - retains it; the holder releases it when the
variable or temporary goes out of scope. Strings and vectors are copy-on-write,
so mutating one holder never affects another. Function arguments are borrowed
rather than moved: programs never fail to compile because of a move. Unlike
Rust, ORC keeps memory safety but not the move checker.

References are the second way to use a value. `&value` creates a non-owning
view: no reference count is changed, no copy is made, and the referent is
released by whoever owns it. The borrow checker only polices aliasing (a
`&mut` cannot coexist with any other borrow of the same place, and a borrow
cannot outlive its referent); it does not police ownership or moves. See
[docs/language/11-references-and-borrowing.md](docs/language/11-references-and-borrowing.md)
for the rules, the diagnostics and the limitations of 1.3 (references are not
yet storable in struct/enum/`Vec` fields).

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

Debug-profile builds - the default for `lsaca run` and `lsaca build` - compile
the runtime with `-DLSACA_DEBUG`, which makes it track reference-counted
allocations and print a leak report at shutdown. To get the same tracking in a
release build, set `LSACA_DEBUG_RC=1` in the environment in which you run the
*compiler* (not the program): `LSACA_DEBUG_RC=1 lsaca build -r program.ls`. The
compiler then defines `-DLSACA_DEBUG_RC` for the C compilation, which turns the
tracking on in every profile; for an emitted CMake project the equivalent
switch is `-DLSACA_DEBUG_RC=ON` at configure time. Cyclic garbage is collected
before the report, so the report only counts genuinely leaked allocations (for
example acyclic data that the compiler failed to release); it indicates a
compiler/runtime bug, not a user bug, unless the program panicked or called
`exit`. The report is:

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

Release 1.2 added the postfix `?` operator and the Option/Result
combinators. `?` unwraps `Some`/`Ok` and returns early on `None`/`Err`; the
enclosing function must return the same kind (`Option` or `Result`), and the
error type must match exactly (there is no `From` conversion):

```rust
fn half(n: i64) -> Option<i64> {
    if n % 2 == 0 { Some(n / 2) } else { None }
}

fn quarter(n: i64) -> Option<i64> {
    Some(half(half(n)?)?)
}

fn main() {
    println!("{:?} {:?}", quarter(8), quarter(6));   // Some(2) None
    let x: Option<i64> = Some(4);
    println!("{:?}", x.map(|v| v + 1));              // Some(5)
}
```

`Box<T>` is owned heap indirection with value semantics (`Box::new`, `.get()`,
`.set()`); unlike `Rc<T>` it is not shared, and it provides the indirection
needed for recursive types. Format macros accept widths, alignment, fill
characters, signs, zero padding and precision:

```rust
fn main() {
    let name = "lsaca";
    println!("[{:*^9}]", name);       // [**lsaca**]
    println!("[{:08.2}]", 3.141592);  // [00003.14]
    let boxed = Box::new(7);
    println!("{}", boxed.get());      // 7
}
```
Release 1.3 adds references. A `&` borrow reads a place without copying it, a
`&mut` borrow writes through it, and a borrow checker rejects aliasing:

```rust
struct Counter { value: i64 }

impl Counter {
    fn value(&self) -> &i64 { &self.value }
    fn bump(&mut self) { self.value = self.value + 1; }
}

fn total(values: &Vec<i64>) -> i64 {
    let mut sum = 0;
    let mut index = 0;
    while index < values.len() {
        sum = sum + values[index];
        index = index + 1;
    }
    sum
}

fn main() {
    let mut counter = Counter { value: 41 };
    counter.bump();
    let reference = counter.value();
    println!("{}", *reference);   // 42
    let numbers = vec![1, 2, 3];
    println!("{}", total(&numbers)); // 6
}
```

## Documentation

The manual is split into one detailed page per topic under [docs/](docs/):

- [docs/README.md](docs/README.md) - the manual hub: overview, reading order and
  a table linking every chapter.
- [docs/language/01-getting-started.md](docs/language/01-getting-started.md)
  through
  [docs/language/16-benchmarks.md](docs/language/16-benchmarks.md) - getting
  started, program structure, lexical structure, types, the ORC memory model
  (`Rc<T>`, `Box<T>` and cycle collection), variables and mutability, operators,
  statements and control flow, functions, structs/enums/`impl`, pattern
  matching, references and borrowing, closures, the standard library (including
  `?` and the combinators) and the roadmap with the deviations from Rust.
- [docs/language/11-references-and-borrowing.md](docs/language/11-references-and-borrowing.md)
  - the v1.3 reference types, borrow rules, lifetime elision, match ergonomics
  and the borrow-checker diagnostics.
- [docs/language/16-benchmarks.md](docs/language/16-benchmarks.md) - benchmark
  methodology, per-workload analysis and the measured numbers.

The same pages are published as a static site with GitHub Pages at
<https://menghuihuan44.github.io/lsaca/> (built by
[`.github/workflows/pages.yml`](.github/workflows/pages.yml) from the Markdown
sources with `scripts/docsite.py`).

## Benchmarks

`python scripts/bench.py` runs a corpus of workloads in lsaca and in C, Rust,
Python and Node.js, verifies each output checksum, and records compile time,
runtime median and peak working set. lsaca programs are built through the normal
cached `lsaca build` pipeline; where the C toolchain is unavailable the harness
can fall back to a labelled direct-compile mode.

Release 1.3.0 was measured on the reference machine (Windows 11, Core i7-1260P,
gcc 15.2.0). A cold `lsaca build -r` of a benchmark program takes 1.15-1.72 s and
a cached rebuild 22-45 ms; the 4,511-line `compile_large` source takes 3.97 s
cold and 308 ms cached. Scalar loops, sorting and floating-point code are in the
same ballpark as C: `loop_sum` 13.9 ms versus C's 14.2 ms, `vec_memory` 30.7
versus 36.4 (peak 18.8 MiB for both), `matrix_multiply` 17.0 versus 17.1,
`sieve` 39.4 versus 41.5 and `mandelbrot` 49.2 versus 63.9. `sort_i64` (63.1
versus 50.2) and `string_build` (45.2 versus 27.0) remain slower than C.

The new 1.3 benchmark is `binary_trees_refs`, which sums the same 131,071-node
tree as `binary_trees` but traverses it through `&Option<Box<Node>>` references
instead of copying it. It runs in 223 ms with a 35.8 MiB peak, versus 1,217 ms
and 150.1 MiB for the by-value traversal: 5.4x faster and 4.2x less memory, and
9.2x C instead of 47x C. Full tables, per-workload analysis, methodology and
caveats are in [docs/language/16-benchmarks.md](docs/language/16-benchmarks.md);
raw data is in [benchmarks/results/](benchmarks/results/).

## Environment variables

- `LSACA_CC` - C compiler used to build generated programs (default: `gcc`,
  `cc` or `clang` from `PATH`, then common MSYS2 locations on Windows).
- `LSACA_JOBS` - parallel build jobs for the C compiler (with `--cmake`, passed
  to the CMake build instead).
- `LSACA_CACHE_DIR` - build cache location (default: tried under the system
  temporary directory first, then `.lsaca-build-cache` in the current
  directory).
- `LSACA_DEBUG_RC` - read by the compiler: set to `1` (`true`, `on` or `yes`
  also work) to compile the runtime with leak tracking in *every* build
  profile, including release; emitted CMake projects expose the same switch as
  the CMake option `LSACA_DEBUG_RC`.

## Troubleshooting

- **`failed to build <file>: ... (os error 5)` / "Access is denied"**: the
  compiler could not write to its build cache. Run with `-v` to see the chosen
  cache directory, or point `LSACA_CACHE_DIR` at a writable directory. The
  compiler probes the cache location and falls back to `.lsaca-build-cache` in
  the current directory instead of failing on an unwritable `%TEMP%`.
- **`no C compiler found`**: install GCC or Clang (MSYS2 UCRT64 on Windows) and
  put it on `PATH`, or set `LSACA_CC` to the compiler executable.
- **`CMake was not found on PATH`** or **`no build generator found`**: these
  errors only occur with `--cmake` (or when you build an emitted project
  yourself). Install CMake 3.16+ and Ninja (recommended) or `mingw32-make`, or
  drop `--cmake` - the default backend needs neither.

## Testing

```sh
cargo test --workspace          # 292 unit + 7 end-to-end + 17 CLI tests
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

CI also builds emitted projects through CMake. In a confined environment where
child processes cannot spawn CMake/Ninja, set `LSACA_SKIP_CMAKE_TESTS=1` to skip
only that CLI test; the dedicated CI job still covers the CMake backend.

## Release history

- **1.3.0** - shared and mutable references (`&T`, `&mut T`), borrow
  expressions, dereference, reference parameters/returns with lifetime elision,
  auto-deref for field/index/method access, match ergonomics, and a conservative
  NLL-lite borrow checker; C ABI for references; new `binary_trees_refs`
  benchmark; CI sanitizer job. Source compatible with 1.2.0.
- **1.2.0** - postfix `?` on `Option`/`Result` and the Option/Result
  combinators; direct C backend by default with cached runtime objects (`--cmake`
  restores the old pipeline); split runtime (`lsaca_rt_string.c`,
  `lsaca_rt_vec.c`, `lsaca_rt_fmt.c`, `lsaca_rt_internal.h`); faster builds,
  faster vector/string workloads and lower peak memory. Source compatible with
  1.1.0.
- **1.1.0** - builtin `Box<T>`; format-specifier widths, alignment, fill,
  signs, zero padding and precision; correctly implemented or-patterns and
  match guards; non-`bool` conditions became a compile-time error.
- **1.0.0** - first release of the rewrite.

## Repository layout

```text
crates/lsaca/            the compiler library (lexer, parser, type checker, C codegen, driver)
crates/lsaca/tests/      end-to-end tests and test programs
crates/lsaca-cli/        the lsaca command line tool
runtime/                 the C17 ORC runtime embedded into generated projects
                         (lsaca_rt.c, lsaca_rt_string.c, lsaca_rt_vec.c,
                         lsaca_rt_fmt.c, lsaca_rt.h, lsaca_rt_internal.h)
runtime/test/            C runtime tests (411-check suite and Windows smoke script)
examples/                example programs
benchmarks/              benchmark programs in five languages, harness and recorded results
scripts/                 benchmark harness (bench.py) and documentation site generator
docs/                    the multi-page language manual (docs/README.md is the hub)
.github/workflows/       CI, benchmark smoke and GitHub Pages workflows
```

## License

Apache License 2.0 - see [LICENSE](LICENSE).
