# Trait objects

Version 2.0 adds trait objects: a value of type `dyn Trait` holds any value whose
type implements `Trait`, and calls are dispatched at run time through a vtable.
Before 2.0 every trait use was static: the concrete type was known at compile time and
the compiler generated one copy of each function per type (see
[Traits](18-traits.md) and [Generics](17-generics.md)). Static dispatch is still the
default and the cheaper choice; trait objects are the opt-in tool for heterogeneous
collections and for APIs that must agree at run time.

The examples on this page use the 2.1.0 syntax. They are part of the 2.1.0
documentation test plan and are executed by the documentation verifier against
the 2.1.0 compiler.

## The `dyn` type

`dyn` is a keyword in 2.0. A trait object type is written `dyn Trait` or
`dyn Trait<Args>`:

```rust
dyn Describe
dyn Iterator<i64>
```

A trait object is unsized: it has no size of its own, because the concrete type it
holds is only known at run time. It can therefore only be used behind a pointer:

| Form | Meaning | Ownership of the concrete value |
| --- | --- | --- |
| `Box<dyn Trait>` | owning trait object | the box owns it and drops it |
| `Rc<dyn Trait>` | shared trait object | the `Rc` cell owns it; copies share it |
| `&dyn Trait` | borrowed trait object | the borrow does not own it |
| `&mut dyn Trait` | mutably borrowed trait object | the borrow does not own it |

Using `dyn Trait` anywhere else (a local binding, a struct field, a parameter,
a return type, an element of a `Vec`) is error `E0001`:

> trait objects are unsized; use `Box<dyn Trait>`, `Rc<dyn Trait>` or `&dyn Trait`

The restriction matches the one on references: references to heap values can be
created and passed around, but they cannot be stored inside aggregates in 2.1.0.

## Creating a trait object

A concrete value becomes a trait object through a coercion, and the compiler checks
that the concrete type really implements the trait. Coercions happen at the usual
places:

* `let` bindings and assignments;
* function and method arguments and return values;
* struct-literal fields and enum payloads;
* `vec![...]` elements and `Vec::push` arguments.

The four coercion forms are:

| From | To |
| --- | --- |
| `&T` | `&dyn Trait` |
| `&mut T` | `&mut dyn Trait` |
| `Box<T>` | `Box<dyn Trait>` |
| `Rc<T>` | `Rc<dyn Trait>` |

where `T` implements `Trait`. If it does not, the coercion is rejected with
`E0277`: the diagnostic reports that the trait is not implemented for the concrete
type and notes that the concrete type must implement the trait to become a trait
object.

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
        return format!("point at ({}, {})", self.x, self.y);
    }
}

struct Circle {
    radius: f64,
}

impl Describe for Circle {
    fn describe(&self) -> String {
        return format!("circle with radius {}", self.radius);
    }
}

fn main() {
    let mut shapes: Vec<Box<dyn Describe>> = Vec::new();
    shapes.push(Box::new(Point { x: 1, y: 2 }));
    shapes.push(Box::new(Circle { radius: 2.5 }));
    for shape in shapes {
        println!("{}", shape.describe());
    }
}
```

Output:
```text
point at (1, 2)
circle with radius 2.5
```

The vector holds two different concrete types. `Box::new(Point { .. })` creates a
`Box<Point>`, and the `push` argument coerces it to `Box<dyn Describe>`. Inside the loop
`shape` has type `Box<dyn Describe>`; the call `shape.describe()` goes through the
vtable, so the implementation for the concrete type runs.

## Methods and receivers

A trait object exposes exactly the methods of the trait. Calls auto-deref the
container the same way ordinary method calls do, so `describe()` works on
`Box<dyn Describe>`, `Rc<dyn Describe>`, `&dyn Describe` and `&mut dyn Describe`.

Receivers keep their meaning:

* `&self` methods need a shared receiver and can be called on any of the four
  forms (as long as the object is not mutably borrowed elsewhere);
* `&mut self` methods need a mutable receiver, so they are only callable when the
  trait object itself is reachable mutably - through `&mut dyn T`, a mutable
  `Box<dyn T>` binding or a mutable `Rc<dyn T>` place. Calling a `&mut self` method
  through a shared receiver is `E0596`;
* traits cannot declare associated functions in 2.1.0 (a trait `fn` without a
  receiver is `E0001`), so a trait object has no associated functions; inherent
  methods of the concrete type are not part of the object's interface, so calling
  one on a trait object is `E0599` (`no method named ... found for type
  Box<dyn Trait>`);

```ls
trait Counter {
    fn bump(&mut self) -> i64;
}

struct Tally {
    count: i64,
}

impl Counter for Tally {
    fn bump(&mut self) -> i64 {
        self.count += 1;
        return self.count;
    }
}

fn main() {
    let mut counter: Box<dyn Counter> = Box::new(Tally { count: 0 });
    println!("{}", counter.bump());
    println!("{}", counter.bump());
}
```

Output:
```text
1
2
```

## Object safety (`E0038`)

Not every trait can become a trait object. A trait is object safe when all of its
methods

* take `&self` or `&mut self` as their receiver (a `self`-by-value method breaks
  object safety);
* use `Self` only in the receiver, not in parameters or return types.

Generic methods, associated functions, associated types and associated constants
cannot be declared in 2.1.0 traits at all, so they can never make a trait that does
compile non-object-safe: a generic method is `E0001` (trait methods cannot be
generic), an associated function is `E0001` (trait methods must take a self
receiver), and an associated type or constant is a parse error. A trait with
`self`-by-value methods or with `Self` outside the receiver is accepted for static
dispatch but cannot be the target of `dyn`. Methods with a default body are allowed
as long as they are object safe themselves. Defaults that call other methods of the
same trait dispatch to the concrete implementation.

A trait that is not object safe is still usable for static dispatch; it just cannot
be the target of `dyn`. Writing `dyn Trait` for it is `E0038`; the note names the
reason, for example "method `into` takes `self` by value" or "method `dup`
mentions `Self` outside its receiver".

```rust
// Object safe: every method takes &self.
trait Describe {
    fn describe(&self) -> String;
}

// Not object safe: `into` takes self by value, so `dyn Take` is E0038.
trait Take {
    fn into(self) -> i64;
}

// Not object safe: Self in the return type, so `dyn Cloneable` is E0038.
trait Cloneable {
    fn dup(&self) -> Self;
}
```

`Take` and `Cloneable` still work when used with static dispatch: `point.into()`
and `point.dup()` are ordinary monomorphized calls.

## Copies, moves and sharing

The four forms behave like their containers:

| Form | Copy? | Behaviour of `.clone()` |
| --- | --- | --- |
| `Box<dyn Trait>` | move-only | deep copy: clones the concrete value into a fresh box through the vtable |
| `Rc<dyn Trait>` | move-only handle | shares the same cell and increments the strong count |
| `&dyn Trait` | `Copy` | copies the fat pointer; nothing is cloned |
| `&mut dyn Trait` | `Copy` | copies the fat pointer; nothing is cloned |

A `Box<dyn Trait>` is therefore the owning, cloneable container: cloning it gives an
independent concrete value. An `Rc<dyn Trait>` is the shared container: cloning it
gives another handle to the same value. Both reference forms are `Copy`: copying a
`&dyn Trait` or a `&mut dyn Trait` copies its two words, exactly like copying any
other reference in 2.1.0.

Coercing an `Rc<T>` to `Rc<dyn Trait>` counts as a share, not a move: the original
`Rc<T>` handle stays valid alongside the trait object, and both point at the same
cell. That is why the following program can still use `point` after creating
`object`:

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
        return format!("point at ({}, {})", self.x, self.y);
    }
}

fn main() {
    let point: Rc<Point> = Rc::new(Point { x: 3, y: 4 });
    let object: Rc<dyn Describe> = point.clone();
    println!("{}", point.get().describe());
    println!("{}", object.describe());
    println!("{}", point.strong_count());
}
```

Output:
```text
point at (3, 4)
point at (3, 4)
2
```

`point` still has type `Rc<Point>`, so `point.get()` copies the `Point` out of the
cell and `describe()` is an ordinary static call; only `object` is a trait object.

A `dyn Trait` coerces to the same `dyn Trait` (for example `Box<dyn Trait>` to
`&dyn Trait` through a borrow), but there is no upcasting: a `dyn Sub` cannot be
turned into a `dyn Super`, because traits cannot declare supertraits in 2.1.0.

## Equality, formatting and drop

Trait objects have no structural equality: `==` and `<` on `dyn Trait` values are
rejected with `E0369`, because the object does not know whether the concrete type
supports the comparison. Compare the concrete values before erasing the type, or add
an explicit method to the trait.

Formatting is the exception: `{}` and `{:?}` both print a trait object through its
vtable using the concrete type's debug rendering, so a `Box<dyn Describe>` holding a
`Point { x: 1, y: 2 }` prints `Point { x: 1, y: 2 }`. This is intended for
diagnostics and logging.

Dropping a `Box<dyn Trait>` or the last `Rc<dyn Trait>` handle first runs the concrete
value's `Drop` hook (if it has one) and then releases the payload, with the same
ordering rules as any other owned value. A borrowed `&dyn Trait` never drops the
referent.

## How trait objects are represented

A trait object is a fat value with two machine words:

```text
struct LsDyn { data: void*, vtable: const LsVTable_* }
```

The compiler builds exactly one vtable per (trait instantiation, concrete type) pair,
and every coercion form shares it, so `Box<dyn Describe>`, `Rc<dyn Describe>` and
`&dyn Describe` for the same `Point` use the same table. Each table has four fixed
slots followed by the trait's methods in declaration order:

| Slot | Contents |
| --- | --- |
| 0 | drop: release the concrete value |
| 1 | clone: deep-copy the concrete value into a fresh managed cell |
| 2 | trace: report the concrete value to the cycle collector |
| 3 | debug: format the concrete value |
| 4, 5, ... | one thunk per trait method, in declaration order |

The table is independent of the container type, so an `Rc<dyn Trait>` does not
need its own table copy. `Box<dyn Trait>` and `Rc<dyn Trait>` are stored in an ordinary
ORC-managed cell whose payload is the fat value; for a `Box` coercion the concrete box
cell moves into the trait-object cell (no copy), while an `Rc` coercion retains the
concrete `Rc` cell. The generated C project uses no new runtime functions for trait
objects.

The order in which trait objects and vtables are registered is deterministic (first
use in the flattened program), which keeps the generated C reproducible.

## Known limitations in 2.1.0

* No upcasting from `dyn Sub` to `dyn Super` (supertraits do not exist).
* No trait objects of non-object-safe traits (`E0038`). Traits cannot declare
  generic methods or associated functions (`E0001`), and associated types or
  associated constants are parse errors, so those features never reach `dyn`.
* No equality or ordering on trait objects; implement an explicit method instead.
* `Box<dyn Trait>::get` and `Box<dyn Trait>::set` are rejected, because the payload is
  unsized. Use the trait's methods.
* No `impl Trait` return types and no `dyn Trait + Send`-style combinations: a trait
  object names exactly one trait, and there are no auto traits.
* Trait objects cannot be stored directly in structs, enums or `Vec` elements -
  always through `Box`, `Rc` or a reference, and references themselves cannot be
  stored in aggregates in 2.1.0.
* The vtable layout is an implementation detail and may change; do not rely on
  slot numbers from lsaca code.

## Summary

* `dyn Trait` is an unsized trait object type; it is only usable as
  `Box<dyn Trait>`, `Rc<dyn Trait>`, `&dyn Trait` or `&mut dyn Trait`.
* Any value whose type implements the trait coerces to a trait object; the compiler
  checks the impl and reports `E0277` otherwise.
* Only object-safe traits (`E0038`) can be trait objects: methods take
  `&self`/`&mut self`, and `Self` appears only in the receiver. Traits cannot
  declare associated functions or generic methods at all.
* `Box<dyn Trait>` clones deeply, `Rc<dyn Trait>` shares the value, `&dyn Trait` and
  `&mut dyn Trait` are `Copy` borrows, and coercing an `Rc` shares rather than moves.
* Calls dispatch through a vtable; one table is shared by every coercion form for
  the same trait and concrete type.
* Equality and ordering are rejected on trait objects; formatting and drop go
  through the vtable.

## What next

* [Traits](18-traits.md) - declarations, default method bodies and coherence.
* [Generics](17-generics.md) - static dispatch and monomorphization.
* [Ownership and moves](11-ownership-and-moves.md) - what `Box` and `Rc` own.
* [Memory model](05-memory-model.md) - ORC, reference counting and the cycle collector.
