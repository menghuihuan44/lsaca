# Iterators

Version 1.5.0 adds the `Iterator<T>` trait and a lazy adapter pipeline over it.
Iterators are ordinary values: an adapter returns a generic struct that stores
its input and the closures it needs, and nothing runs until a terminal operation
asks for the next item. The trait and the adapter functions themselves are
written in lsaca — in 2.2.0 they live in the `std` crate (`std::iter`,
`stdlib/src/iter.ls`), whose root re-exports the prelude names — but a program
only uses the public surface described here. Version 2.1 adds the `zip`,
`chain`, `filter_map`, `take_while`, `skip_while` and `step_by` adapters plus the
`position`, `last`, `nth`, `reduce`, `product`, `inspect` and `for_each`
terminal operations; `rev` and `flat_map` remain planned (see
[iter](../stdlib/iter.md)).

The trait is small:

```rust
pub trait Iterator<T> {
    fn next(&mut self) -> Option<T>;
}
```

`T` is the **item type**. `next` returns `Some(item)` while items remain and
`None` once the iterator is exhausted. It takes `&mut self`, so calling it
borrows the iterator but does not consume it: call `next` as often as needed.
The `Iterator` trait is re-exported by the `std` crate root, so it is in scope in
every module; do not declare another trait with that name.

## Getting an iterator from a Vec

`.iter()` turns a `Vec<T>` into an iterator over its elements:

```ls
fn main() {
    let values = vec![10, 20, 30];
    let mut iter = values.iter();
    println!("{:?}", iter.next());
    println!("{:?}", iter.next());
    println!("{:?}", iter.next());
    println!("{:?}", iter.next());
}
```

Output:
```text
Some(10)
Some(20)
Some(30)
None
```

Unlike Rust's `into_iter`, `.iter()` **consumes the vector**: the iterator owns
the elements from then on, so `values` cannot be used again. That is a
documented deviation from Rust (`.iter()` still consumes the vector in 2.2.0), but
it means `.iter()` works for any element type, including move-only types that
are neither `Copy` nor cloneable.

The iterator value may be stored in a local and advanced later. Its concrete
type is an ordinary generic struct from the `std` crate, so the compiler names
it with the usual generic display form; programs rarely need to spell it.

## Adapters

Adapters take an iterator and a closure and return another iterator. They are
lazy: an adapter only remembers its input and its closure. The following
terminal operations and `for` loops are what drive them.

The closures passed to the adapters in this section are written without
parameter annotations; their parameter and return types are inferred from the
adapter's `Fn(..)` signature.

### `map`

`map` applies a closure to every item. The closure receives the item **by
value** (`Fn(T) -> U`), and the returned values are the new items:

```ls
fn main() {
    let values: Vec<i64> = vec![1, 2, 3, 4];
    let doubled: Vec<i64> = values.iter().map(|value| value * 2).collect();
    println!("{:?}", doubled);
}
```

Output:
```text
[2, 4, 6, 8]
```

### `filter`

`filter` keeps the items for which the predicate is true. The predicate
receives a **reference** to the item (`Fn(&T) -> bool`) so that it can inspect
the item without taking ownership; when the predicate accepts, the item itself
is still moved on to the next stage:

```ls
fn main() {
    let values: Vec<i64> = vec![1, 2, 3, 4, 5, 6];
    let evens: Vec<i64> = values.iter().filter(|value| *value % 2 == 0).collect();
    println!("{:?}", evens);
}
```

Output:
```text
[2, 4, 6]
```

The same rule applies to `find`, `any` and `all` (see
[Terminal operations](#terminal-operations)).

### `take` and `skip`

`take(n)` yields at most the first `n` items; `skip(n)` drops the first `n`
items and yields the rest. Both take a `usize`, like every count:

```ls
fn main() {
    let values: Vec<i64> = vec![10, 20, 30, 40, 50];
    let window: Vec<i64> = values.iter().skip(1).take(2).collect();
    println!("{:?}", window);
}
```

Output:
```text
[20, 30]
```

### `enumerate`

`enumerate()` pairs every item with its zero-based position. The item type
becomes the tuple `(usize, T)`:

```ls
fn main() {
    let words = vec![String::from("alpha"), String::from("beta")];
    for (index, word) in words.iter().enumerate() {
        println!("{}: {}", index, word);
    }
}
```

Output:
```text
0: alpha
1: beta
```

## Laziness and chaining

Adapters compose. Because they are lazy, a chain does no work until something
consumes it, and each stage pulls exactly the items the following stage asks
for:

```ls
fn main() {
    let values: Vec<i64> = vec![1, 2, 3, 4, 5, 6];
    let doubled = values.iter().map(|value| value * 2);
    let divisible = doubled.filter(|value| *value % 4 == 0);
    let total: i64 = divisible.sum();
    println!("{}", total);
}
```

Output:
```text
24
```

Each adapter value is an ordinary value: it can be stored in a local, passed to
a generic function whose parameter has an `Iterator<T>` bound, or dropped. (Its
concrete type is a standard-library type that ordinary code does not name, so an
adapter is not usually written as a return type.) Dropping an unfinished
iterator releases the input and captured closures normally, without running any
remaining elements.

## Terminal operations

Terminal operations consume the iterator and produce a value.

| Operation | Result | Item type | Notes |
| --- | --- | --- | --- |
| `collect()` | `Vec<T>` | `T` | The result type must be known: a `let` annotation, a return type, or the turbofish `collect::<Vec<i64>>()`. Otherwise `E0282`. |
| `fold(init, f)` | `U` | `T` | `f` has type `Fn(U, T) -> U`; `init` fixes `U` and is passed as the first argument of the first call. |
| `count()` | `usize` | any | Number of remaining items. |
| `sum()` | `T` | `i32`, `i64`, `usize` or `f64` | Only those item types; anything else is `E0277`. An unannotated integer vector has `i32` items, so `vec![1, 2, 3].iter().sum()` is `i32`. |
| `product()` | `T` | `i32`, `i64`, `usize` or `f64` | Numeric product of the remaining items; same item types as `sum`. |
| `any(p)` / `all(p)` | `bool` | `T` | `p` has type `Fn(&T) -> bool`; short-circuiting. |
| `find(p)` | `Option<T>` | `T` | Returns the first item for which `p` is true; `p` has type `Fn(&T) -> bool`. |
| `position(p)` | `Option<usize>` | `T` | Zero-based index of the first item for which `p` is true; `p` has type `Fn(T) -> bool`. |
| `last()` | `Option<T>` | `T` | The final remaining item. |
| `nth(n)` | `Option<T>` | `T` | The item at zero-based position `n`; the earlier items are skipped. |
| `reduce(f)` | `Option<T>` | `T` | Like `fold` without an initial value; `f` has type `Fn(T, T) -> T`, and an empty iterator gives `None`. |
| `inspect(f)` | the same iterator | `T` | Calls `f(&T)` on each item and yields it unchanged; used for tracing. |
| `for_each(f)` | nothing | `T` | Calls `f(T)` for every remaining item; `f` has type `Fn(T)`. |

`collect` builds a vector. Either annotate the binding or use the turbofish:

```ls
fn main() {
    let values: Vec<i64> = vec![1, 2, 3];
    let annotation: Vec<i64> = values.iter().map(|value| value + 1).collect();
    println!("{:?}", annotation);

    let turbofish = vec![1i64, 2, 3].iter().map(|value| value * 3).collect::<Vec<i64>>();
    println!("{:?}", turbofish);
}
```

Output:
```text
[2, 3, 4]
[3, 6, 9]
```

If the result type is not known, the compiler cannot choose the element type and
reports `E0282`:

```ls,compile-fail
fn main() {
    let values = vec![1, 2, 3];
    let doubled = values.iter().map(|value| value * 2).collect();
    println!("{:?}", doubled);
}
```

Error:
```text
error[E0282]: type annotations needed: .collect() requires a known Vec<T> result type
  --> example-08.ls:3:56
   |
 3 |     let doubled = values.iter().map(|value| value * 2).collect();
   |                                                        ^^^^^^^

lsaca: example-08.ls: could not compile example-08.ls due to previous error(s)
```

`fold` threads an accumulator through the items:

```ls
fn main() {
    let values = vec![1, 2, 3, 4];
    let total = values.iter().fold(0, |acc, value| acc + value);
    println!("{}", total);

    let words = vec![String::from("a"), String::from("b"), String::from("c")];
    let joined = words.iter().fold(String::from(""), |acc, word| acc + word.as_str());
    println!("{}", joined);
}
```

Output:
```text
10
abc
```

`count`, `sum`, `any`, `all` and `find` each consume the iterator and return a
simple result:

```ls
fn main() {
    println!("{}", vec![1i64, 2, 3, 4, 5].iter().count());
    println!("{}", vec![1i64, 2, 3, 4, 5].iter().sum());
    println!("{}", vec![1.5, 2.25].iter().sum());

    let positive = vec![1i64, 2, 3].iter().any(|value| *value > 2);
    let evens = vec![2i64, 4, 6].iter().all(|value| *value % 2 == 0);
    println!("{} {}", positive, evens);

    let found = vec![3i64, 8, 12].iter().find(|value| *value > 10);
    println!("{:?}", found);
}
```

Output:
```text
5
15
3.75
true true
Some(12)
```

## `for` loops over iterators

`for pattern in value` works when `value` has a concrete type implementing
`Iterator<T>`. The loop calls `next` until it returns `None`, binds the item
with the pattern, and releases the iterator when the loop ends:

```ls
fn main() {
    let values = vec![10, 20, 30];
    for value in values.iter().map(|item| item + 1) {
        println!("{}", value);
    }
}
```

Output:
```text
11
21
31
```

`for` over a `Vec<T>` value and over an integer range keep their earlier
behaviour: the `Vec` case moves the vector and binds its elements by value. See
[Statements and control flow](08-statements-and-control-flow.md).

## Implementing `Iterator` for your own type

Any struct or enum can implement `Iterator<T>` (see
[Traits](18-traits.md)). `next` takes `&mut self`, so the iterator keeps its
position in its own fields. A `for` loop, or a hand-written call to `next`,
drives it:

```ls
struct Countdown {
    value: i64,
}

impl Iterator<i64> for Countdown {
    fn next(&mut self) -> Option<i64> {
        if self.value == 0 {
            return None;
        }
        let current = self.value;
        self.value = self.value - 1;
        return Some(current);
    }
}

fn main() {
    let mut countdown = Countdown { value: 3 };
    println!("{:?}", countdown.next());
    for value in countdown {
        println!("{}", value);
    }
}
```

Output:
```text
Some(3)
2
1
```

Adapters and terminal operations are generic functions with `Iterator<T>`
bounds, so they work with a user-defined iterator exactly as with the builtin
ones. `.iter()` is the one operation specific to `Vec<T>`; a user iterator is
used directly, without calling `.iter()` first:

```ls
struct Countdown {
    value: i64,
}

impl Iterator<i64> for Countdown {
    fn next(&mut self) -> Option<i64> {
        if self.value == 0 {
            return None;
        }
        let current = self.value;
        self.value = self.value - 1;
        return Some(current);
    }
}

fn main() {
    let countdown = Countdown { value: 5 };
    let doubled: Vec<i64> = countdown.map(|value| value * 2).collect();
    println!("{:?}", doubled);
}
```

Output:
```text
[10, 8, 6, 4, 2]
```

Since `Countdown` yields `5, 4, 3, 2, 1`, mapping and collecting gives the
vector shown.

## What is not provided

The 2.1 iterator pipeline is still deliberately small. It has no:

* `size_hint` or double-ended iteration;
* `rev` and `flat_map` adapters;
* mutable borrowing iteration (`Vec::iter_mut`, `VecIterMut`), deferred to a
  later version (`.iter()` consumes the vector);
* `into_iter` (the method is spelled `.iter()`);
* for-loop iteration over a `HashMap` (use `keys()`/`values()`, see
  [HashMap](20-hashmap.md));
* `Iterator` implementations for builtin types: only `Vec<T>` and the adapter
  structs implement the trait, while ranges keep their own `for` lowering;
* iteration over slices (`&[T]`) and `collect` into anything except `Vec<T>`.

Version 2.1 adds `zip`, `chain`, `filter_map`, `take_while`, `skip_while` and
`step_by` to the adapter set, together with the `position`, `last`, `nth`,
`reduce`, `product`, `inspect` and `for_each` terminal operations (see
[iter](../stdlib/iter.md) for the signatures).

## Known limitations in 2.2.0

* `.iter()` still consumes the `Vec`; a borrowed vector is not iterated by
  `.iter()` (borrowing iteration goes through slice references instead).
* `collect` needs a known result type; a bare `collect()` without an
  annotation or turbofish is `E0282`.
* `sum` and `product` support `i32`, `i64`, `usize` and `f64` items only;
  other item types are `E0277`.
* The predicates of `filter`, `take_while`, `skip_while`, `any`, `all` and
  `find` receive `&T`, as does the `inspect` closure; `map`, `filter_map`,
  `position`, `reduce` and `for_each` receive `T` by value (and `fold` folds
  `T` values with an accumulator).
* Adapter names resolve only when the receiver implements `Iterator<T>` (or is
  a `Vec` for `.iter()`). Inherent and trait methods win over the adapter
  sugar, so a user type that defines its own `map` method keeps it.
* Iterators are values: a chain that is never consumed performs no work, and
  dropping an unfinished iterator releases its input without running the rest
  of the elements.

See [Roadmap and deviations from Rust](16-roadmap.md) for the full list.

## Summary

* `Iterator<T>` has one method, `next(&mut self) -> Option<T>`.
* `.iter()` consumes a `Vec<T>` and yields its elements in order.
* Adapters `map`, `filter`, `take`, `skip`, `enumerate`, `zip`, `chain`,
  `filter_map`, `take_while`, `skip_while` and `step_by` are lazy and
  chainable; `enumerate` turns the item type into `(usize, T)` and `count` and
  `position` return `usize`. `rev` and `flat_map` are planned for a later
  release.
* Terminal operations are `collect`, `fold`, `count`, `sum`, `product`,
  `any`, `all`, `find`, `position`, `last`, `nth`, `reduce`, `inspect` and
  `for_each`.
* `for` loops consume any concrete `Iterator<T>`; user structs and enums can
  implement the trait.
* Closures capture by value (see [Closures](14-closures.md)), and iterator
  values are ordinary values that drop normally.

## What next

* Declaring traits and implementing them: [Traits](18-traits.md).
* Writing generic structs, functions and impls: [Generics](17-generics.md).
* The `Vec` methods behind `.iter()`: [Standard library](15-standard-library.md).
* The `std::iter` module: [iter](../stdlib/iter.md).
* Key/value storage: [HashMap](20-hashmap.md).
