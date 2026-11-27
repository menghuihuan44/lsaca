# deque: VecDeque<T>

**Status in 2.4.0.** Implemented in `stdlib/src/collections.ls`. Version 2.4.0
adds `with_capacity`, `from`, `get_mut`, `front`, `back` and `to_vec`.

`VecDeque<T>` is a double-ended queue: a growable `Vec<T>` with values ordered
from front to back, so callers can add and remove at both ends. It is the
container for queues and stacks that must also push at the front. The current
implementation is a plain `Vec`; the front operations shift the stored elements.

## Surface

| Method | Signature | Meaning |
| --- | --- | --- |
| `new` | `() -> VecDeque<T>` | an empty deque |
| `with_capacity` | `(usize) -> VecDeque<T>` | an empty deque with room for at least that many values (2.4.0) |
| `from` | `(Vec<T>) -> VecDeque<T>` | builds a deque from values ordered front to back (2.4.0) |
| `push_back` | `(&mut self, T)` | inserts at the back |
| `push_front` | `(&mut self, T)` | inserts at the front |
| `pop_front` | `(&mut self) -> Option<T>` | removes and returns the front value |
| `pop_back` | `(&mut self) -> Option<T>` | removes and returns the back value |
| `len` | `(&self) -> usize` | number of values |
| `is_empty` | `(&self) -> bool` | whether the deque is empty |
| `clear` | `(&mut self)` | removes every value |
| `get` | `(&self, usize) -> Option<T>` | a clone of the value at a position counted from the front |
| `get_mut` | `(&mut self, usize) -> Option<&mut T>` | a mutable reference to the value at a position counted from the front (2.4.0) |
| `front` / `back` | `(&self) -> Option<T>` | clones of the front / back value (2.4.0) |
| `to_vec` | `(&self) -> Vec<T>` | the same vector as `elements`, front to back (2.4.0) |
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
* `get_mut` returns a reference into the backing vector, so the deque stays
  mutably borrowed until the reference's last use. `front` and `back` clone
  their value, like `get`.
* `VecDeque` has no `sort` of its own; collect into a `Vec` first and use
  `Vec::sort`.

## Limitations in 2.4.0

There is no `for` loop over a `VecDeque` yet: read the values with `elements()`
or `to_vec()`. There is no iterator type and no ring-buffer layout; the ring
representation is planned for a later release.

## See also

* [collections index](README.md), [`Vec`](../vec.md),
  [BinaryHeap](binary-heap.md).
