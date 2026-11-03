# Traits

Version 1.5.0 adds user-defined traits. A trait declares a set of methods that a
type can implement; traits are used as bounds on generic parameters (see
[Generics](17-generics.md)) and are resolved at compile time. Version 2.0 adds
*trait objects*: since 2.0 a trait can also be used as a type behind `dyn`, and
method calls through such a value dispatch at runtime (see [Trait
objects](25-trait-objects.md) and [Default method bodies](#default-method-bodies)).

## Declaring a trait

A trait declaration contains method signatures, each ending with a semicolon,
plus optionally default method bodies (see [Default method
bodies](#default-method-bodies)). Every method takes a receiver: `self`, `&self`
or `&mut self`. For example, a trait that describes drawable values would be
declared as `trait Describe { fn describe(&self) -> String; }` — the declaration
names the method without a body; an impl block supplies it.

A trait may have type parameters; the standard library uses this for
`Iterator<T>`.

## Implementing a trait

Use `impl Trait for Type`. The type must be a user-defined struct or enum, and
the impl block must provide every method of the trait with a matching
signature:

```ls
trait Describe {
    fn describe(&self) -> String;
}

struct Point {
    x: i64,
    y: i64,
}

impl Describe for Point {
    fn describe(&self) -> String {
        format!("point at {} {}", self.x, self.y)
    }
}

fn main() {
    let point = Point { x: 2, y: 3 };
    println!("{}", point.describe());
}
```

Output:
```text
point at 2 3
```

After the impl, the method is called like any other method on the type. The
impl block may be generic (see [Generic impls](#generic-impls)) and may
implement the trait on a generic type.

The impl target must be a **user-defined struct or enum**. Builtin types cannot
be extended with trait implementations; an impl for a builtin target such as
`i64`, `Vec<T>` or `String` is rejected with `E0118`, and the trait method
is therefore not available on that type:

```ls,compile-fail
trait Tag {
    fn tag(&self) -> i64;
}

impl Tag for Vec<i64> {
    fn tag(&self) -> i64 {
        self.len()
    }
}

fn main() {}
```

Error:
```text
error[E0118]: impl target must be a struct or enum type defined in this program, found Vec<i64>
  --> example-02.ls:5:14
   |
 5 | impl Tag for Vec<i64> {
   |              ^^^

lsaca: example-02.ls: could not compile example-02.ls due to previous error(s)
```

## Dispatch rules

A method call looks for an inherent method (`impl Type { ... }`) first. Only
if there is no inherent method does it look at the trait impls for the
receiver type:

```ls
trait Label {
    fn label(&self) -> String;
}

struct Tag {
    name: String,
}

impl Tag {
    fn label(&self) -> String {
        String::from("inherent")
    }
}

impl Label for Tag {
    fn label(&self) -> String {
        String::from("trait")
    }
}

fn main() {
    let tag = Tag { name: String::from("x") };
    println!("{}", tag.label());
}
```

Output:
```text
inherent
```

If no inherent method exists and exactly one trait impl matches, that method is
called. If more than one trait impl provides a method with the same name, the
call is ambiguous and reports `E0034`:

```ls,compile-fail
trait Left {
    fn name(&self) -> String;
}

trait Right {
    fn name(&self) -> String;
}

struct Item {
    value: i64,
}

impl Left for Item {
    fn name(&self) -> String {
        String::from("left")
    }
}

impl Right for Item {
    fn name(&self) -> String {
        String::from("right")
    }
}

fn main() {
    let item = Item { value: 1 };
    println!("{}", item.name());
}
```

Error:
```text
error[E0034]: multiple applicable items in scope
   --> example-04.ls:27:25
    |
 27 |     println!("{}", item.name());
    |                         ^^^^

lsaca: example-04.ls: could not compile example-04.ls due to previous error(s)
```

## Traits as bounds

The main use of a trait is as a bound on a type parameter. `S: Shape` means
"any type that implements `Shape`", and the body of the generic item may call
the trait's methods on values of type `S`:

```ls
trait Shape {
    fn area(&self) -> f64;
}

struct Square {
    side: f64,
}

struct Circle {
    radius: f64,
}

impl Shape for Square {
    fn area(&self) -> f64 {
        self.side * self.side
    }
}

impl Shape for Circle {
    fn area(&self) -> f64 {
        3.14159 * self.radius * self.radius
    }
}

fn total_area<S: Shape>(shape: S) -> f64 {
    shape.area()
}

fn main() {
    let square = Square { side: 2.0 };
    let circle = Circle { radius: 1.0 };
    println!("{:.2}", total_area(square));
    println!("{:.2}", total_area(circle));
}
```

Output:
```text
4.00
3.14
```

The bound is checked when the generic function is instantiated: calling
`total_area` with a type that does not implement `Shape` reports `E0277`. A
type parameter may require several bounds with `+`, as in `<T: Shape +
Describe>`; see [Generics](17-generics.md#bounds).

## Traits with type parameters

A trait may declare type parameters, and an impl names the concrete arguments:

```ls
trait Producer<T> {
    fn produce(&mut self) -> Option<T>;
}

struct Counter {
    value: i64,
}

impl Producer<i64> for Counter {
    fn produce(&mut self) -> Option<i64> {
        if self.value > 3 {
            None
        } else {
            self.value = self.value + 1;
            Some(self.value - 1)
        }
    }
}

fn main() {
    let mut counter = Counter { value: 0 };
    println!("{:?}", counter.produce());
    println!("{:?}", counter.produce());
}
```

Output:
```text
Some(0)
Some(1)
```

The standard library's `Iterator<T>` is a trait of exactly this shape: one
method, `next(&mut self) -> Option<T>`. See [Iterators](19-iterators.md).

## Default method bodies

Since 2.0 a trait method may have a body. The body is the *default*
implementation: an impl block may provide its own version, or omit the method
and inherit the default.

```ls
trait Greet {
    fn greet(&self) -> String {
        String::from("hello")
    }
}

struct Person {
    name: String,
}

impl Greet for Person {}

fn main() {
    let person = Person { name: String::from("a") };
    println!("{}", person.greet());
}
```

Output:
```text
hello
```

A method without a body is still *required*: the impl must provide it or the
error is `E0046`. An impl method that overrides a default wins, and an impl
method that does not exist in the trait is still `E0407`. A default body may call
the other methods of the trait on `self`; for an explicit impl those calls go to
the implementation's version of the method.

## Generic impls

An impl block can be generic, and a trait impl can implement a trait on a
generic type:

```ls
trait First<T> {
    fn first_item(&self) -> Option<T>;
}

struct Pair<T> {
    first: T,
    second: T,
}

impl<T> First<T> for Pair<T> {
    fn first_item(&self) -> Option<T> {
        Some(self.first)
    }
}

fn main() {
    let pair: Pair<i64> = Pair { first: 10, second: 20 };
    println!("{:?}", pair.first_item());
}
```

Output:
```text
Some(10)
```

As with any generic item, the body of `first_item` is checked per instantiation
(here `Pair<i64>`): reading `self.first` out of a borrowed value is fine when
`T` is Copy and would be rejected for `Pair<String>`.

## The Drop trait

`Drop` remains special. It is the only trait with a builtin implementation
form — `impl Drop for Type { fn drop(&mut self) { ... } }` — and it is
documented with the other drop rules on [Ownership and
moves](11-ownership-and-moves.md#the-drop-trait). Generic types can implement
`Drop`, in which case a hook is recorded for each concrete instantiation.

`Copy` and `Clone` are not traits you can implement: both are derived
automatically (see [Copy and move-only
types](11-ownership-and-moves.md#copy-and-move-only-types)).

## Coherence

An impl is checked for coherence when a call resolves to it. The checks are:

* **Duplicate implementations** (`E0119`): the same trait cannot be implemented
  twice for the same type.
* **Missing methods** (`E0046`): an impl must provide every *required* method
  declared by the trait. A method with a default body (see [Default method
  bodies](#default-method-bodies)) may be omitted.
* **Extra methods** (`E0407`): an impl cannot add methods that the trait does
  not declare.
* **Signature mismatch** (`E0053`): an impl method's receiver and parameter
  types must match the declaration after substituting the trait's type
  arguments.

Two of these are shown below. A duplicate impl is reported when a call
resolves to it:

```ls,compile-fail
trait Greet {
    fn greet(&self) -> String;
}

struct Person {
    name: String,
}

impl Greet for Person {
    fn greet(&self) -> String {
        String::from("hi")
    }
}

impl Greet for Person {
    fn greet(&self) -> String {
        String::from("hello")
    }
}

fn main() {
    let person = Person { name: String::from("a") };
    println!("{}", person.greet());
}
```

Error:
```text
error[E0119]: conflicting implementations of trait (Greet) for type (Person)
   --> example-09.ls:23:27
    |
 23 |     println!("{}", person.greet());
    |                           ^^^^^

lsaca: example-09.ls: could not compile example-09.ls due to previous error(s)
```

A missing method is reported when the impl is used:

```ls,compile-fail
trait Greet {
    fn greet(&self) -> String;
    fn farewell(&self) -> String;
}

struct Person {
    name: String,
}

impl Greet for Person {
    fn greet(&self) -> String {
        String::from("hi")
    }
}

fn main() {
    let person = Person { name: String::from("a") };
    println!("{}", person.greet());
}
```

Error:
```text
error[E0046]: not all trait items implemented, missing: farewell
   --> example-10.ls:18:27
    |
 18 |     println!("{}", person.greet());
    |                           ^^^^^

lsaca: example-10.ls: could not compile example-10.ls due to previous error(s)
```

## Trait objects

A bare trait name is still not a type: `dyn` is required. Since 2.0 a trait can
be used as the inner type of `Box`, `Rc`, `&` or `&mut`, and the resulting
*trait object* holds a value of any type implementing the trait and dispatches
its methods at runtime:

```text
Box<dyn Greet>     Rc<dyn Greet>     &dyn Greet     &mut dyn Greet
```

Writing `fn call(greeter: Greet)` is still an error (`E0412`), and so is using
`dyn Greet` outside a `Box`/`Rc`/`&`/`&mut` (`E0001`). A trait can be made into a
trait object only if it is *object safe* (`E0038`): every method takes `&self` or
`&mut self`, there are no generic methods and no associated items. Generic
bounds remain the static alternative, and they also work for traits that are not
object safe.

Conversion from a concrete value to a trait object is one of the standard
coercions — `&T` to `&dyn Trait`, `Box<T>` to `Box<dyn Trait>`, and so on — so
`let greeter: Box<dyn Greet> = Box::new(person);` works when `impl Greet for
Person` exists. See [Trait objects](25-trait-objects.md) for the full rules.

## What traits do not include

* No associated functions (`Trait::method()` without a receiver), no associated
  types, no constants and no generic methods.
* No supertraits and no `where` clauses; bounds are written inline.
* `impl Trait for Type` only for user-defined structs and enums; a
  builtin target is `E0118`.
* No associated items or generic methods, so only *object safe* traits can be
  used as trait objects; a trait object requires the `dyn` keyword, and `dyn`
  is valid only behind `Box`, `Rc`, `&` or `&mut`.
* `Copy`, `Clone` and `Drop` are not ordinary implementable traits: `Copy` and
  `Clone` are derived automatically, and `Drop` has its own special impl form.

See [Roadmap and deviations from Rust](16-roadmap.md) for the full list.

## Summary

* A trait declares methods with `self`/`&self`/`&mut self` receivers. A method
  ends with `;` (required) or carries a default body (since 2.0); an impl may
  omit methods that have a default.
* `impl Trait for Type` implements a trait for a user struct or enum; a
  builtin target is `E0118`. Impls may be generic and traits may have type
  parameters.
* Method dispatch checks inherent methods first, then trait impls; two
  matching trait impls report `E0034`.
* Traits are used as generic bounds, and a bound violation reports `E0277`.
* Coherence reports `E0119` (duplicate impl), `E0046` (missing required
  method), `E0407` (extra method) and `E0053` (signature mismatch).
* A bare trait is not a type (`E0412`). Since 2.0 an object-safe trait can be a
  trait object behind `dyn` and `Box`/`Rc`/`&`/`&mut`, with dynamic dispatch;
  `==` and `<` on trait objects are `E0369`.

## What next

* Type parameters and bounds: [Generics](17-generics.md).
* Trait objects and dynamic dispatch: [Trait objects](25-trait-objects.md).
* The standard library's `Iterator<T>` trait: [Iterators](19-iterators.md).
* Special `Drop` rules: [Ownership and moves](11-ownership-and-moves.md#the-drop-trait).
* Deviations and limitations: [Roadmap and deviations from Rust](16-roadmap.md).
