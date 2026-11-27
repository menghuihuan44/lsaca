# 0002: HashMap entry API

* Status: proposed (recommendation: defer)
* Target release: backlog (revisit in v2.4)
* Author: lsaca compiler team, 2026-11-26

## Summary

This RFC decides whether lsaca should add a Rust-style `Entry` API to
`HashMap<K, V>` in v2.3. The recommendation is to **defer** it: the borrowed
map iteration, `get`, `get_mut`, `get_with`, `insert` and `remove` added in
2.2/2.3 cover the workflows that need the entry pattern today, while a real
`Entry` value would add new intrinsic surface and a subtle lifetime contract
for comparatively little benefit. The minimal operations that an entry API
would make convenient (`or_insert`, `or_insert_with`, update-if-present) are
better added later as plain methods on the map once a concrete need is
measured.

## Motivation

The canonical entry use case is a word count:

```ls
let mut counts: HashMap<String, i64> = HashMap::new();
for word in words.into_iter() {
    match counts.get_mut(&word) {
        Some(count) => { *count += 1; }
        None => { counts.insert(word, 1); }
    }
}
```

This works today, but it needs a second lookup when the key is absent and it
spells out the match. The Rust form is shorter:

```rust
*counts.entry(word).or_insert(0) += 1;
```

The same shape appears in grouping, memoization and caches. In 2.3 the
workaround is `get_mut` plus `insert`, or `get_with` for a read-only
inspection. `get_with` returns an owned `U` derived from `&V`; it cannot
insert, and it cannot return a reference into the map.

A second class of use is "update or insert" where the default is cheap to
compute, for example accumulating a `Vec` of values per key. That currently
needs the same two-step match, and moving the value out of `get_mut` does not
help because the map cannot be mutated while the reference is alive.

## Design

### Representation facts (2.3)

The map wrapper emitted per instantiation is a managed cell containing an
`LsMapTable`:

```c
typedef struct LsMapTable {
    unsigned char* keys;
    unsigned char* values;
    unsigned char* ctrl;
    size_t len, cap, growth_left, key_size, value_size;
    uint64_t (*hash_key)(const void*);
    bool (*eq_key)(const void*, const void*);
    void (*copy_key)(void*, const void*);
    void (*drop_key)(void*);
    void (*copy_value)(void*, const void*);
    void (*drop_value)(void*);
} LsMapTable;
```

Keys and values live in separate byte arrays at parallel slot indexes; the
control byte array marks empty, deleted and occupied slots. This is a Swiss
Table with open addressing. Insertion may grow and rehash the table, which moves
every live slot. There is no stable slot address that survives an insertion,
and no per-slot indirection that could keep an entry valid across a grow.

The map is a managed pointer value. `get`/`get_mut` take `&K`, and the
reference they return keeps the whole map borrowed until its last use. The
borrow checker enforces this at the lsaca level, so within that borrow the map
cannot be mutated through another path.

### Proposed surface (if implemented later)

The smallest API that matches the language's existing method style is:

```rust
impl<K, V> HashMap<K, V> {
    fn entry(&mut self, key: K) -> Entry<K, V>;

    // Entry
    fn or_insert(&mut self, default: V) -> &mut V;
    fn or_insert_with(&mut self, f: Fn() -> V) -> &mut V;
    fn or_insert_with_key(&mut self, f: Fn(&K) -> V) -> &mut V;
    fn and_modify(&mut self, f: Fn(&mut V)) -> &mut Entry<K, V>;
    fn key(&self) -> &K;
}
```

`Entry` would be a stdlib struct holding a mutable borrow of the map plus the
owned key and a state marker (vacant/occupied). `or_insert` would perform the
lookup on first use, insert the default if the key is absent, and return a
mutable reference to the value. Because the entry holds the only mutable
borrow, the borrow checker can rule out a concurrent map mutation during its
life; that is the property that makes the slot safe to hold between the
lookup and the insert.

In v2.3 the language can express the reference field (a direct `&mut
HashMap<K, V>` field is accepted since 2.3) and has explicit `Drop`, so the
design is expressible without unsafe code. What is missing is a sound
contract for "lookup once, then insert at the same probe position": the
current helpers expose `lsaca_map_get_ptr` (a slot pointer for an existing
key) but no insertion position API. Implementing `Entry` would require new
intrinsics and careful handling of a key that is absent, a deleted slot and a
grow triggered at insert time.

### Diagnostics

`entry` on a map whose key or value type is invalid is already rejected where
the map type is formed (`E0277` for the key, `E0498` for reference-containing
values). No new diagnostic is needed for the deferred proposal. If the API is
implemented, use `E0498` for an entry held across a map mutation (the borrow
checker already produces a moved-value/borrow error; the API must not weaken
it).

## Drawbacks

Deferring means:

* the word-count and grouping patterns keep the two-step `get_mut`/match
  shape, and the language keeps its current, slightly verbose idiom;
* `get_with` remains the only non-cloning way to derive a value, so grouping
  into an owned `Vec` per key still needs a match;
* programs ported from Rust need a mechanical rewrite.

Implementing now would mean:

* a new generic stdlib type with a mutable reference field and a non-Copy
  marker, plus `Drop` glue;
* new map intrinsics (find insertion position, insert-at-position) and new
  codegen paths that must stay warning-free and deterministic;
* a larger API and documentation surface, and an entry type that is hard to
  keep stable if the table representation changes again (it changed in 1.6);
* ambiguity about whether `entry` takes the key by value (Rust) or by
  reference (matching the 2.2 `get`/`get_mut` convention). The current
  convention splits lookups (borrowed key) from mutations (owned key); an
  entry would have to choose one and document it.

## Alternatives

1. **Do nothing (recommended).** Use `get_mut` plus `insert`, or `get_with`
   for reads. This is the current 2.3 contract and requires no new machinery.
2. **Add only `or_insert` as a method, without an Entry type.** `fn
   or_insert(&mut self, key: K, value: V) -> &mut V` and
   `get_or_insert_with` would cover most word-count code with one lookup and
   one new intrinsic, no Entry struct, and no state marker. This is the
   smallest useful step and is the recommended follow-up if demand appears.
3. **Rust-style Entry API.** Maximum convenience and familiar syntax, but the
   largest implementation and documentation cost; the entry type must be
   non-Copy and must document that the map is mutably borrowed until the entry
   is consumed.
4. **Functional API** (`map.update(key, default, f)`). Fits the closure-heavy
   adapter style but returns `V` or `&V`, not `&mut V`, so it cannot express
   accumulation through a reference without a new closure shape.

Option 2 is the natural v2.4 candidate. It requires one intrinsic to answer
"where would this key be inserted" but that intrinsic can be kept private and
the public surface stays small.

## Compatibility and migration

Deferring changes nothing: 2.3 programs keep compiling. If an entry-style API
is added in v2.4, it is additive (a new method and a new type), and `get`,
`get_mut`, `get_with`, `insert` and `remove` keep their current semantics.
A later addition should follow the 2.2 migration pattern: the new method is
available immediately, and the docs recommend it only for code that actually
needs the update-or-insert shape.

## Unresolved questions

* Should `entry` take `K` by value (Rust) or `&K` (current lookup style)?
* Should `or_insert` be a method on the map (option 2) or on an `Entry`
  value, and how much of the Rust `Entry` surface is worth porting?
* Does the language need `Entry::and_modify` at all, given that closures can
  already update through `get_mut`, or is `or_insert` enough?
* Is a per-slot generation/state check needed if the entry is ever held across
  a map operation that is not visible to the borrow checker (it should not be
  possible in safe lsaca)?
