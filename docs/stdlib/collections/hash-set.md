# hash_set: HashSet<T>

**Status in 2.0.0.** Implemented in `stdlib/src/collections.ls`.

`HashSet<T>` is an insertion-ordered set: it stores its keys in a
`HashMap<T, ()>` for membership and additionally keeps a `Vec<T>` of the keys in
insertion order, so iteration is deterministic and reflects the order in which
values were first inserted. This differs from `HashMap`, whose iteration order is
internal slot order.

## Surface

| Method | Signature | Meaning |
| --- | --- | --- |
| `new` | `() -> HashSet<T>` | an empty set |
| `insert` | `(&mut self, T) -> bool` | adds a value; reports whether it was newly added |
| `contains` | `(&self, T) -> bool` | whether the set holds a value; the argument is moved or copied |
| `remove` | `(&mut self, T) -> bool` | removes a value; reports whether it was present |
| `len` | `(&self) -> i64` | number of values |
| `is_empty` | `(&self) -> bool` | whether the set is empty |
| `clear` | `(&mut self)` | removes every value |
| `elements` | `(&self) -> Vec<T>` | the values in insertion order |
| `to_vec` | `(&self) -> Vec<T>` | the same vector as `elements` |
| `union` | `(&self, &HashSet<T>) -> HashSet<T>` | values in either set |
| `intersection` | `(&self, &HashSet<T>) -> HashSet<T>` | values in both sets |
| `difference` | `(&self, &HashSet<T>) -> HashSet<T>` | values in this set but not the other |
| `subset` | `(&self, &HashSet<T>) -> bool` | whether every value of this set is in the other |

## Rules

* The element type is restricted to the `HashMap` key types: `i64`, `u64`,
  `isize`, `usize`, `bool`, `String` and `char`.
* Inserting a value that is already present leaves the set unchanged (duplicates
  collapse); the set owns one copy of each value.
* The insertion order is part of the API: reading the set is always the same
  sequence, and it does not depend on hashing.
* `remove` rebuilds the insertion-order vector, so it costs linear time in the
  number of values; the hash lookup itself is constant time on average.

## Limitations in 2.0

There is no `for` loop over a `HashSet` yet: read the values with `elements()`
or `to_vec()` and iterate the resulting `Vec`. There is no `with_capacity`, no
iterator type and no set equality operator.

## See also

* [collections index](README.md), [`HashMap`](hash-map.md),
  [BTreeSet](btree-set.md).
* [`HashMap` chapter](../../language/20-hashmap.md).
