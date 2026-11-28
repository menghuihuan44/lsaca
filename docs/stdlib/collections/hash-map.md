# hash_map: HashMap<K, V>

**Status in 2.5.0.** `HashMap` exists since 1.5 and is a builtin generic value
type; in 2.0.0 its implementation moved into the `std` crate on top of compiler
intrinsics. Since 2.2.0 `get` and `get_mut` borrow the key and return
references to the stored value. Version 2.3.0 added borrowed iteration
(`iter`, `keys`, `values`, `for (key, value) in &map`) and
`HashMap::from(Vec<(K, V)>)`. Version 2.4.0 adds the entry idioms
`get_or_insert` and `get_or_insert_with`; a borrowed `Entry` value remains
deferred. The full language-level reference is the
[`HashMap`](../../language/20-hashmap.md) chapter.

`HashMap<K, V>` maps keys to values with a Swiss-table-style open-addressing
table. It owns its keys and values, is move-only, and releases them when dropped;
`.clone()` copies the table when `K` and `V` are cloneable.

## Surface

| Method | Signature (informal) | Ownership |
| --- | --- | --- |
| `HashMap::new()` | `() -> HashMap<K, V>` | an empty map |
| `HashMap::from(pairs)` | `(Vec<(K, V)>) -> HashMap<K, V>` | moves the pairs into the map; the last duplicate key wins (2.3.0) |
| `insert(key, value)` | `-> Option<V>` | moves both arguments into the map; returns the previous value for the key |
| `get(&key)` | `-> Option<&V>` | borrows the key argument; returns a shared reference to the stored value, no clone |
| `get_mut(&key)` | `-> Option<&mut V>` | borrows the key argument; returns a mutable reference; needs a mutable map binding |
| `get_or_insert(key, default)` | `-> &mut V` | moves the key and default; inserts the default when absent, otherwise drops it; returns the stored value (2.4.0) |
| `get_or_insert_with(key, f)` | `-> &mut V` | moves the key; calls `f: fn() -> V` only when absent and inserts its result (2.4.0) |
| `get_with(key, f)` | `-> Option<U>` | copies the key; `f: Fn(&V) -> U` |
| `contains_key(key)` | `-> bool` | copies the key argument |
| `remove(key)` | `-> Option<V>` | copies the key argument; moves the stored value out |
| `len` / `is_empty` | `-> usize` / `-> bool` | number of entries / emptiness |
| `clear` | `(&mut self)` | removes every entry |
| `iter` | `(&self)` | borrowed iterator yielding `(&K, &V)` (2.3.0) |
| `keys` / `values` | `(&self)` | borrowed iterators yielding `&K` / `&V`, no clone (2.3.0) |

`insert`, `remove`, `get_mut` and `clear` change the map, so they need a mutable
binding (`let mut map`); the other methods take `&self`. `len()` returns `usize`
since 2.1.0, as all lengths do.

## Rules

* The key type must be one of `i64`, `u64`, `isize`, `usize`, `bool`, `char` or
  `String` (`isize` and `usize` were added in 2.0.0). The value type may be any
  value type, including move-only types and types with `Drop`.
* The narrower integer types are not hashable keys. In particular the
  default integer `i32` cannot be a `HashMap` key, even though integer literals
  default to `i32`; write `HashMap<i64, V>` (or another listed key type) when
  the map is keyed by an integer.
* Keys are compared by value: a `String` key is compared by its text, and
  `bool`/`char` keys are compared by their scalar value.
* `insert` moves both of its arguments. `get` and `get_mut` borrow the key as
  `&K`, so pass `&key`; `get_with`, `contains_key` and `remove` copy the key
  argument. Either way the caller keeps ownership of its key, so a `String` key
  remains usable after the call. `get` and `get_mut` return a reference instead
  of cloning.
* `iter()`, `keys()`, `values()` and `for (key, value) in &map` borrow the
  whole map: it cannot be mutated while the iterator is alive. The items are
  references, so dereference with `*key` / `*value` when an owned value is
  needed, and do not collect borrowed items into a `Vec`.
* A `get_mut` result is a mutable borrow of the map. A `match` or `if let` can
  use the call directly as its scrutinee, and the map is usable again when the
  match ends; while the reference is alive, do not use the map elsewhere.
* Two maps are equal when they hold the same entries, irrespective of hashing or
  insertion order, provided `K` and `V` support `==`.
* `{:?}` prints the entries in internal slot order in the form
  `{key: value, ...}`; insertion order does not matter for equality or lookup,
  and no program may depend on the slot order for correctness. Iteration order
  is deterministic for a given sequence of operations but is not insertion
  order.
* `get_or_insert(key, default)` and `get_or_insert_with(key, f)` combine a
  lookup with an insert and return `&mut V`; they need a mutable map binding.
  The key is moved into the call. `get_or_insert` moves its default too and
  drops it when the key is present; `get_or_insert_with` calls `f` only when
  the key is absent. The returned reference borrows the map like a `get_mut`
  result, so do not use the map again until the reference's last use.
* `get_or_insert_with` is an ordinary method, not a generic method, so its
  second parameter is the concrete function type `fn() -> V`, not a type
  parameter `F`. A named function or a closure with that signature fits.
  Closures capture by value, so a capturing closure sees a snapshot of its
  captures taken when the closure is created, not the caller's live variables.
* Both `get_or_insert` and `get_or_insert_with` always look the entry up again
  after an insertion. Growing the table rehashes it, so a reference obtained
  before the insert would not stay valid; the returned `&mut V` comes from a
  fresh lookup after the insert.
* There is no borrowed `Entry` value; see
  [RFC 0002](../../../rfcs/0002-hashmap-entry.md) for the decision and the
  planned surface. Use `get`, `get_mut`, `insert`, `get_or_insert`,
  `get_or_insert_with` and `get_with` for the equivalent read/modify/write
  patterns.

## Planned for a later release

The fixed key-type list stays. A borrowed `Entry` value is deferred pending
[RFC 0002](../../../rfcs/0002-hashmap-entry.md), which recommends a minimal
`entry` surface; the current plan is v2.6. Until then `get_or_insert`,
`get_or_insert_with`, `get_mut` and `get_with` cover lookup, insertion,
borrowed reads and updates.

## See also

* [`HashMap` chapter](../../language/20-hashmap.md) - the complete reference.
* [collections index](README.md), [HashSet](hash-set.md),
  [BTreeMap](btree-map.md).
