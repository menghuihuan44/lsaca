# Types

Every expression in lsaca has a statically known type, checked before any C is
generated. This page lists the complete type language, explains each builtin
type, and describes inference and conversions.

## The type table

| Type | Description | Notes |
| --- | --- | --- |
| `()` | Unit. | The result of expressions that produce nothing; can be stored and printed. |
| `bool` | Boolean. | `true` / `false`. |
| `i8` `i16` `i32` `i64` | Signed integers. | Two's complement; `i64` is the default integer type and the type of all lengths and indices. |
| `u8` `u16` `u32` `u64` | Unsigned integers. | Values are kept in the corresponding C unsigned type. |
| `f32` `f64` | IEEE-754 binary floating point. | `f64` is the default float type. |
| `String` | Owned immutable UTF-8 string. | Reference-counted, copy-on-write; `len()` is a byte count. |
| `Vec<T>` | Growable array. | Builtin generic; copy-on-write. |
| `Option<T>` | `Some(T)` or `None`. | Builtin generic. |
| `Result<T, E>` | `Ok(T)` or `Err(E)`. | Builtin generic. |
| `Rc<T>` | Shared, mutable heap cell. | Copies share one payload; `get()`/`set()`. |
| `Box<T>` | Owned heap indirection. | Added in 1.1.0; copies deep-copy the payload. |
| `(T1, T2, ...)` | Tuple. | Held by value; fields `.0`, `.1`, ...; only flat tuples are supported by the current code generator. |
| `fn(A, B) -> R` | Function type. | Values of this type are first-class; see [Functions](09-functions.md). |
| `Name` | User struct or enum. | Declared with `struct`/`enum`. |
| `Alias` | Type alias. | `type Alias = T;` introduces no new type. |

There is no `usize`, `isize`, `char`, reference type, pointer type, array type or
slice type. `Self` is not a type available to user code in general: it names
"the type of this `impl` block" inside an `impl`, and is accepted in parameter
and return positions there (see [Structs, enums and
impl](10-structs-enums-and-impl.md)).

## Integers

The integer types differ only in width and signedness, and they are **distinct
types**: there are no implicit conversions between them. Arithmetic requires
both operands to have the same type; to change the type, use `as`.

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

The type of an unannotated integer literal is `i64` unless context determines a
different integer type. Debug builds check overflow and division by zero at run
time; release builds wrap. This is described in
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

## Floats

There are two floating-point types, `f32` and `f64`, both IEEE-754. An
unannotated float literal is `f64` unless context requires `f32`. Floats follow
C semantics for special values: division by zero produces an infinity, `0.0 /
0.0` produces NaN, and comparisons involving NaN are false except `!=`. Display
uses the shortest round-tripping decimal form; `{:?}` and format specifiers work
as described in [Standard library](13-standard-library.md).

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

`String` and `Vec<T>` are the two owned container types. Both are
reference-counted with copy-on-write: assigning or passing one copies a handle,
and mutating through one handle clones the buffer first if it is shared, so two
variables never observe each other's mutations. Both are indexed by `i64`, and
`String` methods use **byte** offsets. They are described in
[Memory model](05-memory-model.md) and their full API is in
[Standard library](13-standard-library.md).

```ls
fn main() {
    let mut text = String::from("abc");
    let mut numbers = vec![1, 2, 3];
    let copy = text;
    text.push_str("d");
    numbers.push(4);
    println!("{} {} {} {}", text.len(), copy.len(), numbers.len(), copy);
}
```

Output:
```text
4 3 4 abc
```

## Option and Result

`Option<T>` and `Result<T, E>` are builtin enums. `Option<T>` is either
`Some(value)` or `None`; `Result<T, E>` is either `Ok(value)` or `Err(error)`.
They are the idiomatic way to express a missing value or an operation that can
fail, and they support pattern matching and the methods listed in
[Standard library](13-standard-library.md). Neither type supports `==` in
version 1.1.0: compare them by pattern matching.

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

`Rc<T>` is a shared **cell**: all handles refer to one payload, and mutation
through any handle is visible through all of them. `Box<T>` is an owned heap
allocation: a `Box` has value semantics, and copying a `Box` copies the payload
(deep copy), so boxes never alias. Both exist to break the "everything is a
value" rule in controlled ways: `Rc` when you need shared mutable state or a
cycle, `Box` when you need indirection (a recursive data structure, or a large
value moved without a deep copy on the surface). 1.1.0's `Box` is described in
[Memory model](05-memory-model.md).

```ls
fn main() {
    let cell = Rc::new(1);
    let shared = cell;
    shared.set(shared.get() + 41);
    println!("{} {}", cell.get(), shared.get());

    let boxed = Box::new(String::from("owner"));
    let moved = boxed;
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

## Tuples

A tuple groups a fixed number of values of possibly different types:
`(i64, String, bool)`. A one-element tuple needs a trailing comma — `(x,)` — and
`(x)` is just a parenthesized expression. The unit `()` is the zero-element
tuple. Elements are accessed by index (`t.0`, `t.1`, ...) or destructured in a
`let`. Tuples are held by value and copied element-wise.

**Current limitation:** the C code generator only handles *flat* tuples. A tuple
nested inside another tuple, whether as a value or as a type of a struct field,
generates invalid C and fails at the C compiler, so avoid nested tuples in
1.1.0. Tuples with other element types (including `String`, `Rc<T>`,
`Option<String>`) work.

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
functions and returned from functions; they cannot be compared with `==` and
are not supported as fields of structs or elements of `Vec` by the current code
generator (see [Functions](09-functions.md)).

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

A `struct` introduces a record type with named fields; an `enum` introduces a
tagged union whose variants carry no payload or a tuple of payloads. Both must be
declared before use in a *type position* only in the sense of item order: item
order does not matter, as described in [Program structure](02-program-structure.md).
A `type Alias = T;` gives an alternative name to an existing type; aliases are
transparent (the alias and the underlying type are the same type, not distinct
newtypes).

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

## Type inference

Types are inferred locally by unification: the compiler propagates the expected
type into expressions and unifies the types of operands. There is no whole-body
Hindley-Milner inference and no inference across function boundaries — a
function's parameter and return types are always written explicitly, and a
variable's type is fixed by its initializer and annotation, not by later uses.

Rules that matter in practice:

* An unannotated integer literal defaults to `i64`; an unannotated float
  literal defaults to `f64`.
* Context can force a narrower type: `let x: u8 = 1;` makes the literal `u8`.
* A generic constructor without arguments has no payload type to infer.
  `Vec::new()`, `None`, `Rc::new(None)` and `String::from` need context:
  annotate the binding or use turbofish. An unannotated `let v = Vec::new();`
  is rejected because the element type cannot be inferred from the initializer.
* Turbofish writes the type arguments after the path: `Vec::<i64>::new()`,
  `Rc::<Option<String>>::new(None)`. It is also accepted after a method name
  syntactically, but generic methods do not exist, so it is only useful on paths.

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
error[E0282]: type annotations needed: the type of this expression is not known
  --> example-10.ls:2:9
   |
 2 |     let v = Vec::new();
   |         ^

error[E0282]: type annotations needed: the type of this expression is not known
  --> example-10.ls:2:13
   |
 2 |     let v = Vec::new();
   |             ^^^^^^^^^^

error[E0282]: type annotations needed: the type of this expression is not known
  --> example-10.ls:3:20
   |
 3 |     println!("{}", v.len());
   |                    ^

lsaca: example-10.ls: could not compile example-10.ls due to previous error(s)
```

Inference works from context too, including in arguments and returns:

```ls
fn sum(values: Vec<i64>) -> i64 {
    let mut total = 0;
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
expected is a type error. `as` binds tighter than arithmetic but looser than
unary operators; see the precedence table in [Operators](07-operators.md).

The conversion rules follow Rust:

* Integer to integer: truncate to the target width, sign-extend or zero-extend
  according to the target type's signedness. `300 as u8` is `44`; `-1 as u8` is
  `255`.
* Integer to float: exact value if it fits, otherwise rounded.
* Float to integer: truncate toward zero, saturating at the target type's
  bounds; NaN becomes `0`.
* Boolean to integer: `true` becomes `1`, `false` becomes `0`.
* Integer to boolean is accepted by the `as` operator (`0 as bool` is `false` and
  any non-zero value is `true`) even though it is not part of the intended
  language; do not rely on it. This is a property of the cast only: conditions
  never convert numbers, so `if 1` is `E0308` — see [Statements and control
  flow](08-statements-and-control-flow.md#conditions).

```ls
fn main() {
    println!("{} {}", 300 as u8, -1 as u8);
    println!("{} {}", 3.99 as i64, -3.99 as i64);
    println!("{} {}", 1e20 as i64, (0.0 / 0.0) as i64);
    println!("{} {} {}", 65 as f64, true as i64, (2 as bool));
    println!("{}", 16777217 as f32);
}
```

Output:
```text
44 255
3 -3
9223372036854775807 0
65 1 true
16777216
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
  --> example-13.ls:3:18
   |
 3 |     let y: f64 = x;
   |                  ^
   = note: expected f64, found i64

lsaca: example-13.ls: could not compile example-13.ls due to previous error(s)
```

A type that does not exist gives a name-resolution error rather than a cast
error. `usize` is a good example: it is Rust, not lsaca:

```ls,compile-fail
fn main() {
    let x: usize = 0;
    println!("{}", x);
}
```

Error:
```text
error[E0412]: cannot find type 'usize' in this scope
  --> example-14.ls:2:12
   |
 2 |     let x: usize = 0;
   |            ^^^^^

lsaca: example-14.ls: could not compile example-14.ls due to previous error(s)
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
  --> example-15.ls:2:17
   |
 2 |     let widget: Widget = Widget { value: 1 };
   |                 ^^^^^^

error[E0412]: cannot find struct Widget in this scope
  --> example-15.ls:2:26
   |
 2 |     let widget: Widget = Widget { value: 1 };
   |                          ^^^^^^

lsaca: example-15.ls: could not compile example-15.ls due to previous error(s)
```

## What next

* How values are represented and shared: [Memory model](05-memory-model.md).
* Operators on each type: [Operators](07-operators.md).
* The full built-in API: [Standard library](13-standard-library.md).
