# Structs, enums and impl

lsaca has C-like product types (`struct`) and tagged unions (`enum`), with
methods and associated functions provided by `impl` blocks. Structs and enums are
**value types**: assigning or passing one copies its fields, and the copy shares
heap allocations through copy-on-write handles. There are no classes, no
inheritance, no traits and no generics on user types.

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

error: expected an item (fn, struct, enum, impl, const or type), found )
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

error: expected an item (fn, struct, enum, impl, const or type), found ;
  --> example-05.ls:8:41
   |
 8 |     let shifted = Point { x: 3, ..base };
   |                                         ^

lsaca: example-05.ls: could not compile example-05.ls due to previous error(s)
```

### Value semantics

Assigning a struct or passing it to a function copies the struct value. String
and `Vec` fields are copy-on-write, so a copied struct that is then modified does
not disturb the original:

```ls
struct Profile {
    name: String,
    visits: i64,
}

fn main() {
    let original = Profile { name: String::from("ana"), visits: 1 };
    let mut copy = original;
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
apart — see [Pattern matching](11-pattern-matching.md).

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

`Self` is a type name usable inside `impl` blocks — in `-> Self` return types and
parameter types. It **cannot** be used to construct a value: `Self { … }` and even
`Self::new(…)` in expression position are rejected:

```ls,compile-fail
struct Point {
    x: i64,
}

impl Point {
    fn new(x: i64) -> Self {
        Point { x }
    }

    fn twice() -> Self {
        Self::new(2)
    }
}

fn main() {
    println!("{}", Point::twice().x);
}
```

Error:
```text
error[E0532]: expected an enum, found a struct
   --> example-11.ls:11:9
    |
 11 |         Self::new(2)
    |         ^^^^

lsaca: example-11.ls: could not compile example-11.ls due to previous error(s)
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
   --> example-13.ls:13:5
    |
 13 |     c.bump();
    |     ^

lsaca: example-13.ls: could not compile example-13.ls due to previous error(s)
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
            Shape::Circle(r) => 3.14159 * r * r,
            Shape::Square(s) => s * s,
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
  --> example-15.ls:7:13
   |
 7 |     let p = Point { x: 1 };
   |             ^^^^^^^^^^^^^^

lsaca: example-15.ls: could not compile example-15.ls due to previous error(s)
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
  --> example-16.ls:7:33
   |
 7 |     let p = Point { x: 1, y: 2, z: 3 };
   |                                 ^

lsaca: example-16.ls: could not compile example-16.ls due to previous error(s)
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
  --> example-17.ls:8:20
   |
 8 |     println!("{}", p.z);
   |                    ^^^

lsaca: example-17.ls: could not compile example-17.ls due to previous error(s)
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
  --> example-18.ls:8:22
   |
 8 |     println!("{}", p.frobnicate());
   |                      ^^^^^^^^^^

lsaca: example-18.ls: could not compile example-18.ls due to previous error(s)
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
  --> example-19.ls:3:5
   |
 3 |     x: String,
   |     ^
   = note: example-19.ls:2:5: previous declaration of 'x' here

lsaca: example-19.ls: could not compile example-19.ls due to previous error(s)
```

## Printing and equality

Structs and enums have no user-defined formatting. `Display` and `Debug` both
print a structural form: `Point { x: 1, y: 2 }`, `Shape::Circle(2)`,
`Shape::Empty`. Strings nested inside a composite are printed quoted, exactly as
in Debug; a top-level `String` prints unquoted. Format widths apply to scalars and
built-in containers but are ignored for user structs — see
[Standard library](13-standard-library.md#formatting-and-printing).

Equality (`==`) is not implemented for user structs or enums in 1.2.0: the
compiler reports `E0369`. Compare the fields you care about, or want to compare,
with a `match` expression:

```ls,compile-fail
struct Point {
    x: i64,
    y: i64,
}

fn main() {
    let a = Point { x: 1, y: 2 };
    let b = Point { x: 1, y: 2 };
    println!("{}", a == b);
}
```

Error:
```text
error[E0369]: binary operation == cannot be applied to type Point
  --> example-20.ls:9:20
   |
 9 |     println!("{}", a == b);
   |                    ^^^^^^

lsaca: example-20.ls: could not compile example-20.ls due to previous error(s)
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
  [Pattern matching](11-pattern-matching.md).
* Receiver semantics and copy-on-write effects:
  [Memory model](05-memory-model.md#value-semantics-for-aggregates).
* Structs and enums as type expressions: [Types](04-types.md).
