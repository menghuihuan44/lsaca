# Roadmap and deviations from Rust

lsaca borrows Rust's surface syntax and some of its semantics, but it is a
smaller language with an explicitly different memory model. This page lists the
intentional differences, the known gaps in the current release, and what is
planned next. It describes lsaca **1.6.0**.

## Intentional deviations from Rust

These are design decisions, not bugs. Code written for Rust will need edits in
these areas.

1. **Ownership with automatic `Copy` and `Clone`.** Version 1.4 added ownership
   and move semantics close to Rust's, but the marker traits are derived
   automatically instead of implemented by hand. A type is `Copy` when all of
   its parts are: integers, floats, `bool`, `char`, `()`, tuples,
   `Option<T>` and `Result<T, E>` of `Copy` types, references, and user
   structs/enums whose fields are all `Copy` and which do not implement
   `Drop`. Everything else (`String`, `Vec<T>`, `Rc<T>`, `Box<T>`, function
   values, and user types containing them) is move-only. `Clone` is automatic
   too: every type except those that implement `Drop` has `.clone()`, and
   `impl Copy`/`impl Clone` blocks are rejected. `Drop` is the only builtin
   trait with special code generation; user-defined traits exist since 1.5.0 but
   are purely compile-time method dispatch. There are still no lifetime
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
   There is no lifetime syntax: elision follows fixed rules. See
   [References and borrowing](12-references-and-borrowing.md).
3. **No modules, crates or imports.** A program is one `.ls` file; all items are
   in one namespace. See [Program structure](02-program-structure.md).
4. **Generics are monomorphized and late-checked; traits are static.** Version
   1.5 adds user-defined generics and traits, with deliberate limits: generic
   items are structs, enums, free functions, inherent impls and trait impls;
   there is no `dyn`, no trait objects, no dynamic dispatch and no code sharing
   between instantiations. The body of a generic function is checked once per
   concrete instantiation, so a type error appears only for the instantiations
   that are actually used and an unused generic body is never checked. Unused
   type parameters are accepted (Rust rejects them). There are no `where`
   clauses, associated types, const generics, default type parameters or
   specialization. Traits declare method signatures with receivers only: no
   default bodies, associated functions, generic methods, supertraits, associated
   types or constants. See [Generics](17-generics.md) and
   [Traits](18-traits.md).
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
7. **No `usize`, no `isize`.** Lengths, indices and counts are `i64`. Cast
   explicitly when a builtin needs another integer type.
8. **Characters are scalars; strings are UTF-8.** Since 1.6.0 `char` is a
   `Copy` 32-bit Unicode scalar value with character literals and
   character-literal patterns, and `String` is always valid UTF-8. The string
   API keeps byte-oriented methods (`len`, `find`, `substring`, whose offsets
   are byte indices) next to character-oriented ones (`char_count`, `chars`,
   `char_at`, `push`, `pop`, `is_char_boundary`). There is no `u8` byte-string
   type and no byte-string literal. See
   [Characters and Unicode](22-characters-and-unicode.md) and
   [Standard library](15-standard-library.md#string).
9. **No arrays and no slices.** `Vec<T>` is the growable sequence type, and
   tuples are the fixed-arity product type; tuples can nest.
10. **Restricted patterns.** No `@` bindings, no `ref` patterns, no slice and no
    range patterns. String-literal and character-literal patterns are supported,
    or-patterns (`p | q`) and match guards are fully supported, and a reference
    scrutinee is matched with match ergonomics rather than a `&` pattern. See
    [Pattern matching](13-pattern-matching.md).
11. **No values out of `break`/`continue`.** Loops have type `()`; `break
    value` is a compile error. Use a mutable accumulator variable instead.
12. **No numeric literal suffixes.** There is no `1i32`, `1u8` or `2.0f32`;
    the literal type comes from context or defaults to `i64`/`f64`, and `as`
    converts. NaN and infinity have no literals.
13. **A small standard library with iterators and one map.** Version 1.5 adds
    lazy iterators (adapters `map`, `filter`, `take`, `skip`, `enumerate` and
    terminals `collect`, `fold`, `count`, `sum`, `any`, `all`, `find`) and the
    builtin `HashMap<K, V>`. There are still no other collections (no `HashSet`,
    `BTreeMap`, deque or set), no formatting of integers in other bases, no
    named or indexed format arguments, and no iterator adapters beyond the list
    above (no `rev`, `zip`, `chain`, `flat_map`, `for_each` or `size_hint`).
    Asking a vector for an iterator with `.iter()` **consumes** the vector; a
    `HashMap` cannot be iterated with `for` — use `keys()` or `values()`. The
    `?` operator is supported for `Option` and `Result`; the combinators are
    limited to `Option::map`, `Option::map_or`, `Option::and_then`,
    `Option::filter`, `Option::ok_or` and `Result::map`, `Result::map_err`,
    `Result::and_then` (no `Result::map_or`, matching Rust). What exists is
    documented in [Standard library](15-standard-library.md),
    [Iterators](19-iterators.md) and [HashMap](20-hashmap.md).
14. **Equality is structural for most values, identity for `Rc`.** Since 1.5,
    `==` works for `Vec`, `Option`, `Result`, tuples, user structs and enums,
    references and `HashMap` as well; structs and enums compare field by field,
    references compare the pointed-to values, and maps compare contents ignoring
    insertion order. `Rc<T>` equality remains identity (same cell), `Box<T>`
    compares structurally, characters compare by scalar value, and function
    values have no `==` (`E0369`).
15. **No unsafety.** There is no `unsafe`, no FFI and no threads. The generated C
    is not exposed to the language.

## Known limitations in 1.6.0

The following are implementation gaps or deliberate restrictions in the current
release, verified against the shipped compiler. They are expected to be fixed
in later releases; avoid them in code you intend to keep.

| Area | Limitation | Workaround |
| --- | --- | --- |
| Casts to `bool` | An integer or a float cannot be cast to `bool` (`E0605`); only `bool` values are booleans and conditions never convert numbers | Compare explicitly, for example `value != 0` |
| Loop variables | `for mut i in …` parses and lets you reassign `i` | Keep loop variables immutable |
| Literals | The most negative value of a type cannot be written as a negative literal (`E0080` applies to the token before the sign) | Write `-127 - 1`, `-32_767 - 1` and so on |
| Constants | Constant types are restricted to scalars (including `char`) and `String`; constant expressions allow arithmetic but not comparisons or function calls | Use a `let` or compute at runtime |
| Parameters | `mut` on a parameter parses but has no effect; parameters are immutable | Copy the parameter into a local `let mut` |
| String indexing | `text[i]` is rejected (`E0608`) | Use `char_at`, `substring`, `find` or `split` |
| Character classification | A `char` supports comparison, patterns, casts, formatting and the string APIs; there are no character classification helpers such as `is_alphabetic`, `is_digit` or `is_whitespace`, and no numeric `char` methods | Compare against character ranges or use casts and the provided string methods |
| Unicode properties | Case mapping and whitespace trimming use the generated Unicode tables; there is no general Unicode property or normalisation API | Precompute the characters you need |
| Formatting | Width, fill and alignment apply to a user struct/enum as a whole, but the `+`/`0` flags and precision are rejected for them (`E0001`); character precision and the `+`/`0` flags are also rejected; no named or indexed arguments | Format fields individually, or apply width/fill/alignment to the whole value |
| Numbers | `pow` is float-only; no integer exponentiation builtin | Convert to `f64` or write a loop |
| Patterns | No `@`/`ref`/slice/range patterns, and a reference pattern like `let &x = r` is rejected | Match a reference scrutinee directly: the ergonomic bindings are references, so read and write through `*binding` |
| Reference storage | References cannot be fields, tuple elements, `Vec` elements, `Option`/`Result` payloads, `Box` payloads or closure captures (`E0498`) | Keep references in locals, parameters and return values |
| Mutable references | `&mut` to a user struct/enum containing a heap field is rejected (`E0498`); mutable references to scalars, `Copy` aggregates, `char`, `String`, `Vec<T>`, `Rc<T>` and `Box<T>` work | Take `&T` or restructure the code |
| Method receivers | A by-value `self` method cannot return a reference (`E0106`), and a `&self` method can only return shared references (`E0596`) | Return shared references from `&self` methods, mutable references from `&mut self` methods, and owned values from by-value methods |
| Return elision | No lifetime syntax; a free function returning a reference needs exactly one reference parameter, otherwise `E0106` | Pass exactly one reference parameter or return an owned value |
| Moves | No partial moves: reading a `Copy` field is fine, but moving a non-`Copy` field, element or dereferenced value out of a bigger value is rejected (`E0507`) | `.clone()` the field, or restructure the value |
| Move checking | Moving a value while a live borrow exists is rejected (`E0505`); using a value after a move is rejected (`E0382`) | Order borrows and moves explicitly, or `.clone()` before the move |
| Drop in patterns | `if let Some(value) = maybe` is rejected (`E0509`) when the payload type has drop glue | Match a reference instead: `if let Some(value) = &maybe` |
| Cloning | A type that implements `Drop` cannot be cloned (`E0277`) | Use references, or restructure so the value is not cloned |
| Custom impls | `impl Copy` and `impl Clone` are rejected because both are derived automatically | Rely on the automatic derivation |
| Explicit drop | There is no `drop(value)` function (`E0425`) and no `.drop()` method (`E0599`); a value is dropped when its last owner is released | End the scope early, or move the value into a helper that consumes it |
| Drop at exit | `Drop` hooks do not run when the process exits via `exit()` or a panic; they do run when `main` returns normally | Do not rely on hooks for cleanup at abnormal exits |
| Borrow granularity | `Vec` borrows are whole-container: two `&mut` borrows of different elements conflict (`E0499`) | Mutate elements one at a time or rebuild the vector |
| Borrow checker in one expression | Passing two mutable borrows as the two arguments of one call, or combining them with `+`, is rejected (`E0499`); the short-circuit operators `&&` and `\|\|` accept them because each operand becomes its own condition | Bind each result to a local, then combine the locals |
| `for` over references | `for x in &vec` is rejected (`E0277`); loops iterate over a `Vec<T>` value, an integer range or an iterator | Iterate by value, use `.iter()`, or use an index loop over a `&Vec` |
| Reference casts | References have no `as` conversion (`E0605`) | Convert the pointed-to value with `*r` |
| UTF-8 input | `read_line()` and `args()` panic on invalid UTF-8 (`exit 101`); `read_file()` returns `Err("stream did not contain valid UTF-8")` | Feed the program valid UTF-8, or check the `Result` from `read_file` |
| Trait objects | A trait cannot be used as a type and `dyn` is not supported (`E0412`); dispatch is static | Use generic bounds |
| Trait shape | No default method bodies, associated functions, generic methods, supertraits, associated types or constants; `impl Trait for Type` only for user structs and enums (a builtin target is `E0118`) | Implement every method explicitly; use generic bounds |
| Generic shapes | No `where` clauses, associated types, const generics, default type parameters, specialization or generic type aliases/constants | Write bounds inline; name the concrete types |
| Generic checking | Generic bodies are late-checked once per instantiation; a never-used instantiation is never checked and its errors never appear; instantiating more than 128 levels deep is `E0275` | Instantiate the types you rely on; watch the recursion limit |
| Monomorphization | Each instantiation is its own checked function and C function; generic code is not shared between instantiations | Keep the number of instantiations small |
| Compiler scaling | Type checking one very large function is superlinear in its statement count: about 5,000 `let` statements take 1.1 s, 10,000 take 4.5 s, 20,000 take 17 s and 50,000 do not finish within a minute; a 100,000-element `vec![…]` literal is similarly slow | Split huge functions into helpers and build large vectors with a loop |
| Expression nesting | The parser accepts at most 128 levels of nested expressions, types and patterns. Deeper code is rejected with `E0001` "this code is nested too deeply" (the compiler runs its recursive passes on a large stack so accepted programs cannot overflow it). Ordinary code is unaffected: 100-term arithmetic chains, 100 chained method calls and flat literals of 8,000 or more elements all compile | Split pathological machine-generated code into smaller statements or helpers |
| Iterators | `.iter()` consumes a `Vec`; no `rev`, `zip`, `chain`, `flat_map`, `for_each` or `size_hint`; no `into_iter`/`iter_mut`; `collect` needs a known target type (`E0282`); `sum` supports `i64`/`f64` only (`E0277`) | Use the provided adapters; annotate `collect` results |
| HashMap | Keys are limited to `i64`, `u64`, `bool`, `String` and `char`; no `for`-loop iteration, no entry API and no `HashMap::from`; `get`/`values()` clone the value (`keys()` always clones the keys); printed order is slot order (deterministic for a given sequence of operations, but not insertion order and not guaranteed) | Use `keys()`/`values()` into a `Vec`; use `get_with` for non-cloneable values |
| HashMap hashing | The hash is deterministic and non-cryptographic; it is not designed to resist adversarial inputs | Do not expose a map directly to untrusted keys |
| Function values | Function and closure values are move-only and have no `==` (`E0369`); `.clone()` on a value holding a closure shares the captured environment instead of copying it | Storing them in aggregates works since 1.5; clone when the shared environment is intended |

## Not in version 1.6.0

The roadmap items that are still open:

* references stored inside aggregates or captured by closures;
* lifetime annotations and generic lifetimes;
* arrays and slices;
* trait objects, `dyn`, and dynamic dispatch;
* generic type aliases, `where` clauses, associated types and const generics;
* other collections (`HashSet`, `BTreeMap`, deques) and additional iterator
  adapters;
* modules, imports and multi-file programs;
* user-defined macros and attributes;
* threads;
* a package manager;
* incremental compilation;
* a language server (LSP).

A plausible order for closing these gaps:

* **v1.7** — modules, a package manager and a language server.

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
* `pub` accepted (and ignored) on struct fields, enum variants and trait methods —
  **delivered in 1.5.0**;
* the `char` type and character literals — **delivered in 1.6.0**;
* a `String` that is always valid UTF-8, with character-oriented methods,
  Unicode case mapping and Unicode whitespace trimming — **delivered in 1.6.0**;
* Unicode-aware `Debug` escaping for strings — **delivered in 1.6.0**;
* UTF-8 validation of `read_line`, `args` and `read_file` — **delivered in
  1.6.0**;
* a Swiss-Table `HashMap` with a stronger deterministic hash and `char` keys —
  **delivered in 1.6.0**.

## Language and runtime in 1.6.0

Version 1.6.0 is a **language and runtime release**. It adds:

* **A real character type.** `char` is a `Copy` 32-bit Unicode scalar value
  with character literals, escapes, literal patterns, comparisons, casts and
  formatting. See [Characters and Unicode](22-characters-and-unicode.md).
* **UTF-8 strings with a character API.** Every `String` is valid UTF-8:
  `char_count`, `chars`, `char_at`, `push(char)`, `pop`,
  `is_char_boundary` and `String::from_char` were added, and `substring`
  now panics unless both byte indices are character boundaries. Case mapping
  and `trim` follow Unicode, and `Debug` escaping is Unicode-aware.
* **Validated UTF-8 input.** `read_line()`, `args()` and `read_file()` reject
  invalid UTF-8 instead of silently replacing bytes.
* **A Swiss-Table `HashMap`.** The map is now a real Swiss Table: control-byte
  metadata, 16-slot groups, bit-mask probing, a 7/8 load factor, tombstones and
  a stronger deterministic hash. `char` joins the key types. Its iteration
  order remains unspecified in the documentation.
* **Audit-driven hardening.** Version 1.6.0 fixed a batch of generated-C and
  runtime defects found by the release audits, and added regression tests for
  them.

The compiler and runtime organization keeps the v1.5 shape:

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
  `lsaca_rt_fmt.c`, `lsaca_rt_map.c` and, new in 1.6.0,
  `lsaca_rt_unicode.c` — plus the public `lsaca_rt.h` and internal
  `lsaca_rt_internal.h`, and the generated Unicode table header
  `lsaca_rt_unicode.h`; the generated `CMakeLists.txt` lists every runtime C
  file.
* **Generated Unicode tables.** `lsaca_rt_unicode.c` is generated from Python's
  Unicode database by `scripts/gen_unicode_tables.py` and committed; rerunning
  the script reproduces it byte for byte.
* **Cached runtime objects.** Compiled runtime objects live per profile under
  `<cache>/_runtime/<hash>/` and are reused, so a new program only recompiles
  its own generated program unit or units and links.
* **Embedded prelude.** The `Iterator` trait and its adapters live in
  `crates/lsaca/src/prelude.ls`, written in lsaca and embedded in the compiler.
  A program can use `Iterator` and the adapter methods without importing
  anything.

Compile-time, runtime and memory measurements are republished with each
release; see [Benchmarks](21-benchmarks.md), alongside the comparison with C,
Rust, Python and Node.

## Compatibility

lsaca 1.6.0 is **not** a purely additive release. Programs from 1.5.0 keep
compiling in most cases, but several outputs and a few expressions change:

* case mapping: `to_uppercase`/`to_lowercase` now apply the full Unicode
  mapping, so `"ß"` uppercases to `"SS"` instead of staying `"ß"`;
* trimming: `trim` strips Unicode whitespace, not just ASCII whitespace;
* substring: `substring` still takes a byte start and length, but now panics
  when a boundary falls inside a character instead of returning replacement
  characters;
* `Debug`: `{:?}` escaping follows Rust's `str` rules, so some non-printable
  characters are escaped where they used to be printed raw;
* input: `read_line()` and `args()` panic on invalid UTF-8, and
  `read_file()` returns `Err` instead of a string with replacement characters;
* casts: an integer or float cast to `bool` is now rejected with `E0605`;
* generated C: runtime ABIs and the `HashMap` layout changed, so C projects
  emitted by earlier releases must be regenerated with the 1.6.0 compiler.

Everything else is additive: `char` is a new primitive type, the string
methods are new names, and existing programs that do not rely on the changed
behaviours compile unchanged. Ownership, references, `Drop` order, generics,
traits and iterators behave as in 1.5.0.

## Benchmarks

Numbers for the current compiler — compile time, runtime and peak memory on
eighteen runtime workloads (including the new `string_unicode`, `hashmap_i64`
and `hashmap_string` workloads, plus a compile-only scaling workload), compared
with C, Rust, Python and Node — are in [Benchmarks](21-benchmarks.md).

## What next

* The language tour starts at [Getting started](01-getting-started.md).
* The `char` type, UTF-8 strings and Unicode: [Characters and
  Unicode](22-characters-and-unicode.md).
* User-defined generics: [Generics](17-generics.md).
* Traits and trait bounds: [Traits](18-traits.md).
* Lazy iterators: [Iterators](19-iterators.md).
* The builtin hash map: [HashMap](20-hashmap.md).
* Ownership, moves and destruction: [Ownership and
  moves](11-ownership-and-moves.md).
* Non-owning views and the borrow checker: [References and
  borrowing](12-references-and-borrowing.md).
* Every builtin in detail: [Standard library](15-standard-library.md).
* The memory model that ties ownership and ORC together:
  [Memory model](05-memory-model.md).
