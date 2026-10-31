# HashMap<K, V>

Version 1.5.0 added `HashMap<K, V>`, a builtin generic hash map; version 1.6.0
replaced its table with a Swiss Table and added `char` to the key types. Like
`Vec<T>` the map is a managed value type: it owns its keys and values and
releases them when the map is dropped; keys and values that are themselves
heap values are managed through ORC reference counting. A `HashMap` is
move-only — `let other = map;` moves the map, and `.clone()` produces an
independent copy of the table when the key and value types are cloneable — so
it is passed to functions by value or by
reference like any other value (see [Ownership and
moves](11-ownership-and-moves.md)).

Unlike `Vec`, the key type is restricted: it must be one of `i64`, `u64`,
`bool`, `String` or `char`. The value type may be any valid value type,
including move-only types and types that implement `Drop`.

## Creating a map

`HashMap::new()` is the only associated function. The type arguments come from
the binding annotation, a parameter or return type, or the turbofish:

```ls
fn main() {
    let mut counts: HashMap<String, i64> = HashMap::new();
    counts.insert(String::from("a"), 1);
    counts.insert(String::from("b"), 2);
    println!("{}", counts.len());
    println!("{}", counts.is_empty());
}
```

Output:
```text
2
false
```

The turbofish form names the type arguments explicitly:

```ls
fn main() {
    let mut ages = HashMap::<String, i64>::new();
    ages.insert(String::from("ana"), 30);
    println!("{:?}", ages.get(String::from("ana")));
}
```

Output:
```text
Some(30)
```

`HashMap::new()` also takes its type arguments from a return type or a
parameter type; for example a function declared as `fn empty() -> HashMap<i64,
String>` can return `HashMap::new()` directly. If no context determines both
type arguments, the compiler reports `E0282`.

## Methods

| Method | Signature (informal) | Ownership |
| --- | --- | --- |
| `HashMap::new()` | `→ HashMap<K, V>` | creates an empty map |
| `insert(key, value)` | `→ Option<V>` | moves both arguments into the map; returns the previous value for the key, if any |
| `get(key)` | `→ Option<V>` | copies the key argument; clones the stored value, so `V` must be cloneable |
| `get_with(key, f)` | `→ Option<U>` | copies the key argument; `f` has type `Fn(&V) -> U` and is called with a reference to the stored value |
| `contains_key(key)` | `→ bool` | copies the key argument |
| `remove(key)` | `→ Option<V>` | copies the key argument; moves the stored value out of the map |
| `len()` | `→ i64` | number of entries |
| `is_empty()` | `→ bool` | true when the map has no entries |
| `clear()` | `()` | removes every entry |
| `keys()` | `→ Vec<K>` | clones every key into a vector |
| `values()` | `→ Vec<V>` | clones every value into a vector |

`insert`, `remove` and `clear` mutate the map, so they need a mutable binding
(`let mut map`); the other methods take `&self` and work on a shared binding.

Keys are compared by value. A `String` key is compared by its text, so
`String::from("a")` finds an entry inserted with a different `String` holding
the same bytes.

## Reading and writing entries

`insert` moves the key and the value into the map and returns the value that
was stored for that key before, if any. A map insertion always replaces:

```ls
fn main() {
    let mut ages: HashMap<String, i64> = HashMap::new();
    let first = ages.insert(String::from("ana"), 30);
    let second = ages.insert(String::from("ana"), 31);
    println!("{:?} {:?}", first, second);
    println!("{:?}", ages.get(String::from("ana")));
}
```

Output:
```text
None Some(30)
Some(31)
```

The key argument of `get`, `contains_key`, `remove` and `get_with` is copied by
the compiler, so the caller keeps ownership of it. This example looks up a
`String` key and then prints the same local again:

```ls
fn main() {
    let mut counts: HashMap<String, i64> = HashMap::new();
    counts.insert(String::from("a"), 1);

    let lookup = String::from("a");
    println!("{}", counts.contains_key(lookup));
    println!("{}", lookup);
}
```

Output:
```text
true
a
```

`get` returns an owned `Option<V>`: it clones the stored value, so the map keeps
its own copy and remains usable. The value type must be cloneable for `get` and
`values()` to be available; see [Values that cannot be
cloned](#values-that-cannot-be-cloned).

```ls
fn main() {
    let mut names: HashMap<i64, String> = HashMap::new();
    names.insert(1, String::from("one"));

    let value = names.get(1);
    println!("{:?}", value);
    println!("{}", names.len());
}
```

Output:
```text
Some("one")
1
```

`get_with` avoids the clone. It takes the key and a closure `Fn(&V) -> U` and
returns the closure's result instead of the value:

```ls
fn main() {
    let mut names: HashMap<i64, String> = HashMap::new();
    names.insert(1, String::from("one"));

    let length = names.get_with(1, |value| value.len());
    println!("{:?}", length);
}
```

Output:
```text
Some(3)
```

`remove` returns the stored value as an owned `Option<V>` and leaves the map
without that entry:

```ls
fn main() {
    let mut scores: HashMap<String, i64> = HashMap::new();
    scores.insert(String::from("a"), 1);
    scores.insert(String::from("b"), 2);

    let removed = scores.remove(String::from("a"));
    println!("{:?} {}", removed, scores.len());
    println!("{}", scores.is_empty());
}
```

Output:
```text
Some(1) 1
false
```

`keys()` and `values()` produce vectors of cloned keys or values. The order is
the map's internal slot order: it is deterministic for a given sequence of
operations, but it is **not** insertion order and it is not guaranteed across
releases, so treat it as unspecified:

```ls
fn main() {
    let mut scores: HashMap<String, i64> = HashMap::new();
    scores.insert(String::from("a"), 1);

    let keys = scores.keys();
    let values = scores.values();
    println!("{:?}", keys);
    println!("{:?}", values);

    scores.clear();
    println!("{}", scores.is_empty());
}
```

Output:
```text
["a"]
[1]
true
```

## Equality and formatting

Two maps compare equal with `==` when they hold the same keys mapped to equal
values; insertion order does not matter. `==` requires the key type and the
value type to support `==` themselves:

```ls
fn main() {
    let mut first: HashMap<i64, i64> = HashMap::new();
    first.insert(1, 10);
    first.insert(2, 20);

    let mut second: HashMap<i64, i64> = HashMap::new();
    second.insert(2, 20);
    second.insert(1, 10);

    println!("{}", first == second);
}
```

Output:
```text
true
```

`{}` and `{:?}` print the entries in slot order in the form `{key: value, ...}`;
display formatting on a map falls back to debug formatting, like other
aggregates:

```ls
fn main() {
    let mut map: HashMap<i64, i64> = HashMap::new();
    map.insert(1, 10);
    println!("{}", map);
}
```

Output:
```text
{1: 10}
```

## Key types

The key type must be `i64`, `u64`, `bool`, `String` or `char`. Other key types
are rejected at the point where the map type is formed:

```ls,compile-fail
fn main() {
    let mut map: HashMap<Vec<i64>, i64> = HashMap::new();
    map.insert(vec![1], 2);
}
```

Error:
```text
error[E0277]: HashMap keys must be one of i64, u64, bool, char or String, found Vec<i64>
  --> example-11.ls:2:18
   |
 2 |     let mut map: HashMap<Vec<i64>, i64> = HashMap::new();
   |                  ^^^^^^^
   = note: only the listed key types are supported; use a struct field or a String key instead

lsaca: example-11.ls: could not compile example-11.ls due to previous error(s)
```

A `char` is a `Copy` scalar and a valid key type. Characters compare by
scalar value, so `'a'` and `'\u{61}'` are the same key:

```ls
fn main() {
    let mut letters: HashMap<char, i64> = HashMap::new();
    letters.insert('a', 1);
    letters.insert('é', 2);

    println!("{:?} {:?}", letters.get('a'), letters.get('\u{61}'));
    println!("{}", letters.contains_key('é'));
    println!("{}", letters.len());
}
```

Output:

```text
Some(1) Some(1)
true
2
```

## The table

Since 1.6.0 the map is a Swiss Table, the design popularised by Abseil's
`flat_hash_map`:

* every slot has a control byte holding a 7-bit hash fragment (H2), so a probe
  compares 16 slots at once with a portable SWAR mask instead of following a
  linear probe chain;
* slots are organised in groups of 16, and the group is selected from the upper
  hash bits; the table grows at a 7/8 load factor, which keeps probe groups
  short even when the table is nearly full;
* deleted slots become tombstones that are reused before the table grows, and
  `clear()` marks every slot deleted while keeping the allocated table for
  reuse;
* the hash is a deterministic 64-bit hash with a fixed seed (a wyhash-style
  mixer over the key bytes, integers mixed as their bit pattern, characters as
  their scalar value), so the same sequence of operations produces the same
  table in every run. It is not a cryptographic hash and is not designed to
  resist adversarial keys.

The memory layout and the C ABI are internal and changed in 1.6.0. C projects
emitted by earlier releases must be regenerated with the 1.6.0 compiler.

### Slot order and iteration

`keys()`, `values()` and the printed form of a map all walk the slots in
ascending slot order. That order is deterministic for a given sequence of
operations, but it is not insertion order and it is not a language guarantee:
do not sort or compare programs by output order, and do not assume it stays the
same across releases.

### Performance expectations

The layout is a growth and predictability change, not a uniform speedup. Do not
assume a 1.6.0 map is always faster than the 1.5.0 open-addressed one: the
benchmark suite measures `hashmap_i64` and `hashmap_string` on the same
machine as every other workload, and its numbers are the only ones this
documentation quotes - see [Benchmarks](21-benchmarks.md).

## Values that cannot be cloned

`get` and `values()` clone the stored values, while `keys()` clones the keys
(which are always cloneable). A value type that implements `Drop` cannot be
cloned (`E0277`), so a map with such values supports `insert`, `get_with`,
`contains_key`, `remove`, `clear`, `keys()` and the length queries, but not
`get` or `values()`.
`get_with` is the way to inspect a non-cloneable value in place:

```ls,compile-fail
struct Guard {
    name: String,
}

impl Drop for Guard {
    fn drop(&mut self) {
        println!("drop {}", self.name);
    }
}

fn main() {
    let mut guards: HashMap<i64, Guard> = HashMap::new();
    guards.insert(1, Guard { name: String::from("g") });
    println!("{:?}", guards.get(1));
}
```

Error:

```text
error[E0277]: HashMap::get cannot clone values of type Guard because it implements Drop; use get_with(...) instead
   --> example-13.ls:14:29
    |
 14 |     println!("{:?}", guards.get(1));
    |                             ^^^
    = note: get_with borrows the stored value, so it works for types with drop glue

lsaca: example-13.ls: could not compile example-13.ls due to previous error(s)
```

## No for-loop iteration

A `HashMap` cannot be iterated with `for` in 1.6.0: it does not implement
`Iterator<T>`. Use `keys()` or `values()` and iterate the resulting vector:

```ls,compile-fail
fn main() {
    let mut scores: HashMap<i64, i64> = HashMap::new();
    scores.insert(1, 2);
    for entry in scores {
        println!("{}", entry);
    }
}
```

Error:

```text
error[E0277]: cannot iterate over a value of type HashMap<i64, i64>; for loops require a range, a Vec<T>, or a type that implements Iterator<T>
  --> example-14.ls:4:18
   |
 4 |     for entry in scores {
   |                  ^^^^^^
   = note: call keys() or values() to get a Vec and iterate over that

lsaca: example-14.ls: could not compile example-14.ls due to previous error(s)
```

## Known limitations in 1.6.0

* Key types are limited to `i64`, `u64`, `bool`, `String` and `char`; user
  structs, enums and other types cannot be keys.
* There is no entry API, no `HashMap::from`, no `insert_or_update`, and no
  iteration over the map; `keys()` and `values()` allocate a vector.
* `get` and `values()` require a cloneable value type; a `Drop` value cannot
  be read with `get` or `values()`, only with `get_with` (keys are always
  cloneable).
* The printed order of entries is the internal slot order, not insertion order
  (it is deterministic for a given sequence of operations, but it is not part
  of the documented contract).
* The hash is deterministic and non-cryptographic; do not expose a map directly
  to adversarial keys.
* A map is move-only: passing it to a function by value moves it, and
  `let other = map;` moves it. Use `.clone()` for an independent copy or pass a
  reference.

See [Roadmap and deviations from Rust](16-roadmap.md) for the full list.

## Summary

* `HashMap<K, V>` is a builtin generic map with key types `i64`, `u64`,
  `bool`, `String` and `char`; the value type may be any value type.
* `HashMap::new()` creates a map; type arguments come from annotations,
  parameters, return types or the turbofish.
* `insert` moves key and value and returns the previous value; `get` copies the
  key and clones the value; `get_with` inspects without cloning; `remove` moves
  the value out; `contains_key`, `len`, `is_empty`, `clear`, `keys` and
  `values` round out the API.
* `insert`, `remove` and `clear` need a mutable binding.
* `==` compares contents, ignoring insertion order; `{}` and `{:?}` print the
  entries in slot order.
* There is no `for`-loop iteration over a map in 1.6.0.

## What next

* Iterating `keys()` and `values()` results: [Iterators](19-iterators.md).
* Generic types in general: [Generics](17-generics.md).
* The `Vec` API: [Standard library](15-standard-library.md#vec).
* Ownership and the move checker: [Ownership and moves](11-ownership-and-moves.md).
