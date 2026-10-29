# Roadmap and deviations from Rust

lsaca borrows Rust's surface syntax and some of its semantics, but it is a
smaller language with an explicitly different memory model. This page lists the
intentional differences, the known gaps in the current release, and what is
planned next. It describes lsaca **1.5.0**.

## Intentional deviations from Rust

These are design decisions, not bugs. Code written for Rust will need edits in
these areas.

1. **Ownership with automatic `Copy` and `Clone`.** Version 1.4 added ownership
   and move semantics close to Rust's, but the marker traits are derived
   automatically instead of implemented by hand. A type is `Copy` when all of
   its parts are: integers, floats, `bool`, `()`, tuples, `Option<T>` and
   `Result<T, E>` of `Copy` types, references, and user structs/enums whose
   fields are all `Copy` and which do not implement `Drop`. Everything else
   (`String`, `Vec<T>`, `Rc<T>`, `Box<T>`, function values, and user types
   containing them) is move-only. `Clone` is automatic too: every type except those that implement
   `Drop` has `.clone()`, and `impl Copy`/`impl Clone` blocks are rejected. `Drop` is the
   only builtin trait with special code generation; user-defined traits exist since
   1.5.0 but are purely compile-time method dispatch. There are still no lifetime
   annotations;
   ORC remains the memory manager — heap allocations are reference-counted and
   freed when the last owner is released, so there is no `free`, no
   `drop(value)` builtin and no `std::mem::drop`. See
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
8. **No `char` type.** Character literals (`'a'`) are a parse error. String
   methods work on bytes or on characters depending on the method; see
   [Standard library](15-standard-library.md#string).
9. **No arrays and no slices.** `Vec<T>` is the growable sequence type, and
   tuples are the fixed-arity product type; tuples can nest.
10. **Restricted patterns.** No `@` bindings, no `ref` patterns, no slice and no
    range patterns. String-literal patterns are supported, or-patterns (`p | q`)
    and match guards are fully supported, and a reference scrutinee is matched
    with match ergonomics rather than a `&` pattern. See [Pattern
    matching](13-pattern-matching.md).
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
    compares structurally, and function values have no `==` (`E0369`).
15. **No unsafety.** There is no `unsafe`, no FFI and no threads. The generated C
    is not exposed to the language.

## Known limitations in 1.5.0

The following are implementation gaps or deliberate restrictions in the current
release, verified against the shipped compiler. They are expected to be fixed
in later releases; avoid them in code you intend to keep.

| Area | Limitation | Workaround |
| --- | --- | --- |
| Casts | `int as bool` is accepted (`0` false, nonzero true) although Rust rejects it | Avoid it; compare explicitly |
| Loop variables | `for mut i in …` parses and lets you reassign `i` | Keep loop variables immutable |
| Literals | The most negative value of a type cannot be written as a negative literal (`E0080` applies to the token before the sign) | Write `-127 - 1`, `-32_767 - 1` and so on |
| Constants | Constant types are restricted to scalars and `String`; constant expressions allow arithmetic but not comparisons or function calls | Use a `let` or compute at runtime |
| Parameters | `mut` on a parameter parses but has no effect; parameters are immutable | Copy the parameter into a local `let mut` |
| String indexing | `text[i]` is rejected (`E0608`) | Use `substring` / `find` / `split` |
| String API | No `push(char)`; character literals do not exist | Use `push_str` |
| Formatting | Width, fill and alignment apply to a user struct/enum as a whole, but the `+`/`0` flags and precision are rejected for them (`E0001`); no named or indexed arguments | Format fields individually, or apply width/fill/alignment to the whole value |
| Numbers | `pow` is float-only; no integer exponentiation builtin | Convert to `f64` or write a loop |
| Patterns | No `@`/`ref`/slice/range patterns, and a reference pattern like `let &x = r` is rejected | Match a reference scrutinee directly: the ergonomic bindings are references, so read and write through `*binding` |
| Reference storage | References cannot be fields, tuple elements, `Vec` elements, `Option`/`Result` payloads, `Box` payloads or closure captures (`E0498`) | Keep references in locals, parameters and return values |
| Mutable references | `&mut` to a user struct/enum containing a heap field is rejected (`E0498`); mutable references to scalars, `Copy` aggregates, `String`, `Vec<T>`, `Rc<T>` and `Box<T>` work | Take `&T` or restructure the code |
| Method receivers | A by-value `self` method cannot return a reference (`E0106`), and a `&self` method can only return shared references (`E0596`) | Return shared references from `&self` methods, mutable references from `&mut self` methods, and owned values from by-value methods |
| Return elision | No lifetime syntax; a free function returning a reference needs exactly one reference parameter, otherwise `E0106` | Pass exactly one reference parameter or return an owned value |
| Moves | No partial moves: reading a `Copy` field is fine, but moving a non-`Copy` field, element or dereferenced value out of a bigger value is rejected (`E0507`) | `.clone()` the field, or restructure the value |
| Move checking | Moving a value while a live borrow exists is rejected (`E0505`); using a value after a move is rejected (`E0382`) | Order borrows and moves explicitly, or `.clone()` before the move |
| Drop in patterns | `if let Some(value) = maybe` is rejected (`E0509`) when the payload type has drop glue | Match a reference instead: `if let Some(value) = &maybe` |
| Cloning | A type that implements `Drop` cannot be cloned (`E0277`) | Use references, or restructure so the value is not cloned |
| Custom traits | `impl Copy` and `impl Clone` are rejected because both are derived automatically | Rely on the automatic derivation |
| Explicit drop | There is no `drop(value)` function (`E0425`) and no `.drop()` method (`E0599`); a value is dropped when its last owner is released | End the scope early, or move the value into a helper that consumes it |
| Drop at exit | `Drop` hooks do not run when the process exits via `exit()` or a panic; they do run when `main` returns normally | Do not rely on hooks for cleanup at abnormal exits |
| Borrow granularity | `Vec` borrows are whole-container: two `&mut` borrows of different elements conflict (`E0499`) | Mutate elements one at a time or rebuild the vector |
| Borrow checker in one expression | Two mutable borrows written in a single expression are rejected (`E0499`) even when they run in sequence | Bind each call to a local, then combine the locals |
| `for` over references | `for x in &vec` is rejected (`E0277`); loops iterate over a `Vec<T>` value, an integer range or an iterator | Iterate by value, use `.iter()`, or use an index loop over a `&Vec` |
| Reference casts | References have no `as` conversion (`E0605`) | Convert the pointed-to value with `*r` |
| Trait objects | A trait cannot be used as a type and `dyn` is not supported (`E0412`); dispatch is static | Use generic bounds |
| Trait shape | No default method bodies, associated functions, generic methods, supertraits, associated types or constants; `impl Trait for Type` only for user structs and enums (a builtin target is `E0118`) | Implement every method explicitly; use generic bounds |
| Generic shapes | No `where` clauses, associated types, const generics, default type parameters, specialization or generic type aliases/constants | Write bounds inline; name the concrete types |
| Generic checking | Generic bodies are late-checked once per instantiation; a never-used instantiation is never checked and its errors never appear; instantiating more than 128 levels deep is `E0275` | Instantiate the types you rely on; watch the recursion limit |
| Monomorphization | Each instantiation is its own checked function and C function; generic code is not shared between instantiations | Keep the number of instantiations small |
| Iterators | `.iter()` consumes a `Vec`; no `rev`, `zip`, `chain`, `flat_map`, `for_each` or `size_hint`; no `into_iter`/`iter_mut`; `collect` needs a known target type (`E0282`); `sum` supports `i64`/`f64` only (`E0277`) | Use the provided adapters; annotate `collect` results |
| HashMap | Keys are limited to `i64`, `u64`, `bool` and `String`; no `for`-loop iteration, no entry API and no `HashMap::from`; `get`/`values()` clone the value (`keys()` always clones the keys); printed order is slot order | Use `keys()`/`values()` into a `Vec`; use `get_with` for non-cloneable values |
| Function values | Function and closure values are move-only and have no `==` (`E0369`); `.clone()` on a value holding a closure shares the captured environment instead of copying it | Storing them in aggregates works since 1.5; clone when the shared environment is intended |

## Not in version 1.5.0

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

* **v1.6** — modules, a package manager and a language server.

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
  **delivered in 1.5.0**.

## Language and runtime in 1.5.0

Version 1.5.0 is a **language release**. It adds four user-facing features:

* **User-defined generics.** Structs, enums, free functions, inherent impls and
  trait impls can be generic, with inline bounds. Generics are monomorphized:
  each concrete instantiation is checked and compiled separately, and type
  parameters must be resolved at the call site (directly or with the turbofish).
  See [Generics](17-generics.md).
* **Traits with static dispatch.** A trait declares method signatures with
  receivers; `impl Trait for Type` implements it, and trait bounds constrain
  generic parameters. Dispatch prefers inherent methods and resolves trait
  methods at compile time; there are no trait objects. See
  [Traits](18-traits.md).
* **Lazy iterators.** `Vec<T>.iter()` consumes the vector and returns an
  iterator; adapters such as `map`, `filter`, `take`, `skip` and `enumerate` are
  generic functions in the prelude, and `for` loops work over any type that
  implements `Iterator<T>`. The iterator API is written in lsaca itself, as the
  embedded prelude: a standard-library unit that is parsed with every program
  but contributes no source spans of its own. See
  [Iterators](19-iterators.md).
* **A builtin `HashMap<K, V>`.** The map is a generic value type with the
  methods `insert`, `get`, `get_with`, `contains_key`, `remove`, `len`,
  `is_empty`, `clear`, `keys` and `values`, implemented by a new runtime unit.
  See [HashMap](20-hashmap.md).

1.5.0 also closes several v1.4 gaps: user structs, enums, tuples, `Vec`,
`Option`, `Result`, references and `HashMap` now support `==`; function and
closure values can be stored in struct fields, enums, tuples, `Vec`, `Box` and
as HashMap values; and `Self::new(…)` and `Self { … }` work inside impl blocks.
Ownership, references, `Drop` and the ORC memory manager are unchanged.

The compiler and runtime organization keeps the v1.4 shape:

* **Direct C compiler backend by default.** `lsaca run` and `lsaca build` write
  the generated C into the build cache, invoke the C compiler for `main.c` and
  the runtime units, and link the executable themselves. CMake is only needed
  with the global `--cmake` flag; `emit-c` still writes a complete CMake project.
* **Split runtime sources.** The runtime is built from several translation
  units — `lsaca_rt.c`, `lsaca_rt_string.c`, `lsaca_rt_vec.c`,
  `lsaca_rt_fmt.c` and, new in 1.5.0, `lsaca_rt_map.c` — plus the public
  `lsaca_rt.h` and internal `lsaca_rt_internal.h`; the generated
  `CMakeLists.txt` lists every runtime C file.
* **Cached runtime objects.** Compiled runtime objects live per profile under
  `<cache>/_runtime/<hash>/` and are reused, so a new program only recompiles
  its own `main.c` and links.
* **Embedded prelude.** The `Iterator` trait and its adapters live in
  `crates/lsaca/src/prelude.ls`, written in lsaca and embedded in the compiler.
  A program can use `Iterator` and the adapter methods without importing
  anything.

Compile-time, runtime and memory measurements are republished with each
release; see [Benchmarks](21-benchmarks.md), alongside the comparison with C,
Rust, Python and Node.

## Compatibility

lsaca 1.5.0 is an **additive release** over 1.4.0. Existing 1.4.0 programs
compile unchanged with two caveats:

* the prelude now reserves the names `Iterator` and `HashMap`; a program that
  defines its own trait named `Iterator` (or a type named `HashMap`) must rename
  it;
* `Vec` now has an `.iter()` method; inherent methods on builtin types are not
  possible, so user code cannot shadow it, and code that relied on `Vec` *not*
  having `.iter()` keeps compiling — nothing is removed.

The new capabilities are additive: generics, traits, iterators and `HashMap` are
new syntax and new library items, aggregate `==` turns a former `E0369` into a
successful comparison, and function values previously rejected in aggregates
now work. Move checking, borrow checking, references, `Drop` order, formatting
and the ORC runtime behave as in 1.4.0. Generated C projects from earlier
releases must be regenerated with the 1.5.0 compiler because the runtime gains
a translation unit and the emitted C for generic code is new.

## Benchmarks

Numbers for the current compiler — compile time, runtime and peak memory on
fifteen runtime workloads (plus a compile-only scaling workload), compared with C,
Rust, Python and Node — are in [Benchmarks](21-benchmarks.md).

## What next

* The language tour starts at [Getting started](01-getting-started.md).
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
