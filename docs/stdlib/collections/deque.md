# deque: VecDeque<T>

**Status in 2.1.0.** Implemented in `stdlib/src/collections.ls`.

`VecDeque<T>` is a double-ended queue: a growable `Vec<T>` with values ordered
from front to back, so callers can add and remove at both ends. It is the
container for queues and stacks that must also push at the front. The current
implementation is a plain `Vec`; the front operations shift the stored elements.

## Surface

| Method | Signature | Meaning |
| --- | --- | --- |
| `new` | `() -> VecDeque<T>` | an empty deque |
| `push_back` | `(&mut self, T)` | inserts at the back |
| `push_front` | `(&mut self, T)` | inserts at the front |
| `pop_front` | `(&mut self) -> Option<T>` | removes and returns the front value |
| `pop_back` | `(&mut self) -> Option<T>` | removes and returns the back value |
| `len` | `(&self) -> usize` | number of values |
| `is_empty` | `(&self) -> bool` | whether the deque is empty |
| `clear` | `(&mut self)` | removes every value |
| `get` | `(&self, usize) -> Option<T>` | a clone of the value at a position counted from the front |
| `elements` | `(&self) -> Vec<T>` | the values from front to back |

## Notes

* The container is a managed value type like `Vec`: it owns its elements, is
  move-only, and releases them when dropped. `get` clones the value, so `T` must
  be cloneable when it is called.
* `push_front` inserts at index `0` and `pop_front` removes index `0`, so front
  operations cost linear time: every other element shifts by one position. Back
  operations are amortized constant time.
* `get` takes a `usize` index counted from the front and returns `None` when the
  index is past the end; there is no indexing operator. (In 2.0.0 the index was
  `i64`; 2.1.0 made every position in the standard library `usize`.)
* `VecDeque` has no `sort` of its own; collect into a `Vec` first and use
  `Vec::sort`.

## Limitations in 2.1.0

There is no `for` loop over a `VecDeque` yet: read the values with `elements()`.
There is no `with_capacity`, no `front`/`back` inspection (use `get(0)` and
`get(len - 1)`), and no ring-buffer layout; the ring representation is planned
for a later release.

## See also

* [collections index](README.md), [`Vec`](../vec.md),
  [BinaryHeap](binary-heap.md).
