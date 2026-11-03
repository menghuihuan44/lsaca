# binary_heap: BinaryHeap<T>

**Status in 2.0.0.** Implemented in `stdlib/src/collections.ls`.

`BinaryHeap<T>` is a maximum heap: the largest value is at the top, and the
container is the natural priority queue. Overlapping or duplicate values are
allowed. The current implementation keeps its elements in a `Vec` sorted from
largest to smallest rather than in a tree layout.

## Surface

| Method | Signature | Meaning |
| --- | --- | --- |
| `new` | `() -> BinaryHeap<T>` | an empty heap |
| `push` | `(&mut self, T)` | adds a value |
| `pop` | `(&mut self) -> Option<T>` | removes and returns the largest value |
| `peek` | `(&self) -> Option<T>` | a clone of the largest value, without removing it |
| `len` | `(&self) -> i64` | number of values |
| `is_empty` | `(&self) -> bool` | whether the heap is empty |
| `clear` | `(&mut self)` | removes every value |
| `elements` | `(&self) -> Vec<T>` | the values from largest to smallest |

## Ordering and types

* The element types are `i64`, `u64`, `f64` and `String`. The heap is a
  `max` heap: `peek` and `pop` return the greatest element under the language's
  `>` comparison - numbers by value, `String` in byte order (code point order
  for UTF-8).
* `f64` elements follow the language's float comparison rules; the position of
  `NaN` values in the heap order is unspecified, so do not push `NaN`.
* `push` scans the vector to find the insertion position and `pop` removes index
  `0`, so both cost linear time in the number of values. The values stay sorted,
  which makes `elements()` a sorted read.

## Limitations in 2.0.0

There is no `for` loop over a `BinaryHeap` yet: read the values with
`elements()`; `peek` returns `Option<T>` (a clone), not `Option<&T>`. There
is no min-heap type, no `into_sorted_vec` and no `heapify` constructor; the
heap-array layout is planned for a later release.

## See also

* [collections index](README.md), [`Vec`](../vec.md),
  [VecDeque](deque.md).
* [Operators](../../language/07-operators.md) - comparison rules.
