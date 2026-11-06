# Standard library reference

Version 2.1.0 moves the standard library out of the compiler: it is a normal
lsaca crate named `std`, stored in the repository under `stdlib/src/` and embedded
in the compiler at build time. Programs see it as the prelude (unqualified names)
and can also name its modules explicitly with `std::…` paths.

This page is the index of the library reference. The language-level chapter
[Standard library](../language/15-standard-library.md) keeps the description of
the builtin features that are part of the language surface (formatting macros, the
`?` operator, panics, exit codes) and the method reference for the builtin types.

## Module map

| Module | Page | Status in 2.1.0 |
| --- | --- | --- |
| `std::core` (Option, Result, Iterator) | [core](core.md) | API migrated from 1.7, additively extended |
| `std::string` (String) | [string](string.md) | API migrated from 1.7; `from_str`, `as_str`, `as_mut_str` and the `str` methods implemented; `&str` parameters in 2.1.0 |
| `std::char` (character classification) | [char](char.md) | ASCII classification methods implemented and extended in 2.1.0; Unicode classification not implemented |
| `std::vec` (Vec) | [vec](vec.md) | API migrated from 1.7; `sort`, `as_slice` and `as_mut_slice` implemented; `usize` positions in 2.1.0 |
| `std::iter` (iterators and adapters) | [iter](iter.md) | API migrated from 1.7; `zip`, `chain`, `filter_map`, `take_while`, `skip_while` and `step_by` added in 2.1.0 |
| `std::fmt` (formatting) | [fmt](fmt.md) | compiler-implemented; module is a reference layer |
| `std::math` (math functions) | [math](math.md) | free functions migrated from 1.7; the 16 `f64` constants added in 2.1.0 |
| `std::collections` | [collections](collections/README.md) | HashMap migrated; HashSet, BTreeSet, BTreeMap, VecDeque and BinaryHeap implemented; `usize` lengths in 2.1.0 |
| `std::io` and `std::io::file` | [io](io.md) | error types, file and stream handles and text helpers implemented; the stream traits, `BufReader` and the `io::Result` alias are not implemented |
| `std::fs` | [fs](fs.md) | path, directory and metadata API implemented; `FileType` and `SystemTime` metadata added in 2.1.0 |
| `std::net` | [net](net.md) | implemented (plain TCP, no TLS); `Shutdown` added in 2.1.0 |
| `std::time` | [time](time.md) | implemented; `Duration`, `Instant`, `SystemTime` and `sleep(Duration)` added in 2.1.0 |
| `std::env` | [env](env.md) | implemented; `var` returns `Result<String, VarError>` in 2.1.0 |
| `std::process` | [process](process.md) | `args` and `exit(i32)` implemented |
| `std::error` | [error](error.md) | the minimal `Error` trait is implemented; `io::Error`, `ParseIntError` and `ParseFloatError` implement it |

The collections module has its own index: [collections](collections/README.md),
with one page per container — [HashMap](collections/hash-map.md),
[HashSet](collections/hash-set.md), [BTreeSet](collections/btree-set.md),
[BTreeMap](collections/btree-map.md), [VecDeque](collections/deque.md) and
[BinaryHeap](collections/binary-heap.md).

"API migrated from 1.7" means the surface is described in the language chapter;
the implementation has moved into the `std` crate on top of compiler intrinsics,
so programs do not need to change. "Implemented" means the code exists in
`stdlib/src/` and has been exercised against the 2.1.0 compiler. Modules that are
only partly implemented describe the implemented subset first and mark the rest
*planned*: the API fixed by the design but not implemented yet.

## How the standard library works

* The `std` crate is a normal lsaca crate: its source is type-checked with the
  same rules as user code. It is embedded as `include_str!` data and parsed with a
  reserved `SourceMap` base, so a diagnostic or a panic inside the library is
  reported as `std/vec.ls:line:column`.
* The crate root of `std` re-exports the core types with `pub use`, which is why
  `Vec`, `String`, `Option`, `Result`, `Rc`, `Box`, `HashMap` and `Iterator` need
  no import. There is no separate `prelude` file since 2.1.0: the prelude is the
  crate root of `std`, and the iterator traits and helpers are defined in
  `stdlib/src/iter.ls`. Qualified function, constructor and associated-item paths
  such as `std::fs::read_to_string`, `std::collections::HashMap::new()` and
  `std::io::ErrorKind::NotFound` work, and `use std::…` imports of those items
  work. Qualified type names in type position are not resolved yet
  (`let m: std::collections::HashMap<i64, i64>` is `E0412`), and a trait impl must
  name the trait with a single identifier after importing it
  (`impl std::error::Error for T` is `E0001`: trait paths are not supported in
  impl blocks). These are tracked as `std`-crate gaps.
* `Vec`, `String`, `Option`, `Result`, `Rc`, `Box`, `HashMap`, `char` and the
  slice/text types are builtin markers: the type checker maps them to compiler
  types, not to user structs. User code cannot declare or implement them; a
  redeclaration is `E0428` and an `impl` on a builtin is `E0118`.
* Operations that only the compiler can provide are declared in the `std` crate as
  `extern fn` intrinsics and implemented by code generation. A user crate may not
  declare `extern fn` (error `E0001`: extern functions belong to the standard
  library), and the name `std` is reserved for packages.
* The library is written in safe lsaca; it uses no unsafe code, no panics on OS
  failures (errors are returned as `Result<T, io::Error>`; the `io::Result<T>`
  alias itself is not implemented and is planned for a later release) and no
  locale- or platform-dependent behaviour.
* All APIs are deterministic: iteration order of the collections is documented, and
  the compiler never depends on hash-map order.

## Packaging and paths

The library is versioned with the compiler (2.1.0). There is no separate library
package to download and no version resolution for it. Paths into the library use
the crate name `std`, which cannot be shadowed by a user package.

## Reading this reference

Each page starts with a **status line** and then lists the API. Entries marked
*planned* are not implemented in 2.1.0; they are planned for a later release.
Entries without a marker are either part of the migrated 1.7 surface or are
implemented in the `std` crate. Partly implemented modules say so in the status
line and name the missing parts. Examples in this subtree are written for the
2.1.0 semantics (default `i32`, `usize` lengths, `&str` text parameters). Every
runnable (```ls) example here is part of the compiled example test plan, like the
language chapters: `scripts/docs_verify.py` writes each one to disk, executes it
with the 2.1.0 compiler and compares its output with the stated result. Prose-only
API listings are not examples and make no verified-output claim.

## What next

* [Standard library chapter](../language/15-standard-library.md) - language-level
  library behaviour and the builtin method reference.
* [Trait objects](../language/25-trait-objects.md) and
  [Slices and str](../language/26-slices-and-str.md) - the reference and trait
  object types.
* [Roadmap and differences from Rust](../language/16-roadmap.md) - what 2.1 adds.
