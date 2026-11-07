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

In 2.0 the map is `std::collections::HashMap`: the type stays builtin (its
declaration in the `std` crate is a marker that the compiler maps to the
builtin table) and the API described here is unchanged, so the same methods work
through `use std::collections::HashMap` or through the prelude re-export. See
the standard library reference for [HashMap](../stdlib/collections/hash-map.md).

Version 2.2.0 makes the lookups borrowed: `get` takes the key by reference
(`map.get(&key)`) and returns `Option<&V>`, while `get_mut` returns
`Option<&mut V>`; both avoid cloning the stored value. `contains_key`,
`remove` and `get_with` still take the key by value.

Unlike `Vec`, the key type is restricted: it must be one of `i64`, `u64`,
`isize`, `usize`, `bool`, `String` or `char`. The value type may be any valid
value type, including move-only types and types that implement `Drop`. The
pointer-sized integer types hash like the other integer types.

The list is exhaustive: in particular `i32` is not a key type, even though it is
the default integer type since 2.1.0, and neither are `i8`, `i16`, `u8`, `u16`
or `u32`. A map declared with one of those keys is rejected with `E0277`, whose
diagnostic reads "HashMap keys must be one of i64, u64, isize, usize, bool, char
or String, found i32". Use `i64` (or another listed integer type) as the key and
let the context choose it for unsuffixed literals: `counts.insert(1, 1)` on a
`HashMap<i64, i64>` stores an `i64` key.

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
    let key = String::from("ana");
    println!("{:?}", ages.get(&key));
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
| `get(&key)` | `→ Option<&V>` | borrows the key argument; returns a shared reference to the stored value, no clone |
| `get_mut(&key)` | `→ Option<&mut V>` | borrows the key argument; returns a mutable reference to the stored value; needs a mutable binding |
| `get_with(key, f)` | `→ Option<U>` | copies the key argument; `f` has type `Fn(&V) -> U` and is called with a reference to the stored value |
| `contains_key(key)` | `→ bool` | copies the key argument |
| `remove(key)` | `→ Option<V>` | copies the key argument; moves the stored value out of the map |
| `len()` | `→ usize` | number of entries |
| `is_empty()` | `→ bool` | true when the map has no entries |
| `clear()` | `()` | removes every entry |
| `keys()` | `→ Vec<K>` | clones every key into a vector |
| `values()` | `→ Vec<V>` | clones every value into a vector |

`insert`, `remove`, `get_mut` and `clear` mutate the map, so they need a mutable
binding (`let mut map`); the other methods take `&self` and work on a shared
binding.

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
    let key = String::from("ana");
    println!("{:?}", ages.get(&key));
}
```

Output:
```text
None Some(30)
Some(31)
```

`get` and `get_mut` take the key by reference (`&K`), and `contains_key`,
`remove` and `get_with` copy the key argument, so in every case the caller
keeps ownership of it. This example looks up a `String` key and then prints
the same local again:

```ls
fn main() {
    let mut counts: HashMap<String, i64> = HashMap::new();
    counts.insert(String::from("a"), 1);

    let lookup = String::from("a");
    println!("{:?}", counts.get(&lookup));
    println!("{}", counts.contains_key(lookup));
    println!("{}", lookup);
}
```

Output:
```text
Some(1)
true
a
```

`get` returns a borrowed `Option<&V>`: it does not clone the stored value, so it
works for every value type. The map stays usable; the returned reference borrows
from it until the last use of that reference.

```ls
fn main() {
    let mut names: HashMap<i64, String> = HashMap::new();
    names.insert(1, String::from("one"));

    let value = names.get(&1);
    println!("{:?}", value);
    println!("{}", names.len());
}
```

Output:
```text
Some("one")
1
```

`get_mut` returns `Option<&mut V>` and writes through the returned reference.
A `match` or `if let` can use the call directly as its scrutinee: the mutable
borrow lasts for the whole match, assigning through the bound reference modifies
the stored value in place, and the map is usable again once the match ends:

```ls
fn main() {
    let mut totals: HashMap<String, i64> = HashMap::new();
    totals.insert(String::from("a"), 10);

    let key = String::from("a");
    match totals.get_mut(&key) {
        Some(value) => { *value += 5; }
        None => {}
    }
    println!("{:?}", totals.get_with(key, |value| *value));
}
```

Output:
```text
Some(15)
```

`get_with` copies the key argument and avoids cloning the value. It takes the key
and a closure `Fn(&V) -> U` and returns the closure's result instead:

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

The key type must be `i64`, `u64`, `isize`, `usize`, `bool`, `String` or `char`.
Other key types are rejected at the point where the map type is formed:

```ls,compile-fail
fn main() {
    let mut map: HashMap<Vec<i64>, i64> = HashMap::new();
    map.insert(vec![1], 2);
}
```

Error:
```text
error[E0277]: HashMap keys must be one of i64, u64, isize, usize, bool, char or String, found Vec<i64>
  --> example-12.ls:2:18
   |
 2 |     let mut map: HashMap<Vec<i64>, i64> = HashMap::new();
   |                  ^^^^^^^
   = note: only the listed key types are supported; use a struct field or a String key instead

lsaca: example-12.ls: could not compile example-12.ls due to previous error(s)
```

A `char` is a `Copy` scalar and a valid key type. Characters compare by
scalar value, so `'a'` and `'\u{61}'` are the same key:

```ls
fn main() {
    let mut letters: HashMap<char, i64> = HashMap::new();
    letters.insert('a', 1);
    letters.insert('é', 2);

    println!("{:?} {:?}", letters.get(&'a'), letters.get(&'\u{61}'));
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

`get` and `get_mut` borrow from the map, so they work for any value type,
including a type that implements `Drop`. The methods that clone are `values()`
and `keys()` — a `Drop` value cannot be cloned (`E0277`), so `values()` is not
available for it:

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
    println!("{:?}", guards.values());
}
```

Error:

```text
error[E0277]: HashMap::values cannot clone values of type Guard because it implements Drop; use get_with(...) instead
   --> example-14.ls:14:29
    |
 14 |     println!("{:?}", guards.values());
    |                             ^^^^^^
    = note: get_with borrows the stored value, so it works for types with drop glue

lsaca: example-14.ls: could not compile example-14.ls due to previous error(s)
```

`get`, `get_mut` and `get_with` inspect a value with drop glue without cloning it;
`get_with` hands a reference to the stored value to a closure and returns the
closure result.

## No for-loop iteration

A `HashMap` cannot be iterated with `for` in 2.2.0: it does not implement
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
  --> example-15.ls:4:18
   |
 4 |     for entry in scores {
   |                  ^^^^^^
   = note: call keys() or values() to get a Vec and iterate over that

lsaca: example-15.ls: could not compile example-15.ls due to previous error(s)
```

## Known limitations in 2.2.0

* Key types are limited to `i64`, `u64`, `isize`, `usize`, `bool`, `String` and
  `char`; user structs, enums and other types cannot be keys.
* There is no entry API, no `HashMap::from`, no `insert_or_update`, and no
  iteration over the map; `keys()` and `values()` allocate a vector.
* `values()` requires a cloneable value type; a `Drop` value can be read with
  `get`, `get_mut` or `get_with`, but not with `values()` (keys are always
  cloneable).
* `get` and `get_mut` borrow the map: `get_mut` needs a mutable binding, and the
  returned reference keeps the map borrowed until the last use of that
  reference (a `match` or `if let` scrutinee ends the borrow at the end of the
  statement).
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
  `isize`, `usize`, `bool`, `String` and `char`; the value type may be any
  value type.
* `HashMap::new()` creates a map; type arguments come from annotations,
  parameters, return types or the turbofish.
* `insert` moves key and value and returns the previous value; `get(&key)`
  borrows the key and returns `Option<&V>`, `get_mut(&key)` returns
  `Option<&mut V>`; `get_with` inspects through a closure; `remove` moves the
  value out; `contains_key`, `len`, `is_empty`, `clear`, `keys` and `values`
  round out the API.
* `insert`, `remove`, `get_mut` and `clear` need a mutable binding.
* `==` compares contents, ignoring insertion order; `{}` and `{:?}` print the
  entries in slot order.
* There is no `for`-loop iteration over a map in 2.2.0.

## What next

* Iterating `keys()` and `values()` results: [Iterators](19-iterators.md).
* The `std::collections` reference: [HashMap](../stdlib/collections/hash-map.md)
  and [collections](../stdlib/collections/README.md).
* Generic types in general: [Generics](17-generics.md).
* The `Vec` API: [Standard library](15-standard-library.md#vec).
* Ownership and the move checker: [Ownership and moves](11-ownership-and-moves.md).
