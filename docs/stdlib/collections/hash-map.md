# hash_map: HashMap<K, V>

**Status in 2.1.0.** API migrated from 1.7. `HashMap` exists since 1.5 and is a
builtin generic value type; in 2.0.0 its implementation moved into the `std`
crate on top of compiler intrinsics, without surface changes. The full
language-level reference is the [`HashMap`](../../language/20-hashmap.md)
chapter.

`HashMap<K, V>` maps keys to values with a Swiss-table-style open-addressing
table. It owns its keys and values, is move-only, and releases them when dropped;
`.clone()` copies the table when `K` and `V` are cloneable.

## Surface

| Method | Signature (informal) | Ownership |
| --- | --- | --- |
| `HashMap::new()` | `() -> HashMap<K, V>` | the only associated function |
| `insert(key, value)` | `-> Option<V>` | moves both arguments into the map; returns the previous value for the key |
| `get(key)` | `-> Option<V>` | copies the key argument; clones the stored value |
| `get_with(key, f)` | `-> Option<U>` | copies the key; `f: Fn(&V) -> U` |
| `contains_key(key)` | `-> bool` | copies the key argument |
| `remove(key)` | `-> Option<V>` | copies the key argument; moves the stored value out |
| `len` / `is_empty` | `-> usize` / `-> bool` | number of entries / emptiness |
| `clear` | `(&mut self)` | removes every entry |
| `keys` / `values` | `-> Vec<K>` / `-> Vec<V>` | clones the keys / values into a vector |

`insert`, `remove` and `clear` change the map, so they need a mutable binding
(`let mut map`); the other methods take `&self`. `len()` returns `usize` since
2.1.0, as all lengths do.

## Rules

* The key type must be one of `i64`, `u64`, `isize`, `usize`, `bool`, `char` or
  `String` (`isize` and `usize` were added in 2.0.0). The value type may be any
  value type, including move-only types and types with `Drop`.
* The narrower integer types are not hashable keys in 2.1.0. In particular the
  default integer `i32` cannot be a `HashMap` key, even though integer literals
  default to `i32`; write `HashMap<i64, V>` (or another listed key type) when
  the map is keyed by an integer.
* Keys are compared by value: a `String` key is compared by its text, and
  `bool`/`char` keys are compared by their scalar value.
* `insert` moves both of its arguments, while `get`, `get_with`,
  `contains_key` and `remove` copy the key argument, so a `String` key remains
  usable after the call (the 1.7 copying behaviour is preserved). The value side
  is unchanged: `get` and `values` clone the stored value.
* Two maps are equal when they hold the same entries, irrespective of hashing or
  insertion order, provided `K` and `V` support `==`.
* `{:?}` prints the entries in internal slot order in the form
  `{key: value, ...}`; insertion order does not matter for equality or lookup,
  and no program may depend on the slot order for correctness.
* A map cannot be iterated with `for`. `keys()` and `values()` are the way to
  read all entries; there is no entry API, no `HashMap::from` and no
  `insert_or_update`.

## Planned for a later release

No new `HashMap` methods are planned. The remaining limits are the fixed key
type list and the missing `for` loop; both are tracked as open work for a later
release.

## See also

* [`HashMap` chapter](../../language/20-hashmap.md) - the complete reference.
* [collections index](README.md), [HashSet](hash-set.md),
  [BTreeMap](btree-map.md).
