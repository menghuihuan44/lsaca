# Structs, enums and impl

lsaca has C-like product types (`struct`) and tagged unions (`enum`), with
methods and associated functions provided by `impl` blocks. Structs and enums
are **value types**: a `Copy` one is copied when it is assigned or passed, while
a move-only one (one containing a `String`, `Vec`, `HashMap`, `Box`, `Rc` or
another move-only field) is moved. Heap fields follow the usual ownership rules:
nothing is shared behind your back except that `String` and `Vec` may share a
buffer until one side is mutated. There are no classes and no inheritance;
since 1.5.0 user-defined traits and generics are supported — see
[Traits](18-traits.md) and [Generics](17-generics.md).

## Structs

A struct declares a named set of fields. Each field has a name and a type; every
field must be given a type:

```ls
struct Point {
    x: i64,
    y: i64,
}

fn main() {
    let origin = Point { x: 0, y: 0 };
    let corner = Point { x: 4, y: 3 };
    println!("{} {} {}", origin.x, origin.y, corner.x);
}
```

Output:
```text
0 0 4
```

Field initialization can use the shorthand `field` for `field: field` when a
variable with the same name is in scope. A trailing comma after the last field is
allowed in both the declaration and the literal.

```ls
struct Point {
    x: i64,
    y: i64,
}

fn main() {
    let x = 7;
    let y = 9;
    let p = Point { x, y };
    println!("{} {} {}", p.x, p.y, p.y + p.x);
}
```

Output:
```text
7 9 16
```

A struct with no fields — a marker type — is declared with empty braces and
constructed with the same empty braces:

```ls
struct Ready {}

fn describe(_flag: bool) -> String {
    let _marker = Ready {};
    String::from("done")
}

fn main() {
    println!("{}", describe(true));
}
```

Output:
```text
done
```

`_flag` and `_marker` are ordinary identifiers starting with an underscore;
lsaca has no unused-variable warnings, so this is only a convention.

There are no tuple structs and no struct-update syntax (`..base`); both are parse
errors:

```ls,compile-fail
struct Meters(i64);

fn main() {
    let m = Meters(5);
}
```

Error:
```text
error: expected '{', found (
  --> example-04.ls:1:14
   |
 1 | struct Meters(i64);
   |              ^

error: expected field name, found (
  --> example-04.ls:1:14
   |
 1 | struct Meters(i64);
   |              ^

error: expected ':' after field name, found identifier i64
  --> example-04.ls:1:15
   |
 1 | struct Meters(i64);
   |               ^^^

error: expected '}', found )
  --> example-04.ls:1:18
   |
 1 | struct Meters(i64);
   |                  ^

error: expected an item (fn, struct, enum, impl, const, type or trait), found )
  --> example-04.ls:1:18
   |
 1 | struct Meters(i64);
   |                  ^

lsaca: example-04.ls: could not compile example-04.ls due to previous error(s)
```

```ls,compile-fail
struct Point {
    x: i64,
    y: i64,
}

fn main() {
    let base = Point { x: 1, y: 2 };
    let shifted = Point { x: 3, ..base };
    println!("{}", shifted.x);
}
```

Error:
```text
error: expected field name, found ..
  --> example-05.ls:8:33
   |
 8 |     let shifted = Point { x: 3, ..base };
   |                                 ^^

error: expected '}' after struct literal, found identifier base
  --> example-05.ls:8:35
   |
 8 |     let shifted = Point { x: 3, ..base };
   |                                   ^^^^

error: expected ';' after let statement, found identifier base
  --> example-05.ls:8:35
   |
 8 |     let shifted = Point { x: 3, ..base };
   |                                   ^^^^

error: expected an item (fn, struct, enum, impl, const, type or trait), found ;
  --> example-05.ls:8:41
   |
 8 |     let shifted = Point { x: 3, ..base };
   |                                         ^

lsaca: example-05.ls: could not compile example-05.ls due to previous error(s)
```

### Value semantics

Assigning a struct or passing it to a function **moves** the struct value by
default. Calling `.clone()` keeps the original and produces an independent copy;
`String` and `Vec` fields share their buffers until one side writes
(copy-on-write):

```ls
struct Profile {
    name: String,
    visits: i64,
}

fn main() {
    let original = Profile { name: String::from("ana"), visits: 1 };
    let mut copy = original.clone();
    copy.name.push_str("!");
    copy.visits += 1;
    println!("{} {}", original.name, original.visits);
    println!("{} {}", copy.name, copy.visits);
}
```

Output:
```text
ana 1
ana! 2
```

Fields are mutated through a place expression, so the binding that holds the
struct must be `let mut`:

```ls,compile-fail
struct Counter {
    value: i64,
}

fn main() {
    let counter = Counter { value: 0 };
    counter.value = counter.value + 1;
}
```

Error:
```text
error[E0596]: cannot assign to immutable value: the binding has not been declared as mutable
  --> example-07.ls:7:5
   |
 7 |     counter.value = counter.value + 1;
   |     ^^^^^^^^^^^^^

lsaca: example-07.ls: could not compile example-07.ls due to previous error(s)
```

## Enums

An enum declares alternatives (variants). A variant is either a unit name or a
tuple variant with a parenthesized payload. Every variant must be usable as a
value of the enum type; pattern matching on enums is the main way to take them
apart — see [Pattern matching](13-pattern-matching.md).

```ls
enum Shape {
    Circle(f64),
    Square(f64),
    Empty,
}

fn main() {
    let shapes = vec![Shape::Circle(1.0), Shape::Square(3.0), Shape::Empty];
    for shape in shapes {
        println!("{}", shape);
    }
}
```

Output:
```text
Circle(1.0)
Square(3.0)
Empty
```

An enum with no variants is allowed and useful as an uninhabited type (a value of
that type cannot be constructed):

```ls
enum Void {}

fn main() {
    println!("{}", 1);
}
```

Output:
```text
1
```

Variants are constructed with a path: `Shape::Circle(1.0)`. A payload may be any
type, including `String`, `Vec`, `Rc`, `Box` or another struct/enum. Struct-like
variants (`Variant { field: … }`) are a Rust feature that lsaca does not have;
variants carry either no payload or a parenthesized list.

## Methods and associated functions with impl

An `impl` block adds functions to a named type. Functions whose first parameter
is a form of `self` are **methods**, called with `receiver.name(args)`; functions
without `self` are **associated functions**, called as `Type::name(args)`.
Methods work for both structs and enums.

There are three receiver forms:

| Receiver | Meaning | Callable on |
| --- | --- | --- |
| `self` | takes the value by copy | any binding |
| `&self` | borrowed receiver, read-only | any binding |
| `&mut self` | mutable receiver, may assign fields | a `let mut` binding only |

```ls
struct Point {
    x: i64,
    y: i64,
}

impl Point {
    fn new(x: i64, y: i64) -> Self {
        Point { x, y }
    }

    fn length_squared(&self) -> i64 {
        self.x * self.x + self.y * self.y
    }

    fn translate(&mut self, dx: i64, dy: i64) {
        self.x += dx;
        self.y += dy;
    }

    fn into_sum(self) -> i64 {
        self.x + self.y
    }
}

fn main() {
    let mut p = Point::new(3, 4);
    println!("{}", p.length_squared());
    p.translate(1, 1);
    println!("{} {}", p.x, p.y);
    println!("{}", Point::new(2, 5).into_sum());
}
```

Output:
```text
25
4 5
7
```

### Reference receivers and returns

The `&self` receiver is itself a reference to the value, so a method can return a
reference to one of its fields. The reference is borrowed from the receiver:

```ls
struct Counter {
    count: i64,
}

impl Counter {
    fn new() -> Counter {
        Counter { count: 0 }
    }

    fn increment(&mut self) {
        self.count += 1;
    }

    fn count(&self) -> &i64 {
        &self.count
    }
}

fn main() {
    let mut counter = Counter::new();
    counter.increment();
    let seen = counter.count();
    println!("{}", *seen);
}
```

Output:
```text
1
```

A method that returns `&String` or a `&Vec` element works the same way:

```ls
struct Named {
    name: String,
}

impl Named {
    fn name(&self) -> &String {
        &self.name
    }
}

fn main() {
    let named = Named { name: String::from("lsaca") };
    let name = named.name();
    println!("{}", *name);
}
```

Output:
```text
lsaca
```

Returning a reference from a method is allowed when the receiver decides the lifetime:
a `&self` method may return a shared reference to its fields, and a `&mut self` method
may return a shared or mutable reference to its fields. Returning `&mut` from a `&self`
method is rejected (`E0596`, "cannot return a mutable reference from a shared reference
source"), and a by-value `self` receiver cannot return a reference (`E0106`).

```ls
struct Counter {
    count: i64,
}

impl Counter {
    fn count_mut(&mut self) -> &mut i64 {
        &mut self.count
    }
}

fn main() {
    let mut counter = Counter { count: 0 };
    *counter.count_mut() += 1;
    println!("{}", counter.count);
}
```

Output:
```text
1
```

Struct and enum fields cannot themselves be references; a reference is a
non-owning view and may only live in a local, a parameter or a return value
(see [References and borrowing](12-references-and-borrowing.md)). Storing one
in a field is an `E0498` error.

`Self` is a type name usable inside `impl` blocks — in `-> Self` return types and
parameter types, and since 1.5.0 also in expression position: `Self { … }` builds
a value of the implementing type and `Self::new(…)` calls an associated function
on it. Both forms work in generic impls as well:

```ls
struct Point {
    x: i64,
}

impl Point {
    fn new(x: i64) -> Self {
        Self { x }
    }

    fn twice() -> Self {
        Self::new(2)
    }

    fn shifted(&self) -> Self {
        Self { x: self.x + 1 }
    }
}

fn main() {
    println!("{}", Point::twice().x);
    println!("{}", Point::new(7).shifted().x);
}
```

Output:
```text
2
8
```

An `impl` block may be split into several blocks for the same type; all of them
contribute to the same method set:

```ls
struct Counter {
    value: i64,
}

impl Counter {
    fn new() -> Counter {
        Counter { value: 0 }
    }
}

impl Counter {
    fn bump(&mut self) {
        self.value += 1;
    }
}

fn main() {
    let mut c = Counter::new();
    c.bump();
    c.bump();
    println!("{}", c.value);
}
```

Output:
```text
2
```

Calling a `&mut self` method on a non-`mut` binding is an error:

```ls,compile-fail
struct Counter {
    value: i64,
}

impl Counter {
    fn bump(&mut self) {
        self.value += 1;
    }
}

fn main() {
    let c = Counter { value: 0 };
    c.bump();
}
```

Error:
```text
error[E0596]: cannot mutate through this receiver: the binding is not declared as mutable
   --> example-16.ls:13:5
    |
 13 |     c.bump();
    |     ^

lsaca: example-16.ls: could not compile example-16.ls due to previous error(s)
```

### Methods on enums

`impl` blocks also attach methods to enums; the classic use is a method that
matches on `self` to compute something:

```ls
enum Shape {
    Circle(f64),
    Square(f64),
}

impl Shape {
    fn area(&self) -> f64 {
        match self {
            Shape::Circle(r) => 3.14159 * (*r) * (*r),
            Shape::Square(s) => (*s) * (*s),
        }
    }
}

fn main() {
    let c = Shape::Circle(2.0);
    let s = Shape::Square(3.0);
    println!("{}", c.area());
    println!("{}", s.area());
}
```

Output:
```text
12.56636
9
```

### Drop hooks

`impl Drop for T` declares the destructor hook for a struct or enum. `Drop`
is a special trait: it is the only built-in trait that changes code generation,
and its impl must have exactly one method named `drop`. (Since 1.5.0 a program
can also implement its own traits — see [Traits](18-traits.md) — but none of
them get this special treatment.) The hook runs when a value is released:

```ls
struct Resource {
    name: String,
}

impl Drop for Resource {
    fn drop(&mut self) {
        println!("releasing {}", self.name);
    }
}

fn main() {
    let first = Resource { name: String::from("first") };
    println!("before end");
}
```

Output:

```text
before end
releasing first
```

The compiler validates the impl: exactly one `fn drop(&mut self)` with no other
parameters and a unit return type, at most one `Drop` impl per type (`E0119`),
and no inherent method named `drop`. A type that implements `Drop` is never
`Copy` and cannot be cloned (`E0277`); passing it by value moves it and the new
owner releases it. The hook runs before the fields are released, fields are
released in declaration order, locals in reverse declaration order and
parameters in reverse order, and an overwritten binding is released immediately.
No hooks run after a panic or `exit()`; values still alive when `main` returns
are released normally. See [Ownership and
moves](11-ownership-and-moves.md#the-drop-trait) and [Memory
model](05-memory-model.md#drop).

## Errors when building values

The compiler checks field names and arity at construction time:

```ls,compile-fail
struct Point {
    x: i64,
    y: i64,
}

fn main() {
    let p = Point { x: 1 };
    println!("{}", p.x);
}
```

Error:
```text
error[E0063]: missing field y in initializer of Point
  --> example-19.ls:7:13
   |
 7 |     let p = Point { x: 1 };
   |             ^^^^^^^^^^^^^^

lsaca: example-19.ls: could not compile example-19.ls due to previous error(s)
```

```ls,compile-fail
struct Point {
    x: i64,
    y: i64,
}

fn main() {
    let p = Point { x: 1, y: 2, z: 3 };
    println!("{}", p.x);
}
```

Error:
```text
error[E0560]: struct Point has no field named z; did you mean x?
  --> example-20.ls:7:33
   |
 7 |     let p = Point { x: 1, y: 2, z: 3 };
   |                                 ^

lsaca: example-20.ls: could not compile example-20.ls due to previous error(s)
```

Accessing a field that does not exist and calling a method that does not exist
are checked too:

```ls,compile-fail
struct Point {
    x: i64,
    y: i64,
}

fn main() {
    let p = Point { x: 1, y: 2 };
    println!("{}", p.z);
}
```

Error:
```text
error[E0609]: no field z on type Point; did you mean x?
  --> example-21.ls:8:20
   |
 8 |     println!("{}", p.z);
   |                    ^^^

lsaca: example-21.ls: could not compile example-21.ls due to previous error(s)
```

```ls,compile-fail
struct Point {
    x: i64,
    y: i64,
}

fn main() {
    let p = Point { x: 1, y: 2 };
    println!("{}", p.frobnicate());
}
```

Error:
```text
error[E0599]: no method named frobnicate found for type Point in the current scope
  --> example-22.ls:8:22
   |
 8 |     println!("{}", p.frobnicate());
   |                      ^^^^^^^^^^

lsaca: example-22.ls: could not compile example-22.ls due to previous error(s)
```

Declaring the same field twice is rejected as well:

```ls,compile-fail
struct Broken {
    x: i64,
    x: String,
}

fn main() {
    println!("{}", 1);
}
```

Error:
```text
error[E0124]: field 'x' is already declared
  --> example-23.ls:3:5
   |
 3 |     x: String,
   |     ^
   = note: example-23.ls:2:5: previous declaration of 'x' here

lsaca: example-23.ls: could not compile example-23.ls due to previous error(s)
```

## Printing and equality

Structs and enums have no user-defined formatting. `Display` and `Debug` both
print a structural form: `Point { x: 1, y: 2 }`, `Shape::Circle(2)`,
`Shape::Empty`. Strings nested inside a composite are printed quoted, exactly as
in Debug; a top-level `String` prints unquoted. Width, fill and alignment apply
to the whole structural form (so `{:30}` pads a struct or enum as a unit), while
the `+`/\`0\` sign flags and precision are only accepted for numbers and strings
and are rejected for structs and enums with `E0001` — see
[Standard library](15-standard-library.md#formatting-and-printing).

Equality (`==`) is implemented for user structs and enums since 1.5.0: the
compiler compares them field by field, and enum payloads of the same variant:

```ls
struct Point {
    x: i64,
    y: i64,
}

enum Shape {
    Empty,
    Circle(f64),
}

fn main() {
    let a = Point { x: 1, y: 2 };
    let b = Point { x: 1, y: 2 };
    let c = Point { x: 1, y: 3 };
    println!("{} {}", a == b, a == c);

    println!("{} {}", Shape::Empty == Shape::Empty, Shape::Circle(2.0) == Shape::Circle(2.5));
}
```

Output:
```text
true false
true false
```

To compare points, compare the fields explicitly:

```ls
struct Point {
    x: i64,
    y: i64,
}

fn same(a: Point, b: Point) -> bool {
    a.x == b.x && a.y == b.y
}

fn main() {
    let a = Point { x: 1, y: 2 };
    let b = Point { x: 1, y: 2 };
    let c = Point { x: 1, y: 3 };
    println!("{} {}", same(a, b), same(a, c));
}
```

Output:
```text
true false
```

## What next

* The pattern syntax for destructuring structs and enums:
  [Pattern matching](13-pattern-matching.md).
* Move semantics and receiver effects:
  [Memory model](05-memory-model.md#value-semantics-for-aggregates).
* Reference receivers, match ergonomics and borrowing rules:
  [References and borrowing](12-references-and-borrowing.md).
* Structs and enums as type expressions: [Types](04-types.md).
