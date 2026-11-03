# iter: iterators and adapters

**Status in 2.0.0.** The iterator trait, the six 1.7 adapters and the terminal
operations are migrated from the builtin prelude into the `std` crate (`std::iter`)
without surface changes. The adapters listed under *Planned* are not implemented in
2.0.0 and are planned for a later release.

An iterator is a value implementing `Iterator<T>` (see [core](core.md)). The trait is
an ordinary trait: user types can implement it, and generic code can bound on it
(see [Iterators](../language/19-iterators.md)).

## Migrated API

| Item | Signature | Meaning |
| --- | --- | --- |
| `.iter()` | `(self) -> VecIter<T>` | consumes a `Vec<T>` and returns an iterator over its elements |
| `map` | `(Fn(T) -> U) -> Map<T, U>` | transforms every item |
| `filter` | `(Fn(&T) -> bool) -> Filter<T>` | keeps items for which the predicate is true |
| `take` / `skip` | `(i64) -> Take<T> / Skip<T>` | first `n` items / all but the first `n` items |
| `enumerate` | `() -> Enumerate<T>` | yields `(i64, T)` pairs |
| `collect` | `() -> Vec<T>` | collects; the target type must be known |
| `fold` | `(U, Fn(U, T) -> U) -> U` | folds from an initial value |
| `count` | `() -> i64` | number of items |
| `sum` | `() -> i64` or `f64` | sums numeric items |
| `any` / `all` | `(Fn(&T) -> bool) -> bool` | short-circuit tests |
| `find` | `(Fn(&T) -> bool) -> Option<T>` | first matching item |

Adapters are lazy: they do nothing until a terminal operation or a `for` loop
consumes the iterator. `take` and `skip` take an `i64`; the hidden iterator structs
are ordinary generic values, so they can be stored in locals and passed to generic
functions. `for` over an iterator is a language feature; see
[Iterators](../language/19-iterators.md#for-loops-over-iterators).

## Planned in 2.0

| Adapter (planned) | Signature | Notes |
| --- | --- | --- |
| `zip` | `(R) -> Zip<T, U>` | pairs items until either side ends |
| `chain` | `(R) -> Chain<T>` | yields the first iterator, then the second |
| `rev` | `() -> Rev<T>` | iterates backwards; requires a reversible source |
| `flat_map` | `(Fn(T) -> R) -> FlatMap<T, U>` | maps to an iterator and concatenates |
| `VecIter::next` | `(&mut self) -> Option<T>` | same as 1.7 |
| slice iterators | `&[T]` and `&str` | not implemented in 2.0.0; planned for a later release |

The fusion of adapter chains into a single loop is a compiler optimisation in 2.0
and does not change this surface.

## What is not provided

* `for_each`, `size_hint`, `iter_mut`, `into_iter` and double-ended iteration.
* Iteration over `HashMap` (use `keys()` or `values()` and index the result).
* Iteration over `&Vec<T>`: `.iter()` consumes the vector, and iterating a borrowed
  vector directly is rejected with `E0277`.

## See also

* [Iterators](../language/19-iterators.md) - language-level guide.
* [vec](vec.md) and [collections](collections/README.md).
