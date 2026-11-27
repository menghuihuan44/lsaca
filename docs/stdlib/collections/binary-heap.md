# binary_heap: BinaryHeap<T>

**Status in 2.4.0.** Implemented in `stdlib/src/collections.ls`. Version 2.4.0
adds `from` and `to_vec`.

`BinaryHeap<T>` is a maximum heap: the largest value is at the top, and the
container is the natural priority queue. Overlapping or duplicate values are
allowed. The values live in a `Vec` in the heap's internal order, the layout
that keeps `push` and `pop` cheap. That order is not a sorted sequence and is
not part of the API contract; use `pop` to read values from largest to smallest.

## Surface

| Method | Signature | Meaning |
| --- | --- | --- |
| `new` | `() -> BinaryHeap<T>` | an empty heap |
| `from` | `(Vec<T>) -> BinaryHeap<T>` | builds a heap from owned values (2.4.0) |
| `push` | `(&mut self, T)` | adds a value |
| `pop` | `(&mut self) -> Option<T>` | removes and returns the largest value |
| `peek` | `(&self) -> Option<T>` | a clone of the largest value, without removing it |
| `len` | `(&self) -> usize` | number of values |
| `is_empty` | `(&self) -> bool` | whether the heap is empty |
| `clear` | `(&mut self)` | removes every value |
| `elements` | `(&self) -> Vec<T>` | the values in internal heap order, not sorted |
| `to_vec` | `(&self) -> Vec<T>` | a copy of the internal storage in heap order, not sorted (2.4.0) |

## Ordering and types

* The element types are `i64`, `u64`, `f64` and `String`. The heap is a
  maximum heap: `peek` and `pop` return the greatest element under the
  language's `>` comparison - numbers by value, `String` in byte order (code
  point order for UTF-8).
* `f64` elements follow the language's float comparison rules; the position of
  `NaN` values in the heap order is unspecified, so do not push `NaN`.
* The heap property is the only ordering guarantee: `peek` and `pop` return the
  greatest value, while `elements()` and `to_vec()` expose the internal layout in
  whatever order it happens to have. Do not rely on that order; `pop` in a loop
  yields the values from largest to smallest.
* Lengths are `usize` since 2.1.0.

## Limitations in 2.4.0

There is no `for` loop over a `BinaryHeap` yet: read the values with
`elements()` or `to_vec()` and remember that the order is internal, or `pop` them
in descending order. `peek` returns `Option<T>` (a clone), not `Option<&T>`.
There is no min-heap type, no `into_sorted_vec` and no `heapify` constructor.

## See also

* [collections index](README.md), [`Vec`](../vec.md),
  [VecDeque](deque.md).
* [Operators](../../language/07-operators.md) - comparison rules.
