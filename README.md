# lsaca

lsaca is a small, statically typed programming language with Rust-like syntax and
semantics, implemented in Rust. Source code is compiled to C17 and built into a
native executable with the system C compiler. Memory is managed with **ORC**
(Ownership and Reference Counting): Rust-style ownership and move checking
decides when a value may be used, automatic reference counting (ARC, in the
spirit of Nim's ARC) frees heap values when their last owner is released, and a
trial-deletion cycle collector reclaims reference cycles. **Drop** hooks run
deterministically when an owner releases a value. There is no tracing garbage
collector and no manual `free` calls; references (`&T`/`&mut T`) are checked
by a conservative borrow checker.

This repository is a complete rewrite of an earlier C17 implementation. The
pipeline, the runtime, the memory model, the grammar and the back end were
redesigned rather than ported.

The current release is **1.5.0**. It adds a real **generic and trait
system**, **lazy iterators** and a builtin **`HashMap<K, V>`**:

- Generic `struct`, `enum`, `fn` and `impl` blocks with parameters and bounds
  (including the newly supported `Iterator<T>` and `Fn(A) -> B` bounds,
  turbofish arguments and `Self` in associated items). Instantiations are
  checked lazily, per concrete type, so a generic function is only required to
  type check for the argument types it is actually used with.
- Traits with declaration-only methods and `impl Trait<Args> for Type` blocks,
  inherent-method-first dispatch and the usual conflicts (`E0046`, `E0407`,
  `E0053`, `E0119`, `E0034`). Trait objects and default bodies are not
  supported.
- Lazy iterator adapters - `iter`, `map`, `filter`, `take`, `skip`,
  `enumerate`, `collect`, `fold`, `count`, `sum`, `any`, `all` and `find` -
  implemented in a prelude written in lsaca itself, plus `for` loops over user
  types that implement `Iterator<T>`.
- A builtin `HashMap<K, V>` with `insert`, `get`, `get_with`,
  `contains_key`, `remove`, `len`, `is_empty`, `clear`, `keys` and `values`,
  deterministic hashing, content equality and the same ORC ownership rules as
  the other heap types.

Version 1.5.0 is **source compatible** with 1.4.0: existing programs keep
working; the release fixes a long list of defects found by the new feature
tests (nested tuples, `Option<scalar>` struct fields, function values stored in
aggregates, aggregate equality, `Self::new`, `for`-loop return slots, HashMap
helper dependencies and trait dispatch between impls) and improves the
performance of iterator, HashMap and generic workloads (see
[benchmarks](#benchmarks)). See the [release notes](#release-history) and the
[roadmap](docs/language/16-roadmap.md).

## Highlights

- **Rust-like language**: `fn`, `let`/`let mut`, `struct`, `enum`,
  `impl`, `match` with exhaustiveness checking, `Option`, `Result`,
  `Vec<T>`, `Rc<T>`, `Box<T>`, tuples, closures, `if let`/`while let`,
  `for` loops, compound assignment and `print!`/`println!` macros. Blocks,
  `if` and `match` are expressions.
- **Generics and traits**: generic `struct`/`enum`/`fn`/`impl` blocks with
  type parameters, trait bounds, multiple `impl Trait<Args> for Type` blocks,
  inherent-method-first dispatch, `Self` in associated items and turbofish
  arguments. Instantiations are checked lazily per concrete type. See
  [docs/language/17-generics.md](docs/language/17-generics.md) and
  [docs/language/18-traits.md](docs/language/18-traits.md).
- **Lazy iterators**: `Vec<T>.iter()` and the adapters `map`, `filter`, `take`,
  `skip`, `enumerate`, `collect`, `fold`, `count`, `sum`, `any`, `all` and
  `find`, implemented generically in a prelude written in lsaca itself; `for`
  loops accept any type implementing `Iterator<T>`. See
  [docs/language/19-iterators.md](docs/language/19-iterators.md).
- **Builtin `HashMap<K, V>`**: `insert`, `get`, `get_with`, `contains_key`,
  `remove`, `len`, `is_empty`, `clear`, `keys` and `values`, with
  deterministic hashing, content equality and ORC ownership rules. See
  [docs/language/20-hashmap.md](docs/language/20-hashmap.md).
- **Ownership and moves**: automatically derived `Copy` and `Clone`, move-only
  heap types, deterministic `Drop` hooks, move checking (use after move, moves
  out of projections, moves while borrowed) and the general `clone()` escape
  hatch - all without lifetime annotations. See
  [docs/language/11-ownership-and-moves.md](docs/language/11-ownership-and-moves.md).
- **Memory management without a tracing GC**: every heap value is reference
  counted (strings, vectors, `Box` payloads, `Rc` cells, closure
  environments, and the structs/enums that own them). Owning a value retains
  it, releasing it decrements the count, and the last owner frees it
  deterministically; `Drop` hooks run first. Cycles created through `Rc<T>`
  are reclaimed automatically by a Bacon-Rajan trial-deletion cycle collector.
  There is no manual `free` and no use-after-free.
- **References without ownership**: `&T` and `&mut T` borrow a place without
  taking ownership or changing a reference count. A conservative borrow checker
  enforces the usual aliasing rule (many shared borrows or one mutable borrow at
  a time) with non-lexical, last-use lifetimes; parameters and return values
  work with lifetime elision, and match ergonomics binds `&T` fields when
  matching a reference. Mutable references to `String`, `Vec<T>`, `Rc<T>`
  and `Box<T>` are allowed since 1.4.0.
- **Fallible code without ceremony**: postfix `?` works on `Option` and
  `Result`, returns from the enclosing function with the same return kind, and
  composes with the standard combinators `Option::map`/`map_or`/`and_then`/
  `filter`/`ok_or` and `Result::map`/`map_err`/`and_then`.
- **Static typing with local inference**: annotations are optional where the type
  follows from the initializer; numeric literals default to `i64`/`f64`.
- **Formatting controls**: format strings accept the Rust-style
  `[[fill]align][sign][0][width][.precision][?]` specification, so
  `{:>8}`, `{:*^9}`, `{:+}`, `{:08}`, `{:08.2}`, `{:.3}` and `{:?}` all
  work; invalid specifications are reported at compile time.
- **C17 back end, built directly**: the compiler emits a small portable C
  project (the program plus a vendored runtime) and compiles and links it with
  the system C compiler. `lsaca emit-c` writes the whole project, including a
  complete `CMakeLists.txt`, so it can be inspected or embedded in another
  build; `--cmake` makes `run`/`build` use that CMake project instead.
- **Fast, cached builds**: builds are cached by content hash, and the direct C
  backend skips CMake, so a memoised rebuild of an unchanged program takes tens
  of milliseconds instead of a second. Runtime objects are compiled once per
  profile and shared by every program.
- **Pure Rust compiler, no third-party crates**: the compiler uses only the Rust
  standard library and forbids unsafe code. The runtime is plain C17.
- **Documented and measured**: a detailed 21-chapter language manual under
  [docs/](docs/README.md), published as a static documentation site, plus
  cross-language benchmarks (compile time, runtime and peak memory) against C,
  Rust, Python and Node.js with the raw data checked in.
- **Tested end to end**: 509 compiler unit tests, 9 end-to-end test functions
  that compile and run 99 real programs, 17 CLI tests, a 10,200-check C runtime
  suite and a 100-check runtime smoke script; CI runs the test matrix on Linux,
  macOS and Windows, plus AddressSanitizer and UndefinedBehaviorSanitizer jobs.

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
    |  prelude          generic iterator library, written in lsaca
    |  type checker     typed HIR: name resolution, inference, generic
    |                   instantiation, trait dispatch, exhaustiveness,
    |                   constant evaluation, ? desugaring
    |  move checker     Copy/Clone classification, move and drop analysis
    |  borrow checker   NLL borrow checking and reference safety
    |  C code generator main.c
    v
 C17 project  +  runtime (5 C files, 2 headers, embedded in the compiler)
    |
    +--> system C compiler (default, no CMake) ----> native executable
    |
    +--> CMake + C compiler (--cmake) ------------> native executable
```

The generated project is self contained:

```text
main.c               the user program plus generated clone/drop/format helpers
lsaca_rt.h           public runtime ABI
lsaca_rt_internal.h  internal runtime declarations shared by the C files
lsaca_rt.c           reference counting, cycle collection, panics, checked math
lsaca_rt_string.c    String implementation and formatting support
lsaca_rt_vec.c       Vec implementation
lsaca_rt_fmt.c       format-string machinery
lsaca_rt_map.c       HashMap table (open addressing, deterministic)
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

## Ownership and memory management (ORC)

Every value belongs to its binding. Values whose type is `Copy` - integers,
floats, `bool`, `(...)` (unit), tuples of `Copy` values, `Option`/`Result`
of `Copy` values, references, and user structs/enums with only `Copy` fields
and no `Drop` - are copied on use, so the source binding stays usable. All
other values (`String`, `Vec<T>`, `Rc<T>`, `Box<T>`, function values, and
user types that contain one of them or implement `Drop`) move: after a move
the old binding is unusable, and the compiler reports it:

```rust
fn main() {
    let text = String::from("lsaca");
    let moved = text;                 // moves the value
    // println!("{}", text);          // error[E0382]: use of moved value
    println!("{}", moved);
}
```

`.clone()` keeps the original usable. Cloning is shallow for handles and deep
for owned containers: a cloned `String` shares its buffer copy-on-write, a
cloned `Rc<T>` shares the same cell (the reference count goes up), a cloned
`Vec<T>` or `Box<T>` deep-copies its contents, and a user type clones
field-by-field. A type that implements `Drop` cannot be cloned (`E0277`).

Heap payloads are still reference counted, so the runtime frees them when their
last owner is released; ownership only decides *when* release happens and when
`Drop` hooks run. A `Drop` hook runs before the value's fields are released and
before the memory is freed; fields drop in declaration order, locals in reverse
declaration order, and function parameters in reverse order at function exit.
Pending drops are skipped when the program panics or calls `exit`, and no
drops run after `main` returns beyond the normal scope cleanup.

The builtin `Rc<T>` type is the exception to plain ownership: it is a shared,
mutable heap cell whose handles all alias the same payload, so a program can
build cyclic structures - for example a node whose `next` field points back at
the cell that contains it. To reclaim those, ORC has two parts:

1. automatic reference counting (ARC), which frees acyclic data as soon as its
   last handle is released, deterministically and without pauses;
2. a trial-deletion cycle collector (the Bacon-Rajan mark-gray / scan /
   collect-white algorithm), which reclaims cyclic garbage once no handles
   outside the cycle remain. Reachable cycles are never collected. `Drop` hooks
   of collected values run when the collector reclaims them.

The collector runs automatically once enough candidate objects accumulate; the
threshold is internal and not user-visible, and there is no `collect` function
in the language. A final collection runs during process shutdown, before the
leak report. A shared counter is written like this:

```rust
fn main() {
    let counter = Rc::new(0);
    let copy = counter.clone();      // clone the handle, do not move it
    copy.set(copy.get() + 1);
    println!("{}", counter.get());   // prints 1
}
```

References are the second way to use a value. `&value` creates a non-owning
view: no reference count is changed, no copy is made, and the referent is
released by whoever owns it. The borrow checker polices aliasing (a `&mut`
cannot coexist with any other borrow of the same place, and a borrow cannot
outlive its referent) and the move checker rejects moving a value while it is
borrowed. See
[docs/language/12-references-and-borrowing.md](docs/language/12-references-and-borrowing.md)
for the rules, the diagnostics and the restrictions (references are not yet
storable in struct/enum/`Vec` fields).

Debug-profile builds - the default for `lsaca run` and `lsaca build` - compile
the runtime with `-DLSACA_DEBUG`, which makes it track reference-counted
allocations and print a leak report at shutdown. To get the same tracking in a
release build, set `LSACA_DEBUG_RC=1` in the environment in which you run the
*compiler* (not the program): `LSACA_DEBUG_RC=1 lsaca build -r program.ls`. The
compiler then defines `-DLSACA_DEBUG_RC` for the C compilation, which turns the
tracking on in every profile; for an emitted CMake project the equivalent switch
is `-DLSACA_DEBUG_RC=ON` at configure time. Cyclic garbage is collected before the
report, so the report only counts genuinely leaked allocations (for example
acyclic data that the compiler failed to release); it indicates a
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
syntax, semantics, the standard library, ownership and memory rules and the
deliberate deviations from Rust. See [examples/](examples/) for runnable
programs, including [examples/ownership.ls](examples/ownership.ls) (moves,
`.clone()` and `Drop`).

Release 1.2 added the postfix `?` operator and the Option/Result combinators.
`?` unwraps `Some`/`Ok` and returns early on `None`/`Err`; the enclosing
function must return the same kind (`Option` or `Result`), and the error type
must match exactly (there is no `From` conversion):

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

Release 1.3 added references. A `&` borrow reads a place without copying it, a
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

Release 1.4 added ownership, `.clone()` and `Drop`. The example below shows
the three at once:

```rust
struct Resource { name: String }

impl Resource {
    fn new(name: String) -> Resource { Resource { name: name } }
}

impl Drop for Resource {
    fn drop(&mut self) { println!("releasing {}", self.name); }
}

fn consume(resource: Resource) {
    println!("consuming {}", resource.name);
}

fn main() {
    let original = String::from("lsaca");
    let copy = original.clone();          // keeps both usable
    println!("original = {} copy = {}", original, copy);

    let first = Resource::new(String::from("first"));
    consume(first);                       // moved into consume, dropped there

    let second = Resource::new(String::from("second"));
    println!("second still alive: {}", second.name);  // dropped at end of main
}
```

The full program is [examples/ownership.ls](examples/ownership.ls).

Release 1.5 added user-defined generics and traits, lazy iterators and a builtin
`HashMap`:

```rust
struct Pair<T> { first: T, second: T }

trait Describe {
    fn describe(&self) -> String;
}

impl<T> Pair<T> {
    fn new(first: T, second: T) -> Pair<T> {
        Pair { first: first, second: second }
    }
}

impl Describe for Pair<i64> {
    fn describe(&self) -> String {
        format!("({}, {})", self.first, self.second)
    }
}

fn main() {
    let pair = Pair::new(1, 2);
    println!("{}", pair.describe());                    // (1, 2)

    let total = vec![1, 2, 3, 4, 5]
        .iter()
        .filter(|value| *value % 2 == 0)
        .map(|value| value * 10)
        .sum();
    println!("{}", total);                              // 60

    let mut counts: HashMap<String, i64> = HashMap::new();
    counts.insert(String::from("lsaca"), 2);
    counts.insert(String::from("rust"), 3);
    println!("{:?} {}", counts.get("lsaca"), counts.keys().len());  // Some(2) 2
}
```

The full iterator examples are in [examples/iterators.ls](examples/iterators.ls).

## Documentation

The manual is split into one detailed page per topic under [docs/](docs/):

- [docs/README.md](docs/README.md) - the manual hub: overview, reading order and
  a table linking every chapter.
- [docs/language/01-getting-started.md](docs/language/01-getting-started.md)
  through
  [docs/language/21-benchmarks.md](docs/language/21-benchmarks.md) - getting
  started, program structure, lexical structure, types, the memory model,
  variables and mutability, operators, statements and control flow, functions,
  structs/enums/`impl`, ownership and moves, references and borrowing, pattern
  matching, closures, the standard library (including `?` and the combinators),
  the roadmap with the deviations from Rust, generics, traits, iterators,
  `HashMap`, and the benchmarks.
- [docs/language/17-generics.md](docs/language/17-generics.md) and
  [docs/language/18-traits.md](docs/language/18-traits.md) - user-defined
  generics, trait bounds, trait declarations and implementations, dispatch
  rules and the new diagnostics (`E0107`, `E0277`, `E0034`, `E0046`, `E0119`).
- [docs/language/19-iterators.md](docs/language/19-iterators.md) and
  [docs/language/20-hashmap.md](docs/language/20-hashmap.md) - lazy iterator
  adapters implemented in the lsaca prelude, `for` over iterators, and the
  builtin hash map with its ownership rules.
- [docs/language/11-ownership-and-moves.md](docs/language/11-ownership-and-moves.md)
  - the ownership model: `Copy` versus move-only types, moves, `.clone()`,
  the `Drop` trait, move diagnostics and the differences from Rust.
- [docs/language/12-references-and-borrowing.md](docs/language/12-references-and-borrowing.md)
  - the reference types, borrow rules, lifetime elision, match ergonomics and
  the borrow-checker diagnostics.
- [docs/language/16-roadmap.md](docs/language/16-roadmap.md) - the deliberate
  deviations from Rust and the verified limitations of the current release.
- [docs/language/21-benchmarks.md](docs/language/21-benchmarks.md) - benchmark
  methodology, per-workload analysis and the measured numbers.

The same pages are published as a static site with GitHub Pages at
<https://menghuihuan44.github.io/lsaca/> (built by
[.github/workflows/pages.yml](.github/workflows/pages.yml) from the Markdown
sources with [scripts/docsite.py](scripts/docsite.py)).

## Benchmarks

`python scripts/bench.py` runs fifteen runtime workloads and one compile-only
scaling workload in lsaca and in C, Rust, Python and Node.js, verifies each
output checksum, and records compile time, runtime median and peak working set.
lsaca programs are built through the normal cached `lsaca build -r` pipeline
(the direct C backend); where the C toolchain is unavailable the harness can
fall back to a labelled direct-compile mode.

The latest measured results for the 1.5.0 compiler are in
[benchmarks/results/latest.md](benchmarks/results/latest.md): compile-time,
runtime and peak-memory tables for fifteen runtime workloads plus a compile-only
scaling workload, compared with C, Rust, Python and Node. The same tables,
per-workload analysis, methodology and caveats are in
[docs/language/21-benchmarks.md](docs/language/21-benchmarks.md); raw JSON data
is in [benchmarks/results/](benchmarks/results/).

Headline numbers of the 1.5.0 run (Windows 11, Core i7-1260P, gcc 15.2.0, rustc
1.99, median of 7 runtime runs and 3 compile runs):

| workload | lsaca 1.5.0 | C (gcc -O2) | Rust (rustc -O) |
| --- | ---: | ---: | ---: |
| `fib` (runtime) | 20.4 ms | 21.5 ms | 10.2 ms |
| `loop_sum` | 8.1 ms | 9.2 ms | 10.0 ms |
| `string_build` | 27.0 ms | 10.0 ms | 56.2 ms |
| `vec_memory` (runtime / peak) | 18.6 ms / 18.8 MiB | 15.9 ms / 18.8 MiB | 14.5 ms / 18.9 MiB |
| `sort_i64` | 28.0 ms | 29.8 ms | 24.7 ms |
| `sieve` | 24.0 ms | 22.9 ms | 23.9 ms |
| `mandelbrot` | 26.4 ms | 27.6 ms | 26.9 ms |
| `iterator_pipeline` | 27.8 ms | 12.0 ms | 14.5 ms |
| `hashmap_wordcount` | 46.4 ms | 24.3 ms | 43.4 ms |
| `generic_stack` | 14.2 ms | 17.4 ms | 16.1 ms |
| `fn_pipeline` | 9.9 ms | 11.0 ms | 9.0 ms |
| `binary_trees` (runtime / peak) | 534.4 ms / 101.2 MiB | 17.7 ms / 7.7 MiB | 19.4 ms / 7.7 MiB |
| `binary_trees_refs` (runtime / peak) | 34.1 ms / 19.9 MiB | 15.1 ms / 7.7 MiB | 17.7 ms / 7.7 MiB |
| cold build, typical workload | 0.74-1.05 s | 0.19-0.30 s | 0.20-0.39 s |
| cached build, typical workload | 13-20 ms | - | - |
| `compile_large` (4,511 lines), cold / cached | 3.14 s / 227 ms | - | - |

The direct C backend keeps compile times within a factor of about three to four
of plain gcc: the lsaca front end and code generator contribute a few hundred
milliseconds, and the rest is the system C compiler compiling `main.c`. At run
time most workloads are at parity with C - and some are faster, because the
generated C uses the same algorithms without runtime bounds checks in the
optimised build. The remaining gaps are explained per workload in the
benchmarks page: `string_build` beats Rust thanks to an amortised growable
buffer with append fast paths; `iterator_pipeline` and `hashmap_wordcount` pay
for lazy adapter calls and generic-key hashing; and by-value `binary_trees`
still copies the whole tree when it is summed, which is why the
`binary_trees_refs` variant - traversing through borrowed references - is 15.7x
faster and uses five times less memory.

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
  the current directory instead of failing on an unwritable `TEMP`.
- **`no C compiler found`**: install GCC or Clang (MSYS2 UCRT64 on Windows) and
  put it on `PATH`, or set `LSACA_CC` to the compiler executable.
- **`CMake was not found on PATH`** or **`no build generator found`**: these
  errors only occur with `--cmake` (or when you build an emitted project
  yourself). Install CMake 3.16+ and Ninja (recommended) or `mingw32-make`, or
  drop `--cmake` - the default backend needs neither.

## Testing

```sh
cargo test --workspace          # 509 unit + 9 end-to-end (99 programs) + 17 CLI tests
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

The end-to-end tests in `crates/lsaca/tests/` compile and run the programs under
`crates/lsaca/tests/programs/` and `examples/`, and are skipped when no C
compiler is available. The C runtime has its own 10,200-check suite
(`runtime/test/main.c`, including ORC stress and HashMap tests) plus a 100-check
Windows smoke script (`runtime/test/run_smoke.ps1`). Every generated project is
compiled without warnings under `-Wall -Wextra -Wpedantic -Werror` in both
profiles by [scripts/check_generated_c.py](scripts/check_generated_c.py), which
also verifies byte-identical output across runs and runs every program; CI also
runs the C runtime in release, debug and leak-tracking modes, and checks the
documentation site with `scripts/docsite.py --check`. The complete CI definition
is in [.github/workflows/ci.yml](.github/workflows/ci.yml).

CI also builds emitted projects through CMake. In a confined environment where
child processes cannot spawn CMake/Ninja, set `LSACA_SKIP_CMAKE_TESTS=1` to skip
only that CLI test; the dedicated CI job still covers the CMake backend.

## Release history

- **1.5.0** - user-defined generics and traits: generic `struct`/`enum`/`fn`/
  `impl` blocks with type parameters and trait bounds, multiple
  `impl Trait for Type` blocks, inherent-method-first dispatch, `Self` in
  associated items, turbofish arguments and lazy per-instantiation checking;
  lazy iterator adapters (`map`, `filter`, `take`, `skip`, `enumerate`,
  `collect`, `fold`, `count`, `sum`, `any`, `all`, `find`) implemented
  generically in a prelude written in lsaca, with `for` over any
  `Iterator<T>`; builtin `HashMap<K, V>` with deterministic slot order and ORC
  ownership; aggregate `==` for vectors, options, results, tuples, structs,
  enums, references, boxes and maps; fixes for nested tuples, `Option<scalar>`
  struct fields, function values stored in aggregates, `assert_eq!` on boxes,
  `Self::new`/`Self { .. }` and `pub` fields/variants; new `iterator_pipeline`,
  `hashmap_wordcount`, `generic_stack` and `fn_pipeline` benchmarks. Source
  compatible with 1.4.0.
- **1.4.0** - ownership and move semantics with automatically derived `Copy`
  and `Clone`, `.clone()`, the `Drop` trait with deterministic drop order,
  mutable references to `String`/`Vec<T>`/`Rc<T>`/`Box<T>`, the move
  checker (`E0382`, `E0505`, `E0507`, `E0509`) and clone restrictions
  (`E0277`); ORC remains the memory manager underneath. Source-breaking for
  programs that relied on implicit copying. Removing implicit copies made the
  tree benchmark 1.8x faster with lower peak memory; this release also fixes
  short-circuit lowering for `||`, `Vec::extend` on copy-element vectors, a
  `Result::clone().unwrap_or` leak, and double drops when a whole value is
  bound from a `match` scrutinee.
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
crates/lsaca/            the compiler library (lexer, parser, type checker, borrow
                         checker, move checker, C codegen, driver) plus the prelude
                         (generic iterator library written in lsaca)
crates/lsaca/tests/      end-to-end tests and test programs
crates/lsaca-cli/        the lsaca command line tool
runtime/                 the C17 ORC runtime embedded into generated projects
                         (lsaca_rt.c, lsaca_rt_string.c, lsaca_rt_vec.c,
                         lsaca_rt_fmt.c, lsaca_rt_map.c, lsaca_rt.h,
                         lsaca_rt_internal.h)
runtime/test/            C runtime tests (10,200-check suite and Windows smoke script)
examples/                example programs
benchmarks/              benchmark programs in five languages, harness and recorded results
scripts/                 benchmark harness (bench.py), generated-C checker
                         (check_generated_c.py) and documentation site generator
docs/                    the multi-page language manual (docs/README.md is the hub)
.github/workflows/       CI, benchmark smoke and GitHub Pages workflows
```

## License

Apache License 2.0 - see [LICENSE](LICENSE).
