# hash_map: HashMap<K, V>

**Status in 2.2.0.** API migrated from 1.7. `HashMap` exists since 1.5 and is a
builtin generic value type; in 2.0.0 its implementation moved into the `std`
crate on top of compiler intrinsics, without surface changes. Since 2.2.0
`get` and `get_mut` borrow the key and return references to the stored value.
The full language-level reference is the [`HashMap`](../../language/20-hashmap.md)
chapter.

`HashMap<K, V>` maps keys to values with a Swiss-table-style open-addressing
table. It owns its keys and values, is move-only, and releases them when dropped;
`.clone()` copies the table when `K` and `V` are cloneable.

## Surface

| Method | Signature (informal) | Ownership |
| --- | --- | --- |
| `HashMap::new()` | `() -> HashMap<K, V>` | the only associated function |
| `insert(key, value)` | `-> Option<V>` | moves both arguments into the map; returns the previous value for the key |
| `get(&key)` | `-> Option<&V>` | borrows the key argument; returns a shared reference to the stored value, no clone |
| `get_mut(&key)` | `-> Option<&mut V>` | borrows the key argument; returns a mutable reference; needs a mutable map binding |
| `get_with(key, f)` | `-> Option<U>` | copies the key; `f: Fn(&V) -> U` |
| `contains_key(key)` | `-> bool` | copies the key argument |
| `remove(key)` | `-> Option<V>` | copies the key argument; moves the stored value out |
| `len` / `is_empty` | `-> usize` / `-> bool` | number of entries / emptiness |
| `clear` | `(&mut self)` | removes every entry |
| `keys` / `values` | `-> Vec<K>` / `-> Vec<V>` | clones the keys / values into a vector |

`insert`, `remove`, `get_mut` and `clear` change the map, so they need a mutable
binding (`let mut map`); the other methods take `&self`. `len()` returns `usize`
since 2.1.0, as all lengths do.

## Rules

* The key type must be one of `i64`, `u64`, `isize`, `usize`, `bool`, `char` or
  `String` (`isize` and `usize` were added in 2.0.0). The value type may be any
  value type, including move-only types and types with `Drop`.
* The narrower integer types are not hashable keys in 2.2.0. In particular the
  default integer `i32` cannot be a `HashMap` key, even though integer literals
  default to `i32`; write `HashMap<i64, V>` (or another listed key type) when
  the map is keyed by an integer.
* Keys are compared by value: a `String` key is compared by its text, and
  `bool`/`char` keys are compared by their scalar value.
* `insert` moves both of its arguments. `get` and `get_mut` borrow the key as
  `&K`, so pass `&key`; `get_with`, `contains_key` and `remove` copy the key
  argument. Either way the caller keeps ownership of its key, so a `String` key
  remains usable after the call. `get` and `get_mut` return a reference instead
  of cloning; `values` clones the stored values.
* A `get_mut` result is a mutable borrow of the map. A `match` or `if let` can
  use the call directly as its scrutinee, and the map is usable again when the
  match ends; while the reference is alive, do not use the map elsewhere.
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
release. `get` and `get_mut` are the only lookups that borrow the key and return
a reference: `get_with`, `contains_key` and `remove` still take the key by
value, and `values` clones each value.

## See also

* [`HashMap` chapter](../../language/20-hashmap.md) - the complete reference.
* [collections index](README.md), [HashSet](hash-set.md),
  [BTreeMap](btree-map.md).
