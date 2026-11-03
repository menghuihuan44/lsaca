# vec: Vec<T>

**Status in 2.0.0.** The `Vec<T>` type and its 1.7 methods are migrated from the
builtin prelude into the `std` crate (`std::vec`) without surface changes. `sort`,
`as_slice` and `as_mut_slice` are implemented in `stdlib/src/vec.ls`; the methods
listed under *Planned* are not implemented in 2.0.0 and are planned for a later
release.

`Vec<T>` is the growable array type. It owns its elements, is move-only, and
releases the elements when it is dropped; `.clone()` produces an independent copy
when `T` is cloneable. A `for` loop over a `Vec` consumes it (see
[Ownership and moves](../language/11-ownership-and-moves.md)).

## Migrated API

| Method | Signature | Meaning |
| --- | --- | --- |
| `Vec::new` | `() -> Vec<T>` | empty vector |
| `Vec::with_capacity` | `(i64) -> Vec<T>` | empty vector with reserved room |
| `len` / `is_empty` | → `i64` / `bool` | number of elements / emptiness |
| `push` | `(&mut self, T)` | appends a value |
| `pop` | `(&mut self) -> Option<T>` | removes the last element |
| `insert` | `(&mut self, i64, T)` | inserts at an index |
| `remove` | `(&mut self, i64) -> T` | removes at an index, returns the element |
| `clear` | `(&mut self)` | removes every element |
| `first` / `last` | → `Option<T>` | first/last element |
| `get` | `(i64) -> Option<T>` | element by index |
| `extend` | `(&mut self, Vec<T>)` | appends every element of another vector |
| `reverse` | `(&mut self)` | reverses in place |
| `v[i]` / `v[i] = value` | index | element access; panics when out of bounds |

The full signatures, complexity notes and examples are in the
[Standard library chapter](../language/15-standard-library.md#vec). In 2.0.0 a
`Vec<T>` also coerces to `&[T]` and `&mut [T]` at call boundaries, and
`as_slice` / `as_mut_slice` return those slices directly; see
[Slices and str](../language/26-slices-and-str.md).

## Implemented additions

| Item | Signature | Notes |
| --- | --- | --- |
| `sort` | `(&mut self)` | sorts in place for every element type that supports `>` comparisons: the integer types, `f64`, `bool`, `char` and `String` |
| `as_slice` | `(&self) -> &[T]` | borrows the elements as `&[T]` without copying |
| `as_mut_slice` | `(&mut self) -> &mut [T]` | mutable slice over the elements; requires a unique borrow |

`sort` is implemented in `stdlib/src/vec.ls` as a small shell sort driven by `>`
comparisons; it is not stable, takes no comparator and is not locale-aware. There is
no operator overloading, so sorting a user-defined type still needs `sort_by` with
an explicit comparison, which is not implemented in 2.0.0. `as_slice` and `as_mut_slice` produce the fat
references described in [Slices and str](../language/26-slices-and-str.md); slice
methods such as `len` are implemented for `[T]` in the `std` crate.

## Planned for a later release

| Item | Signature | Notes |
| --- | --- | --- |
| `retain` | `(&mut self, Fn(&T) -> bool)` | keeps elements satisfying the predicate; removes the rest and drops them |
| `extend_from_slice` | `(&mut self, &[T])` | appends copies of a slice; `T` must be `Copy` |
| `sort_by` | `(&mut self, Fn(&T, &T) -> i64)` | sorts with a comparator returning negative/zero/positive |
| `iter` | `(self) -> VecIter<T>` | migrated, stays a consuming iterator |
| slice iterators | - | iteration over `&[T]` is not implemented in 2.0.0; `[T]` only has `len` and `is_empty` |

## See also

* [Iterators](iter.md) and [Slices and str](../language/26-slices-and-str.md).
* [Standard library chapter](../language/15-standard-library.md#vec) - full 1.7
  reference.
