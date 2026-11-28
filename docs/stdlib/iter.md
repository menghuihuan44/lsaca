# iter: iterators and adapters

**Status in 2.5.0.** Implemented. `Iterator<T>`, the adapters and the terminal
operations live in `stdlib/src/iter.ls` (`std::iter`) and are re-exported
through the prelude, so they need no import. Version 2.3.0 added borrowed
iteration (`iter`, `iter_mut`, `into_iter`, slice iterators), the `rev` and
`flat_map` adapters, and borrowed `HashMap` iteration. Version 2.4.0 adds the
`min` and `max` terminals and widens `product` to every numeric item type
accepted by `min`/`max`.

An iterator is a value implementing `Iterator<T>` (see [core](core.md)). The trait is
an ordinary trait: user types can implement it, and generic code can bound on it
(see [Iterators](../language/19-iterators.md)).

## Getting an iterator

| Constructor | Signature | Meaning |
| --- | --- | --- |
| `Vec::iter` | `(&self) -> VecIterRef<T>` | borrows the vector and yields `&T`; the vector stays usable |
| `Vec::iter_mut` | `(&mut self) -> VecIterMut<T>` | mutably borrows the vector and yields `&mut T` |
| `Vec::into_iter` | `(self) -> VecIter<T>` | consumes the vector and yields `T` |
| slice `.iter` / `.iter_mut` | `(&[T])` / `(&mut [T])` | yields `&T` / `&mut T` from a slice |
| `HashMap::iter` | `(&self) -> HashMapIter<K, V>` | yields `(&K, &V)` pairs without cloning |
| `HashMap::keys` / `values` | `(&self)` | yields `&K` / `&V` without cloning |

A `for` loop can also take the borrowed collection directly:
`for value in &vec`, `for value in &mut vec`, `for value in &slice`,
`for value in &mut slice` and `for (key, value) in &map`. The loop borrows the
collection for its duration and leaves it owned by the original binding.

## Adapters

| Adapter | Signature | Meaning |
| --- | --- | --- |
| `map` | `(Fn(T) -> U) -> Map<T, U>` | transforms every item |
| `filter` | `(Fn(&T) -> bool) -> Filter<T>` | keeps items for which the predicate is true |
| `filter_map` | `(Fn(T) -> Option<U>) -> FilterMap<T, U>` | transforms and drops the `None` results |
| `take` / `skip` | `(usize)` | first `n` items / all but the first `n` items |
| `take_while` | `(Fn(&T) -> bool)` | items while the predicate holds |
| `skip_while` | `(Fn(&T) -> bool)` | all items after the first predicate failure |
| `step_by` | `(usize)` | every `n`-th item |
| `zip` | `(R) -> Zip<T, U>` | pairs items until either side ends |
| `chain` | `(R) -> Chain<T>` | yields the first iterator, then the second |
| `enumerate` | `() -> Enumerate<T>` | yields `(usize, T)` pairs; over a borrowed chain the item is a reference |
| `inspect` | `(Fn(&T)) -> Inspect<T>` | calls the closure on each item as it passes |
| `rev` | `() -> Rev<T>` | yields the remaining items in reverse order |
| `flat_map` | `(Fn(T) -> Vec<U> or Iterator<U>)` | maps every item to a sequence and yields it item by item |

Adapters are lazy: they do nothing until a terminal operation or a `for` loop
consumes the iterator. `rev` is lazy at construction but must buffer the
remaining items the first time it is advanced, because an iterator is
forward-only. The hidden iterator structs are ordinary generic values, so they
can be stored in locals and passed to generic functions.

## Terminal operations

| Operation | Signature | Meaning |
| --- | --- | --- |
| `collect` | `() -> Vec<T>` | collects; the target type must be known |
| `fold` | `(U, Fn(U, T) -> U) -> U` | folds from an initial value |
| `reduce` | `(Fn(T, T) -> T) -> Option<T>` | folds without an initial value |
| `count` | `() -> usize` | number of items |
| `sum` | `() -> T` | sums numeric items; `T` must be `i32`, `i64`, `usize` or `f64` (`E0277` otherwise) |
| `product` | `() -> T` | multiplies numeric items; since 2.4.0 `T` may be any numeric type accepted by `min` and `max` |
| `min` | `() -> Option<T>` | the smallest numeric item; empty gives `None`; ties keep the first occurrence |
| `max` | `() -> Option<T>` | the largest numeric item; empty gives `None`; ties keep the last occurrence |
| `any` / `all` | `(Fn(&T) -> bool) -> bool` | short-circuit tests |
| `find` | `(Fn(&T) -> bool) -> Option<T>` | first matching item |
| `position` | `(Fn(T) -> bool) -> Option<usize>` | index of the first matching item |
| `last` | `() -> Option<T>` | the last item |
| `nth` | `(usize) -> Option<T>` | the n-th item, consuming the prefix |
| `for_each` | `(Fn(T))` | runs the closure on every item |

`sum` accepts `i32`, `i64`, `usize` and `f64` items. `product`, `min` and
`max` accept `i32`, `i64`, `u32`, `u64`, `isize`, `usize`, `f32` and `f64`
items. Any other item type is `E0277`. A borrowed vector has item type `&T`,
so map to owned values first. `min` and `max` need the item type to support the
comparison used by the numeric helper; the eight types above are the supported
numeric types.

`collect` needs a known `Vec<T>` target type and rejects borrowed items such as
`Vec<&T>` with `E0498`: map them to owned values first, for example
`.map(|value| *value)`. `sum` and `product` likewise need owned numeric items.
Over a borrowed chain the predicate and `find` closures receive a reference to
the item, so dereference as needed.

```ls
fn main() {
    let values: Vec<i64> = vec![4, 9, 2, 7, 2];
    println!("{:?} {:?}", values.iter().map(|value| *value).min(), values.iter().map(|value| *value).max());
    println!("{:?}", values.iter().map(|value| *value).product());
}
```

Output:
```text
Some(2) Some(9)
1008
```

An empty iterator has no minimum or maximum, so both terminals return `None`;
`product` of an empty iterator is the multiplicative identity `1`. The tie rule
mirrors Rust: `min` keeps the first smallest item and `max` keeps the last
largest item.

`for` over an iterator is a language feature; see
[Iterators](../language/19-iterators.md#for-loops-over-iterators).

## Not provided

* `size_hint` and double-ended iteration.
* `collect` into anything but `Vec<T>`, and collecting borrowed items.
* Iterators over `&str`; use `.chars()` or byte access instead.

## See also

* [Iterators](../language/19-iterators.md) - language-level guide.
* [vec](vec.md), [core](core.md) and [collections](collections/README.md).
