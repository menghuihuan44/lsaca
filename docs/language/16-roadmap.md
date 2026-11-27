# Roadmap

lsaca borrows Rust's surface syntax and some of its semantics, but it is a
smaller language with an explicitly different memory model. This page records
what is planned next and the compatibility story. The detailed reference lives
on two companion pages: deliberate design differences are catalogued in
[Deviations from Rust](deviations.md), and verified gaps are listed in
[Known limitations](limitations.md).

The forward-looking plan, the completion matrix and the release history live at
the repository root: [ROADMAP.md](../../ROADMAP.md) is the contract,
[STATUS.md](../../STATUS.md) records what is complete, layer by layer, and
[CHANGELOG.md](../../CHANGELOG.md) is the user-visible history.

Version-by-version history — what each release added — lives in
[CHANGELOG.md](../../CHANGELOG.md); the plan is in [ROADMAP.md](../../ROADMAP.md)
and the completion matrix in [STATUS.md](../../STATUS.md).

## Compatibility

lsaca 2.2.0 is an additive release for most code: arrays, loop labels,
reference fields and `From`/`?` conversions are new forms, and `lsaca fmt` is a
new command. The largest break remains 2.1.0, which breaks programs that relied
on 2.0's numeric and text defaults; the rest of the standard-library surface is
additive. The 2.2-specific changes are:

* **New syntax.** Array types `[T; N]` and literals, array repeat literals
  `[value; N]`, loop labels `'outer: …` with `break 'outer`/`continue 'outer`,
  lifetime parameters (`struct View<'a>`, `impl<'a>`) and reference fields are
  accepted; previously these were parse errors.
* **New library surface.** `From`, `Error::source`, `Vec::get_mut`/
  `first_mut`/`last_mut`, slice `get_mut` and `HashMap::get`/`get_mut` (taking
  `&K` — call with `&key`); `get` on a map now borrows its key and returns
  `Option<&V>`, while `get_with` and `values()` keep cloning.
* **New command.** `lsaca fmt <FILE.ls>` with `--check` and `--stdout`.

The 2.1 incompatibilities remain when upgrading from 2.0 or earlier:

* **Integer literals default to `i32`.** Code that let an unsuffixed literal or
  a `0` initializer infer `i64` needs a suffix (`1i64`, `1_000_000_000i64`) or an
  annotation, and an accumulator such as `let mut total = 0;` now counts in
  `i32`.
* **Lengths, indices and counts are `usize`.** `len()`, indexing, `find()`
  offsets, `count()`, `enumerate()`, the `with_capacity` argument and the
  truncation and swap arguments are `usize`; combine them with arithmetic
  values through `as`.
* **Text parameters take `&str`.** Standard-library methods that used to take a
  `String` (`contains`, `starts_with`, `ends_with`, `find`, `split`,
  `split_whitespace`, `trim_start`, `trim_end`, `replace`, `push_str`, …) now take
  `&str`: pass a literal directly or `&owned` for a `String` variable. A string
  literal in a `&str` position is sliced statically with no allocation.
* **Owned text constructors stay owned.** `String::from` and `String::from_str`
  return `String`, `replace` returns `String`, and concatenation is limited to
  `String + &str` and `String += &str` (no `String + String`, no
  `&str + &str`).
* **New error types.** `str::parse_i64` and `str::parse_f64` return
  `Result<_, ParseIntError>` and `Result<_, ParseFloatError>` instead of a
  `String` error; `env::var` returns `Result<String, VarError>`; `io::Error` and
  `env::VarError` implement `error::Error`.
* **Time, file and network API shapes.** `time` gains
  `Duration`/`Instant`/`SystemTime` and the epoch function `unix_epoch()`;
  `File::read`/`write` return
  `Result<usize, Error>`, `File::seek` returns `Result<u64, Error>`,
  `fs::Metadata::size()` returns `u64`, `fs::Metadata::modified()` returns
  `Result<SystemTime, Error>`, and `TcpStream::shutdown` takes a `Shutdown`
  value.
* **Tuple structs are legal syntax** (`struct Meters(i64);`) and `mut` on a
  parameter now has an effect; both were previously an error or ignored.
* **There are no `i128`/`u128` types**, and their literal suffixes are rejected
  by the lexer.
* **New diagnostics**: a literal that does not fit its type is `E0080`, and an
  `i32`/`i64` used as an index or length is a type mismatch (`E0308`) — use
  `as `usize`.

The 2.0 changes (`dyn` and `extern` keywords, trait objects and slices, the
embedded `std` crate, the new `E0038`/`E0046`/`E0407`/`E0053`/`E0118`/`E0369`
diagnostics) still apply when upgrading directly from 1.7.0.

## Benchmarks

Compile-time, runtime and peak-memory measurements are republished with each
release and compared with C, Rust, Python and Node; see
[Benchmarks](21-benchmarks.md) for the current tables, methodology and archive.

## What next

* The language tour starts at [Getting started](01-getting-started.md).
* Modules, files and visibility: [Modules](23-modules.md).
* Packages, `lsaca.toml` and path dependencies: [Packages](24-packages.md).
* The `char` type, UTF-8 strings and Unicode: [Characters and
  Unicode](22-characters-and-unicode.md).
* User-defined generics: [Generics](17-generics.md).
* Traits and trait bounds: [Traits](18-traits.md).
* Lazy iterators: [Iterators](19-iterators.md).
* The builtin hash map: [HashMap](20-hashmap.md).
* Trait objects and dynamic dispatch: [Trait objects](25-trait-objects.md).
* Slices and `str`: [Slices and str](26-slices-and-str.md).
* Fixed-length arrays: [Arrays](27-arrays.md).
* Labelled loops: [Loop labels](28-loop-labels.md).
* Reference fields and lifetimes: [Lifetimes](29-lifetimes.md).
* Error conversion with `From` and `?`: [From and ?](30-from-try.md).
* The standard library modules: [Standard library
  reference](../stdlib/README.md).
* Ownership, moves and destruction: [Ownership and
  moves](11-ownership-and-moves.md).
* Non-owning views and the borrow checker: [References and
  borrowing](12-references-and-borrowing.md).
* Every builtin in detail: [Standard library](15-standard-library.md).
* The memory model that ties ownership and ORC together:
  [Memory model](05-memory-model.md).
