# Known limitations

This page lists the implementation gaps and restrictions of the current
release, verified against the current compiler. They are expected to be fixed
in later releases; avoid them in code you intend to keep. Where a module or API
is fixed by the design but not implemented yet, the [standard library
reference](../stdlib/README.md) and the module pages mark it *planned* for a
later release.

## Current limitations

| Area | Limitation | Workaround |
| --- | --- | --- |
| Casts to `bool` | An integer or a float cannot be cast to `bool` (`E0605`); only `bool` values are booleans and conditions never convert numbers | Compare explicitly, for example `value != 0` |
| Loop variables | `for mut i in …` parses and lets you reassign `i` | Keep loop variables immutable |
| Constants | Constant types are restricted to scalars (including `char`) and `String`; constant expressions allow arithmetic but not comparisons or function calls. This is why the epoch is the function `time::unix_epoch()` rather than a `UNIX_EPOCH` constant | Use a `let` or compute at runtime |
| String indexing | `text[i]` is rejected (`E0608`) | Use `char_at`, `substring`, `find` or `split` |
| Unicode case conversion | `char::to_uppercase`/`to_lowercase` return the first scalar of the full Unicode mapping (Rust returns an iterator over all of them); `String::to_uppercase`/`to_lowercase` keep the full one-to-many mappings | Use `String` case mapping when the full expansion matters |
| Unicode properties | Case mapping, whitespace trimming and character classification use the generated Unicode 16.0.0 tables; there is no general Unicode property (script, category, grapheme) or normalisation API | Precompute the characters you need |
| Formatting | Width, fill and alignment apply to a user struct/enum as a whole, but the `+`/`0` flags and precision are rejected for them (`E0001`); character precision and the `+`/`0` flags are also rejected; no named or indexed arguments | Format fields individually, or apply width/fill/alignment to the whole value |
| Numbers | `pow` is float-only; no integer exponentiation builtin | Convert to `f64` or write a loop |
| Generic `parse` | `parse::<T>()` accepts only `i64` and `f64` as the target type in the current release; any other numeric target is `E0107` | Use `parse_i64()`/`parse_f64()`; for another integer type parse an `i64` and cast |
| Patterns | Since 2.4.0 fixed-size array patterns, slice patterns and `@` bindings are accepted; there are still no `ref`, range or `&` patterns, and a slice pattern has at most one `..` | Match a reference scrutinee directly: the ergonomic bindings are references, so read and write through `*binding` |
| Reference storage | Since 2.2.0 a struct, enum or tuple struct field may hold a direct `&'a T`/`&'a mut T`; `Option<&T>`, tuple-typed fields, `Vec`/`HashMap` elements, `Result` payloads, `Box` payloads, array elements and closure captures still reject references (`E0498`) | Keep references in locals, parameters, return values and the supported aggregates |
| Mutable references | `&mut` to a user struct/enum containing a heap field is rejected (`E0498`); mutable references to scalars, `Copy` aggregates, `char`, `String`, `Vec<T>`, `Rc<T>` and `Box<T>` work | Take `&T` or restructure the code |
| Method receivers | A by-value `self` method cannot return a reference (`E0106`), and a `&self` method can only return shared references (`E0596`) | Return shared references from `&self` methods, mutable references from `&mut self` methods, and owned values from by-value methods |
| Method call form | A method can only be called as `value.method()`; UFCS form `Type::method(&value)` is rejected with `E0599` and a "call it on a value instead" note (only true associated functions such as `Rc::new` work with `Type::function(...)`) | Call methods on a value: `handle.strong_count()`, not `Rc::strong_count(&handle)` |
| Return elision | A free function returning a reference needs exactly one input reference (elided) or an explicit lifetime shared by the output and an input; methods use the receiver; otherwise `E0106` | Pass exactly one reference parameter, write explicit `'a` lifetimes, or return an owned value |
| Moves | A `let` pattern that binds an owning value moves the whole aggregate, so unbound `Copy` fields cannot be read through the source afterwards (deviation 15); moving a field out of a value without destructuring it is rejected (`E0507`) | Clone or restructure the value before the pattern |
| Move checking | Moving a value while a live borrow exists is rejected (`E0505`); using a value after a move is rejected (`E0382`) | Order borrows and moves explicitly, or `.clone()` before the move |
| Drop in patterns | A value whose type implements `Drop` cannot be destructured by its own fields by value (`E0509`); moving the whole value, a tuple element or an enum payload through a pattern is allowed and drops it exactly once | Match a reference (`match &value`) or restructure the value |
| Cloning | A type that implements `Drop` cannot be cloned (`E0277`) | Use references, or restructure so the value is not cloned |
| Custom impls | `impl Copy` and `impl Clone` are rejected because both are derived automatically | Rely on the automatic derivation |
| Explicit drop | There is no `drop(value)` function (`E0425`) and no `.drop()` method (`E0599`); a value is dropped when its last owner is released | End the scope early, or move the value into a helper that consumes it |
| Drop at exit | `Drop` hooks do not run when the process exits via `exit()` or a panic; they do run when `main` returns normally | Do not rely on hooks for cleanup at abnormal exits |
| Borrow granularity | `Vec` borrows are whole-container: two `&mut` borrows of different elements conflict (`E0499`) | Mutate elements one at a time or rebuild the vector |
| Borrow checker in one expression | Passing two mutable borrows as the two arguments of one call, or combining them with `+`, is rejected (`E0499`); the short-circuit operators `&&` and `\|\|` accept them because each operand becomes its own condition | Bind each result to a local, then combine the locals |
| `for` over references | Since 2.3.0 `for x in &vec`, `for x in &mut vec`, `for x in &slice`, `for x in &mut slice` and `for (k, v) in &map` borrow instead of consuming; a loop over a plain `Vec<T>` still moves it | Use the borrowed form when the collection is needed after the loop; use `.into_iter()` when it should be consumed |
| Reference casts | References have no `as` conversion (`E0605`) | Convert the pointed-to value with `*r` |
| UTF-8 input | `read_line()` and `args()` panic on invalid UTF-8 (`exit 101`); `read_file()` returns an `Err` instead (the invalid-data kind) | Feed the program valid UTF-8, or check the `Result` from `read_file` |
| Trait objects | `dyn Trait` values exist only behind `Box`, `Rc`, `&` or `&mut` (bare `dyn Trait` is `E0001`); the trait must be object-safe (`E0038`: every method `&self`/`&mut self`, no generics, no associated items, no `Self` outside the receiver); there is no upcasting between trait objects; `Box<dyn Trait>` has no `get`/`set` (`E0001`) and `==`/`<` are `E0369`; a non-object-safe trait still works for generic/static dispatch | Make the trait object-safe, or use generic bounds when the concrete type is known |
| Trait shape | No associated functions, generic methods, supertraits, associated types or constants; default method bodies are supported since 2.0; `impl Trait for Type` only for user structs and enums (a builtin target is `E0118`, except in the `std` crate) | Implement every required method explicitly; use generic bounds for the rest |
| Slices and `str` | `[T]` and `str` are unsized and exist only behind `&`/`&mut` (`E0001` otherwise); slice indices are `usize`, so an `i32`/`i64` index is a type error (`E0308`) — cast with `as`; user crates cannot write `impl [T]` or `impl str` (`E0118`) | Use `Vec<T>`/`String` for storage and take `&[T]`/`&str` parameters for borrowing |
| Arrays | `[T; N]` values have a fixed length; since 2.4.0 a fixed-size array pattern must match the length exactly (`E0308`), and there is still no by-value iteration (`E0277`; iterate the indices). An array length must be an integer literal or integer constant (`E0435`), the length is capped at 1,048,576, and `&mut array` does not coerce to `&mut [T]` | Index with `0..array.len()`, borrow as `&array` for `&[T]`, and use `Vec<T>` when the length changes |
| Loop labels | Labels are supported on `loop`, `while`, `for` and before `while let`; a label on another statement is `E0001`, a missing `:` is `E0001`, an undeclared label is `E0426`. Since 2.4.0 a `loop` can carry a value out of `break`; a value in a `while`/`for` break is `E0571` and a value on `continue` is `E0308` | Use `break value` from a `loop` when a result is needed; otherwise keep `break` bare |
| Lifetimes | Reference fields require a declared lifetime (`E0106`) and the field lifetime must be a named parameter — a `'static` field is `E0498` ("the 'static lifetime is not supported yet"); lifetime arguments are checked (`E0261`); checking is conservative: no variance, no outlives relations and no equality check between named lifetimes; a `&self` method cannot return a mutable borrow (`E0596`) | Keep reference fields few, declare a named lifetime parameter for each, and return owned values when the relations get complicated |
| `From` conversions | `?` converts through a user `impl From<Found> for Expected`; there is no blanket `From<T> for T`, no `Into`, no `Try` and no `From<&str> for String`; `Error::source` has a default body and nothing calls it automatically | Write the `impl From` for each error pair you rely on |
| Formatter | `lsaca fmt` normalises indentation, trailing whitespace, blank-line runs and line endings. It does not reflow or merge lines and does not change token spacing inside a line; `--stdout` works on a single file only, and a file or package with a syntax error is left untouched | Format a package directory (it discovers `lsaca.toml`) or a file; keep hand-written line breaks |
| Integer types | There are no `i128`/`u128` types, and their literal suffixes are rejected while lexing; an unsuffixed integer literal is `i32`, so a value outside that range needs a suffix or an annotation (a suffixed literal that does not fit its type is `E0080`) | Write `1i64` or annotate the binding; use `i64`/`u64` for very large values |
| Extern functions | `extern fn` intrinsics are accepted only in the `std` crate (user code is `E0001`) and are not first-class values (`E0425`). Since 2.5.0 `extern "C"` blocks declare real C functions in any crate, but only the C ABI is accepted: no function pointers, callbacks or variadics, no `#[link]` or extra libraries, and the call itself is unsafe (`E0133`) | Call the public `std` function, or declare the exact C function in an `extern "C"` block inside an unsafe context |
| Unsafe operations | The unsafe permission covers only raw pointer dereference and assignment, `extern "C"` calls, `MaybeUninit::assume_init`, `CStr::from_ptr`, `CString::from_raw`, and union construction/field access. There is no inline assembly, allocator API, `Box::into_raw`/`Rc` raw interop, or self-referential structure support, and a raw pointer is never ORC-managed | Keep unsafe code at the FFI boundary and put the pointer operations in one small function |
| FFI boundary | Only FFI-safe types may appear in an `extern "C"` signature (integers, floats, `bool`, `char`, raw pointers, references over FFI-safe types, `()`, `repr(C)` structs and unions); managed values, tuples, enums, `Option`/`Result`, slices, `str` and `CString`/`CStr`/`MaybeUninit` are `E0770`. A `CString` buffer comes from the lsaca allocator, so `CString::from_raw` accepts only pointers from `into_raw`; there is no C-allocator bridge | Pass pointers to plain storage and convert at the boundary; keep the owning value alive for the whole call |
| C-layout types | `#[repr(C)]` is accepted only on a struct and as a no-op on a union; `repr(packed)`, `repr(align)`, `repr(transparent)` and `repr(C)` enums are `E0771`. A union has C layout but no generics, methods, `Drop`, destructuring patterns, generated equality or formatting (`E0772`/`E0773`) | Use a `repr(C)` struct when you need fields at distinct offsets; use a union only as a C overlay |
| `MaybeUninit` | The compiler does not track whether the storage is initialized; `assume_init` is unsafe, and dropping an initialized `MaybeUninit` leaks the value instead of dropping it. There is no `zeroed`, `uninit_array` or `transmute` | Initialize exactly once and call `assume_init` on every initialized path, or keep the value in a normal local |
| Standard library status | The `std` crate is implemented module by module in the current release; the [standard library reference](../stdlib/README.md) marks every page *implemented* or *planned*. Remaining gaps in the current release: `net` has TCP and UDP but no TLS, Unix-domain sockets, dedicated DNS lookup API or nonblocking mode; `time` has the clocks, `Duration`, `Instant` and `SystemTime` but no time-zone database; there is no `Ordering` or `FromUtf8Error`; `parse::<T>()` accepts only `i64` and `f64` targets; a few `str`/`Vec` helpers are still planned, and the `From` conversions used by `?` cover only the pairs the library defines. `std::process` has `args`, `exit` and `Command`; `std::net` has `SocketAddr` and `UdpSocket`; 2.5.0 adds `ptr`, `cstring`, `cstr` and `mem`. `use std::…` imports work, while qualified type names in type position do not resolve (`E0412`) and trait paths in `impl` blocks are rejected (`E0001`, import the trait instead) | Read the module page for what exists in the current release; OS failures return a `Result` and never panic |
| Generic shapes | No `where` clauses, associated types, const generics, default type parameters, specialization or generic type aliases/constants | Write bounds inline; name the concrete types |
| Generic checking | Generic bodies are late-checked once per instantiation; a never-used instantiation is never checked and its errors never appear; instantiating more than 128 levels deep is `E0275` | Instantiate the types you rely on; watch the recursion limit |
| Monomorphization | Each instantiation is its own checked function and C function; generic code is not shared between instantiations | Keep the number of instantiations small |
| Compiler scaling | Type checking one very large function is superlinear in its statement count: about 5,000 `let` statements take 1.1 s, 10,000 take 4.5 s, 20,000 take 17 s and 50,000 do not finish within a minute; a 100,000-element `vec![…]` literal is similarly slow | Split huge functions into helpers and build large vectors with a loop |
| Expression nesting | The parser accepts at most 128 levels of nested expressions, types and patterns. Deeper code is rejected with `E0001` "this code is nested too deeply" (the compiler runs its recursive passes on a large stack so accepted programs cannot overflow it). Ordinary code is unaffected: 100-term arithmetic chains, 100 chained method calls and flat literals of 8,000 or more elements all compile | Split pathological machine-generated code into smaller statements or helpers |
| Iterators | Since 2.3.0 `.iter()` borrows and yields `&T`, `.iter_mut()` yields `&mut T` and `.into_iter()` consumes; `rev` and `flat_map` exist, `size_hint` does not (and is not planned); `collect` needs a known `Vec<T>` target type (`E0282`) and cannot collect borrowed items such as `Vec<&T>` (`E0498`); `sum` and `product` accept `i32`, `i64`, `usize` and `f64` items (`E0277` for any other item type) | Use the provided adapters; map borrowed items to owned values before `collect`, `sum` or `product` |
| HashMap | Keys are limited to the integer types, `bool`, `String` and `char`; 2.4.0 adds the `get_or_insert` and `get_or_insert_with` idioms, but there is still no borrowed `Entry` value (see [RFC 0002](../../rfcs/0002-hashmap-entry.md)); `HashMap::from(Vec<(K, V)>)` builds a map, with the last duplicate key winning; `iter()`, `keys()` and `values()` borrow the map and do not clone; printed order is slot order (deterministic for a given sequence of operations, but not insertion order and not guaranteed) | Iterate with `for (key, value) in &map`; use `get_or_insert_with` when the value must be computed lazily |
| HashMap hashing | The hash is deterministic and non-cryptographic; it is not designed to resist adversarial inputs | Do not expose a map directly to untrusted keys |
| Module files | A module is found only by the `name.ls` / `name/mod.ls` rule: no path attributes, no conditional compilation, no directory scan and no include mechanism; a module file must be valid UTF-8 (`E0583` otherwise) | Keep the conventional layout; rename or move files instead of annotating them |
| Module cycles | The tree of `mod` declarations must be acyclic: circular module files are `E0001` | Move shared items down into a module that does not depend back |
| Glob imports | `use path::*;` parses but the resolver rejects it (`E0432`) | List the names, use a group `{ … }`, or re-export with `pub use` |
| Restricted visibility | `pub(super)` and `pub(in path)` exist since 2.4.0, but `pub(in path)` must name an ancestor module: a sibling or descendant path is `E0742` and an unresolved path is `E0433` | Point `pub(in …)` at an ancestor, or use `pub(crate)`/private visibility |
| Package dependencies | Path dependencies only: `util = { path = "../util" }`; no registry, git, version resolution, workspace, features or lockfile, and dependency versions are metadata | Keep dependencies as sibling directories in the repository |
| Package layout | A package is a binary (`src/main.ls`) or a library (`src/lib.ls`); a library-only package can only be `check`ed (run, build and emit-c need `src/main.ls`), a binary root without `fn main` fails `check` with `E0601`, and a package that has both files is rejected with `E0001` | Split libraries and binaries into separate packages |
| Manifest subset | `lsaca.toml` is a strict line-oriented TOML subset: only `[package]` and `[dependencies]`, double-quoted strings and one inline `{ path = … }` per dependency; any other key or section is `E0001` | Keep manifests minimal |
| Package graph | Two different paths providing the same package name, dependency cycles and self-dependencies are rejected (`E0001`) | Give every package a unique name and keep the dependency graph acyclic |
| Separate compilation | The whole crate graph is resolved, checked and emitted as one program on every build; there is no per-module or per-package incremental artifact | Rely on the build cache, which skips a whole unchanged program |
| Function values | Function and closure values are move-only and have no `==` (`E0369`); `.clone()` on a value holding a closure shares the captured environment instead of copying it | Storing them in aggregates works since 1.5; clone when the shared environment is intended |

## Not in the current release

The roadmap items that are still open:

* references stored inside `Vec`, `HashMap`, `Result`, `Box` or arrays, or
  captured by closures;
* full lifetime relations (variance, outlives, named-lifetime equality) and
  generic lifetime bounds;
* `&'static` reference fields: a field lifetime must be a named parameter;
* values out of a `while`/`for` break or from `continue` (a `loop` break
  value is supported since 2.4.0);
* `ref`, range and `&` patterns (array, slice and `@` patterns are supported
  since 2.4.0);
* `where` clauses, associated types, supertraits, generic methods, associated
  functions and const generics;
* operator traits, `derive` attributes, user `Display`/`Debug`/`Default`/`Ord`/
  `Hash` impls, associated types on traits and `impl Trait` — protocols are
  compiler-defined in 2.x per [RFC 0003](../../rfcs/0003-derive-and-protocols.md);
* iterator `size_hint` (not planned);
* glob imports and a borrowed `HashMap` `Entry` value (deferred to v2.6 by
  [RFC 0002](../../rfcs/0002-hashmap-entry.md));
* registry dependencies, git dependencies, workspaces, features, lockfiles and
  Cargo compatibility;
* user-defined macros and attributes other than the C-layout `#[repr(C)]`;
* the rest of the FFI surface: function pointers and callbacks, variadics,
  `#[link]` and extra libraries, `repr(packed/align/transparent)`, `repr(C)`
  enums, inline assembly, allocator APIs and `Box::into_raw`/`Rc` raw interop;
* threads (2.x is single-threaded by contract; see
  [RFC 0004](../../rfcs/0004-concurrency.md));
* separate compilation and per-crate incremental compilation;
* a language server (LSP).

The order of work and the frozen release contracts are maintained in
[ROADMAP.md](../../ROADMAP.md); the v2.4 release contract adds:

* fixed-size array, slice and `@` patterns in every pattern position, and
  break values on `loop`;
* `pub(super)`, `pub(in path)` and module aliases;
* `HashMap` entry idioms, constructors/accessors on the other collection
  types, the `min`/`max` iterator terminals, `process::Command` and
  `SocketAddr`/`UdpSocket`;
* formatter package mode;
* the protocol and concurrency boundaries recorded in
  [RFC 0003](../../rfcs/0003-derive-and-protocols.md) and
  [RFC 0004](../../rfcs/0004-concurrency.md).

The v2.4 release delivered fixed-size array, slice and `@` patterns, break
values on `loop`, restricted visibility, module aliases, the `HashMap` entry
idioms, new collection constructors/accessors, the `min`/`max` iterator
terminals, `process::Command`, `SocketAddr`/`UdpSocket` and formatter package
mode. The v2.5 release contract adds:

* `unsafe` blocks and `unsafe fn`, raw pointers and the exact unsafe operation
  list in [Unsafe code and C FFI](32-unsafe-and-ffi.md);
* `extern "C"` declarations and the FFI-safe type boundary;
* `#[repr(C)]` structs and C-layout unions;
* `CString`, `CStr`, `MaybeUninit<T>` and the `ptr`, `cstring`, `cstr` and `mem`
  modules;
* the ORC invariants for raw pointers and the FFI boundary.

The v2.3 release delivered borrowed iteration, `Vec::iter_mut`, `HashMap`
iteration, I/O streams, `PathBuf`/`Path`, Unicode character classification,
the missing combinators and partial moves (recorded in the delivered list
below). Later releases track references in aggregates, full lifetimes and
threads.

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
  `rev` and `flat_map` — **delivered in 2.3.0**;
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
* array types (`[T; N]`, array literals including `[value; N]`, `Copy` arrays,
  `usize` indexing, `&array` to `&[T]` coercion, and arrays in aggregates) —
  **delivered in 2.2.0**;
* loop labels on `loop`, `while`, `for` and `while let`, with `break 'label`
  and `continue 'label` — **delivered in 2.2.0**;
* direct reference fields in structs, tuple structs and enums, with lifetime
  parameters and `impl<'a>` blocks — **delivered in 2.2.0** (`Option<&T>` and
  tuple-typed fields remain rejected);
* the `From` trait, `?` error conversion through `From` and the `Error::source`
  default method — **delivered in 2.2.0**;
* `lsaca fmt` with `--check` and `--stdout` — **delivered in 2.2.0**;
* borrowed access: `Vec::get_mut`/`first_mut`/`last_mut`, slice `get_mut`, and
  `HashMap::get`/`get_mut` taking `&K` — **delivered in 2.2.0**; mutable
  borrowing iterators — **delivered in 2.3.0**.
* partial moves in destructuring `let` patterns: binding an owning value moves
  the aggregate instead of deep-copying it, and the unbound fields are dropped —
  **delivered in 2.3.0**;
* fixed-size array, slice and `@` patterns — **delivered in 2.4.0**;
* `break value` from a `loop`, with `E0571` for a value in a `while`/`for`
  break and `E0308` for a value on `continue` — **delivered in 2.4.0**;
* `pub(super)`, `pub(in path)` restricted visibility and module aliases —
  **delivered in 2.4.0**;
* `HashMap::get_or_insert`/`get_or_insert_with` and the new collection
  constructors/accessors — **delivered in 2.4.0**;
* the `min`/`max` iterator terminals — **delivered in 2.4.0**;
* `process::Command` and `net::SocketAddr`/`UdpSocket` — **delivered in 2.4.0**;
* `lsaca fmt` package mode with `--check` over a file or directory —
  **delivered in 2.4.0**;
* the compiler-defined protocol and single-threaded concurrency boundaries —
  recorded in [RFC 0003](../../rfcs/0003-derive-and-protocols.md) and
  [RFC 0004](../../rfcs/0004-concurrency.md) for 2.x;
* `unsafe` blocks/functions, raw pointers and the `ptr` module — **delivered in
  2.5.0**;
* `extern "C"` declarations, `#[repr(C)]` structs, C-layout unions and the
  FFI-safe boundary — **delivered in 2.5.0**;
* `CString`, `CStr`, `MaybeUninit<T>` and the `cstring`/`cstr`/`mem` modules —
  **delivered in 2.5.0**.
