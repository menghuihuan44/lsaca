# Types

Every expression in lsaca has a statically known type, checked before any C is
generated. This page lists the complete type language, explains each builtin
type, and describes inference and conversions. It describes lsaca 2.2.0, where
unannotated integer literals are `i32` by default, number literals accept
Rust-style type suffixes, and lengths and indices are `usize`.

## The type table

| Type | Description | Notes |
| --- | --- | --- |
| `()` | Unit. | The result of expressions that produce nothing; can be stored and printed. |
| `bool` | Boolean. | `true` / `false`. |
| `i8` `i16` `i32` `i64` | Signed integers. | Two's complement; `i32` is the default integer type. Lengths and indices are `usize`. |
| `u8` `u16` `u32` `u64` | Unsigned integers. | Values are kept in the corresponding C unsigned type. |
| `isize` `usize` | Pointer-sized signed / unsigned integers. | Added in 2.0.0; C `intptr_t`/`uintptr_t`, 64 bits on every supported target; `usize` is the standard length, index and count type; see [Integers](#integers). |
| `f32` `f64` | IEEE-754 binary floating point. | `f64` is the default float type. |
| `char` | Unicode scalar value. | Added in 1.6.0; written with single quotes; a `Copy` scalar stored as a 32-bit value; see [Characters and Unicode](22-characters-and-unicode.md). |
| `String` | Owned UTF-8 string. | Move-only refcounted buffer; `.clone()` produces an independent copy; `len()` is a byte count (`usize`). A string literal is a `String` by default but is compiled as a static `&str` slice in `&str` contexts, with no allocation. |
| `Vec<T>` | Growable array. | Builtin generic, move-only; `.clone()` deep-copies. |
| `HashMap<K, V>` | Hash table. | Added in 1.5.0; builtin generic, move-only; keys are the integer types, `bool`, `char` or `String`; `==` compares contents. |
| `Option<T>` | `Some(T)` or `None`. | Builtin generic. |
| `Result<T, E>` | `Ok(T)` or `Err(E)`. | Builtin generic. |
| `Rc<T>` | Shared, mutable heap cell. | Move-only handle; `.clone()` shares one payload; `get()`/`set()`. |
| `Box<T>` | Owned heap indirection. | Added in 1.1.0; move-only; `.clone()` deep-copies the payload. |
| `&T` / `&mut T` | Shared / mutable reference. | Added in 1.3.0; a non-owning borrow of a place. A struct, enum, tuple or `Option` field may store one since 2.2.0 (with a lifetime parameter); `Vec`, `Result`, `HashMap`, `Rc`, `Box`, arrays and closures still cannot hold references; mutable references to `String`, `Vec`, `Rc` and `Box` are allowed since 1.4; see [References and borrowing](12-references-and-borrowing.md) and [Lifetimes](29-lifetimes.md). |
| `&[T]` / `&mut [T]` / `&str` / `&mut str` | References to an unsized slice or UTF-8 text value. | Added in 2.0.0; `&[T]` is a `Copy` fat reference (data pointer plus length); produced by coercions from `Vec`/`String`, by slicing a place with `&`/`&mut`, and for string literals in `&str` contexts; see [Slices and str](26-slices-and-str.md). |
| `[T; N]` | Fixed-length array value. | Added in 2.2.0; `N` is an integer literal or a path to an integer constant; indexed with `usize`; `Copy` when `T` is `Copy`; `&array` coerces to `&[T]`; no array patterns and no by-value `for`; see [Arrays](#arrays). |
| `(T1, T2, ...)` | Tuple. | Held by value; fields `.0`, `.1`, ...; tuples can nest. |
| `fn(A, B) -> R` | Function type. | Values of this type are first-class but move-only; since 1.5 they can be stored in aggregates. See [Functions](09-functions.md). |
| `dyn Trait` | Trait object (unsized). | Added in 2.0.0; only behind a pointer: `Box<dyn Trait>`, `Rc<dyn Trait>`, `&dyn Trait`, `&mut dyn Trait`; see [Trait objects](25-trait-objects.md). |
| `Name` | User struct or enum. | Declared with `struct`/`enum` — named, tuple (`struct Pair(i32, String);`) and unit (`struct Marker;`) structs since 2.1.0; generic types are written `Name<T, ...>` since 1.5. `Copy` when every kind of field is `Copy` and there is no `Drop` impl; otherwise move-only. `.clone()` is derived automatically unless the type implements `Drop`; `impl Drop for Name` adds a destructor hook. |
| `Alias` | Type alias. | `type Alias = T;` introduces no new type. |

`isize` and `usize` are the pointer-sized integer types: their C types are
`intptr_t` and `uintptr_t`, and on every supported 64-bit target their ranges
equal `i64` and `u64`. They are distinct types like every other integer type.
An unannotated integer literal is an `i32` by default, but it takes the
type required by its context — including `usize` in a length, index or explicit
`usize` position; see [Integers](#integers).
There is no raw pointer type. `[T; N]` is a fixed-length array **value** (see
[Arrays](#arrays)); `[T]` and `str` are *unsized* types: they have no values of
their own, and the only usable forms are the references `&[T]`, `&mut [T]`,
`&str` and `&mut str` (see [Slices and str](26-slices-and-str.md)). Generic user types are written with
generic type arguments, `Name<T, ...>` (see [Generics](17-generics.md)). A bare
trait name is not a type and using one as a type is `E0412`; since 2.0.0 a trait
becomes a type only behind `dyn`, as in `Box<dyn Greet>` (see
[Traits](18-traits.md) and [Trait objects](25-trait-objects.md)). References are
written `&T` (shared) and `&mut T` (mutable); they are **non-owning** views of
an existing place, added in 1.3.0. Since 2.2.0 a struct, enum, tuple or
`Option` may hold references in fields when its declaration names a lifetime
parameter; `Vec`, `Result`, `HashMap`, `Rc`, `Box`, arrays and closures still
reject reference payloads. They are described in
[References and borrowing](12-references-and-borrowing.md) and
[Lifetimes](29-lifetimes.md). `Self` is not a type available to
user code in general: it names "the type of this `impl` block" inside an
`impl`, and is accepted in parameter and return positions there (see [Structs,
enums and impl](10-structs-enums-and-impl.md)).

## Integers

The integer types differ only in width and signedness, and they are **distinct
types**: there are no implicit conversions between them. Arithmetic requires
both operands to have the same type; to change the type, use `as`. A literal
can spell its type with a suffix (`1u8`, `42i64`, `0xFFusize`); an unsuffixed
literal defaults to `i32`, or takes the type that context requires.

| Type | Range |
| --- | --- |
| `i8` | -128 ..= 127 |
| `i16` | -32,768 ..= 32,767 |
| `i32` | -2,147,483,648 ..= 2,147,483,647 |
| `i64` | -9,223,372,036,854,775,808 ..= 9,223,372,036,854,775,807 |
| `u8` | 0 ..= 255 |
| `u16` | 0 ..= 65,535 |
| `u32` | 0 ..= 4,294,967,295 |
| `u64` | 0 ..= 18,446,744,073,709,551,615 |
| `isize` | same range as `i64` on every supported (64-bit) target |
| `usize` | same range as `u64` on every supported (64-bit) target |

The type of an unannotated integer literal is `i32` unless context determines
a different integer type. The integer type names are the same as in Rust except
that lsaca has no `i128`/`u128` types: `i8`, `i16`, `i32`, `i64`, `isize` and
`u8`, `u16`, `u32`, `u64`, `usize`. A suffix fixes the type exactly (`1i64`,
`1u8`, `1usize`, `1isize`); it must be a legal number suffix, so `1.5u8`,
`1i32f`, `1i128` and `1u128` are rejected while lexing.
Debug builds check overflow and division by zero at run time; release builds
wrap. This is described in
[Operators](07-operators.md), and the compile-time literal range check is
covered in [Lexical structure](03-lexical-structure.md).

```ls
fn main() {
    let a: i8 = 127;
    let b: i16 = -32_767;
    let c: i32 = 2_147_483_647;
    let d: i64 = 9_223_372_036_854_775_807;
    let e: u8 = 255;
    let f: u16 = 65_535;
    let g: u32 = 4_294_967_295;
    let h: u64 = 18_446_744_073_709_551_615;
    println!("{} {} {}", a, b, c);
    println!("{} {} {} {}", d, e, f, g);
    println!("{}", h);
}
```

Output:
```text
127 -32767 2147483647
9223372036854775807 255 65535 4294967295
18446744073709551615
```

Literals can carry a suffix instead of relying on an annotation, and all the
usual bases with `_` separators are accepted:

```ls
fn main() {
    println!("{} {}", 1u8, 42i64);
    println!("{} {}", 0xFFu16, 0b1010u32);
    println!("{} {}", 0o755u32, 1_000_000usize);
    println!("{}", 1.5f32);
}
```

Output:
```text
1 42
255 10
493 1000000
1.5
```

An unsuffixed literal takes the type its context requires, including `usize`
for lengths and indices:

```ls
fn main() {
    let values = vec![10, 20, 30];
    let index = 1;
    let count = 0;
    println!("{} {}", values[index], count + values.len());
}
```

Output:
```text
20 3
```

## Floats

There are two floating-point types, `f32` and `f64`, both IEEE-754. An
unannotated float literal is `f64` unless context requires `f32`. Floats follow
C semantics for special values: division by zero produces an infinity, `0.0 /
0.0` produces NaN, and comparisons involving NaN are false except `!=`. Display
uses the shortest round-tripping decimal form; `{:?}` and format specifiers work
as described in [Standard library](15-standard-library.md).

```ls
fn main() {
    let x: f64 = 1.0 / 3.0;
    let y: f32 = 0.5;
    let z = x as f64;
    println!("{} {} {}", x, y, z);
    println!("{} {}", 0.0 / 0.0, 1.0 / 0.0);
    println!("{} {}", 0.0 / 0.0 == 0.0, 0.0 / 0.0 != 0.0);
}
```

Output:
```text
0.3333333333333333 0.5 0.3333333333333333
NaN inf
false true
```

## Unit

The unit type `()` has exactly one value, also written `()`. It is the implicit
return type of functions that do not return a value, and the type of statements
such as `println!`, assignments, loops and `if` without a value. Unlike Rust's
unit in most positions, lsaca lets you store it in a variable and print it, which
is occasionally useful in generic-looking code.

## Strings and Vecs

`String`, `Vec<T>` and `HashMap<K, V>` are the owned container types. All three
are **move-only**: assigning or passing one by value moves it, so clone first
when both sides must stay usable. `.clone()` produces an independent copy (the
runtime may share storage internally until one side is modified, but you cannot
observe that). They are addressed with `usize` indices and lengths. Since 1.6.0
every `String` is guaranteed valid UTF-8: the byte-oriented methods keep byte
semantics (`len`, `find`, `substring`), while `char_count`, `chars`, `char_at`,
`push(char)`, `pop` and `is_char_boundary` work with Unicode scalar values (see
[Characters and Unicode](22-characters-and-unicode.md)). A string literal is a
`String` by default, but where a `&str` is expected it is compiled as a static
UTF-8 slice with no allocation. They are described in
[Memory model](05-memory-model.md) and their full API is in
[Standard library](15-standard-library.md).

```ls
fn main() {
    let mut text = String::from("abc");
    let mut numbers = vec![1, 2, 3];
    let copy = text.clone();
    text.push_str("d");
    numbers.push(4);
    println!("{} {} {} {}", text.len(), copy.len(), numbers.len(), copy);
}
```

Output:
```text
4 3 4 abc
```

## Arrays

`[T; N]` is a fixed-length array of `N` values of type `T`, added in 2.2.0. It
is a value type: the length is part of the type and is written as an integer
literal or a path to an integer constant (`[i64; 4]`, `[u8; WIDTH]`). A computed
length is rejected with `E0435`; the largest array has 1,048,576 elements, and
`[T; 0]` is a legal zero-length array.

Array literals write their elements in brackets: `[1, 2, 3]`; a repeat literal
`[value; N]` copies or clones `value` `N` times; `[]` is an empty array whose
element type must be inferable (`E0282` otherwise). Elements are indexed with a
`usize`, and an element place can be borrowed (`&array[i]`, `&mut array[i]`).
`[T; N]` is `Copy` exactly when `T` is `Copy`; otherwise assigning or passing
the array moves it. Arrays compare with `==` and `!=` element by element, print
with `{:?}` as `[1, 2, 3]`, and coerce behind a reference: `&array` is a
`&[T]`, and `&array[a..b]` is a slice. There is no by-value `for` over an array
(`E0277`) and no array pattern; iterate over the indices or borrow a slice.
`&mut array` and references inside an array are not supported. See
[Arrays](27-arrays.md).

## Option and Result

`Option<T>` and `Result<T, E>` are builtin enums. `Option<T>` is either
`Some(value)` or `None`; `Result<T, E>` is either `Ok(value)` or `Err(error)`.
They are the idiomatic way to express a missing value or an operation that can
fail, and they support pattern matching and the methods listed in
[Standard library](15-standard-library.md). Both types support `==` since
1.5.0, comparing payloads structurally.

```ls
fn classify(n: i64) -> Result<String, String> {
    if n < 0 {
        return Err(String::from("negative"));
    }
    Ok(String::from("non-negative"))
}

fn main() {
    let some: Option<i64> = Some(3);
    let none: Option<i64> = None;
    println!("{} {}", some.is_some(), none.is_none());

    match classify(-1) {
        Ok(text) => println!("ok: {}", text),
        Err(reason) => println!("err: {}", reason),
    }
    match classify(5) {
        Ok(text) => println!("ok: {}", text),
        Err(reason) => println!("err: {}", reason),
    }
}
```

Output:
```text
true true
err: negative
ok: non-negative
```

## Rc and Box

`Rc<T>` is a shared **cell**: handles made with `.clone()` all refer to one
payload, and mutation through any handle is visible through all of them.
`Box<T>` is an owned heap allocation with value semantics: `.clone()` copies the
payload (a deep copy), while assigning or passing a box by value moves it.
Boxes never alias. Both exist to break the "everything is a
value" rule in controlled ways: `Rc` when you need shared mutable state or a
cycle, `Box` when you need indirection (a recursive data structure, or a large
value moved without a deep copy on the surface). `Box` is described in
[Memory model](05-memory-model.md).

```ls
fn main() {
    let cell = Rc::new(1);
    let shared = cell.clone();
    shared.set(shared.get() + 41);
    println!("{} {}", cell.get(), shared.get());

    let boxed = Box::new(String::from("owner"));
    let moved = boxed.clone();
    println!("{} {}", boxed.get(), moved.get());
    println!("{}", boxed == moved);
}
```

Output:
```text
42 42
owner owner
true
```

## References

A reference is a non-owning view of a place: a local variable, a function
parameter, a field, an index, or a dereferenced reference. Writing `&place`
creates a shared reference (`&T`); `&mut place` creates a mutable reference
(`&mut T`). A reference is created with a *borrow expression* and read or
written with the dereference operator `*`:

```ls
fn main() {
    let mut value = 10;
    let shared = &value;
    println!("{}", *shared);
    let exclusive = &mut value;
    *exclusive += 1;
    println!("{}", value);
}
```

Output:
```text
10
11
```

References are first-class for *reading*: they can be copied, passed to
functions, returned from functions, and stored in locals. Unlike `String`,
`Vec`, `Rc<T>` and `Box<T>`, a reference does **not** own anything and does
**not** change any reference count; it must not outlive the value it points to,
and the compiler's lightweight borrow checker enforces that (see
[References and borrowing](12-references-and-borrowing.md)).

Field access, method calls and indexing automatically dereference a reference,
so `r.field`, `r.method()` and `r.field = value` all work without an explicit
`*`; `(*r).field = value` is an equivalent spelling. Since 2.2.0 a reference can be a field of a struct, enum, tuple or
`Option` when the declaration names a lifetime parameter (see
[Lifetimes](29-lifetimes.md)); it still cannot be stored in a `Vec`, a
`Result`, a `HashMap`, `Rc`, `Box`, an array or a closure, and it cannot be
converted with `as`. Since 1.5.0 `==` compares the pointed-to values.

* `&T` — shared reference: any number of shared references may exist; reads only.
* `&mut T` — mutable reference: exactly one at a time, and no shared reference
  may be live at the same time.

The full rules, including reference parameters, elision and match ergonomics,
are on [References and borrowing](12-references-and-borrowing.md).

## Tuples

A tuple groups a fixed number of values of possibly different types:
`(i64, String, bool)`. A one-element tuple needs a trailing comma — `(x,)` — and
`(x)` is just a parenthesized expression. The unit `()` is the zero-element
tuple. Elements are accessed by index (`t.0`, `t.1`, ...) or destructured in a
`let`. Tuples are held by value and copied element-wise.

Tuples can nest: a tuple may contain another tuple as a value, as a struct
field, or as a pattern (`let (a, (b, c)) = (1, (2, 3));`). Tuples with element
types such as `String`, `Rc<T>` or `Option<String>` work too.

```ls
fn main() {
    let pair = (1, String::from("one"));
    let (number, name) = pair;
    println!("{} {}", pair.0, pair.1);
    println!("{} {}", number, name);

    let triple = (1, 2.5, true);
    println!("{} {} {}", triple.0, triple.1, triple.2);

    let single = (7,);
    println!("{}", single.0);
}
```

Output:
```text
1 one
1 one
1 2.5 true
7
```

## Function types

A function value has a type written `fn(A, B) -> R`, with the return type
omitted for `()`. Functions can be stored in `let` bindings, passed to other
functions and returned from functions. Like closures they are **move-only**:
`let g = f;` moves `f` unless you write `f.clone()`, and a call borrows the
handle, so the same value stays callable. Since 1.5.0 they can be stored in
struct fields, enums, tuples, `Vec`, `Box` and as HashMap values; `==` is
`E0369`, and `{:?}`/`{}` print a symbol such as `fn(...)`. Function types may
mention references, for example `fn(&i64) -> i64` or `fn(&i64) -> &i64`; see
[Functions](09-functions.md) and [Closures](14-closures.md).

```ls
fn inc(x: i64) -> i64 {
    x + 1
}

fn twice(f: fn(i64) -> i64, x: i64) -> i64 {
    f(f(x))
}

fn main() {
    let f: fn(i64) -> i64 = inc;
    println!("{}", twice(f, 40));
    println!("{}", twice(inc, 1));
}
```

Output:
```text
42
3
```

## Structs, enums and type aliases

A `struct` introduces a record type with named fields, a tuple struct
(`struct Pair(i32, String);`), or a unit struct (`struct Marker;`). Tuple structs
construct like functions — `Pair(1, String::from("one"))` — expose their fields
as `.0`, `.1`, ... and destructure like tuples; a one-field tuple struct is the
idiomatic *newtype* (`struct Meters(i64);`) and is a distinct type, unlike an
alias. An `enum` introduces a tagged union whose variants carry no payload or a
tuple of payloads. Item order does not matter, as described in
[Program structure](02-program-structure.md). A `type Alias = T;` gives an
alternative name to an existing type; aliases are transparent (the alias and the
underlying type are the same type, not distinct newtypes).

```ls
struct Point {
    x: i64,
    y: i64,
}

enum Direction {
    North,
    South,
    Degrees(i64),
}

type Grid = Vec<Vec<i64>>;

fn main() {
    let p = Point { x: 1, y: 2 };
    let d = Direction::Degrees(90);
    let grid: Grid = vec![vec![1, 2], vec![3]];
    println!("{} {} {}", p.x, p.y, grid.len());
    match d {
        Direction::North => println!("north"),
        Direction::South => println!("south"),
        Direction::Degrees(deg) => println!("{} degrees", deg),
    }
}
```

Output:
```text
1 2 2
90 degrees
```

Tuple and unit structs are declared and used like this:

```ls
struct Meters(i64);
struct Pair(i32, String);
struct Marker;

fn main() {
    let m = Meters(42);
    println!("{}", m.0);
    let copy = m;
    let Meters(value) = copy;
    println!("{}", value);

    let p = Pair(1, String::from("one"));
    println!("{} {}", p.0, p.1);

    let _unit = Marker;
}
```

Output:
```text
42
42
1 one
```

## Type inference

Types are inferred locally by unification: the compiler propagates the expected
type into expressions and unifies the types of operands. There is no whole-body
Hindley-Milner inference and no inference across function boundaries — a
function's parameter and return types are always written explicitly, and a
variable's type is fixed by its initializer and annotation, not by later uses.

Rules that matter in practice:

* An unannotated integer literal defaults to `i32`; an unannotated float
  literal defaults to `f64`.
* A literal suffix fixes the type: `1u8`, `1i64`, `1usize`. An unsuffixed
  literal takes the type its context requires, so `0` is accepted as a `usize`
  index or in a comparison with a length.
* A string literal is a `String` unless a `&str` is expected, in which case it
  is a static slice.
* An array literal unifies its elements into one type, and a repeat literal
  `[value; N]` takes the type of `value`. An empty `[]` has no element type of
  its own and needs context, otherwise `E0282`; array lengths are checked
  against the literal (`E0308`).
* Context can force a narrower type: `let x: u8 = 1;` makes the literal `u8`.
* A generic constructor without arguments has no payload type to infer.
  `Vec::new()`, `None`, `Rc::new(None)` and `String::from` need context:
  annotate the binding or use turbofish. An unannotated `let v = Vec::new();`
  is rejected because the element type cannot be inferred from the initializer.
* Turbofish writes the type arguments after the path: `Vec::<i64>::new()`,
  `Rc::<Option<String>>::new(None)`, `Pair::<i64>::new(3, 4)`,
  `pair::<i64>(1, 2)`. It is also accepted after a method name syntactically,
  but generic methods do not exist, so it is only useful on paths.
* Generic functions and generic methods resolve their type parameters from the
  arguments and the expected type. When neither fixes them the compiler
  reports `E0282` or `E0283` and the turbofish is the way out; see
  [Generics](17-generics.md).

```ls
fn main() {
    let inferred: Vec<i64> = Vec::new();
    let none: Option<String> = None;
    let explicit = Vec::<i64>::new();
    let cell = Rc::<Option<String>>::new(None);
    println!("{} {} {} {}", inferred.len(), none.is_none(), explicit.len(), cell.get().is_none());
}
```

Output:
```text
0 true 0 true
```

Without context a generic constructor is ambiguous, and the compiler asks for an
annotation:

```ls,compile-fail
fn main() {
    let v = Vec::new();
    println!("{}", v.len());
}
```

Error:
```text
error[E0282]: type annotations needed
  --> example-14.ls:2:13
   |
 2 |     let v = Vec::new();
   |             ^^^^^^^^^^
   = note: cannot infer the type arguments for this call; annotate the binding or the expected type

lsaca: example-14.ls: could not compile example-14.ls due to previous error(s)
```

Inference works from context too, including in arguments and returns:

```ls
fn sum(values: Vec<i64>) -> i64 {
    let mut total: i64 = 0;
    for value in values {
        total += value;
    }
    total
}

fn main() {
    let v = vec![1, 2, 3];
    let total: i64 = sum(v);
    println!("{}", total);
    let maybe = if total > 5 { Some(total) } else { None };
    println!("{}", maybe.unwrap_or(0));
}
```

Output:
```text
6
6
```

## Casts

The `as` operator converts between scalar types only. It is the *only*
conversion: assigning an `i32` to an `i64` or passing it where a `f64` is
expected is a type error. References (`&T` / `&mut T`), arrays, `String`, `Vec<T>` and
the other aggregates cannot be converted with `as`. `as` binds tighter than arithmetic but looser than
unary operators; see the precedence table in [Operators](07-operators.md).

The conversion rules follow Rust:

* Integer to integer: truncate to the target width, sign-extend or zero-extend
  according to the target type's signedness. `300 as u8` is `44`; `-1 as u8` is
  `255`. `isize` and `usize` cast exactly like the other integer types.
* Integer to float: exact value if it fits, otherwise rounded.
* Float to integer: truncate toward zero, saturating at the target type's
  bounds; NaN becomes `0`.
* `char` to an integer or float: the scalar value, truncated by the target
  integer width (`'A' as i64` is `65`, `'é' as u8` is `233`).
* `u8` to `char`: the byte value as a scalar; other integer types are rejected
  with `E0308` (only `u8` can be cast as `char`), and `char` can never be cast
  to or from `bool`.
* Boolean to integer: `true` becomes `1`, `false` becomes `0`.
* Casting to `bool`: an integer or a float cannot be cast to `bool` (`E0605`);
  only `bool` values are booleans. Conditions never convert numbers, so `if 1`
  is `E0308` — see [Statements and control
  flow](08-statements-and-control-flow.md#conditions).

Characters convert explicitly too:

```ls
fn main() {
    println!("{} {}", 'A' as i64, 'é' as u8);
    println!("{:?} {}", 65 as u8 as char, '😀' as u32);
}
```

Output:

```text
65 233
'A' 128512
```

```ls
fn main() {
    println!("{} {}", 300 as u8, -1 as u8);
    println!("{} {}", 3.99 as i64, -3.99 as i64);
    println!("{} {}", 1e20 as i64, (0.0 / 0.0) as i64);
    println!("{} {}", 65 as f64, true as i64);
    println!("{} {}", 16777216 as f32, 16777217 as f32);
    println!("{}", 123456789 as f32);
}
```

Output:

```text
44 255
3 -3
9223372036854775807 0
65 1
16777216 16777216
123456790
```

A cast to `bool` is rejected:

```ls,compile-fail
fn main() {
    let flag = 1 as bool;
    println!("{}", flag);
}
```

Error:

```text
error[E0605]: non-primitive cast: {integer} as bool
  --> example-18.ls:2:16
   |
 2 |     let flag = 1 as bool;
   |                ^^^^^^^^^

lsaca: example-18.ls: could not compile example-18.ls due to previous error(s)
```

There are no implicit conversions, so the following is rejected:

```ls,compile-fail
fn main() {
    let x: i64 = 3;
    let y: f64 = x;
    println!("{}", y);
}
```

Error:

```text
error[E0308]: mismatched types
  --> example-19.ls:3:18
   |
 3 |     let y: f64 = x;
   |                  ^
   = note: expected f64, found i64

lsaca: example-19.ls: could not compile example-19.ls due to previous error(s)
```

Since 2.0.0 `isize` and `usize` are ordinary integer types, and since 2.1.0
`usize` is the type of lengths and indices. They print and cast like the
others:

```ls
fn main() {
    let x: usize = 0;
    let y: isize = -1;
    println!("{} {}", x, y);
    println!("{} {}", x as u64, y as i64);
}
```

Output:

```text
0 -1
0 -1
```

Finally, an unknown type name in any position is reported as `E0412`:

```ls,compile-fail
fn main() {
    let widget: Widget = Widget { value: 1 };
    println!("{}", widget.value);
}
```

Error:

```text
error[E0412]: cannot find type 'Widget' in this scope
  --> example-21.ls:2:17
   |
 2 |     let widget: Widget = Widget { value: 1 };
   |                 ^^^^^^

error[E0412]: cannot find struct Widget in this scope
  --> example-21.ls:2:26
   |
 2 |     let widget: Widget = Widget { value: 1 };
   |                          ^^^^^^

lsaca: example-21.ls: could not compile example-21.ls due to previous error(s)
```

## What next

* How values are represented and shared: [Memory model](05-memory-model.md).
* Operators on each type: [Operators](07-operators.md).
* Borrow values without giving up ownership: [References and
  borrowing](12-references-and-borrowing.md).
* Fixed-length arrays: [Arrays](27-arrays.md).
* Reference fields and lifetimes: [Lifetimes](29-lifetimes.md).
* Characters and Unicode text: [Characters and
  Unicode](22-characters-and-unicode.md).
* The full built-in API: [Standard library](15-standard-library.md).
