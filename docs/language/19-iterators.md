# Iterators

Version 1.5.0 added the `Iterator<T>` trait and a lazy adapter pipeline over
it. Iterators are ordinary values: an adapter returns a generic struct that
stores its input and the closures it needs, and nothing runs until a terminal
operation or a `for` loop asks for the next item. The trait and the adapter
functions themselves are written in lsaca. Since 2.3.0 they live in the `std`
crate (`std::iter`, `stdlib/src/iter.ls`), whose root re-exports the prelude
names, but a program only uses the public surface described here.

The iterator surface grew over several releases. Version 2.1 added `zip`,
`chain`, `filter_map`, `take_while`, `skip_while` and `step_by`, together
with the `position`, `last`, `nth`, `reduce`, `product`, `inspect` and
`for_each` terminal operations. Version 2.3 added borrowed iteration
(`iter`, `iter_mut`, `into_iter`), iteration over slices, `rev` and
`flat_map`. Version 2.4.0 adds the `min` and `max` terminals and widens
`product` to the same eight numeric item types (see [iter](../stdlib/iter.md)).

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

## Getting iterators from a Vec

A `Vec<T>` offers three ways to start iterating, matching Rust:

| Expression | Item type | Vector afterwards |
| --- | --- | --- |
| `values.iter()` | `&T` | still usable (shared borrow) |
| `values.iter_mut()` | `&mut T` | still usable (mutable borrow) |
| `values.into_iter()` | `T` | moved into the iterator |

`.iter()` borrows the vector and yields shared references. The vector is
unchanged and can be read again after the iterator is dropped:

```ls
fn main() {
    let values = vec![10, 20, 30];
    for value in values.iter() {
        println!("{}", *value);
    }
    println!("len {}", values.len());
}
```

Output:
```text
10
20
30
len 3
```

`.iter_mut()` borrows the vector mutably and yields `&mut T`. The loop body can
write through the reference, and the vector keeps the new values:

```ls
fn main() {
    let mut values = vec![1, 2, 3];
    for value in values.iter_mut() {
        *value = *value * 10;
    }
    println!("{:?}", values);
}
```

Output:
```text
[10, 20, 30]
```

`.into_iter()` consumes the vector and yields owned items. It is the right
choice for move-only element types and for chains whose closures take `T` by
value:

```ls
fn main() {
    let words = vec![String::from("hi"), String::from("there")];
    let lengths: Vec<usize> = words.into_iter().map(|word| word.len()).collect();
    println!("{:?}", lengths);
}
```

Output:
```text
[2, 5]
```

The iterator values may be stored in locals and advanced later. Their concrete
types are ordinary generic structs from the `std` crate, so the compiler names
them with the usual generic display form; programs rarely need to spell them.

## Borrowing loops and slices

A `for` loop borrows directly when it iterates a reference. `for value in
&values` is the loop form of `values.iter()`, and `for value in &mut values`
is the loop form of `values.iter_mut()`:

```ls
fn main() {
    let values: Vec<i64> = vec![4, 8, 15];
    let mut sum: i64 = 0;
    for value in &values {
        sum += *value;
    }
    println!("{} {}", sum, values.len());

    let mut more = vec![1, 2, 3];
    for value in &mut more {
        *value += 1;
    }
    println!("{:?}", more);
}
```

Output:
```text
27 3
[2, 3, 4]
```

Slices iterate the same way. `&[T]` yields `&T` and `&mut [T]` yields
`&mut T`; `Vec::as_slice()` and `Vec::as_mut_slice()` produce those slice
references:

```ls
fn main() {
    let values: Vec<i64> = vec![10, 20, 30];
    let slice: &[i64] = values.as_slice();
    let doubled: Vec<i64> = slice.iter().map(|value| *value * 2).collect();
    println!("{:?}", doubled);

    let mut mutable: Vec<i64> = vec![1, 2, 3];
    {
        let mutable_slice: &mut [i64] = mutable.as_mut_slice();
        for value in mutable_slice {
            *value += 100;
        }
    }
    println!("{:?}", mutable);
}
```

Output:
```text
[20, 40, 60]
[101, 102, 103]
```

When the items are references, dereference before doing arithmetic or passing
the value to a function that expects `T`: a chain over `.iter()` has item type
`&T`, so `.map(|value| *value * 2)` produces owned `T` items. Adapter
predicates such as `filter` already receive `&Item`; over a borrowed iterator
that is `&&T`, which is why the examples below dereference the item first
(`.map(|value| *value).filter(|value| *value % 2 == 0)`).

The vector cannot be mutated while a shared borrow is alive, and it cannot be
used at all while a mutable borrow is alive. End the iteration (or the block
holding the iterator) before touching the collection again. A `for` loop drops
its iterator when the loop ends, which ends the borrow.

## Adapters

Adapters take an iterator and a closure and return another iterator. They are
lazy: an adapter only remembers its input and its closure. Terminal operations
and `for` loops are what drive them.

The closures passed to the adapters in this section are written without
parameter annotations; their parameter and return types are inferred from the
adapter's `Fn(..)` signature.

### `map`

`map` applies a closure to every item. The closure receives the item **by
value** (`Fn(T) -> U`), and the returned values are the new items. Over
`.iter()` the item is `&T`, so the closure usually dereferences it:

```ls
fn main() {
    let values: Vec<i64> = vec![1, 2, 3, 4];
    let doubled: Vec<i64> = values.iter().map(|value| *value * 2).collect();
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
    let evens: Vec<i64> = values
        .iter()
        .map(|value| *value)
        .filter(|value| *value % 2 == 0)
        .collect();
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
    let window: Vec<i64> = values.iter().map(|value| *value).skip(1).take(2).collect();
    println!("{:?}", window);
}
```

Output:
```text
[20, 30]
```

### `enumerate`

`enumerate()` pairs every item with its zero-based position. The item type
becomes the tuple `(usize, T)`; over a borrowed vector `T` is `&T`, so the
pattern binds a reference:

```ls
fn main() {
    let words = vec![String::from("alpha"), String::from("beta")];
    for (index, word) in words.iter().enumerate() {
        println!("{}: {}", index, word.as_str());
    }
}
```

Output:
```text
0: alpha
1: beta
```

### `rev`

`rev()` yields the remaining items back to front:

```ls
fn main() {
    let values: Vec<i64> = vec![1, 2, 3];
    let reversed: Vec<i64> = values.iter().rev().map(|value| *value * 10).collect();
    println!("{:?}", reversed);
}
```

Output:
```text
[30, 20, 10]
```

`rev` is lazy when it is created, but it is not double-ended: the first time
it is advanced it drains the remaining input into an internal buffer, then
yields that buffer from the back. That costs one `Vec` of items and one pass
over the input, so prefer it for finite materialized sources rather than for
very long or infinite pipelines. The buffer owns the items exactly as the
upstream iterator did; for a borrowed chain the buffered items are references,
so the source stays borrowed until the `rev` iterator is dropped.

### `flat_map`

`flat_map(f)` applies `f` to every item and yields all items of the returned
sequence in order. The closure may return a `Vec<U>` or any iterator whose
item type is `U`:

```ls
fn main() {
    let values: Vec<i64> = vec![1, 2, 3];
    let flat: Vec<i64> = values
        .iter()
        .flat_map(|value| vec![*value, *value * 2])
        .collect();
    println!("{:?}", flat);

    let mut mixed: i64 = 0;
    for value in values.iter().flat_map(|value| vec![*value, -*value].into_iter()) {
        mixed += value;
    }
    println!("{}", mixed);
}
```

Output:
```text
[1, 2, 2, 4, 3, 6]
0
```

`flat_map` is lazy between outer items: it pulls one outer item, expands it,
and yields the expansion before asking the outer iterator for the next item.
The expansion returned as an iterator is itself advanced lazily. Items of the
current expansion that have not been yielded are held in a small buffer, so the
adapter allocates only when an expansion actually has items to store.

## Laziness and chaining

Adapters compose. Because they are lazy, a chain does no work until something
consumes it, and each stage pulls exactly the items the following stage asks
for:

```ls
fn main() {
    let values: Vec<i64> = vec![1, 2, 3, 4, 5, 6];
    let doubled = values.iter().map(|value| *value * 2);
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
| `collect()` | `Vec<T>` | `T` | The result type must be known: a `let` annotation, a return type, or the turbofish `collect::<Vec<i64>>()`. Otherwise `E0282`. Collecting borrowed items (`Vec<&T>`) is rejected with `E0498`; map them to owned values first. |
| `fold(init, f)` | `U` | `T` | `f` has type `Fn(U, T) -> U`; `init` fixes `U` and is passed as the first argument of the first call. |
| `count()` | `usize` | any | Number of remaining items. |
| `sum()` | `T` | `i32`, `i64`, `usize` or `f64` | Only those item types; anything else is `E0277`. A borrowed iterator has item type `&T`, so use `.map(|value| *value)` or `.into_iter()` before `sum`. An unannotated integer vector has `i32` items. |
| `product()` | `T` | `i32`, `i64`, `u32`, `u64`, `isize`, `usize`, `f32` or `f64` | Numeric product of the remaining items; the item type must be one of those eight. |
| `min()` / `max()` | `Option<T>` | same eight numeric types | Smallest / largest remaining item; `None` when empty. `min` keeps the first smallest item, `max` the last largest, matching Rust. Added in 2.4.0. |
| `any(p)` / `all(p)` | `bool` | `T` | `p` has type `Fn(&T) -> bool`; short-circuiting. Over `.iter()` the predicate receives `&&T`. |
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
    let annotation: Vec<i64> = values.iter().map(|value| *value + 1).collect();
    println!("{:?}", annotation);

    let turbofish = vec![1i64, 2, 3].iter().map(|value| *value * 3).collect::<Vec<i64>>();
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
    let doubled = values.into_iter().map(|value| value * 2).collect();
    println!("{:?}", doubled);
}
```

Error:
```text
error[E0282]: type annotations needed: .collect() requires a known Vec<T> result type
  --> example-14.ls:3:61
   |
 3 |     let doubled = values.into_iter().map(|value| value * 2).collect();
   |                                                             ^^^^^^^

lsaca: example-14.ls: could not compile example-14.ls due to previous error(s)
```

A `Vec<&T>` result is not allowed: the language does not store references in
`Vec`, and a collected snapshot of borrowed items would outlive the borrow it
points into. Map to owned values (or `clone` them) first:

```ls,compile-fail
fn main() {
    let values = vec![1i64, 2, 3];
    let refs: Vec<&i64> = values.iter().collect();
    println!("{:?}", refs);
}
```

Error:
```text
error[E0498]: references cannot be stored in this type (found &i64)
  --> example-15.ls:3:15
   |
 3 |     let refs: Vec<&i64> = values.iter().collect();
   |               ^^^

error[E0498]: cannot collect borrowed values into a Vec; map them to owned values first, for example .map(|value| *value)
  --> example-15.ls:3:41
   |
 3 |     let refs: Vec<&i64> = values.iter().collect();
   |                                         ^^^^^^^

lsaca: example-15.ls: could not compile example-15.ls due to previous error(s)
```

`fold` threads an accumulator through the items:

```ls
fn main() {
    let values = vec![1, 2, 3, 4];
    let total = values.iter().fold(0, |acc, value| acc + *value);
    println!("{}", total);

    let words = vec![String::from("a"), String::from("b"), String::from("c")];
    let joined = words
        .iter()
        .fold(String::from(""), |acc, word| acc + word.as_str());
    println!("{}", joined);
}
```

Output:
```text
10
abc
```

`count`, `sum`, `any`, `all` and `find` each consume the iterator and return a
simple result. Borrowed arithmetic usually goes through `into_iter()` so that
`sum` sees owned numbers:

```ls
fn main() {
    println!("{}", vec![1i64, 2, 3, 4, 5].iter().count());
    println!("{}", vec![1i64, 2, 3, 4, 5].into_iter().sum());
    println!("{}", vec![1.5, 2.25].into_iter().sum());

    let positive = vec![1i64, 2, 3].into_iter().any(|value| *value > 2);
    let evens = vec![2i64, 4, 6].into_iter().all(|value| *value % 2 == 0);
    println!("{} {}", positive, evens);

    let found = vec![3i64, 8, 12].into_iter().find(|value| *value > 10);
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

`min` and `max` return the smallest and largest numeric items. They need the
same owned numeric item types as `product`, return `None` for an empty
iterator, and compare with the item type's ordering:

```ls
fn main() {
    let values: Vec<i64> = vec![4, 9, 2, 7, 2];
    let smallest = values.iter().map(|value| *value).min();
    let largest = values.iter().map(|value| *value).max();
    println!("{:?} {:?}", smallest, largest);

    let empty: Vec<i64> = Vec::new();
    let none: Option<i64> = empty.into_iter().min();
    println!("{:?}", none);
}
```

Output:
```text
Some(2) Some(9)
None
```

## `for` loops over iterators

`for pattern in value` works when `value` has a concrete type implementing
`Iterator<T>`. The loop calls `next` until it returns `None`, binds the item
with the pattern, and releases the iterator when the loop ends:

```ls
fn main() {
    let values = vec![10, 20, 30];
    for value in values.iter().map(|item| *item + 1) {
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

A `for` loop over a `Vec<T>` **value** (not a reference) still moves the
vector and binds its elements by value. That is the older behaviour and it
remains the loop form of `into_iter()`. See
[Statements and control flow](08-statements-and-control-flow.md).

## Iterating a HashMap

A borrowed map iterates as `(&K, &V)` pairs. `for (key, value) in &map` is the
loop form of `map.iter()`; `map.keys()` and `map.values()` yield `&K` and
`&V` without cloning. None of these forms consumes the map, and the order is
the map's internal slot order (deterministic for a given sequence of
operations, but not insertion order):

```ls
fn main() {
    let mut table: HashMap<String, i64> = HashMap::new();
    table.insert(String::from("a"), 1);
    table.insert(String::from("bb"), 2);

    let mut total: i64 = 0;
    for (key, value) in &table {
        total += (key.len() as i64) * *value;
    }
    println!("{}", total);

    for key in table.keys() {
        println!("{}", key.as_str());
    }
    for value in table.values() {
        println!("{}", *value);
    }
    println!("{}", table.len());
}
```

Output:
```text
5
a
bb
1
2
2
```

See [HashMap](20-hashmap.md) for construction with `HashMap::from` and the
full map API.

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
ones. The borrowing constructors are specific to `Vec`, slices and
`HashMap`; a user iterator is used directly, without calling `.iter()` first:

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

The 2.4 iterator pipeline is still deliberately small. It has no:

* `size_hint` or exact-size hints; a collection cannot reserve from an
  iterator;
* double-ended iteration protocol: `rev` buffers the remaining items instead;
* `collect` into anything except `Vec<T>`, and no `Vec<&T>` results;
* iteration over arrays by value or by reference (use an index loop and
  `.iter()` is not defined on arrays);
* `Iterator` implementations for builtin container values themselves: `Vec<T>`
  and `HashMap<K, V>` are entered only through their borrowing constructors,
  while ranges keep their own `for` lowering;
* `HashSet` iteration beyond `HashSet::from` and `elements()` (which returns
  an owned `Vec<K>`).

## Known limitations in 2.4.0

* `collect` needs a known result type; a bare `collect()` without an
  annotation or turbofish is `E0282`.
* `sum` supports `i32`, `i64`, `usize` and `f64` items; `product`,
  `min` and `max` support `i32`, `i64`, `u32`, `u64`, `isize`, `usize`,
  `f32` and `f64` items. Other item types are `E0277`. Borrowed items must be
  mapped to owned values first.
* The predicates of `filter`, `take_while`, `skip_while`, `any`, `all` and
  `find` receive `&T`, as does the `inspect` closure; `map`, `filter_map`,
  `position`, `reduce` and `for_each` receive `T` by value (and `fold` folds
  `T` values with an accumulator). Over `.iter()` those reference parameters
  are references to references.
* `rev` materializes the remaining items once it is first advanced; it is not
  a streaming reverse and must not be used on an unbounded pipeline.
* Adapter names resolve only when the receiver implements `Iterator<T>` (or is
  a `Vec`, slice, or `HashMap` for the borrowing constructors). Inherent and
  trait methods win over the adapter sugar, so a user type that defines its own
  `map` method keeps it.
* Iterators are values: a chain that is never consumed performs no work, and
  dropping an unfinished iterator releases its input without running the rest
  of the elements.

See [Known limitations](limitations.md) for the full list.

## Summary

* `Iterator<T>` has one method, `next(&mut self) -> Option<T>`.
* `values.iter()` borrows a `Vec<T>` and yields `&T`, `values.iter_mut()`
  borrows it mutably and yields `&mut T`, and `values.into_iter()` consumes it
  and yields `T`.
* `for value in &values`, `for value in &mut values`, `for value in slice`
  and `for (key, value) in &map` borrow the collection for the duration of the
  loop; a `for` loop over `values` still moves the vector.
* Adapters `map`, `filter`, `take`, `skip`, `enumerate`, `zip`, `chain`,
  `filter_map`, `take_while`, `skip_while`, `step_by`, `rev` and
  `flat_map` are lazy and chainable; `enumerate` turns the item type into
  `(usize, T)`, and `count` and `position` return `usize`.
* Terminal operations are `collect`, `fold`, `count`, `sum`, `product`,
  `min`, `max`, `any`, `all`, `find`, `position`, `last`, `nth`, `reduce`,
  `inspect` and `for_each`.
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
