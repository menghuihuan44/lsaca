# collections: maps, sets, queues and heaps

The `std::collections` module groups the containers that are not built into the
language: `HashMap` (available since 1.5 and migrated to the `std` crate in 2.0.0)
and the ordered and specialized containers implemented in 2.0.0: `HashSet`,
`BTreeSet`, `BTreeMap`, `VecDeque` and `BinaryHeap`.

## Containers

| Container | Page | Status in 2.0.0 | Iteration order | Key or element types |
| --- | --- | --- | --- | --- |
| `HashMap<K, V>` | [hash_map](hash-map.md) | API migrated from 1.7 | internal slot order, not insertion order | `i64`, `u64`, `isize`, `usize`, `bool`, `String`, `char` keys |
| `HashSet<T>` | [hash_set](hash-set.md) | implemented | insertion order, deterministic, read with `elements()` / `to_vec()` | same keys as `HashMap` |
| `BTreeSet<T>` | [btree_set](btree-set.md) | implemented | ascending key order, read with `elements()` | `i64`, `u64`, `isize`, `usize`, `char`, `String` |
| `BTreeMap<K, V>` | [btree_map](btree-map.md) | implemented | ascending key order, read with `keys()` / `values()` | same keys as `BTreeSet` |
| `VecDeque<T>` | [deque](deque.md) | implemented | front to back, read with `elements()` | any value type |
| `BinaryHeap<T>` | [binary_heap](binary-heap.md) | implemented | largest to smallest, read with `elements()` | `i64`, `u64`, `f64`, `String` |

"API migrated from 1.7" means the surface is the one described in
[`HashMap`](../../language/20-hashmap.md); the implementation moved into the
`std` crate on top of compiler intrinsics, so programs do not change.
"Implemented" means the container is written in lsaca in `stdlib/src/collections.ls`
and has been exercised against the 2.0.0 compiler. None of the new containers can be
read with a `for` loop yet: use the accessors named in the iteration-order column,
which return a `Vec`.

## Design rules

The containers share a few design rules that matter to programs:

* All of them are ordinary managed value types, like `Vec`: they own their
  elements, are move-only, are released when dropped, and `.clone()` produces an
  independent copy when the element type is cloneable (see [Ownership and
  moves](../../language/11-ownership-and-moves.md)).
* Iteration order is part of the documented API and is deterministic: the ordered
  containers keep a sorted `Vec` and scan it with linear comparisons, the
  insertion-ordered set keeps its keys in a `Vec` alongside a `HashMap`, the
  double-ended queue keeps its values in a `Vec` ordered from front to back, and
  the binary heap keeps its values in a `Vec` sorted from largest to smallest.
* Set operations `union`, `intersection`, `difference` and `subset` are provided
  by `HashSet`; `BTreeSet` does not have them yet.
* `HashMap` keeps the restrictions it had in 1.7: a fixed set of key types and no
  `for` loop over the map.

## Planned for a later release

All six containers are available. Not implemented in 2.0.0 and planned for a
later release: a `for` loop or iterator over the new containers (use the
`elements` / `to_vec` / `keys` / `values` accessors for now), a ring-buffer
layout for `VecDeque`, a heap-array layout for `BinaryHeap`, and set algebra for
`BTreeSet`. There is no linked list, no `BTree` cursor API, no array map and no
concurrent container in 2.0.

## See also

* [Standard library reference](../README.md) - the module index.
* [Standard library chapter](../../language/15-standard-library.md#hashmap) and
  [`HashMap`](../../language/20-hashmap.md).
* [Iterators](../../language/19-iterators.md) and
  [Slices and str](../../language/26-slices-and-str.md).
