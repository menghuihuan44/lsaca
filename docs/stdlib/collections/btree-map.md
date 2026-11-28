# btree_map: BTreeMap<K, V>

**Status in 2.5.0.** Implemented in `stdlib/src/collections.ls`. Version 2.4.0
adds `get_mut`, `first_key`, `last_key`, `from` and `to_vec`.

`BTreeMap<K, V>` is the ordered map: it stores keys and values in two parallel
`Vec`s sorted by key and finds entries by scanning the key vector in order.
`keys()` and `values()` follow ascending key order, which makes the map the one
to use when the output must be sorted.

## Surface

| Method | Signature | Meaning |
| --- | --- | --- |
| `new` | `() -> BTreeMap<K, V>` | an empty map |
| `from` | `(Vec<(K, V)>) -> BTreeMap<K, V>` | builds a map from owned pairs; a repeated key keeps the last value (2.4.0) |
| `insert` | `(&mut self, K, V) -> Option<V>` | moves both arguments into the map; returns the previous value for the key |
| `get` | `(&self, K) -> Option<V>` | moves or copies the key argument; returns a clone of the stored value |
| `get_mut` | `(&mut self, K) -> Option<&mut V>` | moves or copies the key; returns a mutable reference to the stored value (2.4.0) |
| `contains_key` | `(&self, K) -> bool` | whether the map holds the key |
| `remove` | `(&mut self, K) -> Option<V>` | removes the entry and moves the stored value out |
| `len` | `(&self) -> usize` | number of entries |
| `is_empty` | `(&self) -> bool` | whether the map is empty |
| `clear` | `(&mut self)` | removes every entry |
| `keys` | `(&self) -> Vec<K>` | clones the keys, in ascending order |
| `values` | `(&self) -> Vec<V>` | clones the values, ordered by their keys |
| `first_key` / `last_key` | `(&self) -> Option<K>` | clones the smallest / largest key (2.4.0) |
| `to_vec` | `(&self) -> Vec<(K, V)>` | clones the entries as owned pairs in ascending key order (2.4.0) |

## Ordering and types

* Keys use the same ordering and the same key types as
  [`BTreeSet`](btree-set.md): the integer types (including the default `i32`),
  `char` and `String`. `bool` is not an ordered type and floats are excluded
  because of `NaN`.
* `insert`, `get`, `get_mut`, `contains_key` and `remove` scan the sorted key
  vector with linear comparisons, so they cost linear time in the worst case.
* `get_mut` returns a reference into the value vector, so the map stays
  mutably borrowed until the reference's last use; `first_key`, `last_key`,
  `to_vec` and the older accessors clone because the map does not expose its
  storage.

## Limits in 2.5.0

There is no `for` loop over a `BTreeMap` yet: read the entries with `keys()`
and `values()`. `get_with(key, f)` exists only on [`HashMap`](hash-map.md); it
is not provided for `BTreeMap`. There is no range query, no cursor and no
`entry` API.

## See also

* [collections index](README.md), [BTreeSet](btree-set.md),
  [`HashMap`](hash-map.md).
* [`HashMap` chapter](../../language/20-hashmap.md) - the hashed map.
