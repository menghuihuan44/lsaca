# Roadmap and deviations from Rust

lsaca borrows Rust's surface syntax and some of its semantics, but it is a
smaller language with an explicitly different memory model. This page lists the
intentional differences, the known gaps in the current release, and what is
planned next. It describes lsaca **2.1.0**.

## Intentional deviations from Rust

These are design decisions, not bugs. Code written for Rust will need edits in
these areas.

1. **Ownership with automatic `Copy` and `Clone`.** Version 1.4 added ownership
   and move semantics close to Rust's, but the marker traits are derived
   automatically instead of implemented by hand. A type is `Copy` when all of
   its parts are: integers, floats, `bool`, `char`, `()`, tuples,
   `Option<T>` and `Result<T, E>` of `Copy` types, references (including the
   2.0 fat references `&[T]`, `&str` and `&dyn Trait`), and user
   structs/enums whose fields are all `Copy` and which do not implement
   `Drop`. Everything else (`String`, `Vec<T>`, `Rc<T>`, `Box<T>`,
   `Box<dyn Trait>`, `Rc<dyn Trait>`, function values, and user types containing
   them) is move-only. `Clone` is automatic
   too: every type except those that implement `Drop` has `.clone()`, and
   `impl Copy`/`impl Clone` blocks are rejected. `Drop` is the only builtin
   trait with special code generation; user-defined traits exist since 1.5.0 and
   since 2.0 can be used behind a pointer as trait objects, where calls dispatch
   through a vtable. There are still no lifetime
   annotations; ORC remains the memory manager — heap allocations are
   reference-counted and freed when the last owner is released, so there is no
   `free`, no `drop(value)` builtin and no `std::mem::drop`. See
   [Ownership and moves](11-ownership-and-moves.md) and
   [Memory model](05-memory-model.md).
2. **References are non-owning and restricted.** Version 1.3 added `&T` and
   `&mut T`, with borrow expressions (`&place`, `&mut place`), dereference
   (`*r`) and match ergonomics — but there are no raw pointers, no address
   arithmetic and no way to observe an address. References cannot be stored in
   structs, enums, tuples, `Vec`, `Option`, `Result` or `Box`, cannot be captured
   by closures, and cannot be cast with `as`. Version 1.5 adds `==` between
   references, which compares the pointed-to values. Mutable references work for
   scalars, tuples, structs and enums, and for the owned heap types
   `&mut String`, `&mut Vec<T>`, `&mut Rc<T>` and `&mut Box<T>`; `&mut` to a
   user struct or enum that contains a heap field is still rejected (`E0498`).
   There is no lifetime syntax: elision follows fixed rules. Version 2.0 adds
   unsized borrowed forms — `&[T]`, `&mut [T]`, `&str`, `&mut str` and
   `&dyn Trait`/`&mut dyn Trait` — produced by coercion from `&Vec<T>`,
   `&String`, `&T` or by a slice expression; they are two-word values, still
   never stored in aggregates. See
   [References and borrowing](12-references-and-borrowing.md) and
   [Slices and str](26-slices-and-str.md).
3. **Modules and packages are compile-time organisation.** Version 1.7 adds
   `mod`/`use`, visibility and multi-file crates, plus a small package manager for
   path dependencies. Modules are erased before code generation: the crate graph
   is resolved and flattened into one program, with no dynamic linking, no
   separate compilation and no runtime lookup. Imports are explicit and non-glob
   (glob imports are rejected), visibility has three levels (private, `pub(crate)`
   and `pub`) with no `pub(super)`/`pub(in path)`, and only the root crate has an
   entry point. Packages use a strict `lsaca.toml` subset with path dependencies
   only: no registry, git dependencies, workspaces, features or lockfile. See
   [Modules](23-modules.md) and [Packages](24-packages.md).
4. **Generics are monomorphized and late-checked; dispatch is static unless you
   ask for a trait object.** Version 1.5 adds user-defined generics and traits,
   with deliberate limits: generic items are structs, enums, free functions,
   inherent impls and trait impls; generic code is monomorphized, with no code
   sharing between instantiations. The body of a generic function is checked once
   per concrete instantiation, so a type error appears only for the
   instantiations that are actually used and an unused generic body is never
   checked. Unused type parameters are accepted (Rust rejects them). There are
   no `where` clauses, associated types, const generics, default type parameters
   or specialization. Version 2.0 adds trait objects (`dyn Trait` behind
   `Box`, `Rc`, `&` or `&mut`) for dynamic dispatch and default method
   bodies; traits still cannot declare associated functions, generic methods,
   supertraits, associated types or constants, and only object-safe traits can
   be used as types. See [Generics](17-generics.md),
   [Traits](18-traits.md) and [Trait objects](25-trait-objects.md).
5. **No user macros.** The only macros are the builtin formatting/assertion
   macros and `vec!`. There is no `macro_rules!`, no derive and no attributes.
6. **Closures capture by value.** A closure owns its captured environment: it
   copies `Copy` values and moves move-only values into the closure, so the
   original binding becomes unusable unless its value is `Copy`. `move` is
   accepted but changes nothing, and capturing a reference is rejected
   (`E0498`). Shared mutable state inside a closure is done with a captured
   `Rc<T>`. Closures and function values are move-only; since 1.5 they can be
   stored in structs, enums, tuples, `Vec`, `Box` and as HashMap values. See
   [Closures](14-closures.md).
7. **Integer defaults, suffixes and pointer-sized types.** The integer types are
   `i8`, `i16`, `i32`, `i64`, `isize`, `u8`, `u16`, `u32`, `u64` and `usize`
   (there are no `i128`/`u128`). An unsuffixed integer literal has type `i32`
   and an unsuffixed float literal has type `f64`; a literal may carry a type
   suffix (`1i64`, `0xFFu8`, `1_000_000usize`, `1.5f32`), and a suffixed literal
   is checked to fit its type. `usize` is the standard type for lengths,
   indices and counts: collection and slice lengths and capacities, indexing,
   `find` offsets, iterator `count` and `enumerate` and `Rc::strong_count` are
   `usize`. An unsuffixed literal in a `usize` context (an index, a `len()`
   comparison, a `usize` parameter) is `usize`, elsewhere it is `i32`. `isize`
   and `usize` are pointer-sized (64 bits on every supported target), cast with
   `as` and print like the other integers.
8. **Characters are scalars; strings are UTF-8.** Since 1.6.0 `char` is a
   `Copy` 32-bit Unicode scalar value with character literals and
   character-literal patterns, and `String` is always valid UTF-8. The string
   API keeps byte-oriented methods (`len`, `find`, `substring`, whose offsets
   are byte indices and whose lengths and indices are `usize`) next to
   character-oriented ones (`char_count`, `chars`, `char_at`, `push`, `pop`,
   `is_char_boundary`). String concatenation borrows its right operand: only
   `String + &str` and `String += &str` are supported, while `String + String`
   and `&str + &str` are rejected. There is no `u8` byte-string type and no
   byte-string literal. See
   [Characters and Unicode](22-characters-and-unicode.md) and
   [Standard library](15-standard-library.md#string).
9. **No arrays; slices exist only behind references.** There is no array type
   `[T; N]`. Version 2.0 adds the unsized types `[T]` and `str`, which exist
   only behind `&` or `&mut`: `&[T]`, `&mut [T]`, `&str` and `&mut str`.
   A slice expression (`&values[1..3]`) produces one, and `&Vec<T>`/`&String`
   coerce to them; `[T]` and `str` methods live in the `std` crate. `Vec<T>`
   remains the growable sequence type, and tuples nest. See
   [Slices and str](26-slices-and-str.md).
10. **Restricted patterns.** No `@` bindings, no `ref` patterns, no slice and no
    range patterns. String-literal and character-literal patterns are supported,
    tuple struct and unit struct patterns were added in 2.1.0, or-patterns
    (`p | q`) and match guards are fully supported, and a reference scrutinee is
    matched with match ergonomics rather than a `&` pattern. See
    [Pattern matching](13-pattern-matching.md).
11. **No loop labels and no values out of `break`/`continue`.** Loops have
    type `()`; `break value` is a compile error, and `break`/`continue` always
    target the innermost loop — labelled loops are not in 2.1.0. Use a mutable
    accumulator variable, a flag or a helper function instead.
12. **A standard library crate with iterators and collections.** The 1.5
    release added lazy iterators and the builtin `HashMap<K, V>`; version 2.0
    moved the whole standard library into a normal `std` crate that is embedded
    in the compiler (`stdlib/src/lib.ls`, parsed as `std/*.ls`). In 2.1.0 it
    covers `core` (`Option`, `Result`, `Iterator`), `string`, `char`, `fmt`,
    `math`, `vec`, `iter`, `collections` (`HashSet`, `BTreeSet`,
    `BTreeMap`, `VecDeque`, `BinaryHeap`), `io` (`File`, `Error`/`ErrorKind`; the `Read`/`Write`/`Seek`/
    `BufRead` traits and `BufReader` are planned), `fs`, `net`
    (plain TCP), `time`, `env`, `process` and `error` (the `Error` trait).
    Iterators provide `zip`, `chain`, `filter_map`, `take_while`, `skip_while`
    and `step_by` next to `map`, `filter`, `take`, `skip` and `enumerate`, with
    the terminals `collect`, `fold`, `count`, `sum`, `product`, `any`, `all`,
    `find`, `position`, `last`, `nth`, `reduce`, `inspect` and `for_each`;
    the `rev` and `flat_map` adapters remain planned for a later release. `HashMap` is still the hash map
    (keys are the integer types, `bool`, `char` and `String`) and still cannot
    be iterated with `for` — use `keys()` or `values()`; asking a vector for
    an iterator with `.iter()` still **consumes** the vector. Formatting still
    has no named or indexed arguments and no integer bases. The `?` operator is
    supported for `Option` and `Result`. The 2.1.0 standard library adds
    `Option::or`, `Option::xor` and `Result::unwrap_err`; the other Rust
    combinators (`Option::or_else`/`map_or_else`/`ok_or_else`/
    `unwrap_or_else`/`unwrap_or_default`/`is_some_and`/`is_none_or`/`and`,
    `Result::map_or`/`map_or_else`/`or`/`or_else`/`is_ok_and`/`is_err_and`/
    `inspect`/`inspect_err`) are not implemented in 2.1.0. Text parameters of
    the standard library take `&str` instead of an owned `String` (`env::var`
    is the one exception: `&str` in, `Result<String, VarError>` out;
    `io::Error::new`/`other` also keep taking an owned `String` message), while
    constructors such as `String::from` keep taking an owned `String`, and a
    string literal in a `&str` position is a static slice that does not
    allocate. The design is fixed; which parts the 2.1.0 compiler implements is
    recorded module by module in the [standard library
    reference](../stdlib/README.md). What exists is documented in
    [Standard library](15-standard-library.md),
    [Standard library reference](../stdlib/README.md),
    [Iterators](19-iterators.md) and [HashMap](20-hashmap.md).
13. **Equality is structural for most values, identity for `Rc`.** Since 1.5,
    `==` works for `Vec`, `Option`, `Result`, tuples, user structs and enums,
    references and `HashMap` as well; structs and enums compare field by field,
    references compare the pointed-to values, and maps compare contents ignoring
    insertion order. `Rc<T>` equality remains identity (same cell), `Box<T>`
    compares structurally, characters compare by scalar value, and function
    values and trait objects have no `==` or `<` (`E0369`).
14. **No unsafety.** There is no `unsafe`, no FFI and no threads. The generated
    C is not exposed to the language. The `extern fn` declaration added in 2.0
    is not an FFI mechanism: it declares the compiler's own intrinsics, only the
    `std` crate may write one (user code is `E0001`), and it is not a
    first-class value (`E0425`).
15. **Aggregate patterns copy instead of moving.** A `let` pattern that
    destructures a tuple or struct copies the value into the new bindings:
    `Copy` parts are copied, and `String`, `Vec`, `Rc` and `Box` parts are
    deep-copied (an `Rc`/`Box` binding still points at the same cell or heap
    value, and the reference count is retained). The source value remains fully
    usable after the pattern. This differs from Rust, where destructuring moves
    non-`Copy` fields out; partial moves are planned for v2.2. Values with a
    `Drop` implementation keep their move-only pattern behaviour (a `Drop`
    type's own fields stay `E0509`). `match` is unaffected and still moves its
    scrutinee. See
    [Variables](06-variables-and-mutability.md#destructuring-bindings) and
    [Ownership and moves](11-ownership-and-moves.md).

## Known limitations in 2.1.0

The following are implementation gaps or deliberate restrictions, verified
against the 2.1.0 compiler. They are expected to be fixed in later releases;
avoid them in code you intend to keep. Where a module or API is fixed by the
design but not implemented in 2.1.0, the [standard library
reference](../stdlib/README.md) and the module pages mark it *planned* for a
later release.

| Area | Limitation | Workaround |
| --- | --- | --- |
| Casts to `bool` | An integer or a float cannot be cast to `bool` (`E0605`); only `bool` values are booleans and conditions never convert numbers | Compare explicitly, for example `value != 0` |
| Loop variables | `for mut i in …` parses and lets you reassign `i` | Keep loop variables immutable |
| Constants | Constant types are restricted to scalars (including `char`) and `String`; constant expressions allow arithmetic but not comparisons or function calls. This is why the epoch is the function `time::unix_epoch()` rather than a `UNIX_EPOCH` constant | Use a `let` or compute at runtime |
| String indexing | `text[i]` is rejected (`E0608`) | Use `char_at`, `substring`, `find` or `split` |
| Character classification | A `char` supports comparison, patterns, casts, formatting and the string APIs. The 2.1.0 `std::char` module provides the ASCII classification methods (`is_ascii_alphabetic`, `is_ascii_digit`, `is_ascii_whitespace`, `is_ascii_punctuation` and so on), the `to_ascii_uppercase`/`to_ascii_lowercase` conversions and `to_digit`; the Unicode helpers (`is_alphabetic`, `is_numeric`, `is_whitespace`, Unicode case conversion) are not implemented in 2.1.0 and are planned for a later release, and there are no `std::char` free functions in 2.1.0 | Use the ASCII predicates for ASCII input; compare against character ranges or use casts and the provided string methods for the rest |
| Unicode properties | Case mapping and whitespace trimming use the generated Unicode tables; there is no general Unicode property or normalisation API | Precompute the characters you need |
| Formatting | Width, fill and alignment apply to a user struct/enum as a whole, but the `+`/`0` flags and precision are rejected for them (`E0001`); character precision and the `+`/`0` flags are also rejected; no named or indexed arguments | Format fields individually, or apply width/fill/alignment to the whole value |
| Numbers | `pow` is float-only; no integer exponentiation builtin | Convert to `f64` or write a loop |
| Generic `parse` | `parse::<T>()` accepts only `i64` and `f64` as the target type in 2.1.0; any other numeric target is `E0107` | Use `parse_i64()`/`parse_f64()`; for another integer type parse an `i64` and cast |
| Patterns | No `@`/`ref`/slice/range patterns, and a reference pattern like `let &x = r` is rejected | Match a reference scrutinee directly: the ergonomic bindings are references, so read and write through `*binding` |
| Reference storage | References cannot be fields, tuple elements, `Vec` elements, `Option`/`Result` payloads, `Box` payloads or closure captures (`E0498`) | Keep references in locals, parameters and return values |
| Mutable references | `&mut` to a user struct/enum containing a heap field is rejected (`E0498`); mutable references to scalars, `Copy` aggregates, `char`, `String`, `Vec<T>`, `Rc<T>` and `Box<T>` work | Take `&T` or restructure the code |
| Method receivers | A by-value `self` method cannot return a reference (`E0106`), and a `&self` method can only return shared references (`E0596`) | Return shared references from `&self` methods, mutable references from `&mut self` methods, and owned values from by-value methods |
| Return elision | No lifetime syntax; a free function returning a reference needs exactly one reference parameter, otherwise `E0106` | Pass exactly one reference parameter or return an owned value |
| Moves | No partial moves: reading a `Copy` field is fine, but moving a non-`Copy` field, element or dereferenced value out of a bigger value is rejected (`E0507`); a `let` pattern that destructures a whole tuple or struct copies it instead of moving parts out (deviation 15) | `.clone()` the field, or restructure the value |
| Move checking | Moving a value while a live borrow exists is rejected (`E0505`); using a value after a move is rejected (`E0382`) | Order borrows and moves explicitly, or `.clone()` before the move |
| Drop in patterns | A value whose type implements `Drop` cannot be destructured by its own fields by value (`E0509`); moving the whole value, a tuple element or an enum payload through a pattern is allowed and drops it exactly once | Match a reference (`match &value`) or restructure the value |
| Cloning | A type that implements `Drop` cannot be cloned (`E0277`) | Use references, or restructure so the value is not cloned |
| Custom impls | `impl Copy` and `impl Clone` are rejected because both are derived automatically | Rely on the automatic derivation |
| Explicit drop | There is no `drop(value)` function (`E0425`) and no `.drop()` method (`E0599`); a value is dropped when its last owner is released | End the scope early, or move the value into a helper that consumes it |
| Drop at exit | `Drop` hooks do not run when the process exits via `exit()` or a panic; they do run when `main` returns normally | Do not rely on hooks for cleanup at abnormal exits |
| Borrow granularity | `Vec` borrows are whole-container: two `&mut` borrows of different elements conflict (`E0499`) | Mutate elements one at a time or rebuild the vector |
| Borrow checker in one expression | Passing two mutable borrows as the two arguments of one call, or combining them with `+`, is rejected (`E0499`); the short-circuit operators `&&` and `\|\|` accept them because each operand becomes its own condition | Bind each result to a local, then combine the locals |
| `for` over references | `for x in &vec` is rejected (`E0277`); loops iterate over a `Vec<T>` value, an integer range or an iterator | Iterate by value, use `.iter()`, or use an index loop over a `&Vec` |
| Reference casts | References have no `as` conversion (`E0605`) | Convert the pointed-to value with `*r` |
| UTF-8 input | `read_line()` and `args()` panic on invalid UTF-8 (`exit 101`); `read_file()` returns an `Err` instead (the invalid-data kind) | Feed the program valid UTF-8, or check the `Result` from `read_file` |
| Trait objects | `dyn Trait` values exist only behind `Box`, `Rc`, `&` or `&mut` (bare `dyn Trait` is `E0001`); the trait must be object-safe (`E0038`: every method `&self`/`&mut self`, no generics, no associated items, no `Self` outside the receiver); there is no upcasting between trait objects; `Box<dyn Trait>` has no `get`/`set` (`E0001`) and `==`/`<` are `E0369`; a non-object-safe trait still works for generic/static dispatch | Make the trait object-safe, or use generic bounds when the concrete type is known |
| Trait shape | No associated functions, generic methods, supertraits, associated types or constants; default method bodies are supported since 2.0; `impl Trait for Type` only for user structs and enums (a builtin target is `E0118`, except in the `std` crate) | Implement every required method explicitly; use generic bounds for the rest |
| Slices and `str` | `[T]` and `str` are unsized and exist only behind `&`/`&mut` (`E0001` otherwise); there is no array type `[T; N]`; slice indices are `usize`, so an `i32`/`i64` index is a type error (`E0308`) — cast with `as`; references still cannot be stored in aggregates; user crates cannot write `impl [T]` or `impl str` (`E0118`) | Use `Vec<T>`/`String` for storage and take `&[T]`/`&str` parameters for borrowing |
| Integer types | There are no `i128`/`u128` types, and their literal suffixes are rejected while lexing; an unsuffixed integer literal is `i32`, so a value outside that range needs a suffix or an annotation (a suffixed literal that does not fit its type is `E0080`) | Write `1i64` or annotate the binding; use `i64`/`u64` for very large values |
| Extern functions | `extern fn` declarations are accepted only in the `std` crate (user code is `E0001`) and are not first-class values (`E0425`) | Call the public `std` function or method instead |
| Standard library status | The `std` crate is implemented module by module in 2.1.0; the [standard library reference](../stdlib/README.md) marks every page *implemented* or *planned*. Remaining gaps in 2.1.0: `io` has the error types, `File` and the byte/text helpers but not the `Read`/`Write`/`Seek`/`BufRead` traits or `BufReader`; `fs` lacks `PathBuf`/`Path` and directory iteration; `net` is plain TCP without TLS, UDP, DNS or `SocketAddr`; `time` has the clocks, `Duration`, `Instant` and `SystemTime` but no time-zone database; there is no `Command`, `Ordering` or `FromUtf8Error`; `parse::<T>()` accepts only `i64` and `f64` targets in 2.1.0; iteration over `&[T]` and a few `str`/`Vec` helpers are still planned. `use std::…` imports work, while qualified type names in type position do not resolve (`E0412`) and trait paths in `impl` blocks are rejected (`E0001`, import the trait instead) | Read the module page for what exists in 2.1.0; OS failures return a `Result` and never panic |
| Generic shapes | No `where` clauses, associated types, const generics, default type parameters, specialization or generic type aliases/constants | Write bounds inline; name the concrete types |
| Generic checking | Generic bodies are late-checked once per instantiation; a never-used instantiation is never checked and its errors never appear; instantiating more than 128 levels deep is `E0275` | Instantiate the types you rely on; watch the recursion limit |
| Monomorphization | Each instantiation is its own checked function and C function; generic code is not shared between instantiations | Keep the number of instantiations small |
| Compiler scaling | Type checking one very large function is superlinear in its statement count: about 5,000 `let` statements take 1.1 s, 10,000 take 4.5 s, 20,000 take 17 s and 50,000 do not finish within a minute; a 100,000-element `vec![…]` literal is similarly slow | Split huge functions into helpers and build large vectors with a loop |
| Expression nesting | The parser accepts at most 128 levels of nested expressions, types and patterns. Deeper code is rejected with `E0001` "this code is nested too deeply" (the compiler runs its recursive passes on a large stack so accepted programs cannot overflow it). Ordinary code is unaffected: 100-term arithmetic chains, 100 chained method calls and flat literals of 8,000 or more elements all compile | Split pathological machine-generated code into smaller statements or helpers |
| Iterators | `.iter()` consumes a `Vec`; there is no `into_iter`/`iter_mut`; iteration over `&[T]` is not implemented in 2.1.0; `size_hint` is not planned; `collect` needs a known target type (`E0282`); `sum` and `product` accept `i32`, `i64`, `usize` and `f64` items (`E0277` for any other item type) | Use the provided adapters; annotate `collect` results |
| HashMap | Keys are limited to the integer types, `bool`, `String` and `char`; no `for`-loop iteration, no entry API and no `HashMap::from`; `get`/`values()` clone the value (`keys()` always clones the keys); printed order is slot order (deterministic for a given sequence of operations, but not insertion order and not guaranteed) | Use `keys()`/`values()` into a `Vec`; use `get_with` for non-cloneable values |
| HashMap hashing | The hash is deterministic and non-cryptographic; it is not designed to resist adversarial inputs | Do not expose a map directly to untrusted keys |
| Module files | A module is found only by the `name.ls` / `name/mod.ls` rule: no path attributes, no conditional compilation, no directory scan and no include mechanism; a module file must be valid UTF-8 (`E0583` otherwise) | Keep the conventional layout; rename or move files instead of annotating them |
| Module cycles | The tree of `mod` declarations must be acyclic: circular module files are `E0001` | Move shared items down into a module that does not depend back |
| Glob imports | `use path::*;` parses but the resolver rejects it (`E0432`) | List the names, use a group `{ … }`, or re-export with `pub use` |
| Restricted visibility | `pub(super)` and `pub(in path)` are rejected (`E0001`); the only levels are private, `pub(crate)` and `pub` | Use `pub(crate)` or restructure the module tree |
| Package dependencies | Path dependencies only: `util = { path = "../util" }`; no registry, git, version resolution, workspace, features or lockfile, and dependency versions are metadata | Keep dependencies as sibling directories in the repository |
| Package layout | A package is a binary (`src/main.ls`) or a library (`src/lib.ls`); a library-only package can only be `check`ed (run, build and emit-c need `src/main.ls`), a binary root without `fn main` fails `check` with `E0601`, and a package that has both files is rejected with `E0001` | Split libraries and binaries into separate packages |
| Manifest subset | `lsaca.toml` is a strict line-oriented TOML subset: only `[package]` and `[dependencies]`, double-quoted strings and one inline `{ path = … }` per dependency; any other key or section is `E0001` | Keep manifests minimal |
| Package graph | Two different paths providing the same package name, dependency cycles and self-dependencies are rejected (`E0001`) | Give every package a unique name and keep the dependency graph acyclic |
| Separate compilation | The whole crate graph is resolved, checked and emitted as one program on every build; there is no per-module or per-package incremental artifact | Rely on the build cache, which skips a whole unchanged program |
| Function values | Function and closure values are move-only and have no `==` (`E0369`); `.clone()` on a value holding a closure shares the captured environment instead of copying it | Storing them in aggregates works since 1.5; clone when the shared environment is intended |

## Not in version 2.1.0

The roadmap items that are still open:

* references stored inside aggregates or captured by closures;
* lifetime annotations and generic lifetimes;
* array types (`[T; N]`);
* loop labels (`'outer: loop` with `break 'outer`) and values out of `break`/`continue`;
* `where` clauses, associated types, supertraits, generic methods, associated
  functions and const generics;
* operator traits, `derive` attributes, `Display`/`Debug`/`Default`/`From`/`Ord`/
  `Hash` protocols, associated types on traits and `impl Trait`;
* iterator adapters that need `&[T]` iteration, `size_hint`, `into_iter` and
  `iter_mut`;
* the `io` stream traits (`Read`/`Write`/`Seek`/`BufRead`), `BufReader` and
  `PathBuf`/`Path`;
* glob imports, restricted visibility (`pub(super)`, `pub(in path)`) and module
  aliases;
* registry dependencies, git dependencies, workspaces, features, lockfiles and
  Cargo compatibility;
* user-defined macros and attributes;
* threads;
* separate compilation and per-crate incremental compilation;
* a language server (LSP) and a formatter.

A plausible order for closing these gaps:

* **v2.2** — loop labels, `break` values, partial moves (a `let` pattern
  destructuring an aggregate currently copies it), references in aggregates and
  the remaining standard-library system interfaces; array types and the trait
  protocol surface as separate steps. Further out: a language server, a
  formatter and incremental compilation.

Items already delivered, intentionally no longer on the list:

* an owned `Box<T>` type — **delivered in 1.1.0**;
* string formatting widths and precision — **delivered in 1.1.0**;
* the `?` error-propagation operator and the `Option`/`Result` combinators —
  **delivered in 1.2.0**;
* non-owning references, borrow checking and match ergonomics — **delivered in
  1.3.0**;
* ownership and move semantics, automatic `Copy`/`Clone` and the `Drop` trait —
  **delivered in 1.4.0**;
* mutable references to owned heap types (`&mut String`, `&mut Vec<T>`,
  `&mut Rc<T>`, `&mut Box<T>`) — **delivered in 1.4.0**;
* user-defined generics and traits with static dispatch — **delivered in
  1.5.0**;
* lazy iterators, iterator adapters and the builtin `HashMap<K, V>` —
  **delivered in 1.5.0**;
* structural `==` for aggregates, references and maps, and storing function
  values in aggregates — **delivered in 1.5.0**;
* nested tuples (values, patterns and struct fields), `Option<scalar>` and
  tuple struct fields, `Box<Option<…>>` payloads, and `assert_eq!` on aggregates
  and `Box` — **delivered in 1.5.0**;
* `?` directly after block-like expressions, and `Self { … }`/`Self::new(…)`
  in expression position — **delivered in 1.5.0**;
* `pub` accepted on struct fields, enum variants and trait methods —
  **delivered in 1.5.0** (field and module visibility are enforced since
  1.7.0);
* the `char` type and character literals — **delivered in 1.6.0**;
* a `String` that is always valid UTF-8, with character-oriented methods,
  Unicode case mapping and Unicode whitespace trimming — **delivered in 1.6.0**;
* Unicode-aware `Debug` escaping for strings — **delivered in 1.6.0**;
* UTF-8 validation of `read_line`, `args` and `read_file` — **delivered in
  1.6.0**;
* a Swiss-Table `HashMap` with a stronger deterministic hash and `char` keys —
  **delivered in 1.6.0**;
* modules, imports and multi-file crates, with cross-file diagnostics —
  **delivered in 1.7.0**;
* a package manager with `lsaca.toml`, path dependencies, dependency-cycle
  detection, `lsaca new` and package mode for `run`/`build`/`check`/`emit-c` —
  **delivered in 1.7.0**;
* a type checking fix that rejects by-value destructuring of a `Drop`-containing
  enum payload instead of emitting code that drops it twice (`E0509`), and the
  `assert_ne!` macro — **delivered in 1.7.0** (the rejection was relaxed again in
  2.0.0; see the 2.0.0 list below);
* the pointer-sized integer types `isize` and `usize` — **delivered in 2.0.0**;
* trait method default bodies, trait objects (`dyn Trait` behind `Box`, `Rc`,
  `&` and `&mut`), dynamic dispatch and the object-safety rules — **delivered in
  2.0.0**;
* unsized slices and `str` (`&[T]`, `&mut [T]`, `&str`, `&mut str`), slice
  expressions, and the `&Vec<T>`/`&String` coercions — **delivered in 2.0.0**;
* `extern fn` intrinsic declarations, and the standard library as a normal
  `std` crate embedded in the compiler, with `core`, `string`, `char`, `vec`,
  `iter`, `collections`, `fmt`, `math`, `io`, `fs`, `net`, `time`,
  `env`, `process` and `error` — **delivered in 2.0.0**;
* a relaxation of the pattern rule for drop values: by-value bindings of a
  `Drop`-containing tuple element or enum payload are accepted and the moved
  value is dropped exactly once; destructuring a `Drop` type's own fields by
  value stays `E0509` — **delivered in 2.0.0**;
* iterator adapters `zip`, `chain`, `filter_map`, `take_while`,
  `skip_while` and `step_by`, and the terminal operations `position`, `last`,
  `nth`, `reduce`, `product`, `inspect` and `for_each` — **delivered in 2.1.0**;
  `rev` and `flat_map` remain planned;
* `i32` default integers, literal suffixes and `usize` lengths, indices and
  counts — **delivered in 2.1.0**;
* `&str` text parameters, the zero-allocation string-literal-to-`&str`
  conversion and the `String + &str` concatenation rule — **delivered in
  2.1.0**;
* tuple structs, unit structs and effective `mut` parameters — **delivered in
  2.1.0**;
* `ParseIntError`/`ParseFloatError` and the typed `parse` surface, the new
  iterator/`str`/`char` methods, `Option::or`/`xor`, `Result::unwrap_err`, and
  the `error::Error` implementations — **delivered in 2.1.0**;
* the 2.1 system types: `time::Duration`/`Instant`/`SystemTime` and the
  `unix_epoch()` function, `env::VarError`, `net::Shutdown`, the `math`
  constants, and the integer-shaped file and metadata return types —
  **delivered in 2.1.0**.

## Language and runtime in 2.1.0

Version 2.1.0 is a **Rust-alignment release**: it keeps the 2.0 standard-library
crate and dynamic-dispatch surface and aligns the numeric and text defaults with
Rust, filling in the standard library where 2.0 was thin. It adds:

* **Default integers, literal suffixes and `usize` lengths.** An
  unsuffixed integer literal is `i32` in ordinary context and `usize` where a
  length, index, count or capacity is expected; a suffix picks any integer type
  (`1i64`, `0xFFu8`, `1_000_000usize`, `1.5f32`) and an out-of-range literal is
  rejected (`E0080`). `len()`, indexing, `find()` offsets, `count()`,
  `enumerate()` and `Rc::strong_count()` return `usize`. See
  [Types](04-types.md#integers) and [Lexical
  structure](03-lexical-structure.md#integer-literals).
* **`&str` text parameters and a single concatenation rule.** Read-only text
  parameters of the standard library take `&str`, while constructors and
  replacements keep returning `String`; a string literal in a `&str` position
  becomes a statically allocated slice with no allocation, and a `String`
  variable is passed as `&owned`. `String + &str` and `String += &str` are the
  only concatenation forms. See [Slices and str](26-slices-and-str.md) and the
  [standard library reference](../stdlib/README.md).
* **Tuple and unit structs, and effective `mut` parameters.**
  `struct Meters(i64);`, `struct Pair(i32, String);` and `struct Marker;` are
  legal, with `.0` fields and matching patterns; `fn f(mut x: i64)` really
  enables reassignment inside the function. See
  [Structs](10-structs-enums-and-impl.md#tuple-and-unit-structs),
  [Patterns](13-pattern-matching.md) and [Functions](09-functions.md).
* **Typed parse errors and the new combinator surface.** `str::parse_i64` and
  `str::parse_f64` return `Result<_, ParseIntError>` and
  `Result<_, ParseFloatError>`, the generic `parse::<T>()` is available with
  `i64` and `f64` as targets in 2.1.0, the iterator adapters, `str` and `char`
  gain the 2.1 methods, `Option` gains `or`/`xor`, `Result` gains `unwrap_err`,
  and `io::Error`/`env::VarError` implement `error::Error`. See the
  [standard library reference](../stdlib/README.md).
* **New system types.** `time` gains `Duration`, `Instant`, `SystemTime` and
  `unix_epoch()` (the epoch is a function because a constant cannot have a
  `SystemTime` type); `env` gains `VarError`; `net` gains `Shutdown`; `math` exposes
  the `f64` constants; `File::read`/`write` return `Result<usize, Error>`,
  `File::seek` returns `Result<u64, Error>`, `fs::Metadata::size()` returns
  `u64` and `fs::Metadata::modified()` returns `Result<SystemTime, Error>`.
  OS failures still return a `Result` and never panic.

### The 2.0 foundation

The 2.0 surface remains the base of 2.1.0:

* **A standard library crate.** `stdlib/src/lib.ls` is a normal lsaca crate: its
  modules use the ordinary module rules, its root re-exports the prelude names
  with `pub use`, and the compiler embeds the files at build time through
  `include_str!`. `Vec`, `String`, `Option`, `Result`, `Rc`, `Box` and
  `HashMap` remain builtin types with marker declarations in the crate, and the
  standard library may write the `impl` blocks for them. See [The standard
  library](15-standard-library.md) and the [standard library
  reference](../stdlib/README.md).
* **Trait objects and default methods.** `dyn Trait` behind `Box`, `Rc`, `&` or
  `&mut` dispatches through a per-(trait, concrete type) vtable; traits may give
  methods default bodies. See [Trait objects](25-trait-objects.md) and
  [Traits](18-traits.md#default-method-bodies).
* **Slices and `str`.** `[T]` and `str` are unsized types that exist only behind
  references, with slice expressions and the `&Vec<T>`/`&String` coercions; the
  `std` crate implements their methods. See [Slices and str](26-slices-and-str.md).
* **Pointer-sized integers.** `isize` and `usize` are 64 bits on every supported
  target and behave like the other integer types for casts, comparison,
  formatting and hash-map keys; since 2.1.0 `usize` is the length, index, count
  and capacity type.
* **The larger collections surface.** `HashSet` (insertion ordered),
  `BTreeSet`, `BTreeMap`, `VecDeque` and `BinaryHeap` join `HashMap`, and the
  iterators gain `zip`, `chain`, `filter_map`, `take_while`, `skip_while` and
  `step_by`; `rev` and `flat_map` remain planned. See
  [Standard library reference](../stdlib/README.md).
* **`extern fn` intrinsics.** The compiler's builtin operations are now declared
  in the `std` crate as body-less `extern fn` declarations implemented by
  code generation; only the `std` crate may declare them.
* **One program, one C project.** Module and package boundaries are erased and
  the whole crate graph is still type-checked and emitted as one C project; the
  value ABI of the pre-2.0 types is unchanged.

The 2.0 surface (`dyn` trait objects, slices and `str`, `isize`/`usize`, default
method bodies, `extern fn` declarations) and the 2.1 surface (literal suffixes,
`i32` defaults, `usize` lengths, `&str` parameters, tuple and unit structs,
`mut` parameters, the new standard-library methods and error types) are
implemented in the 2.1.0 compiler, including trait-object dispatch,
`Box<dyn Trait>`/`Rc<dyn Trait>` coercion and the `&Vec<T>`/`&String`
coercions (see [25-trait-objects](25-trait-objects.md) and
[26-slices-and-str](26-slices-and-str.md)). Library gaps are listed module by
module in the [standard library reference](../stdlib/README.md); slice
iteration, the `io` stream traits and a few `std` helpers are not implemented in
2.1.0 and are planned for a later release.

The compiler and runtime organization keeps the v1.7 shape:

* **Direct C compiler backend by default.** `lsaca run` and `lsaca build` write
  the generated C into the build cache, invoke the C compiler for every generated
  program unit — `main.c`, or `main.c` plus `lsaca_unit_1.c` … for very large
  programs — and the runtime units, and link the executable themselves. CMake is
  only needed with the global `--cmake` flag; `emit-c` still writes a complete
  CMake project.
* **Split generated code for very large programs.** A program whose emitted user
  functions exceed 256 KiB is written as `main.c` plus `lsaca_unit_1.c` …
  `lsaca_unit_N.c` (at most eight units), with `main.c` keeping the declarations
  and the entry point. The split is deterministic, and a small program still
  produces a single, unchanged `main.c`.
* **Split runtime sources.** The runtime is built from several translation
  units — `lsaca_rt.c`, `lsaca_rt_string.c`, `lsaca_rt_vec.c`,
  `lsaca_rt_fmt.c`, `lsaca_rt_map.c` and `lsaca_rt_unicode.c` — plus the public
  `lsaca_rt.h` and internal `lsaca_rt_internal.h`, and the generated Unicode
  table header `lsaca_rt_unicode.h`; the generated `CMakeLists.txt` lists every
  runtime C file.
* **Generated Unicode tables.** `lsaca_rt_unicode.c` is generated from Python's
  Unicode database by `scripts/gen_unicode_tables.py` and committed; rerunning
  the script reproduces it byte for byte.
* **Cached runtime objects.** Compiled runtime objects live per profile under
  `<cache>/_runtime/<hash>/` and are reused, so a new program only recompiles
  its own generated program unit or units and links.
* **One content-hashed build cache.** The cache key hashes the complete generated
  C program — every source file, module and package dependency — together with
  the runtime sources, the compiler path and the build mode, so editing any file
  in the crate graph invalidates the cache, while an unchanged program is reused
  as a whole.
* **Embedded standard library.** The standard library lives in `stdlib/src/*.ls`
  and is embedded in the compiler by `scripts/gen_stdlib_embed.py` (the generated
  file is `crates/lsaca/src/stdlib_embed.rs`). The `std` crate root is the
  prelude: the common names are in scope in every module of every crate.
  Qualified `std::…` paths work for functions, constructors and associated
  items, and `use std::…` imports work. Qualified type names in type position
  (`E0412`) and trait paths in `impl` blocks (`E0001`: import the trait and use
  its single identifier) are known gaps in 2.1.0.

Compile-time, runtime and memory measurements are republished with each
release; see [Benchmarks](21-benchmarks.md), alongside the comparison with C,
Rust, Python and Node.

## Compatibility

lsaca 2.1.0 breaks programs that relied on 2.0's numeric and text defaults; the
rest of the standard-library surface is additive. The main incompatibilities:

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

The published tables carry the official **2.1.0** run, measured on 2026-11-06:
compile time, runtime and peak memory on the runtime workloads plus compile-only
scaling workloads that cover multi-file crates and path-dependency packages,
compared with C, Rust, Python and Node. The previous 2.0.0 run (2026-11-03) is kept
in the results archive and is used for the explicit before/after comparison in the
analysis. See [Benchmarks](21-benchmarks.md).

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
* The standard library modules: [Standard library
  reference](../stdlib/README.md).
* Ownership, moves and destruction: [Ownership and
  moves](11-ownership-and-moves.md).
* Non-owning views and the borrow checker: [References and
  borrowing](12-references-and-borrowing.md).
* Every builtin in detail: [Standard library](15-standard-library.md).
* The memory model that ties ownership and ORC together:
  [Memory model](05-memory-model.md).
