# Changelog

All notable user-visible changes to lsaca. The format follows Keep a Changelog
and the project follows Semantic Versioning; see [ROADMAP.md](ROADMAP.md) for the
policy and [STATUS.md](STATUS.md) for the current completion matrix.

Release dates before 2.3 were not recorded in the repository. This file was
reconstructed on 2026-11-15 from the release documentation; entries describe
user-visible behaviour, not the internal refactoring history.

## [2.5.0] - 2026-11-28

This release opens the system boundary with a deliberately small unsafe layer and
a direct C FFI. The safe subset and the ORC memory model are unchanged: unsafe is
a compile-time permission for the few operations the safe subset cannot express,
not a second language and not a runtime mode. The scope was frozen in
[ROADMAP.md](ROADMAP.md), the design is [RFC 0005](rfcs/0005-controlled-unsafe-and-ffi.md),
and every item has a [STATUS.md](STATUS.md) row.

### Added

- `unsafe { ... }` blocks and `unsafe fn` declarations. An unsafe context is the
  only place that can dereference or assign through a raw pointer, call an
  `extern "C"` function, call `MaybeUninit::assume_init`, call
  `CStr::from_ptr` or `CString::from_raw`, and construct, read or write a union.
  Violations are E0133 with a note that names the operation. The C backend lowers
  unsafe syntax to the same statements as safe syntax, so unsafe costs nothing at
  run time.
- Raw pointers `*const T` and `*mut T`: Copy, never reference counted or traced by
  the cycle collector, legal in aggregates, created with `&value as *const T`,
  `&mut value as *mut T`, integer-to-pointer casts and `std::ptr::null`/`null_mut`.
  Operations: `is_null`, `addr`, `add` and `sub` (the last two need unsafe), `*p`
  reads, `*p = value` writes (unsafe) and `==`/`!=` comparisons by address.
  Pointer-to-pointer, pointer-to-integer and integer-to-pointer casts are allowed.
  `String::as_ptr`, `Vec::as_ptr` and `Vec::as_mut_ptr` are the FFI accessors.
- `extern "C" { ... }` declarations of platform C functions. Parameter and return
  types must be FFI-safe: integers, floats, `bool`, `char`, raw pointers, `&T` and
  `&mut T` over FFI-safe `T`, `repr(C)` structs and unions by value or pointer,
  and `()`; everything else is E0770. Calling an extern function requires unsafe
  (E0133). Code generation binds the real symbol through a compiler-generated
  declaration with a GCC/Clang asm label, so libc and standard-header declarations
  do not conflict; the generated project links the C library and libm.
- `#[repr(C)]` on structs (and as a no-op on unions): declaration order, C
  alignment and padding, and the C size become part of the language contract.
  Other placements or arguments are E0771.
- `union` declarations whose fields overlay at offset 0. Fields are restricted to
  FFI-safe scalar, pointer, `repr(C)` struct and union types (E0772); a union has
  no methods, `Drop`, destructuring, equality or formatting (E0773);
  construction, reads and writes happen in unsafe. Unions pass by value across
  FFI.
- `CString`, `CStr` and `MaybeUninit` in the standard library: `CString::new`,
  `as_ptr`, `into_raw` and unsafe `from_raw`; `CStr::from_ptr`, `to_str` and
  `len`; `MaybeUninit::uninit`, `write`, `as_mut_ptr` and unsafe `assume_init`
  with exactly `T`'s size and alignment. `std::ptr::null` and `null_mut` are new.
- `docs/language/32-unsafe-and-ffi.md` states the safety contract, the FFI-safe
  type table and the ORC invariants unsafe code must preserve (never retain or
  release an ORC object by hand, never forge or rely on reference counts, raw
  pointers never own, managed values never cross the FFI boundary), together with
  the `ptr`, `cstring`, `cstr` and `mem` standard-library pages.
- A real FFI acceptance suite in `crates/lsaca/tests/programs/cffi/` that binds
  libc `strlen`, `strcmp`, `memcmp`, `div` and `getenv` and libm `frexp`,
  `tgamma` and `erf`, covering a `repr(C)` struct returned by value, out
  parameters through raw pointers and `MaybeUninit`, `CString`/`CStr` round
  trips, null pointer handling and a union round trip. It runs in the end-to-end
  tests, the generated-C checker and the ASan/UBSan CI job; a negative suite pins
  E0133, E0770, E0771, E0772, E0773 and E0001.

### Changed

- `unsafe` and `union` are now keywords, so they can no longer be identifiers
  (source breaking, announced in [ROADMAP.md](ROADMAP.md)).
- `HashSet::union` was renamed `union_with` because `union` became a keyword
  (source breaking).

### Fixed

- `is_null` compares the address with null instead of testing a cast to `bool`.
- Pointer-typed values inside aggregates, closures and tuples get the complete
  generated helper family (copy, release, debug, equality and trace), which fixed
  135 calls to missing helpers in generated C.
- `MaybeUninit<T>` works through generic impls and as a mutable-reference target;
  `assume_init`/`write`/`as_mut_ptr` forward correctly.
- Writes through `*mut T` no longer require the binding to be declared `mut`.
- `*const`/`*mut` equality compares addresses.
- Struct and enum definitions that only appear inside function signatures are
  registered recursively, fixing unknown-type C errors for `io::ErrorKind`.
- `#[repr(C)]` unions emit designated initializers, and unions inside aggregates
  get debug and equality helpers instead of invalid C.
- Extern declarations no longer collide with libc header declarations on GCC.
- Union methods, patterns, `Drop`, formatting and equality report E0773 instead
  of internal errors.

### Testing

1,211 workspace tests (1,035 library unit tests, 22 end-to-end tests over the full
program corpus including the FFI acceptance suite, 2 module audits, 121
robustness tests, 1 v2.3, 1 v2.4 and 1 v2.5 feature test and 28 CLI tests), plus
the 121-check C runtime smoke suite and the FFI acceptance suite under
AddressSanitizer and UndefinedBehaviorSanitizer.

### Benchmarks

- The official 2.5.0 matrix was rerun on 2026-11-28 on the reference laptop and
  published in `docs/language/21-benchmarks.md`; the raw run is
  `benchmarks/results/latest.*` and is frozen in
  `benchmarks/results/archive/v2.5/`.
- 14 of the 22 executed workloads run within 1.4x of C, 16 within 1.5x and 12
  within 1.15x, with a median of 1.14x. Cold single-file builds take
  0.83-2.76 s (median 1.06 s) and cached rebuilds 140-233 ms (median 176 ms).
- Absolute medians are not comparable with the 2.4.0 archive because host state
  moves short workloads; a same-host A/B run of the 2.4.0 and 2.5.0 compilers
  over fifteen workloads measured a median runtime ratio of 0.906 and a
  cold-compile median ratio of 0.951 (the C reference itself moved by
  0.72x-1.19x in that run), so there is no compiler regression. Seven
  C-normalised ratios moved outside the 10% budget in the official-run
  comparison and are documented in the benchmark page's regression table.
- The incremental benchmark guard baseline advances to
  `benchmarks/results/archive/v2.5/latest.json`.

## [2.4.0] - 2026-11-27

This release makes the language comfortable for programs bigger than benchmarks.
The scope was frozen in [ROADMAP.md](ROADMAP.md) before the work started; every
item has a [STATUS.md](STATUS.md) row and a definition-of-done trail.

### Added

- Array patterns `[a, b, c]`: they match a fixed-size array and the pattern's
  length must equal the array's length (E0308).
- Slice patterns `[a, b, ..]`, `[.., y]`, `[first, .., last]` and `[..]` over
  `&[T]`/`&mut [T]` and arrays, with match ergonomics (`&T`/`&mut T` bindings)
  and at most one `..`. Exhaustiveness is Rust compatible: a match whose arms
  cover every slice length is accepted, so `[]`, `[only]` and
  `[first, .., last]` together are exhaustive; element refutability inside the
  sub-patterns participates as usual and the non-exhaustive witness is rendered
  as a slice pattern.
- `name @ pattern` bindings (including `mut name @ pattern`) in every pattern
  position, with E0382 when the whole by-move binding and a moved-out non-`Copy`
  part overlap.
- Values out of `break`: `break value` and `break 'label value` give the loop
  expression that value's type (a bare `break` gives `()`), including loops
  nested inside the moved-from scope. A value in a `while`/`for` break is E0571
  and `continue value` is E0308.
- `HashMap::get_or_insert` and `get_or_insert_with` (the callable is an
  `fn() -> V`, capturing closures included); the borrowed `Entry` value stays
  deferred to v2.6 per [RFC 0002](rfcs/0002-hashmap-entry.md), so the methods
  always re-look-up the key after a potential insert.
- Collection API density: `HashSet::extend`, `BTreeSet::from`/`to_vec`,
  `BTreeMap::get_mut`/`first_key`/`last_key`/`from`/`to_vec`,
  `VecDeque::with_capacity`/`from`/`get_mut`/`front`/`back`/`to_vec` and
  `BinaryHeap::from`/`to_vec`; `BinaryHeap::to_vec` returns the internal heap
  order, not sorted order.
- Iterator completion: `filter_map`, `zip`, `chain`, `take_while`,
  `skip_while`, `position`, `nth`, `last`, and `min`/`max` returning
  `Option<T>`; `product` is widened to i32/i64/u32/u64/isize/usize/f32/f64
  while `sum` keeps its four integer types.
- System surface: `process::Command` (arg/args/current_dir/status/output),
  `ExitStatus` and `Output`; `net::SocketAddr` (parse/ip/port/to_string/display)
  and `UdpSocket` (bind/local_port/send_to/recv_from/set_timeout/close; Drop
  closes). The C runtime gains one additive ABI family implemented with
  `CreateProcessW` on Windows and `fork`/`execvp` on POSIX; the existing TCP API
  is unchanged.
- Visibility: `pub(super)` and `pub(in path)` with Rust semantics (E0433 for an
  unknown path, E0742 for a path that does not name an ancestor);
  `use path::module as alias;` and `pub use path::module as alias;`.
- Formatter crate mode: `lsaca fmt PATH` formats every module file of a package
  (auto-discovered `lsaca.toml`) after validating all of them; `--check` lists
  the files that would change and exits 1 without writing; `--stdout` keeps the
  single-file mode.
- [RFC 0003](rfcs/0003-derive-and-protocols.md) records the refusal of derive
  macros, operator overloading and user-defined `Display`/`Debug`/`Default`/
  `Ord`/`Hash` protocols in 2.x and fixes what each protocol means;
  [RFC 0004](rfcs/0004-concurrency.md) records the v3.0 concurrency decision.
- The diagnostic-code index grows to 89 codes (E0571 and E0742 are new) and
  `scripts/check_error_index.py` keeps it exact.

### Changed

- `lsaca fmt DIR` formats a whole package instead of failing; single-file mode
  is unchanged.
- Every workflow pins one Rust toolchain through `RUST_TOOLCHAIN` (1.100.0), so
  a new stable clippy lint cannot break main without a reviewed version bump.
- `continue value` is reported as E0308 with a "use break with a value" note
  instead of surfacing as a parse error.

### Fixed

- Slice and array patterns bind ergonomic references (`&T`/`&mut T`) by
  emitting the address of the element instead of its value, and match ergonomics
  now works identically for prefix, suffix and array patterns.
- Destructuring an array by value no longer double-releases elements that the
  pattern does not bind.
- The unreachable-pattern warning is emitted once per arm instead of twice.

### Testing

- 1,185 Rust tests pass in the workspace: 1,011 library unit tests, 21
  end-to-end suite functions (compiling and running the whole fixture set), 2
  module audits, 121 robustness tests, 1 v2.3 feature test, 1 v2.4 feature test
  and 28 CLI tests.
- The C runtime smoke suite passes its 121 checks in the release, debug and
  leak-tracking profiles.
- The generated-C checker, the documented-example runner, the link checker, the
  prose checker, the diagnostic-code index, the status traceability check, the
  documentation drift check and the real-program gate (four programs, eight
  invocations including the hand-written C references) pass.

### Benchmarks

- The official 2.4.0 matrix was rerun on 2026-11-27: 16 of the 22 executed
  workloads are within 1.4x of C, 17 within 1.5x and 9 within 1.15x, with a
  median of 1.20x (13, 14, 6 and 1.34x at 2.3.0). Cold single-file builds take
  0.57-2.54 s (median 0.88 s) and cached rebuilds 130-219 ms (median 164 ms).
  Four C-normalised runtime medians are outside the 10% budget against the
  2.3.0 archive; a same-host A/B run of the two compilers measured a median
  runtime ratio of 0.993, so those deltas are host-state noise, and the release
  page records the details. The results are archived in
  benchmarks/results/archive/v2.4/ and the page is regenerated by
  scripts/gen_bench_page.py from the machine-readable run.

## [2.3.0] - 2026-11-27

This release closes the promises the v2.0-v2.2 releases left open. The scope was
frozen in [ROADMAP.md](ROADMAP.md) before the work started; the completion matrix
is [STATUS.md](STATUS.md).

### Added

- Borrowed iteration: `Vec::iter()` now borrows the vector and yields `&T`
  (`Vec::iter_mut()` yields `&mut T`; `Vec::into_iter()` is the consuming
  iterator), and `for` loops accept `&Vec<T>`, `&mut Vec<T>`, `&[T]`,
  `&mut [T]` and `&HashMap<K, V>`.
- Borrowed `HashMap` access: `iter()` yields `(&K, &V)`, `keys()` yields
  `&K` and `values()` yields `&V` without cloning; `for (k, v) in &map`
  works. `HashMap::from(Vec<(K, V)>)` and `HashSet::from(Vec<K>)` were added;
  a later duplicate key wins. Iteration order is the map's internal slot order
  and is never insertion order.
- Lazy `rev()` and `flat_map()` iterator adapters. `rev()` buffers its source
  on the first advance; `flat_map()` accepts a closure returning either a
  `Vec<U>` or an iterator.
- Remaining `Option` combinators: `map_or_else`, `or_else`, `ok_or_else`,
  `unwrap_or_else`, `is_some_and`, `is_none_or` and `and`.
- Remaining `Result` combinators: `map_or`, `map_or_else`, `or`,
  `or_else`, `is_ok_and`, `is_err_and`, `inspect`, `inspect_err` and
  `unwrap_or_else`.
- Unicode character classification and case conversion on `char`:
  `is_alphabetic()`, `is_numeric()`, `is_alphanumeric()`,
  `is_whitespace()`, `to_uppercase()` and `to_lowercase()`, backed by
  Unicode 16.0.0 tables generated by `scripts/gen_unicode_tables.py`.
- Stream traits in `std::io`: `Read`, `Write`, `Seek` and `BufRead`,
  the `BufReader<R>` and `BufWriter<W>` adapters (the writer flushes on
  drop) and strict UTF-8 `read_line`/`read_to_string`. `File` implements
  all four traits, and `Box<dyn Read/Write/Seek/BufRead>` forwards to the
  concrete value.
- Path handling in `std::fs`: `Path` and `PathBuf` with `join`,
  `parent`, `file_name`, `extension`, `display`, `exists`,
  `is_file`, `is_dir`, `push`, `pop`, `as_str` and `as_path`, plus
  `DirEntry`/`ReadDir`/`read_dir_entries()` for directory iteration.
- Partial moves: destructuring a pattern moves each non-`Copy` field
  out of the value, and the source binding is then treated as moved as a whole
  (deliberately more conservative than Rust).
- [docs/language/31-error-codes.md](docs/language/31-error-codes.md) indexes all
  87 diagnostic codes with the phase that raises them;
  `scripts/check_error_index.py` keeps the page in sync with the compiler.
- Project governance: [ROADMAP.md](ROADMAP.md), [STATUS.md](STATUS.md) and the
  [RFC process](rfcs/README.md) (first RFC: [HashMap entry API](rfcs/0002-hashmap-entry.md),
  deferred to v2.4). New CI gates: docs drift (`scripts/docs_drift.py`),
  real example programs (`scripts/real_programs.py`), and a benchmark
  regression guard (`scripts/bench_guard.py`).
- The language manual was split further: [deviations.md](docs/language/deviations.md)
  and [limitations.md](docs/language/limitations.md) are now separate pages,
  and the roadmap page was rewritten as a short pointer.

### Changed

- `Vec::iter()` borrows instead of consuming. Code that relied on the old
  consuming behaviour must use `Vec::into_iter()`; this was announced in
  [ROADMAP.md](ROADMAP.md) as part of the v2.3 contract.
- `HashMap::iter()`, `keys()` and `values()` now yield references instead
  of owned copies, so iterating no longer duplicates strings or vectors.
- It is now legal to declare and assign through a `&mut T` struct field;
  a struct containing one is not `Copy`.

### Fixed

- Calling a method as an associated function (UFCS mistake) now reports E0599
  with a note explaining the method-call form, instead of an unrelated error.
- Collecting borrowed items (for example `Vec<&T>`) is rejected with a clear
  E0498 referring to the storage restriction instead of a codegen error.
- Method dispatch on types that have both lifetime and type parameters (for
  example `Pair<'a, T>`) resolves the right `impl` block.
- `&mut Box<dyn Trait>` forwarding implementations now codegen correct dynamic
  calls.
- Partial moves no longer copy the whole aggregate or double-drop the moved
  fields.
- POSIX path helpers for `join`/`parent`/`file_name`/`extension` match the
  Windows behaviour.

### Testing

- 1,149 Rust tests pass (978 library, 21 end-to-end suite functions compiling
  164 programs, 2 module audits, 120 robustness, 1 v2.3 feature file, 27 CLI),
  plus the 121-check C runtime smoke suite. The end-to-end suite now covers
  every fixture added by this release.

### Benchmarks

- The official 2.3.0 matrix was rerun on 2026-11-27 (22 executed and 3
  compile-only workloads, gcc.EXE 16.2.0) and published in
  docs/language/21-benchmarks.md; the raw results are archived in
  benchmarks/results/archive/v2.3/.
- 13 of the 22 executed workloads run within 1.4x of C and 14 within 1.5x, up
  from 9 and 14 at 2.2.0; the median lsaca/C ratio is 1.34x.
- Five C-normalised runtime medians (binary_trees, hashmap_string, sieve,
  loop_sum and fib_iter) sit outside the 10% budget against 2.2.0; they are
  listed in the benchmarks page's regression table together with a same-host A/B
  run of the 2.2.0 and 2.3.0 compilers that found no consistent slowdown (lsaca
  medians 0.88x-1.24x of the 2.2.0 compiler's, with the C reference medians
  moving by up to 16% between runs). The incremental speed guard now compares
  against the 2.3.0 archive.

## [2.2.0]

### Added

- Fixed-size arrays: the type `[T; N]`, literals `[1, 2, 3]` and `[value; N]`,
  the empty literal `[]`, `usize` indexing, `.len()`/`.is_empty()`, elementwise
  equality, `Copy` arrays, zero-length arrays, arrays in aggregates, `for` over
  `.iter()`, and the implicit `&array` to `&[T]` coercion.
- Loop labels on `loop`, `while`, `for` and before `while let`, with
  `break 'outer` and `continue 'outer`.
- Reference fields: structs, enums and tuple structs may store
  `&'a T`/`&'a mut T`, with lifetime parameters, `impl<'a>` blocks, elision
  rules and a conservative checker. Reference fields in anonymous tuples and in
  `Option` are rejected (E0498); this restriction is listed in
  [docs/language/limitations.md](docs/language/limitations.md).
- The `From` trait, `?` conversion of error types through `impl From`, and the
  `Error` trait's `source` default method.
- `lsaca fmt` with `--check` and `--stdout`.
- Borrowed access: `Vec::get_mut`, `first_mut`, `last_mut`, slice `get_mut`, and
  `HashMap::get`/`get_mut` taking `&K`.

### Changed

- `HashMap::get` now borrows the key and returns `Option<&V>`; `get_with` and
  `values` still clone.
- The documentation set is the 2.2.0 reference; known limitations are listed in
  [docs/language/limitations.md](docs/language/limitations.md).

## [2.1.0]

### Changed

- Integer literals infer `i32`; typed suffixes are range checked (`E0080`).
- Lengths, capacities, indices and counts are `usize`; indexing requires
  `usize`; `isize` is a complete type.
- Read-only text parameters take `&str`; string literals in `&str` position are
  zero-allocation static slices; only `String + &str` and `String += &str`
  concatenate.
- `parse_i64`/`parse_f64` return `Result<_, ParseIntError>` and
  `Result<_, ParseFloatError>`; `env::var` returns `Result<String, VarError>`.

### Added

- Tuple structs, unit structs and effective `mut` parameters.
- `isize`/`usize`, literal suffixes and annotation range checks.
- `Option::or`, `Option::xor`, `Result::unwrap_err`, iterator adapters
  `zip`, `chain`, `filter_map`, `take_while`, `skip_while`, `step_by` and the
  terminals `position`, `last`, `nth`, `reduce`, `product`, `inspect`,
  `for_each`.
- `time::Duration`, `Instant`, `SystemTime` and `unix_epoch()`; `net::Shutdown`;
  `math` constants; integer-shaped file and metadata return types.

## [2.0.0]

### Added

- The standard library as a normal, embedded `std` crate with modules `core`,
  `string`, `char`, `vec`, `iter`, `collections`, `fmt`, `math`, `io`, `fs`,
  `net`, `time`, `env`, `process` and `error`. The crate source is embedded at
  build time by `scripts/gen_stdlib_embed.py` into
  `crates/lsaca/src/stdlib_embed.rs`.
- `isize` and `usize`.
- `dyn Trait` behind `Box`, `Rc`, `&` and `&mut`, with per-trait-object vtables,
  object-safety rules and default trait method bodies.
- Unsized `[T]` and `str`, slice expressions (`base[a..b]`), and the
  `&Vec<T>`/`&String` coercions.
- `extern fn` intrinsic declarations, restricted to the `std` crate.
- Larger collections: `HashSet`, `BTreeSet`, `BTreeMap`, `VecDeque` and
  `BinaryHeap`.

### Changed

- The prelude names are re-exported from the `std` crate root; qualified
  `std::…` paths work for functions, constructors and associated items.
- The pattern rule for `Drop` values was relaxed: by-value bindings of a moved
  tuple element or enum payload are accepted and dropped exactly once.

## [1.7.0]

### Added

- Modules: `mod`/`use`, visibility, multi-file crates, `crate`/`self`/`super`
  paths, use trees and multi-file diagnostics.
- Packages: `lsaca.toml` with path dependencies, dependency graphs,
  `lsaca new`, binary and library packages, and package mode for
  `run`/`build`/`check`/`emit-c`.
- `assert_ne!`.

### Fixed

- Double `Drop` hook in destructuring patterns.
- Negative tail expressions in the parser.
- Synthetic main handling for library packages and unused `let _` temporaries.

## [1.6.0]

### Added

- The `char` type, character literals and escapes.
- `String` is always valid UTF-8, with character-oriented methods, Unicode case
  mapping, Unicode whitespace trimming, `Debug` escaping and validated
  `read_line`/`args`/`read_file` input.
- A Swiss-Table `HashMap` with a stronger deterministic hash and `char` keys.

### Changed

- The runtime is split into multiple translation units; the collector is
  iterative; float formatting uses shortest round-trip output; large programs
  emit `main.c` plus `lsaca_unit_N.c` units.

## [1.5.0]

### Added

- User-defined generics and traits with static dispatch and monomorphization,
  coherence checking and default trait method bodies.
- Lazy iterators and iterator adapters; the builtin `HashMap`.
- Structural equality for aggregates, references and maps; function values
  storable in aggregates.

## [1.4.0]

### Added

- Automatic `Copy` versus move-only types; move semantics and a move checker
  (`E0382`, `E0505`, `E0507`, `E0509`); `.clone()`; `impl Drop`;
  `&mut String`, `&mut Vec<T>`, `&mut Rc<T>` and `&mut Box<T>`.
- Fixed match whole-value double drop, `||` short-circuit lowering,
  `Vec::extend` code generation and a `Result::clone` leak.

## [1.3.0]

### Added

- `&T` and `&mut T`, borrow expressions, dereference, match ergonomics and a
  conservative borrow checker without explicit lifetimes.
- References are not storable in `Vec`, `HashMap`, `Result`, `Box` or arrays.

## [1.2.0]

### Added

- The postfix `?` operator and Option/Result combinators.
- The direct C compiler backend as the default; `--cmake` opts into the emitted
  CMake project.

### Changed

- Code generation performance: move-on-bind, in-place `for` over `Vec`, format
  fusion, a `Vec` push fast path and copy elision.
- The C runtime is split into several translation units.

## [1.1.0]

### Added

- `Box<T>` and format width/precision.
- A split language manual, cross-language benchmarks (compile time, runtime,
  memory), a documentation site, GitHub Pages and a comprehensive CI workflow.

## [1.0.0]

### Added

- The Rust implementation: a Cargo workspace with the `lsaca` library and the
  `lsaca` CLI, a C17 plus CMake backend, the ORC memory model, Rust-like
  language syntax and semantics, documentation and tests. The earlier C17
  implementation was archived during the rewrite.
