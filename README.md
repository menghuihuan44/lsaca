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

The current release is **2.4.0**, the tool-grade language release: array, slice
and `name @ pattern` patterns with Rust-compatible exhaustiveness, values out
of `break`, `HashMap::get_or_insert` plus a denser collection API, iterator
`min`/`max`/`product`, `process::Command`, UDP networking with `SocketAddr`,
`pub(super)`/`pub(in path)` visibility and module aliases, formatter crate mode,
and RFCs that freeze the protocol and concurrency boundaries. Version 2.3.0 made
iterators borrow, added `HashMap` iteration, the `io` stream traits and
`Path`/`PathBuf`, Unicode `char` methods, and partial moves; 2.2.0 added arrays,
loop labels, reference fields and `From`/`?`. Earlier releases grew the standard
library and system interfaces, starting with:

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

- **Fixed-size arrays**: `[T; N]` types and value semantics, the literals
  `[1, 2, 3]`, `[0; 4]` and `[]`, `.len()`/`.is_empty()`, bounds-checked
  indexing, elementwise `==`, `Debug` output such as `[1, 2, 3]`, implicit
  `&array` to `&[T]` and `&array[a..b]` slicing, and zero-length arrays.
  Arrays are copyable when their elements are; a borrowed array coerces to
  `&[T]` for slice-based code.
- **Loop labels**: `'outer: loop`, `'outer: while` and `'outer: for`, with
  `break 'outer` and `continue 'outer`, so nested loops can name their
  target instead of using flags.
- **Lightweight lifetimes**: lifetime parameters (`struct View<'a>`, `impl<'a>
  View<'a>`), `&'a T` and `&'a mut T`, references stored in struct, enum,
  tuple and `Option` fields, references returned through elided lifetimes, and
  the usual `&T` elision rules. Lifetimes are erased after checking and never
  reach code generation. See
  [docs/language/29-lifetimes.md](docs/language/29-lifetimes.md).
- **`From` and `?`**: the `From` trait, trait associated functions such as
  `AppError::from(error)`, and `?` in `Result` functions that converts the
  error through a user `impl From<Found> for Expected`. `Error` gains the
  default `source` method. See
  [docs/language/30-from-try.md](docs/language/30-from-try.md).
- **`lsaca fmt`**: a deterministic source formatter (`lsaca fmt file.ls`,
  `--check`, `--stdout`) with four-space indentation, one level of extra
  indentation for method chains, trailing-whitespace removal, CRLF
  conversion, collapsed blank lines and a single final newline; it refuses to
  rewrite a file that has syntax errors and is idempotent.
- **Real programs in lsaca**: a JSON parser and serializer, a Markdown to HTML
  converter and a tiny Lisp interpreter under `examples/programs/`, each with
  a C implementation that produces byte-identical output; they exercise the
  new features and run in CI.
- **Faster**: `push_str(&format!(...))` fuses the formatted fragments
  directly into the destination string again, and array handling compiles to
  plain C array operations. See [Benchmarks](#benchmarks).

Version 2.2.0 is source compatible with 2.1.0 apart from the borrowed map
accessors: `HashMap::get` and `get_mut` now take the key by reference
(`map.get(&key)`) and return `Option<&V>` / `Option<&mut V>`, so old call
sites are reported as an ordinary type mismatch that shows the expected
reference. Everything else is additive: arrays, loop labels, lifetimes,
`From`-based `?` and the formatter only add syntax or rewrite layout. The
[release history](#release-history) lists the details, and
[docs/language/deviations.md](docs/language/deviations.md) records the
deliberate deviations from Rust.

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
- **Lazy iterators**: `Vec<T>.iter()` borrows and yields `&T`,
  `iter_mut()` yields `&mut T`, and `into_iter()` consumes; the adapters
  `map`, `filter`, `filter_map`, `take`, `skip`, `take_while`, `skip_while`,
  `step_by`, `zip`, `chain`, `enumerate`, `inspect`, `rev` and `flat_map`,
  plus the terminal operations `collect`, `fold`, `reduce`, `count`, `sum`,
  `product`, `any`, `all`, `find`, `position`, `last`, `nth` and `for_each`,
  are implemented generically in the standard library (`stdlib/src/iter.ls`);
  `for` loops accept any type implementing `Iterator<T>` as well as `&Vec<T>`,
  `&mut Vec<T>`, `&[T]`, `&mut [T]` and `&HashMap<K, V>`, and simple adapter
  chains are fused into a single C loop by the code generator. See
  [docs/language/19-iterators.md](docs/language/19-iterators.md).
- **Builtin `HashMap<K, V>`**: `insert`, `get`, `get_with`, `contains_key`,
  `remove`, `len`, `is_empty`, `clear`, borrowed iteration with `iter`/`keys`/
  `values` and `for (key, value) in &map`, and `HashMap::from(Vec<(K, V)>)`,
  with deterministic hashing, content equality and ORC ownership rules. The map uses
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
- **Documented and measured**: a detailed 30-chapter language manual and a
  standard-library reference under [docs/](docs/README.md), published as a
  static documentation site, plus
  cross-language benchmarks (compile time, runtime and peak memory) against C,
  Rust, Python and Node.js with the raw data checked in.
- **Tested end to end**: more than 960 compiler unit tests, 21 end-to-end test
  functions that compile and run more than 150 programs (single files, the real
  programs under `examples/programs/` checked byte for byte, module trees and
  packages), 120 robustness tests, 27 CLI tests, a 10,357-check C runtime
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
lsaca fmt <FILE.ls> [--check] [--stdout]     rewrite a file with canonical indentation
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

`lsaca fmt` is a deliberately conservative formatter: it fixes indentation
(four spaces per nesting level, one extra level for leading-dot method chains),
trailing whitespace, line endings and runs of blank lines, but it never
reflows expressions, splits or joins lines, or changes token spacing.
Files that already use the canonical layout are left untouched, so the command
is idempotent. It refuses to rewrite a file that has syntax errors, `--check`
reports the files that would change (exit code `1`) without writing, and
`--stdout` prints the result instead of rewriting.

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
        .map(|value| *value * 10)
        .sum();
    println!("{}", total);                              // 60

    let mut counts: HashMap<String, i64> = HashMap::new();
    counts.insert(String::from("lsaca"), 2);
    counts.insert(String::from("rust"), 3);
    println!("{:?} {}", counts.get("lsaca"), counts.keys().count());  // Some(2) 2
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
  [docs/language/30-from-try.md](docs/language/30-from-try.md) -
  the 30-chapter language manual: getting started, program structure, lexical
  structure, types, the memory model, variables and mutability, operators,
  statements and control flow, functions, structs/enums/`impl`, ownership and
  moves, references and borrowing, pattern matching, closures, the standard
  library, the roadmap and deviations from Rust, generics, traits, iterators,
  `HashMap`, benchmarks, characters and Unicode, modules, packages, trait
  objects, slices and `str`, fixed-size arrays, loop labels, lightweight
  lifetimes for reference fields, and `From` with the `?` operator.
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
- [docs/language/deviations.md](docs/language/deviations.md) - the deliberate
  deviations from Rust.
- [docs/language/limitations.md](docs/language/limitations.md) - the verified
  limitations of the current release.
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

The official 2.4.0 result is [benchmarks/results/latest.md](benchmarks/results/latest.md),
archived under [benchmarks/results/archive/v2.4/](benchmarks/results/archive/v2.4/),
with the run published in
[docs/language/21-benchmarks.md](docs/language/21-benchmarks.md); the previous
release is frozen in [benchmarks/results/archive/v2.3/](benchmarks/results/archive/v2.3/)
and the 2.2.0 baseline analysis is in
[benchmarks/results/v2.2-report.md](benchmarks/results/v2.2-report.md). The run
used Windows 11 on a Core i7-1260P (16 threads, 15.6 GiB), gcc.EXE (Rev6, MSYS2)
16.2.0 (-O2), rustc 1.100 (-O), Node 26 and Python 3.14; runtimes are medians of
seven runs after one warmup and are lower-is-better. The 2.3.0 baseline was
measured with the same gcc 16.2.0 but an earlier host state, so cross-release
claims are made about C-normalised ratios only; short workloads on this laptop
vary noticeably between runs, so the tables below are one recorded official run.

| workload | lsaca 2.4.0 | C (gcc -O2) | Rust (rustc -O) | ratio vs C |
| --- | ---: | ---: | ---: | ---: |
| `fib` | 38.6 ms | 32.2 ms | 18.6 ms | 1.20x |
| `fib_iter` | 16.1 ms | 16.3 ms | - | 0.99x |
| `fib_memo` | 17.7 ms | 17.2 ms | - | 1.03x |
| `loop_sum` | 23.4 ms | 19.6 ms | 16.1 ms | 1.19x |
| `string_build` | 99.7 ms | 20.9 ms | 83.1 ms | 4.77x |
| `vec_memory` | 27.4 ms | 22.7 ms | 19.0 ms | 1.21x |
| `sort_i64` | 44.8 ms | 42.5 ms | 46.0 ms | 1.05x |
| `sort_insertion` | 17.9 ms | 14.4 ms | - | 1.24x |
| `matrix_multiply` | 23.2 ms | 21.7 ms | 18.9 ms | 1.07x |
| `binary_trees` | 321.7 ms | 28.0 ms | 34.2 ms | 11.48x |
| `binary_trees_refs` | 42.7 ms | 23.9 ms | 25.6 ms | 1.79x |
| `string_split` | 31.2 ms | 26.7 ms | 20.0 ms | 1.17x |
| `sieve` | 47.8 ms | 43.0 ms | 39.8 ms | 1.11x |
| `mandelbrot` | 44.3 ms | 43.7 ms | 33.7 ms | 1.01x |
| `iterator_pipeline` | 20.1 ms | 13.6 ms | 15.7 ms | 1.48x |
| `hashmap_wordcount` | 43.1 ms | 32.5 ms | 52.5 ms | 1.33x |
| `array_wordcount` | 19.3 ms | 18.4 ms | - | 1.05x |
| `generic_stack` | 23.2 ms | 20.5 ms | 19.5 ms | 1.13x |
| `fn_pipeline` | 17.0 ms | 13.8 ms | 15.5 ms | 1.23x |
| `string_unicode` | 42.4 ms | 23.6 ms | 20.7 ms | 1.80x |
| `hashmap_i64` | 355.6 ms | 227.7 ms | 309.0 ms | 1.56x |
| `hashmap_string` | 263.0 ms | 242.3 ms | 248.6 ms | 1.09x |

Compile times on the same machine (cold: fresh program cache with a warm
shared runtime-object cache; cached: only relinking):

| build | lsaca 2.4.0 |
| --- | ---: |
| cold build, typical single file | 0.57-2.54 s (median 0.88 s) |
| cached build, typical single file | 130-219 ms (median 164 ms) |
| `compile_large` (4,511 lines), cold / cached | 3.53 s / 1,105 ms |
| `modules_compile` (9 files), cold / cached | 5.17 s / 1,032 ms |
| `package_compile` (13 files, 3 packages), cold / cached | 2.38 s / 352 ms |

Peak memory is at parity with C for most workloads: `fib` 4.5 vs 4.0 MiB,
`vec_memory` 19.2 vs 18.8 MiB, `sieve` 19.3 vs 18.8 MiB, `hashmap_i64` 55.9
vs 52.5 MiB. The exceptions are documented workload-fairness effects: by-value
`binary_trees` deep-copies the tree when summing (52.3 vs 7.6 MiB, 11.48x
runtime) while the borrowed `binary_trees_refs` variant is the fair comparison
(16.2 vs 7.6 MiB, 1.79x), and `hashmap_string` allocates one shared `String`
buffer per unique word (30.0 vs 19.6 MiB).

The single-file cold build is dominated by the system C compiler compiling the
generated translation units; the runtime translation units are compiled once per
toolchain and profile into a shared runtime-object cache, so a normal program
build compiles only its own generated unit(s) and links. Cached rebuilds skip
the C compiler for unchanged content.

At run time, 16 of the 22 executed workloads are within 1.4x of C, 17 are within
1.5x and 9 are within 1.15x; the median lsaca/C ratio is 1.20x. `fib_iter`
(0.99x), `mandelbrot` (1.01x), `fib_memo` (1.03x), `array_wordcount` (1.05x),
`sort_i64` (1.05x), `matrix_multiply` (1.07x) and `hashmap_string` (1.09x) are
the closest workloads. The five above 1.5x are analyzed individually on the
benchmarks page: `binary_trees` (11.48x) is the deliberate by-value semantics
comparison; `string_build` (4.77x) builds a temporary `String` per token in the
frozen two-step form; `string_unicode` (1.80x) pays the documented O(n)
character API plus Unicode table lookups; `binary_trees_refs` (1.79x) pays
reference-count traffic on the borrowed traversal; and `hashmap_i64` (1.56x)
pays hashing and allocations on top of C's hand-written open-addressing table.

Against the archived 2.3.0 run, compared as C-normalised ratios because the two
runs were taken on different host states, the largest gains are `fib_memo`
(1.51x to 1.03x), `sieve` (1.50x to 1.11x), `string_split` (1.55x to 1.17x),
`string_build` (6.23x to 4.77x), `fib_iter` (1.27x to 0.99x) and
`array_wordcount` (1.34x to 1.05x); the largest normalised regressions are
`iterator_pipeline` (1.07x to 1.48x), `generic_stack` (0.85x to 1.13x), `fib`
(0.99x to 1.20x), `fn_pipeline` (1.05x to 1.23x) and `vec_memory`
(1.18x to 1.21x). A same-host A/B run of the 2.3.0 and 2.4.0 compilers on
twelve of these workloads measured a median runtime ratio of 0.993 with the C
reference medians themselves moving by up to 42% between the two runs, so the
normalised deltas are within this laptop's run-to-run noise. The
[benchmarks page](docs/language/21-benchmarks.md) has the complete before/after
tables.

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
cargo test --workspace   # more than 960 unit + 21 end-to-end + 120 robustness + 27 CLI tests
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

`cargo test --workspace` runs more than 960 unit tests across the compiler
library - including the lexer, parser, type checker, borrow checker, move
checker, C code generator and the embedded standard library loader - plus 21
end-to-end test functions that compile and run more than 150 programs: single
files under `crates/lsaca/tests/programs/`, the four real programs under
`examples/programs/` whose output is compared byte for byte with stored golden
files, module trees under
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

### Real example programs

`scripts/real_programs.py` is the end-to-end gate for the four programs under
`examples/programs/`. It runs the JSON, Markdown, Lisp and word-statistics
programs with and
without their sample data through the release compiler, compiles the matching
hand-written C references with `gcc -std=c17 -O2 -Wall -Wextra -Werror`, and
compares every stdout byte with the goldens under
`crates/lsaca/tests/real_programs/expected/`. Any compiler, runtime, C-reference
or golden mismatch fails the check.

```sh
cargo build --release -p lsaca-cli
python scripts/real_programs.py --bin target/release/lsaca
python scripts/real_programs.py --bin target/release/lsaca --no-c
python scripts/real_programs.py --bin target/release/lsaca --json target/real-programs.json
```

The C comparison is on by default and is skipped with `--no-c`. GCC is taken
from `PATH`, with `C:\msys64\ucrt64\bin\gcc.exe` as the Windows fallback;
`--keep` preserves the per-case stdout/stderr under `target/real-programs/runs`.
The `Real example programs` CI job runs all eight invocations with both
implementations on Ubuntu.

### Documentation drift

`scripts/docs_drift.py` treats the `[workspace.package]` version in `Cargo.toml`
as the single source of truth. It checks the version statements in `README.md`,
`docs/README.md` and `CHANGELOG.md`, rejects future-looking and current-state
version claims that no longer match, verifies that the numbered
`docs/language/` chapters run contiguously from `01` (supplementary pages such
as `deviations.md` may be unnumbered), and proves every language and
standard-library page is reachable from `docs/README.md` within two relative
links.

```sh
python scripts/docs_drift.py
python scripts/docs_drift.py --json target/docs-drift.json
```

On success it prints a one-line summary. On failure it prints `DRIFT ERROR:`
lines, exits 1 and writes the findings to the JSON report. The `site` CI job
runs the check and tees its output to `target/docs-drift.log`.

### Status traceability

`scripts/check_status.py` enforces the completion matrix in `STATUS.md`. Every
row must use one of the four documented states; a partial or missing row must
name a target release, and every row marked `complete` must have at least one
test reference and one documentation reference in
[docs/status-traceability.md](docs/status-traceability.md). The traceability
file keeps an entry for every row in the current matrix, so a row can flip to
complete without losing its evidence.

Test references are `path[::symbol]` values; the script checks that the file
exists and that the symbol (or `mod name` for a module) occurs in it. Document
references are `docs/...md#heading-anchor` values; the script checks the file
and recomputes the GitHub heading slug. It also validates the traceability
section and row names against `STATUS.md`.

```sh
python scripts/check_status.py
python scripts/check_status.py --json target/check-status.json
```

A passing run prints one line with the state counts, the number of traced
complete rows and the number of documentation files. The `site` CI job runs the
check and tees its output to `target/check-status.log`; the
`diagnostics-site` artifact includes that log and the JSON report.

### Incremental speed guard

`scripts/bench_guard.py` compares a result file produced by `scripts/bench.py`
with the most recently released baseline,
`benchmarks/results/archive/v2.4/latest.json` (2.4.0). The limits are a runtime
median regression of at most 10% and a cold compile median regression of at
most 20% against the previous release in each case. The guard is a hard check
when the measurement host matches the baseline; on the shared CI runners it
reports advisory results, and a release is validated with a same-host A/B run. The guard compares the primary runtime, every data scale present in
both runs, and the cold compile median.

```sh
python scripts/bench_guard.py --self-test
python scripts/bench_guard.py --results benchmarks/results/latest.json
python scripts/bench_guard.py --results target/bench-matrix/results.json \
  --runtime-tolerance 10 --compile-tolerance 20 \
  --json target/bench-matrix/speed-guard.json \
  --md target/bench-matrix/speed-guard.md
```

`--mode auto` (the default) compares absolute medians when the baseline and
current host platforms match and switches to C-normalized medians otherwise, so
a slower CI runner does not count as a compiler regression; `absolute` and
`normalized` force either behavior. `--tolerance` sets both limits, and
`--runtime-tolerance`/`--compile-tolerance` override them individually. The
benchmark matrix workflow runs the self-test and then the guard on every
platform; the quick smoke workflow runs the self-test. When a compile-only
benchmark has no C reference, `normalized` mode emits a warning and skips that
cold-compile comparison.

Compiler- and script-running CI jobs capture their output in `target/*.log` and,
when a step fails, upload a `diagnostics-*` artifact with the relevant logs and
reports. `diagnostics-real-programs` contains the real-program log, JSON report
and any failing case output, and `diagnostics-site` now also carries
`target/docs-drift.log`, `target/check-status.log` and
`target/check-status.json`. Benchmark matrix results, including `speed-guard.json`
and `speed-guard.md`, are uploaded on every run as `benchmark-matrix-<os>`. A red
run can therefore be inspected from the Actions page or downloaded with
`gh run download <run-id>` instead of being reproduced locally.

CI also builds emitted projects through CMake. In a confined environment where
child processes cannot spawn CMake/Ninja, set `LSACA_SKIP_CMAKE_TESTS=1` to skip
only that CLI test; the dedicated CI job still covers the CMake backend.

## Release history

- **2.4.0** - the tool-grade language release. Array, slice and `name @ pattern`
  patterns with Rust-compatible exhaustiveness, values out of `break`,
  `HashMap::get_or_insert`/`get_or_insert_with` plus a denser collection
  surface, iterator `min`/`max`/`product`, `process::Command`, UDP
  networking with `SocketAddr`, `pub(super)`/`pub(in path)` visibility and
  module aliases, formatter crate mode (`lsaca fmt DIR`), a fourth real program
  (word statistics), the error-code index, and RFCs 0003/0004 that freeze the
  protocol and concurrency boundaries. The CI Rust toolchain is pinned to
  1.100.0. Source compatible with 2.3.0 apart from `continue` with a value,
  which is now reported as `E0308`.
- **2.3.0** - closes the promises made in 2.2.0. Borrowed iteration
  (`Vec::iter`, `iter_mut`, `into_iter`, `for` over `&Vec`/`&mut Vec`/
  `&[T]`/`&mut [T]`), borrowed `HashMap` iteration plus
  `HashMap::from`/`HashSet::from`, `rev` and `flat_map`, the remaining
  Option/Result combinators, Unicode `char` classification and case mapping,
  the `io` stream traits with `BufReader`/`BufWriter`, `Path`/`PathBuf` and
  `read_dir_entries`, partial moves, the error-code index, and the
  ROADMAP/STATUS governance pages with their CI checks. Source compatible with
  2.2.0 except that `Vec::iter` now borrows (the consuming iterator is
  `into_iter`) and `HashMap` iteration yields references.
- **2.2.0** - the expressiveness release. Fixed-size arrays: `[T; N]` types,
  the literals `[1, 2, 3]`, `[0; 4]` and `[]`, `.len()`/`.is_empty()`,
  bounds-checked indexing, elementwise equality, `Debug`, `[T; 0]`, implicit
  `&array` to `&[T]` and `&array[a..b]` slicing, and copy semantics when the
  element type is copy. Loop labels: `'outer: loop`/`while`/`for`,
  `break 'outer`, `continue 'outer`, also on fused iterator loops.
  Lightweight lifetimes: lifetime parameters on structs, enums and impls,
  `&'a T`/`&'a mut T`, references in struct/enum/tuple/`Option` fields,
  elided lifetime handling in function signatures and return types, and
  E0106/E0498 diagnostics; lifetimes are erased before HIR and never affect
  layout. `From` trait and trait associated functions, `?` error conversion
  through `impl From<Found> for Expected`, and `Error::source` with a default
  implementation. `lsaca fmt` (conservative, idempotent, `--check`/
  `--stdout`, refuses syntactically invalid input). Three real programs under
  `examples/programs/` (JSON, Markdown, Lisp) with C counterparts and
  byte-identical output, plus `[T; N]` and labeled-loop coverage in the test
  suite. Performance: `push_str(&format!(...))` fusion restored; cold and
  cached build times and the benchmark matrix were re-measured. Source
  compatible with 2.1.0.
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
