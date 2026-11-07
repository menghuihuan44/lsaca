# btree_set: BTreeSet<T>

**Status in 2.2.0.** Implemented in `stdlib/src/collections.ls`.

`BTreeSet<T>` is the ordered set: it stores its elements in a sorted `Vec<T>`
and finds them by scanning the vector in order. Reading the set is in ascending
key order, so sets of numbers, characters and strings come out sorted without a
separate sort step. (Despite the name, the implementation is a sorted vector,
not a tree; the name is kept for familiarity.)

## Surface

| Method | Signature | Meaning |
| --- | --- | --- |
| `new` | `() -> BTreeSet<T>` | an empty set |
| `insert` | `(&mut self, T) -> bool` | adds a value; reports whether it was newly added |
| `contains` | `(&self, T) -> bool` | whether the set holds a value; the argument is moved or copied |
| `remove` | `(&mut self, T) -> bool` | removes a value; reports whether it was present |
| `len` | `(&self) -> usize` | number of values |
| `is_empty` | `(&self) -> bool` | whether the set is empty |
| `clear` | `(&mut self)` | removes every value |
| `elements` | `(&self) -> Vec<T>` | the values in ascending order |
| `first` | `(&self) -> Option<T>` | the smallest value, or `None` |
| `last` | `(&self) -> Option<T>` | the largest value, or `None` |

## Ordering and types

* Keys are ordered with the language's comparison rules: numbers by value,
  `char` by Unicode scalar value, and `String` in byte order (which for UTF-8 is
  code point order).
* The key types are the integer types (including the default `i32`), `char` and
  `String`. `bool` is not an ordered type (the comparison operators are not
  defined for it), and floats are not used as keys because of `NaN`.
* `insert`, `contains` and `remove` scan the sorted vector with linear
  comparisons, so they cost linear time in the worst case; the values are kept
  sorted, which makes ordered reading cheap and needs no pointer chasing. Binary
  search is not used yet.
* Lengths are `usize` since 2.1.0.

## Limitations in 2.2.0

There is no `for` loop over a `BTreeSet` yet: read the values with `elements()`.
Set algebra (`union`, `intersection`, `difference`, `subset`) exists only on
[HashSet](hash-set.md); it is not provided for `BTreeSet`. There is no range
query, no cursor and no `BTreeSet`-specific iterator API.

## See also

* [collections index](README.md), [BTreeMap](btree-map.md),
  [HashSet](hash-set.md).
* [Operators](../../language/07-operators.md) - comparison rules.
