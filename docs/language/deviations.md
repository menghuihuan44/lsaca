# Deviations from Rust

lsaca borrows Rust's surface syntax and some of its semantics, but it is a
smaller language with an explicitly different memory model. This page catalogs
the deliberate design differences from Rust — decisions, not bugs, that change
how Rust code must be written. Verified gaps and restrictions of the current
release are listed in [Known limitations](limitations.md); the plan is in
[ROADMAP.md](../../ROADMAP.md), the completion matrix in
[STATUS.md](../../STATUS.md), and the user-visible history in
[CHANGELOG.md](../../CHANGELOG.md).

1. **Ownership with automatic `Copy` and `Clone`.** Version 1.4 added ownership
   and move semantics close to Rust's, but the marker traits are derived
   automatically instead of implemented by hand. A type is `Copy` when all of
   its parts are: integers, floats, `bool`, `char`, `()`, tuples, arrays of
   `Copy` elements, `Option<T>` and `Result<T, E>` of `Copy` types, references (including the
   2.0 fat references `&[T]`, `&str` and `&dyn Trait`), and user
   structs/enums whose fields are all `Copy` and which do not implement
   `Drop`. Everything else (`String`, `Vec<T>`, `Rc<T>`, `Box<T>`,
   `Box<dyn Trait>`, `Rc<dyn Trait>`, function values, and user types containing
   them) is move-only. `Clone` is automatic
   too: every type except those that implement `Drop` has `.clone()`, and
   `impl Copy`/`impl Clone` blocks are rejected. `Drop` is the only builtin
   trait with special code generation; user-defined traits exist since 1.5.0 and
   since 2.0 can be used behind a pointer as trait objects, where calls dispatch
   through a vtable. Lifetime annotations exist only for reference fields:
   since 2.2.0 a struct, enum or tuple struct may hold a direct `&'a T`/`&'a mut T`
   field (`Option<&T>` and tuple-typed fields are rejected with `E0498`),
   checked by a small conservative pass with no full Rust inference (see
   [Lifetimes](29-lifetimes.md)). ORC remains the memory manager — heap allocations are
   reference-counted and freed when the last owner is released, so there is no
   `free`, no `drop(value)` builtin and no `std::mem::drop`. See
   [Ownership and moves](11-ownership-and-moves.md) and
   [Memory model](05-memory-model.md).
2. **References are non-owning and restricted.** Version 1.3 added `&T` and
   `&mut T`, with borrow expressions (`&place`, `&mut place`), dereference
   (`*r`) and match ergonomics — but there are no raw pointers, no address
   arithmetic and no way to observe an address. References cannot be stored in
   `Vec`, `Result`, `HashMap`, `Rc`, `Box` or an array, cannot be captured
   by closures, and cannot be cast with `as`. Since 2.2.0 a struct, enum or tuple struct may hold a direct reference field
   (`&'a T`/`&'a mut T`) and then carries a lifetime parameter; `Option<&T>` and
   tuple-typed fields are rejected with `E0498`. Version 1.5 adds `==` between
   references, which compares the pointed-to values. Mutable references work for
   scalars, tuples, structs and enums, and for the owned heap types
   `&mut String`, `&mut Vec<T>`, `&mut Rc<T>` and `&mut Box<T>`; `&mut` to a
   user struct or enum that contains a heap field is still rejected (`E0498`).
   Lifetime syntax is limited to reference fields: a type path may omit its
   lifetime argument, and elision in function signatures follows fixed rules.
   Version 2.0 adds
   unsized borrowed forms — `&[T]`, `&mut [T]`, `&str`, `&mut str` and
   `&dyn Trait`/`&mut dyn Trait` — produced by coercion from `&Vec<T>`,
   `&String`, `&T` or by a slice expression; they are two-word values, and since
   2.2.0 a direct reference field of a struct, enum or tuple struct may hold one
   (`Option<&T>` and tuple-typed fields remain rejected). See
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
9. **Arrays are fixed-length values; slices exist only behind references.**
   Version 2.2.0 adds the array type `[T; N]` with the literals `[1, 2, 3]`,
   `[value; N]` and `[]`, `usize` indexing and `.len()`/`.is_empty()`; an array
   is `Copy` when its element type is `Copy`, and `&array` coerces to `&[T]`.
   There are no array patterns, no iteration over an array by value, and an
   array length must be an integer literal or integer constant (`E0435`);
   the length is limited to 1,048,576 elements. Slices are the unsized types
   `[T]` and `str` from 2.0, which exist only behind `&` or `&mut`: `&[T]`,
   `&mut [T]`, `&str` and `&mut str`. A slice expression (`&values[1..3]`)
   produces one, and `&Vec<T>`/`&String`/`&array` coerce to them; `[T]` and
   `str` methods live in the `std` crate. `Vec<T>` remains the growable
   sequence type, and tuples nest. See [Arrays](27-arrays.md) and
   [Slices and str](26-slices-and-str.md).
10. **Restricted patterns.** No `@` bindings, no `ref` patterns, no slice and no
    range patterns. String-literal and character-literal patterns are supported,
    tuple struct and unit struct patterns were added in 2.1.0, or-patterns
    (`p | q`) and match guards are fully supported, and a reference scrutinee is
    matched with match ergonomics rather than a `&` pattern. See
    [Pattern matching](13-pattern-matching.md).
11. **Loop labels exist; `break` values do not.** Since 2.2.0 loops can be
    labelled (`'outer: loop`, `'outer: while`, `'outer: for`, and a label may
    precede `while let`) and `break 'outer;`/`continue 'outer;` leave or
    continue the named loop. Loops still have type `()`, `break value` is
    rejected (`E0308`), and a label on anything but a loop is `E0001`. See
    [Loop labels](28-loop-labels.md).
12. **A standard library crate with iterators and collections.** The 1.5
    release added lazy iterators and the builtin `HashMap<K, V>`; version 2.0
    moved the whole standard library into a normal `std` crate that is embedded
    in the compiler (`stdlib/src/lib.ls`, parsed as `std/*.ls`). In 2.3.0 it
    covers `core` (`Option`, `Result`, `Iterator`), `string`, `char`, `fmt`,
    `math`, `vec`, `iter`, `collections` (`HashSet`, `BTreeSet`,
    `BTreeMap`, `VecDeque`, `BinaryHeap`), `io` (`File`, `Error`/`ErrorKind`,
    the `Read`/`Write`/`Seek`/`BufRead` traits, `BufReader` and `BufWriter`), `fs`, `net`
    (plain TCP), `time`, `env`, `process` and `error` (the `Error` trait).
    Iterators provide `zip`, `chain`, `filter_map`, `take_while`, `skip_while`,
    `step_by`, `rev` and `flat_map` next to `map`, `filter`, `take`, `skip`
    and `enumerate`, with the terminals `collect`, `fold`, `count`, `sum`,
    `product`, `any`, `all`, `find`, `position`, `last`, `nth`, `reduce`,
    `inspect` and `for_each`. Since 2.3.0 `.iter()` **borrows** the vector and
    yields `&T`, `.iter_mut()` yields `&mut T`, and `.into_iter()` is the
    consuming form. `HashMap` (keys are the integer types, `bool`, `char` and
    `String`) can be iterated with `for (key, value) in &map`, and its
    `iter()`, `keys()` and `values()` borrow instead of cloning; `HashMap::from`
    builds a map from a vector of pairs. Formatting still
    has no named or indexed arguments and no integer bases. The `?` operator is
    supported for `Option` and `Result`. The 2.1.0 standard library adds
    `Option::or`, `Option::xor` and `Result::unwrap_err`. Version 2.3
    completes the Rust combinator surface: `Option` gains `or_else`,
    `map_or_else`, `ok_or_else`, `unwrap_or_else`, `is_some_and`,
    `is_none_or` and `and`, and `Result` gains `map_or`, `map_or_else`,
    `or`, `or_else`, `is_ok_and`, `is_err_and`, `inspect`,
    `inspect_err` and `unwrap_or_else`. `Option::unwrap_or_default` still
    waits for the `Default` trait, which is planned with the other formatting
    and utility protocols for v2.4. Text parameters of
    the standard library take `&str` instead of an owned `String` (`env::var`
    is the one exception: `&str` in, `Result<String, VarError>` out;
    `io::Error::new`/`other` also keep taking an owned `String` message), while
    constructors such as `String::from` keep taking an owned `String`, and a
    string literal in a `&str` position is a static slice that does not
    allocate. The design is fixed; which parts the 2.3.0 compiler implements is
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
15. **Partial moves are all-or-nothing.** A `let` pattern that binds an owning
    value (`String`, `Vec`, `Rc`, `Box`, a move-only struct, ...) moves the whole
    initialiser: the bound fields move into the new bindings and the remaining
    fields are dropped, so the source binding is moved from then on (`E0382`).
    Rust keeps the unbound fields usable through the source. A pattern that
    binds only `Copy` values copies them, exactly like Rust. A type with a
    `Drop` impl cannot be destructured by value (`E0509`, same as Rust), and
    `match` moves its scrutinee. See
    [Variables](06-variables-and-mutability.md#destructuring-bindings) and
    [Ownership and moves](11-ownership-and-moves.md).

16. **Lightweight lifetimes, not Rust's.** Since 2.2.0 a struct, enum or tuple
    struct may store direct `&'a T`/`&'a mut T` fields (`Option<&T>` and
    tuple-typed fields are rejected with `E0498`), declared with a lifetime
    parameter (`struct View<'a> { data: &'a [i64] }`) and named on `impl` blocks
    (`impl<'a> View<'a>`). A reference field must name a declared lifetime
    (`E0106` otherwise), a lifetime argument must name a declared parameter
    (`E0261`), and `'static` is **not** accepted as a field lifetime:
    `struct View { text: &'static str }` is
    `error[E0498]: the 'static lifetime is not supported yet`, so every field
    lifetime must be a declared named parameter. The checker is conservative:
    it has no variance, no outlives relations, does not check that named
    lifetimes are equal, and extends the `E0515`/`E0597` borrow checks to
    aggregates. References are still rejected inside `Vec`, `Result`,
    `HashMap`, `Rc`, `Box`, arrays and closures (`E0498`). See
    [Lifetimes](29-lifetimes.md).
17. **Error conversion through `From`.** The library provides the `From<T>`
    trait and the `Error` trait's `source` default method. In a function that
    returns a `Result`, `?` converts the found error type into the expected
    error type through a user-written `impl From<Found> for Expected`; without
    one the program is `E0308` with a `From` note. There is no blanket
    `From<T> for T`, no `Into`, no `Try` trait and no `From<&str>` for `String`,
    so conversions are always explicit. See [From and ?](30-from-try.md).
18. **`lsaca fmt` is conservative.** The formatter rewrites indentation (four
    spaces, a dedent for leading closing delimiters, one extra level for a
    leading `.` in a method chain), trailing whitespace, blank-line runs and
    line endings (CRLF becomes LF), and guarantees exactly one final newline;
    it never reflows or merges lines, never changes token spacing inside a
    line, and formats one file at a time (it does not follow `mod`
    declarations). A syntax error leaves the file untouched; `--check` reports
    the files that would change (exit 1, `lsaca: would format <file>`) and
    `--stdout` prints the result instead of rewriting. The transformation is
    idempotent. See [Getting started](01-getting-started.md#formatting-source-files).
19. **Methods are not callable as associated functions.** A method exists only in
    method-call position: `value.method()`. Writing it in UFCS form, as
    `Type::method(&value)` or `Rc::strong_count(&handle)`, fails with `E0599`
    ("method `Type::method` cannot be called as an associated function") and a
    note that suggests `value.method()`. Since 2.3.0 this is a deliberate,
    documented restriction with an explanatory diagnostic, not a confusing type
    error: genuine associated functions — `Rc::new(...)`, `String::from(...)` and
    any user `impl` function without `self` — do work with `Type::function(...)`.
