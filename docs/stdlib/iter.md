# iter: iterators and adapters

**Status in 2.2.0.** Implemented. `Iterator<T>`, the adapters and the terminal
operations live in `stdlib/src/iter.ls` (`std::iter`) and are re-exported
through the prelude, so they need no import. Version 2.1.0 adds `zip`, `chain`,
`filter_map`, `take_while`, `skip_while`, `step_by` and `inspect`, and the
`usize` counts and positions described below.

An iterator is a value implementing `Iterator<T>` (see [core](core.md)). The trait is
an ordinary trait: user types can implement it, and generic code can bound on it
(see [Iterators](../language/19-iterators.md)).

## Adapters

| Adapter | Signature | Meaning |
| --- | --- | --- |
| `.iter()` | `(self) -> VecIter<T>` | consumes a `Vec<T>` and iterates its elements |
| `map` | `(Fn(T) -> U) -> Map<T, U>` | transforms every item |
| `filter` | `(Fn(&T) -> bool) -> Filter<T>` | keeps items for which the predicate is true |
| `filter_map` | `(Fn(T) -> Option<U>) -> FilterMap<T, U>` | transforms and drops the `None` results |
| `take` / `skip` | `(usize)` | first `n` items / all but the first `n` items |
| `take_while` | `(Fn(&T) -> bool)` | items while the predicate holds |
| `skip_while` | `(Fn(&T) -> bool)` | all items after the first predicate failure |
| `step_by` | `(usize)` | every `n`-th item |
| `zip` | `(R) -> Zip<T, U>` | pairs items until either side ends |
| `chain` | `(R) -> Chain<T>` | yields the first iterator, then the second |
| `enumerate` | `() -> Enumerate<T>` | yields `(usize, T)` pairs |
| `inspect` | `(Fn(&T)) -> Inspect<T>` | calls the closure on each item as it passes |

Adapters are lazy: they do nothing until a terminal operation or a `for` loop
consumes the iterator. The hidden iterator structs are ordinary generic values, so
they can be stored in locals and passed to generic functions.

## Terminal operations

| Operation | Signature | Meaning |
| --- | --- | --- |
| `collect` | `() -> Vec<T>` | collects; the target type must be known |
| `fold` | `(U, Fn(U, T) -> U) -> U` | folds from an initial value |
| `reduce` | `(Fn(T, T) -> T) -> Option<T>` | folds without an initial value |
| `count` | `() -> usize` | number of items |
| `sum` | `() -> T` | sums numeric items; `T` must be `i32`, `i64`, `usize` or `f64` (`E0277` otherwise) |
| `product` | `() -> T` | multiplies numeric items; same item types as `sum` |
| `any` / `all` | `(Fn(&T) -> bool) -> bool` | short-circuit tests |
| `find` | `(Fn(&T) -> bool) -> Option<T>` | first matching item |
| `position` | `(Fn(T) -> bool) -> Option<usize>` | index of the first matching item |
| `last` | `() -> Option<T>` | the last item |
| `nth` | `(usize) -> Option<T>` | the n-th item, consuming the prefix |
| `for_each` | `(Fn(T))` | runs the closure on every item |

`for` over an iterator is a language feature; see
[Iterators](../language/19-iterators.md#for-loops-over-iterators).

## Planned for a later release

* `rev` (reverse iteration) and `flat_map` are not implemented.
* `size_hint`, `iter_mut`, `into_iter` and double-ended iteration are not
  provided.
* Iteration over `HashMap` (use `keys()` and index the result) and over
  `&Vec<T>` are not provided; `.iter()` consumes the vector.
* Slice iterators over `&[T]` and `&str` are not implemented.

## See also

* [Iterators](../language/19-iterators.md) - language-level guide.
* [vec](vec.md), [core](core.md) and [collections](collections/README.md).
