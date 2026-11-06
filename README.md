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

The current release is **2.1.0**. It is the Rust-alignment release: numeric
defaults and suffixes, `usize` lengths and indices, `&str` text parameters,
tuple and unit structs, and `mut` parameters now follow Rust, and the standard
library and system interfaces grew the types and combinators that were
missing:

- **Rust-style numeric defaults and suffixes**: integer literals infer `i32`
  and float literals `f64`; every integer type accepts a Rust suffix
  (`i8`, `i16`, `i32`, `i64`, `isize`, `u8`, `u16`, `u32`,
  `u64`, `usize`, plus `f32`/`f64` for floats), suffixed and annotated
  values are range checked at compile time (`128i8` and `let x: u8 = 300`
  are errors), and an unsuffixed literal adapts to the integer type its context
  asks for.
- **`usize` for every length and index**: `Vec`, `String`, slices,
  `HashMap` and the iterator protocol use `usize` for lengths, capacities,
  indices and counts, `find` returns `Option<usize>`, and indexing requires
  `usize` exactly like Rust. `isize` is a complete type as well.
- **Borrowed text in signatures**: read-only text parameters of the standard
  library (and `expect`) take `&str`; a string literal in a `&str`
  context compiles to a zero-allocation static view, `&String`/`&mut String`
  coerce to `&str`, and `String + &str` / `String += &str` replace
  owned-string concatenation.
- **Tuple, newtype and unit structs**: `struct Point(i64, i64);`,
  `struct Meters(i64);` and `struct Marker;` with `.0` access, tuple
  patterns, methods and `Debug` output such as `Point(10, 4)`.
- **`mut` parameters**: `fn f(mut x: i64)` may assign to its parameter
  binding; `mut self` is still rejected.
- **More standard library**: the new `Option::or`/`Option::xor` and
  `Result::unwrap_err` combinators, the iterator adapters `zip`, `chain`,
  `filter_map`, `take_while`,
  `skip_while`, `step_by`, `position`, `last`, `nth`, `reduce`,
  `product`, `inspect` and `for_each`, `str` helpers
  (`chars`, `split_whitespace`, `lines`, `trim_start`, `trim_end`,
  `replace`), `char` ASCII helpers, `Vec::truncate`/`swap`/`contains`,
  and `parse_i64`/`parse_f64` returning `Result<_, ParseIntError>` and
  `Result<_, ParseFloatError>`.
- **Typed system interfaces**: `time::{Duration, Instant, SystemTime}` with
  `sleep`, `io::Error::new`/`other` and `error::Error`,
  `File::read`/`write` returning the byte count, `seek` returning `u64`,
  `fs::Metadata::size()` returning `u64` with `modified`/`file_type`,
  `env::VarError`, `net::Shutdown`, `process::exit` and the `f64`
  constants from `math`.

Version 2.1.0 is not fully source compatible with 2.0.0: unannotated numeric
literals now infer `i32`, lengths and indices are `usize`, standard-library
text parameters are `&str`, `parse_i64`/`parse_f64` return the new dedicated
error types instead of `Result<_, String>`, and `String + String` is no
longer accepted. Each change is reported by a targeted diagnostic rather than
a type mismatch somewhere else; the [release history](#release-history) lists
the details.

- **Standard library written in lsaca**: `Vec`, `String`, `Option`, `Result`,
  `Rc`, `Box`, `HashMap`, `HashSet`, `BTreeSet`, `BTreeMap`, `VecDeque`,
  `BinaryHeap`, the iterator adapters, `io`, `fs`, `net`, `time`,
  `env`, `process` and `error` are ordinary lsaca source in
  [stdlib/src/](stdlib/src/), compiled into the compiler and shipped with it.
  Only the thin `extern fn` intrinsics that talk to the C runtime are external,
  so the shipped standard library is readable, debuggable and generic, and user
  code can be written in exactly the same style. See
  [docs/stdlib/README.md](docs/stdlib/README.md).
- **Trait objects**: `dyn Trait`, `Box<dyn Trait>`, `Rc<dyn Trait>` and
  `&dyn Trait` support dynamic dispatch through per-(trait, type) vtables,
  including object safety checking (`E0038`), automatic coercion at
  let/assignment/argument/return/`Vec::push` sites (`E0277` points at the
  missing impl), and heterogeneous collections such as `Vec<Box<dyn Shape>>`.
  See [docs/language/25-trait-objects.md](docs/language/25-trait-objects.md).
- **Slices and `str`**: `&[T]`, `&mut [T]`, `&str` and `&mut str`, Rust
  range slicing (`&values[1..4]`, `&text[..6]`, `&values[..]`), implicit
  coercions from `&Vec<T>`/`&mut Vec<T>`/`&String`/`&mut String`, and the
  usual UTF-8 boundary and range panics with Rust wording. `impl [T]` and
  `impl str` methods (`len`, `is_empty`, `get`, `first`, `last`,
  `char_count`, `starts_with`, `find`, `trim`, `split`, `parse_i64`, ...)
  live in the standard library. See
  [docs/language/26-slices-and-str.md](docs/language/26-slices-and-str.md).
- **`isize` and `usize`**: pointer-sized integer types available everywhere
  (including `HashMap` keys and `as` casts). Since 2.1.0 every length,
  capacity, index and count in the standard library is `usize`, and indexing
  requires `usize`.
- **New collections**: insertion-ordered `HashSet`, sorted `BTreeSet` and
  `BTreeMap`, `VecDeque` and `BinaryHeap`, all written in lsaca on top of the
  builtin `HashMap` and `Vec`, with set operations (`union`, `intersection`,
  `difference`, `subset`). See
  [docs/stdlib/collections/README.md](docs/stdlib/collections/README.md).
- **System interfaces**: `std::io` (16 `ErrorKind` variants, `File` with
  deterministic `Drop`, standard streams), `std::fs` (read/write/append files,
  `read_dir`, metadata, create/remove directories, canonicalize, copy, rename,
  current and temporary directories), `std::net` (plain TCP
  `TcpListener`/`TcpStream` with timeouts and shutdown), `std::time`
  (millisecond timestamps, monotonic clock, sleep, local/UTC date-time
  breakdown and formatting), `std::env`, `std::process` and
  `std::error::Error`. See [docs/stdlib/README.md](docs/stdlib/README.md).
- **Faster**: the runtime allocator, strings, formatting and `HashMap` were
  retuned, simple iterator chains are fused into C loops, and the generated
  code does less reference-count traffic. `iterator_pipeline` went from
  27.7 ms to 14.2 ms (C: 12.3 ms), `hashmap_string` from 243.7 ms to
  193.2 ms, and a cold single-file build from roughly 1.0-1.2 s to
  0.4-0.6 s once the shared runtime-object cache is warm. See
  [Benchmarks](#benchmarks).

Version 2.0.0 is source compatible with 1.7.0 programs apart from deliberate
changes: `dyn` and `extern` are now reserved keywords; the standard library
became the `std` crate (its root re-exports the names into every program's
prelude scope, so unqualified `Vec`, `String`, `HashMap`, `println!` and
friends keep working);
`extern fn` declarations are accepted only inside the standard library, and
user code cannot implement `[T]` or `str`; emitted C projects must be
regenerated. Several limitations that 1.7.0 only documented are now
implemented instead.

Version 1.7.0 (the previous release) made lsaca a multi-file language with a
package manager: `mod`/`use`/`pub` visibility, `lsaca.toml` manifests with
path dependencies, and a multi-file `SourceMap` so diagnostics and panic
locations name the real file and line. The
[release history](#release-history) keeps the details.

## Highlights

- **Rust-like language**: `fn`, `let`/`let mut`, `mut` parameters,
  named, tuple, newtype and unit `struct`s, `enum`, `impl`, `match` with
  exhaustiveness checking, `Option`, `Result`, `Vec<T>`, `Rc<T>`,
  `Box<T>`, tuples, closures, `if let`/`while let`, `for` loops, compound
  assignment and `print!`/`println!` macros. Blocks, `if` and `match` are
  expressions.
- **Modules, multi-file programs and packages**: `mod` declarations,
  `use` imports with aliases and groups, `pub`/`pub(crate)`
  visibility and `pub use` re-exports checked through multi-segment paths
  and across package boundaries; `lsaca.toml` manifests with path
  dependencies built dependency-first. See
  [docs/language/23-modules.md](docs/language/23-modules.md) and
  [docs/language/24-packages.md](docs/language/24-packages.md).
- **Generics and traits**: generic `struct`/`enum`/`fn`/`impl` blocks with
  type parameters, trait bounds, multiple `impl Trait<Args> for Type` blocks,
  inherent-method-first dispatch, `Self` in associated items and turbofish
  arguments. Instantiations are checked lazily per concrete type. See
  [docs/language/17-generics.md](docs/language/17-generics.md) and
  [docs/language/18-traits.md](docs/language/18-traits.md).
- **Trait objects and dynamic dispatch**: `dyn Trait` behind `Box`, `Rc`,
  `&` and `&mut`, object safety (`E0038`), coercion diagnostics (`E0277`),
  one vtable per (trait instantiation, concrete type), and `Box<dyn Trait>`
  values that can be stored together in a `Vec`. See
  [docs/language/25-trait-objects.md](docs/language/25-trait-objects.md).
- **Slices and string views**: `&[T]`, `&mut [T]`, `&str`, `&mut str`,
  range slicing syntax, `&Vec<T>`/`&String` coercions, and slice/`str`
  methods implemented in the standard library. See
  [docs/language/26-slices-and-str.md](docs/language/26-slices-and-str.md).
- **A standard library you can read**: `Vec`, `String`, the collections,
  iterators, formatting, math, `io`, `fs`, `net`, `time`, `env`, `process`
  and `error` are lsaca source compiled into the compiler, backed by a small
  set of `extern fn` intrinsics. See [docs/stdlib/README.md](docs/stdlib/README.md).
- **Lazy iterators**: `Vec<T>.iter()` and the adapters `map`, `filter`,
  `filter_map`, `take`, `skip`, `take_while`, `skip_while`, `step_by`, `zip`,
  `chain`, `enumerate` and `inspect`, plus the terminal operations `collect`,
  `fold`, `reduce`, `count`, `sum`, `product`, `any`, `all`, `find`, `position`,
  `last`, `nth` and `for_each`, implemented generically in the standard library
  (`stdlib/src/iter.ls`); `for` loops accept any type implementing `Iterator<T>`,
  and simple adapter chains are fused into a single C loop by the code
  generator. See
  [docs/language/19-iterators.md](docs/language/19-iterators.md).
- **Builtin `HashMap<K, V>`**: `insert`, `get`, `get_with`, `contains_key`,
  `remove`, `len`, `is_empty`, `clear`, `keys` and `values`, with
  deterministic hashing, content equality and ORC ownership rules. The map uses
  a Swiss-Table-style open-addressed layout and also accepts `char`, `isize`
  and `usize` keys; iteration order (`keys()`, `values()` and the printed form)
  is unspecified.
  See [docs/language/20-hashmap.md](docs/language/20-hashmap.md).
- **Characters and UTF-8 strings**: a Copy `char` scalar type with character
  literals and escapes, comparisons, patterns and formatting; strings
  guaranteed valid UTF-8 with character APIs (`chars`, `char_count`,
  `char_at`, `is_char_boundary`, `push(char)`, `pop`,
  `String::from_char`) next to byte-based `len`/`find`/`substring`,
  Unicode case mapping and whitespace trimming, and Rust-style `{:?}`
  escaping. See
  [docs/language/22-characters-and-unicode.md](docs/language/22-characters-and-unicode.md).
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
  follows from the initializer; integer literals default to `i32`, float
  literals to `f64`, and Rust numeric suffixes override the default while
  being range checked.
- **Borrowed standard-library signatures**: text parameters are `&str`
  instead of owned `String`, so passing a literal allocates nothing and
  passing a `String` requires an explicit borrow, exactly as in Rust.
- **Formatting controls**: format strings accept the Rust-style
  `[[fill]align][sign][0][width][.precision][?]` specification, so
  `{:>8}`, `{:*^9}`, `{:+}`, `{:08}`, `{:08.2}`, `{:.3}` and `{:?}` all
  work; invalid specifications are reported at compile time.
- **C17 back end, built directly**: the compiler emits a small portable C
  project (the program plus a vendored runtime) and compiles and links it with
  the system C compiler. `lsaca emit-c` writes the whole project, including a
  complete `CMakeLists.txt`, so it can be inspected or embedded in another
  build; `--cmake` makes `run`/`build` use that CMake project instead. Very
  large programs are split across several generated translation units
  (`main.c` plus `lsaca_unit_N.c`).
- **Fast, cached builds**: builds are cached by content hash, and the direct C
  backend skips CMake, so a memoised rebuild of an unchanged program takes tens
  of milliseconds instead of a second. Runtime objects are compiled once per
  profile and shared by every program.
- **Pure Rust compiler, no third-party crates**: the compiler uses only the Rust
  standard library and forbids unsafe code. The runtime is plain C17.
- **Documented and measured**: a detailed 26-chapter language manual and a
  standard-library reference under [docs/](docs/README.md), published as a
  static documentation site, plus
  cross-language benchmarks (compile time, runtime and peak memory) against C,
  Rust, Python and Node.js with the raw data checked in.
- **Tested end to end**: more than 870 compiler unit tests, 20 end-to-end test
  functions that compile and run 158 programs (single files, module trees and
  packages), 91 robustness tests, 22 CLI tests, a 10,357-check C runtime
  suite and a 121-check runtime smoke script; CI runs the test matrix on Linux,
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

A package with several source files (run these commands from the repository
root; once the compiler is on `PATH` the invocations are just
`lsaca new` and `lsaca run`):

```sh
./target/release/lsaca new hello-pkg
cd hello-pkg
../target/release/lsaca run
```

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
lsaca run [FILE.ls] [-- <program args...>]   compile, build (cached) and run
lsaca build [FILE.ls] [-o <dir>]             build a native executable
lsaca emit-c [FILE.ls] [-o <dir>]            write the generated C project only
lsaca check [FILE.ls]                        type-check and report diagnostics
lsaca new <DIR>                              create a binary package
lsaca version | help
```

When `FILE.ls` is omitted, `run`, `build`, `emit-c` and
`check` search the current directory and its parents for a
`lsaca.toml` manifest and operate on that package, including its path
dependencies. `run`, `build` and `emit-c` require a binary
package with `src/main.ls`; `check` also works on library packages.
`lsaca new` refuses to overwrite an existing non-empty directory.

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
 source files (.ls)  +  lsaca.toml manifests
    |  module loader    SourceMap, module tree, package dependency graph
    |  lexer            tokens
    |  parser           AST
    |  standard library   Vec, String, collections, io, fs, net, ... in lsaca
    |  type checker     typed HIR: module/use/visibility resolution, inference,
    |                   generic instantiation, trait dispatch, exhaustiveness,
    |                   constant evaluation, ? desugaring
    |  move checker     Copy/Clone classification, move and drop analysis
    |  borrow checker   NLL borrow checking and reference safety
    |  C code generator main.c (+ lsaca_unit_N.c)
    v
 C17 project  +  runtime (7 C files, 3 headers, embedded in the compiler)
    |
    +--> system C compiler (default, no CMake) ----> native executable
    |
    +--> CMake + C compiler (--cmake) ------------> native executable
```

The generated project is self contained:

```text
main.c               the user program plus generated clone/drop/format helpers
lsaca_unit_N.c       additional generated translation units for very large programs
lsaca_rt.h           public runtime ABI
lsaca_rt_internal.h  internal runtime declarations shared by the C files
lsaca_rt_unicode.h   generated Unicode table declarations
lsaca_rt.c           reference counting, cycle collection, panics, checked math
lsaca_rt_string.c    String implementation, UTF-8 helpers, formatting support
lsaca_rt_vec.c       Vec implementation
lsaca_rt_fmt.c       format-string machinery
lsaca_rt_map.c       Swiss-Table-style HashMap (control bytes, group probing)
lsaca_rt_io.c        files, directories, standard streams, TCP, time, env, args
lsaca_rt_unicode.c   Unicode tables, char/string case mapping
CMakeLists.txt       portable build description (Debug: -g -O0 -DLSACA_DEBUG,
                     Release: -O2 -DNDEBUG)
```

Builds are cached by content hash (source, generated C, runtime sources,
compiler version and path, build mode and leak-tracking mode), so repeated runs
of unchanged programs are fast. The runtime's C objects are compiled once per
profile and stored in a separate shared runtime-object cache, outside the
per-program build cache, so even a cold program build only compiles its own
generated unit(s) (`main.c` and any `lsaca_unit_N.c`) and then links.
The per-program build cache location is chosen defensively:

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
floats, `bool`, `char`, `(...)` (unit), tuples of `Copy` values, `Option`/`Result`
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

Release 1.6 added `char` and UTF-8 strings:

```rust
fn main() {
    let text = String::from("Héllo, 世界");
    println!("{} bytes, {} characters", text.len(), text.char_count());
    println!("upper = {}", text.to_uppercase());
    println!("first = {:?}", text.char_at(0));
    println!("substring = {}", text.substring(1, 2));

    let mut greeting = String::from("hi");
    greeting.push('!');
    greeting.push('😀');
    println!("{} {:?}", greeting, 'é');
    println!("{}", String::from("straße").to_uppercase());
}
```

The output is `14 bytes, 9 characters`, `upper = HÉLLO, 世界`,
`first = Some('H')`, `substring = é`, `hi!😀 'é'` and `STRASSE`.
The full program is [examples/unicode.ls](examples/unicode.ls).

Release 2.0 added trait objects, slices and `str`, and the lsaca standard
library:

```rust
use std::collections::HashSet;
use std::fs;

trait Shape {
    fn name(&self) -> String;
    fn area(&self) -> f64;
}

struct Rect { width: f64, height: f64 }
struct Square { side: f64 }

impl Shape for Rect {
    fn name(&self) -> String { String::from("rect") }
    fn area(&self) -> f64 { self.width * self.height }
}

impl Shape for Square {
    fn name(&self) -> String { String::from("square") }
    fn area(&self) -> f64 { self.side * self.side }
}

fn describe(shape: &dyn Shape) -> String {
    format!("{} area {:.2}", shape.name(), shape.area())
}

fn main() {
    let values = vec![10, 20, 30, 40, 50];
    let window: &[i64] = &values[1..4];
    let mut total = 0;
    let mut index = 0;
    while index < window.len() {
        total = total + window[index];
        index = index + 1;
    }

    let text = String::from("héllo wörld");
    let world: &str = &text[7..];
    println!("{:?} {} {} {}", window, total, world, world.char_count());

    let rect = Rect { width: 4.0, height: 5.0 };
    println!("{}", describe(&rect));

    let mut shapes: Vec<Box<dyn Shape>> = Vec::new();
    shapes.push(Box::new(Rect { width: 2.0, height: 5.0 }));
    shapes.push(Box::new(Square { side: 3.0 }));
    let mut index = 0;
    while index < shapes.len() {
        println!("{} area {:.2}", shapes[index].name(), shapes[index].area());
        index = index + 1;
    }

    let mut seen: HashSet<String> = HashSet::new();
    seen.insert(String::from("alpha"));
    seen.insert(String::from("beta"));
    println!("{} {}", seen.len(), seen.contains(String::from("alpha")));

    fs::write(String::from("notes.txt"), String::from("hello")).unwrap();
    let readback = fs::read_to_string(String::from("notes.txt")).unwrap();
    println!("{}", readback);
    fs::remove_file(String::from("notes.txt")).unwrap();
}
```

The output is:

```text
[20, 30, 40] 90 wörld 5
rect area 20.00
rect area 10.00
square area 9.00
2 true
hello
```

The example writes and removes `notes.txt` in the current directory. The
standard library is ordinary lsaca code under [stdlib/src/](stdlib/src/) - the
full API is listed in [docs/stdlib/README.md](docs/stdlib/README.md).

## Documentation

The manual is split into one detailed page per topic under [docs/](docs/):

- [docs/README.md](docs/README.md) - the manual hub: overview, reading order,
  and a table linking every chapter and the standard-library reference.
- [docs/language/01-getting-started.md](docs/language/01-getting-started.md)
  through
  [docs/language/26-slices-and-str.md](docs/language/26-slices-and-str.md) -
  the 26-chapter language manual: getting started, program structure, lexical
  structure, types, the memory model, variables and mutability, operators,
  statements and control flow, functions, structs/enums/`impl`, ownership and
  moves, references and borrowing, pattern matching, closures, the standard
  library, the roadmap and deviations from Rust, generics, traits, iterators,
  `HashMap`, benchmarks, characters and Unicode, modules, packages, trait
  objects, and slices and `str`.
- [docs/stdlib/README.md](docs/stdlib/README.md) - the standard-library
  reference: `core` (`Option`, `Result`, `Iterator`), `String`, `char`,
  `Vec`, iterators, `fmt`, `math`, `io`, `fs`, `net`, `time`, `env`,
  `process` and `error`, plus the collections
  ([`HashSet`](docs/stdlib/collections/hash-set.md),
  [`BTreeSet`](docs/stdlib/collections/btree-set.md),
  [`BTreeMap`](docs/stdlib/collections/btree-map.md),
  [`VecDeque`](docs/stdlib/collections/deque.md) and
  [`BinaryHeap`](docs/stdlib/collections/binary-heap.md)), with exact
  signatures, ownership notes and examples.
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
- [docs/language/23-modules.md](docs/language/23-modules.md) and
  [docs/language/24-packages.md](docs/language/24-packages.md) - modules,
  multi-file crates, `lsaca.toml` and path dependencies.

The same pages are published as a static site with GitHub Pages at
<https://menghuihuan44.github.io/lsaca/> (built by
[.github/workflows/pages.yml](.github/workflows/pages.yml) from the Markdown
sources with [scripts/docsite.py](scripts/docsite.py)).

## Benchmarks

`python scripts/bench.py` runs **25 programs** - 22 executed workloads and
three compile-only multi-file workloads - in lsaca and in C, Rust, Python and
Node.js (thirteen scalable workloads are measured at small, medium and large
scales as well). Every output is checked against an independent Python oracle
before it is timed; the harness records cold and cached build time, runtime
median and peak working set. lsaca programs go through the normal cached
`lsaca build -r` pipeline (the default direct C backend).

The official 2.1.0 result is [benchmarks/results/latest.md](benchmarks/results/latest.md),
archived under [benchmarks/results/archive/v2.1/](benchmarks/results/archive/v2.1/),
with the full analysis in
[benchmarks/results/v2.1-report.md](benchmarks/results/v2.1-report.md) and the
per-workload commentary on
[docs/language/21-benchmarks.md](docs/language/21-benchmarks.md). The run used
Windows 11 on a Core i7-1260P (16 threads, 15.6 GiB), gcc 15.2.0 (`-O2`),
rustc 1.99 (`-O`), Node 24 and Python 3.14; runtimes are medians of seven
runs after one warmup and are lower-is-better. Short workloads on this laptop
vary noticeably between runs, so the tables below are the recorded official
run; the benchmarks page discusses the noise and repeats the affected cases.

| workload | lsaca 2.1.0 | C (gcc -O2) | Rust (rustc -O) | ratio vs C |
| --- | ---: | ---: | ---: | ---: |
| `fib` | 27.6 ms | 23.7 ms | 11.1 ms | 1.17x |
| `fib_iter` | 24.1 ms | 9.2 ms | - | 2.63x |
| `fib_memo` | 13.6 ms | 10.2 ms | - | 1.33x |
| `loop_sum` | 20.6 ms | 13.4 ms | 13.0 ms | 1.54x |
| `string_build` | 97.2 ms | 13.0 ms | 59.5 ms | 7.48x |
| `vec_memory` | 19.9 ms | 14.5 ms | 18.5 ms | 1.37x |
| `sort_i64` | 56.6 ms | 33.1 ms | 29.3 ms | 1.71x |
| `sort_insertion` | 14.3 ms | 9.8 ms | - | 1.45x |
| `matrix_multiply` | 16.2 ms | 12.7 ms | 21.7 ms | 1.27x |
| `binary_trees` | 212.5 ms | 19.2 ms | 22.2 ms | 11.05x |
| `binary_trees_refs` | 38.2 ms | 26.2 ms | 20.0 ms | 1.46x |
| `string_split` | 39.3 ms | 14.3 ms | 13.2 ms | 2.75x |
| `sieve` | 37.8 ms | 29.3 ms | 30.7 ms | 1.29x |
| `mandelbrot` | 39.5 ms | 31.7 ms | 31.1 ms | 1.25x |
| `iterator_pipeline` | 16.0 ms | 12.2 ms | 12.3 ms | 1.30x |
| `hashmap_wordcount` | 32.5 ms | 21.7 ms | 41.9 ms | 1.50x |
| `array_wordcount` | 13.1 ms | 9.0 ms | - | 1.45x |
| `generic_stack` | 17.2 ms | 13.9 ms | 15.0 ms | 1.24x |
| `fn_pipeline` | 12.1 ms | 8.3 ms | 8.9 ms | 1.46x |
| `string_unicode` | 38.1 ms | 15.6 ms | 14.3 ms | 2.44x |
| `hashmap_i64` | 196.3 ms | 145.6 ms | 181.2 ms | 1.35x |
| `hashmap_string` | 181.8 ms | 163.5 ms | 213.6 ms | 1.11x |

Compile times on the same machine (cold: fresh program cache with a warm
shared runtime-object cache; cached: only relinking):

| build | lsaca 2.1.0 | C (gcc -O2) |
| --- | ---: | ---: |
| cold build, typical single file | 0.46-0.82 s (median 0.56 s) | 0.22-0.47 s (median 0.26 s) |
| cached build, typical single file | 53-108 ms (median 62 ms) | - |
| `compile_large` (4,511 lines), cold / cached | 1.96 s / 549 ms | - |
| `modules_compile` (9 files), cold / cached | 2.96 s / 510 ms | - |
| `package_compile` (13 files, 3 packages), cold / cached | 1.33 s / 116 ms | - |

Peak memory is at parity with C for most workloads: `fib` 5.1 vs 4.1 MiB,
`vec_memory` 20.1 vs 18.8 MiB, `sieve` 20.1 vs 18.8 MiB, `hashmap_i64` 56.8
vs 52.6 MiB. The exceptions are documented workload-fairness effects: by-value
`binary_trees` deep-copies the tree when summing (53.1 vs 7.7 MiB, 11.05x
runtime) while the borrowed `binary_trees_refs` variant is the fair comparison
(17.1 vs 7.7 MiB, 1.46x), and `hashmap_string` allocates one shared `String`
buffer per unique word (30.8 vs 19.7 MiB).

The single-file cold build is dominated by the system C compiler compiling the
generated translation units; the seven runtime translation units are compiled
once per toolchain and profile into a shared runtime-object cache, so a normal
program build compiles only its own generated unit(s) and links. The first
build on a fresh machine (empty runtime cache) still pays for the runtime and
takes roughly 1.8 s. Cached rebuilds skip the C compiler for unchanged
content.

At run time, 15 of the 22 executed workloads are within 1.5x of C and ten are
within 1.4x; `hashmap_string` (1.11x) is the closest to C and `fib` (1.17x) is
second. The twelve workloads above 1.4x are analyzed individually on the
benchmarks page and in the report: `string_build` (7.48x) is the one real
2.1.0 regression - the `&str` alignment means `s.push_str(format!(...))` has
to be written as `let piece = format!(...); s.push_str(&piece)`, so the old
format-into-push fusion no longer applies and every token now allocates a
second string; restoring that fusion is the top 2.2 performance item.
`string_split` (2.75x) has the same cause plus per-split allocation.
`fib_iter` (2.63x) and `sort_i64` (1.71x) are the loudest of the short-workload
noise: an immediate repeat of the same suite measured `fib_iter` at 1.18x and
`sort_i64` at 1.35x while the C baselines also moved (`fib` C 23.7 -> 38.4 ms),
so only same-run comparisons are exact; `sort_i64` additionally pays `as usize`
casts on every element access in the migrated quicksort. `string_unicode`
(2.44x) spends its time in the documented O(n) `char_at` API walking UTF-8
from the start of the string, `binary_trees_refs` (1.46x) pays reference-count
traffic on boxed nodes, `fn_pipeline` (1.46x), `array_wordcount` (1.45x) and
`sort_insertion` (1.45x) sit just outside the target, `hashmap_wordcount`
(1.50x) and `loop_sum` (1.54x) mix hashing / short loops with the same noise,
and `binary_trees` (11.05x) remains the deliberate by-value semantics
comparison.

Against the archived 2.0.0 run the same-suite wins are `binary_trees_refs`
(59.5 -> 38.2 ms, -36%), `hashmap_string` (193.2 -> 181.8 ms) and
`hashmap_i64` (212.5 -> 196.3 ms); `string_build` moved from 26.8 to 97.2 ms
because of the fusion loss described above, and the other rows moved together
with their C baselines on a host that was busier than the 2.0.0 host (gcc
needed 0.22-0.47 s for the same workloads here versus 0.21-0.37 s in 2.0.0).
The [benchmarks page](docs/language/21-benchmarks.md) and
[benchmarks/results/v2.1-report.md](benchmarks/results/v2.1-report.md) have the
complete before/after tables.

## Environment variables

- `LSACA_CC` - C compiler used to build generated programs (default: `gcc`,
  `cc` or `clang` from `PATH`, then common MSYS2 locations on Windows).
- `LSACA_JOBS` - parallel build jobs for the C compiler (with `--cmake`, passed
  to the CMake build instead).
- `LSACA_CACHE_DIR` - per-program build cache location (default: tried under
  the system temporary directory first, then `.lsaca-build-cache` in the
  current directory).
- `LSACA_RUNTIME_CACHE_DIR` - shared compiled-runtime object cache location
  (default: `lsaca-runtime-objects` under the system temporary directory, then
  `.lsaca-runtime-cache` in the current directory). It is kept out of the
  per-program build cache so clearing that cache (for example before a
  benchmark run) does not force a runtime recompile.
- `LSACA_DEBUG_RC` - read by the compiler: set to `1` (`true`, `on` or `yes`
  also work) to compile the runtime with leak tracking in *every* build
  profile, including release; emitted CMake projects expose the same switch as
  the CMake option `LSACA_DEBUG_RC`.

## Troubleshooting

- **`failed to build <file>: ... (os error 5)` / "Access is denied"**: the
  compiler could not write to its build cache. Run with `-v` to see the chosen
  cache directory, or point `LSACA_CACHE_DIR` at a writable directory. The
  compiler probes the cache location and falls back to `.lsaca-build-cache` in
  the current directory instead of failing on an unwritable `TEMP`. The
  shared runtime-object cache has the same fallback and can be redirected with
  `LSACA_RUNTIME_CACHE_DIR`.
- **`no C compiler found`**: install GCC or Clang (MSYS2 UCRT64 on Windows) and
  put it on `PATH`, or set `LSACA_CC` to the compiler executable.
- **`CMake was not found on PATH`** or **`no build generator found`**: these
  errors only occur with `--cmake` (or when you build an emitted project
  yourself). Install CMake 3.16+ and Ninja (recommended) or `mingw32-make`, or
  drop `--cmake` - the default backend needs neither.

## Testing

```sh
cargo test --workspace   # more than 870 unit + 20 end-to-end + 91 robustness + 22 CLI tests
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

`cargo test --workspace` runs more than 870 unit tests across the compiler
library - including the lexer, parser, type checker, borrow checker, move
checker, C code generator and the embedded standard library loader - plus 20
end-to-end test functions that compile and run 158 programs: single files under
`crates/lsaca/tests/programs/` and `examples/`, module trees under
`tests/programs/modules/`, package projects (including path dependencies)
under `tests/packages/`, and IO/FS fixtures that run in a private scratch
directory with controlled stdin and environment. They are skipped when no C
compiler is available. `crates/lsaca/tests/modules_audit.rs` replays a 60-case
module and visibility corpus, and `crates/lsaca/tests/robustness.rs` fuzzes the
module, manifest and package loaders with malformed input under timeouts.

The C runtime has its own 10,357-check suite (`runtime/test/main.c`, including
ORC stress, HashMap and Phase C IO tests) plus a 121-check Windows smoke script
(`runtime/test/run_smoke.ps1`). Every generated project is compiled without
warnings under `-Wall -Wextra -Wpedantic -Werror` in both profiles by
[scripts/check_generated_c.py](scripts/check_generated_c.py), which also
verifies byte-identical output across runs and runs every program; CI also runs
the C runtime in release, debug and leak-tracking modes, and checks the
documentation site with `scripts/docsite.py --check`. The documentation examples
are executed against the real compiler by `scripts/docs_verify.py` (every
`ls`, `ls,compile-fail` and `ls,panic` fence), and
`scripts/check_bench21.py`, `scripts/linkcheck.py` and
`scripts/prose_check.py` validate the benchmark page, every relative link and
anchor, and the prose claims. The complete CI definition is in
[.github/workflows/ci.yml](.github/workflows/ci.yml).

Every CI job captures its output in a log file and, when a step fails, uploads a
`diagnostics-*` artifact containing that log, the generated-C report and the
documentation verification output. A red run can therefore be inspected from the
Actions page or downloaded with `gh run download <run-id>` instead of being
reproduced locally.

CI also builds emitted projects through CMake. In a confined environment where
child processes cannot spawn CMake/Ninja, set `LSACA_SKIP_CMAKE_TESTS=1` to skip
only that CLI test; the dedicated CI job still covers the CMake backend.

## Release history

- **2.1.0** - the Rust-alignment release. Numeric defaults and suffixes: integer
  literals infer `i32`, float literals `f64`, every integer type accepts a
  Rust suffix (`i8` to `i64`, `isize`, `u8` to `u64`, `usize`, plus
  `f32`/`f64`), suffixed and annotated literals are range checked, an
  unsuffixed literal adapts to a contextual integer type, `for` ranges iterate
  the operand's integer type and negative literal folding follows Rust. Every
  length, index, capacity and count is `usize` (vectors, strings, slices,
  `HashMap`, `find`, iterator counts, `Rc::strong_count`) and indexing
  requires `usize`. Standard-library read-only text parameters and `expect`
  take `&str`; literals in `&str` contexts become zero-allocation static
  views and `String + &str` / `+= &str` replace owned concatenation.
  Tuple/newtype/unit structs and `mut` parameters. Standard library:
  `Option::or`/`xor` and `Result::unwrap_err`, iterator adapters (`zip`, `chain`,
  `filter_map`, `take_while`, `skip_while`, `step_by`, `position`,
  `last`, `nth`, `reduce`, `product`, `inspect`, `for_each`), `str`
  whitespace/line helpers and `replace`, `char` ASCII helpers,
  `Vec::truncate`/`swap`/`contains`, `ParseIntError` and
  `ParseFloatError`. System interfaces: `time::{Duration, Instant,
  SystemTime}`, `io::Error::new`, `error::Error`, byte-count
  `File::read`/`write`, `File::seek`, `fs::Metadata` accessors and
  `FileType`, `env::VarError`, `net::Shutdown`, `process::exit` and the
  `math` float constants. Source breaking: changed default integer type,
  `usize` lengths and indices, `&str` parameters, parse error types and
  `String + String`.
- **2.0.0** - the standard library is now written in lsaca and ships as the
  `std` crate under `stdlib/src` (embedded into the compiler): `Option`,
  `Result`, `Iterator`, `String`, `char`, `Vec`, formatting, math,
  `io`, `fs`, `net`, `time`, `env`, `process`, `error` and the
  collections (`HashSet`, `BTreeSet`, `BTreeMap`, `VecDeque`,
  `BinaryHeap`). Builtin marker types are lowered to the C runtime through
  `extern fn` intrinsics that only the standard library may declare. Trait
  default method bodies; trait objects (`dyn Trait` with object safety,
  `Box<dyn Trait>`, `Rc<dyn Trait>` and `&dyn Trait`, one vtable per trait
  and implementing type, automatic coercion at `&`, `Box` and `Rc` sites);
  slices and string views (`[T]` behind references, `&str`, slice
  expressions, `&Vec<T>`/`&mut Vec<T>` and `&String`/`&mut String`
  coercions, UTF-8 boundary checks) with `isize`/`usize`; system
  interfaces for files, directories, TCP, time and the environment; managed
  closure environments and `Rc` cells allocated through the ORC collector;
  performance work: iterator-chain fusion, allocator size-class pool,
  Swiss-table and string fast paths, less reference traffic, and a shared
  runtime-object cache outside the per-program build cache (cold build median
  0.50 s, cached 42 ms); a 25-program benchmark matrix. Source compatible with
  1.7.0 except programs using `dyn` or `extern` as identifiers.
- **1.7.0** - modules, multi-file programs and a package manager: inline and
  file modules (`mod foo;` resolving to `foo.ls` or
  `foo/mod.ls`), arbitrary module nesting, `use` trees with aliases,
  groups, `self`/`super`/`crate` and dependency crates,
  `pub`/`pub(crate)` visibility and `pub use` re-exports
  enforced through multi-segment paths and across packages
  (`E0603`, `E0616`, `E0624`, `E0364`,
  `E0365`, `E0451`); a multi-file `SourceMap` so
  diagnostics, labels, panic messages and leak reports name the real file and
  line; `lsaca.toml` manifests, `lsaca new`, package mode for
  `run`/`build`/`check`/`emit-c` and path dependencies
  resolved into a dependency-first graph with cycle, missing-path and manifest
  diagnostics; module/crate-qualified C names with the root crate's names
  unchanged; `assert_ne!`; fixes for double `Drop` hooks when
  destructuring, a negative tail expression after a block-like statement, module
  nesting deeper than 256 levels (`E0001` instead of a stack overflow),
  library packages and `src/main.ls`-without-`main` error messages;
  new `modules_compile` and `package_compile` compile-time
  benchmarks. Source compatible with single-file 1.6.0 programs apart from the
  new keywords, enforced field visibility and regenerated emitted C names.
- **1.6.0** - real `char` type (Copy 32-bit Unicode scalar values, character
  literals and escapes, comparisons, patterns, casts and formatting); UTF-8
  String invariant with character APIs (`chars`, `char_count`, `char_at`,
  `is_char_boundary`, `push(char)`, `pop`, `String::from_char`),
  Unicode-aware case mapping and whitespace trimming, Rust-style `escape_debug`
  Debug output and validation of `read_line`/`read_file`/arguments; Swiss-Table
  style `HashMap` (control bytes, 16-slot group probing) with `char` keys,
  deterministic fixed-seed hashing and unspecified iteration order; generated C
  split into `main.c` plus up to eight `lsaca_unit_N.c` translation units for
  very large programs; audit fixes: `E0498` for references stored in
  aggregates, `E0605` for integer/float to `bool` casts, `E0119` for
  `impl Copy`/`impl Clone`, most-negative integer literal folding, iterative
  ORC cycle collector and destructor cascade (no stack overflow on 100,000-deep
  graphs), debug stale/double-release detection, 40-byte ORC headers, buffered
  reads and stdout, Unicode tables, Rust-parity Dragon4 shortest floats,
  NUL-safe interned string literals and match-arm ownership stealing; three new
  benchmark workloads (`string_unicode`, `hashmap_i64`, `hashmap_string`).
  Not fully source compatible with 1.5.0 (Unicode case/trim/Debug output,
  substring boundary panics, invalid UTF-8 rejection, `E0605` casts, changed
  emitted C/HashMap ABI).
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
crates/lsaca/            the compiler library (lexer, parser, module loader, package
                         manifest resolver, type checker, borrow checker, move
                         checker, C code generator, driver) and the embedded standard
                         library
crates/lsaca/tests/      end-to-end tests and test programs (single files, module
                         trees, packages and IO/FS fixtures)
crates/lsaca-cli/        the lsaca command line tool
stdlib/src/              the standard library, written in lsaca (the std crate: core,
                         String, char, Vec, iterators, collections, io, fs, net,
                         time, env, process, error); embedded into the compiler by
                         scripts/gen_stdlib_embed.py
runtime/                 the C17 ORC runtime embedded into generated projects
                         (lsaca_rt.c, lsaca_rt_string.c, lsaca_rt_vec.c,
                         lsaca_rt_fmt.c, lsaca_rt_map.c, lsaca_rt_io.c,
                         lsaca_rt_unicode.c and headers): reference-counted allocator
                         with cycle collector and a Swiss-Table-style map
runtime/test/            C runtime tests (10,357-check suite, ORC stress and the
                         Windows smoke script)
examples/                example programs
benchmarks/              benchmark programs in five languages, harness, oracles and
                         recorded results (archived per release)
scripts/                 benchmark harness (bench.py), report generator
                         (bench_report.py), oracle verifier (oracles.py), generated-C
                         checker (check_generated_c.py), documentation verifier
                         (docs_verify.py), benchmark-page checker
                         (check_bench21.py), link and prose checkers, site generator
                         (docsite.py), standard-library embedder
                         (gen_stdlib_embed.py) and Unicode table generator
docs/                    the manual: docs/README.md is the hub, 26 language chapters
                         plus the standard-library reference under docs/stdlib/
.github/workflows/       CI, benchmark smoke, benchmark matrix and GitHub Pages
```

## License

Apache License 2.0 - see [LICENSE](LICENSE).
