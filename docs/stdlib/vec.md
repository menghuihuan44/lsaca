# vec: Vec<T>

**Status in 2.4.0.** Implemented. `Vec<T>` and its methods live in
`stdlib/src/vec.ls` (`std::vec`); capacities, lengths and indices are `usize`
since 2.1.0. The slice methods `len`, `is_empty`, `get`, `get_mut`, `first_mut`
and `last_mut` are implemented for `[T]` in the same file. Version 2.3.0 adds
borrowing iteration: `iter`, `iter_mut` and `into_iter`, and `for` loops over
`&Vec<T>`, `&mut Vec<T>`, `&[T]` and `&mut [T]`.

`Vec<T>` is the growable array type. It owns its elements, is move-only, and
releases the elements when it is dropped; `.clone()` produces an independent copy
when `T` is cloneable. A `for` loop over a `Vec` value consumes it; a loop over
`&vec` or `&mut vec` borrows it (see
[Ownership and moves](../language/11-ownership-and-moves.md)).

## Method reference

| Method | Signature | Meaning |
| --- | --- | --- |
| `Vec::new` | `() -> Vec<T>` | empty vector |
| `Vec::with_capacity` | `(usize) -> Vec<T>` | empty vector with reserved room |
| `len` / `is_empty` | `(&self) -> usize` / `(&self) -> bool` | number of elements / emptiness |
| `push` | `(&mut self, T)` | appends a value |
| `pop` | `(&mut self) -> Option<T>` | removes the last element |
| `insert` | `(&mut self, usize, T)` | inserts at an index |
| `remove` | `(&mut self, usize) -> T` | removes at an index, returns the element |
| `clear` | `(&mut self)` | removes every element |
| `first` / `last` | `(&self) -> Option<T>` | first/last element |
| `get` | `(&self, usize) -> Option<T>` | element by index |
| `get_mut` | `(&mut self, usize) -> Option<&mut T>` | mutable reference to the element at an index (2.2.0) |
| `first_mut` / `last_mut` | `(&mut self) -> Option<&mut T>` | mutable reference to the first/last element (2.2.0) |
| `iter` | `(&self) -> VecIterRef<T>` | borrowed iterator yielding `&T` (2.3.0) |
| `iter_mut` | `(&mut self) -> VecIterMut<T>` | borrowed mutable iterator yielding `&mut T` (2.3.0) |
| `into_iter` | `(self) -> VecIter<T>` | consuming iterator yielding `T` (2.3.0) |
| `extend` | `(&mut self, Vec<T>)` | appends every element of another vector |
| `reverse` | `(&mut self)` | reverses in place |
| `truncate` | `(&mut self, usize)` | keeps the first `n` elements and drops the rest |
| `swap` | `(&mut self, usize, usize)` | swaps two elements |
| `contains` | `(&self, &T) -> bool` | whether an equal element is present |
| `sort` | `(&mut self)` | sorts in place for integer, float, `bool`, `char` and `String` elements |
| `as_slice` / `as_mut_slice` | `(&self) -> &[T]` / `(&mut self) -> &mut [T]` | borrows the elements as a slice |
| `v[i]` / `v[i] = value` | index | element access; panics when out of bounds |

A `Vec<T>` also coerces to `&[T]` and `&mut [T]` at call boundaries. A `for`
loop over `vec` consumes the vector, while `for value in &vec` and
`for value in &mut vec` borrow it and yield `&T` / `&mut T` items; a `&[T]`
or `&mut [T]` slice can be iterated the same way. See
[Slices and str](../language/26-slices-and-str.md) and
[Iterators](../language/19-iterators.md).

The three borrowed accessors return an `Option<&mut T>`. A `match` or `if let`
can use the call directly as its scrutinee; the mutable borrow lasts for the
whole match, and assigning through the bound reference (`*value = ...`) changes
the element in place. Slices support the same accessors: `[T]` has `len`,
`is_empty`, `get`, `get_mut`, `first_mut` and `last_mut`.

`sort` is a small shell sort driven by `>` comparisons; it is not stable and
takes no comparator, so sorting a user-defined type needs `sort_by`, which is not
implemented. `contains` compares through `==` and therefore needs an element type
that supports equality.

## Planned for a later release

| Item (planned) | Signature | Notes |
| --- | --- | --- |
| `retain` | `(&mut self, Fn(&T) -> bool)` | keeps elements satisfying the predicate |
| `extend_from_slice` | `(&mut self, &[T])` | appends copies of a slice; `T` must be `Copy` |
| `sort_by` | `(&mut self, Fn(&T, &T) -> i64)` | sorts with a comparator |

## See also

* [Iterators](iter.md) and [Slices and str](../language/26-slices-and-str.md).
* [Standard library chapter](../language/15-standard-library.md#vec) - the
  language-level reference.
