# Memory model: ownership, moves and ORC

lsaca gives owned values **ownership and move semantics**: a value has one owner
at a time, moving it transfers ownership, and the compiler rejects uses of a
value after it has been moved. Copies are never implicit for heap-owning values;
call `.clone()` when you really need a second, independent value. Types made only
of copyable parts are **Copy** and are copied implicitly, as in Rust.

Memory itself is still managed by **ORC** — *Owned Reference Counting*: owned
values are reference-counted handles, and a trial-deletion cycle collector
reclaims `Rc<T>` cycles. Version 1.4 adds `Drop` hooks, so destruction is
observable: a value runs its `Drop` implementation when it is released. Version
1.3 added non-owning references with a lightweight borrow checker that enforces
aliasing (it never owns or frees anything — see [References and
borrowing](12-references-and-borrowing.md)). Version 1.5 adds user-defined
generics and traits, lazy iterators, the builtin `HashMap<K, V>`, and extends
`==` to aggregates, references and maps.

This page is the authoritative description of ownership, moves, `.clone()`,
`Drop`, copy-on-write mutation, `Rc<T>`, `Box<T>`, iterator ownership,
`HashMap<K, V>` ownership, cycles and leak tracking. The finer rules for moves
in expressions are also collected in [Ownership and moves](11-ownership-and-moves.md).

## The kinds of value representation

Every lsaca value is represented in one of three ways:

1. **Copy values** are stored directly and copied bit by bit: the scalar types
   `bool`, all integer and float types, `()`, and references (`&T`, `&mut T`).
   Copying a value of these types is always allowed and never invalidates the
   original.
2. **Move-only values** are owned values with a heap payload or drop glue:
   `String`, every `Vec<T>`, `Rc<T>`, `Box<T>`, function values and closures, and
   any struct, enum, tuple, `Option` or `Result` that contains one of those or
   implements `Drop`. These values are *moved* when used by value; a move
   transfers ownership and the source binding may not be used again.
3. **References** are a borrowed representation: a plain C pointer that never
   participates in reference counting. A reference is a non-owning view of an
   existing place; it is never retained or released, and it cannot be stored
   inside a struct, enum, tuple, `Vec`, `Option`, `Result` or `Box`, so it never
   appears as a field or element of an aggregate.

`Copy` and `Clone` are derived automatically from a type's definition;
`Drop` is the one builtin trait you can implement (user-defined traits also
exist since 1.5.0, but they do not change value semantics). The rules are given
in the next sections.

| Category | Types | What use-by-value does |
| --- | --- | --- |
| Copy | `bool`, integers, floats, `()`, `&T`, `&mut T`; tuples, `Option` and `Result` all of whose parts are Copy; user structs and enums whose fields are all Copy and that do not implement `Drop` | Copies the value; the original stays usable |
| Move-only | `String`, `Vec<T>`, `HashMap<K, V>`, `Rc<T>`, `Box<T>`, function values and closures; any type containing one of them; every type that implements `Drop` | Moves ownership to the new place; using the old binding is an error |

```ls
struct Point {
    x: i64,
    y: i64,
}

fn main() {
    let p = Point { x: 1, y: 2 };
    let q = p; // Point is Copy: q is a copy, p stays usable
    println!("{} {}", p.x, q.y);
}
```

Output:

```text
1 2
```

## Moves

A move is the transfer of ownership from one place to another. These positions
use their value by value and therefore move it when the type is move-only:

* binding it to a new name: `let other = value;` and tuple or struct patterns
  that bind a whole move-only value;
* assigning it: `slot = value;`;
* passing it to a function or method parameter that takes the value by value,
  including a by-value `self`;
* returning it: `return value;` and a block tail expression;
* storing it in an aggregate: a struct or tuple literal field, a `vec!`
  element, `Some`/`Ok`/`Err` payload, an `Rc::new`/`Box::new` payload, or
  `Box::set`/`Rc::set`;
* pushing it into a `Vec` with `push`, `insert` or `extend`, or inserting it
  into a `HashMap` with `insert` (both key and value are moved);
* calling a receiver-consuming builtin such as `unwrap`, `expect` or
  `unwrap_or` on it;
* iterating a `Vec` by value with `for item in values`;
* matching it as a match, `if let` or `while let` scrutinee;
* capturing it in a closure (closures capture by value, moving non-Copy
  variables);
* an expression statement that discards the value.

These positions only borrow and never move their operand: `format!`,
`println!` and the other formatting macros, `assert!`/`assert_eq!`,
`==`/`!=`, `&place`/`&mut place`, field reads of Copy fields, method receivers
of `&self`/`&mut self` methods, mutating builtin methods such as `push_str`,
`push`, `set` or the `Option`/`Result` combinators, and the map methods whose
key is copied (`get`, `get_with`, `contains_key`, `remove`).

After a move the old binding is gone. Using it is the error that makes ownership
visible:

```ls,compile-fail
fn main() {
    let text = String::from("hello");
    let moved = text;
    println!("{}", text);
}
```

Error:

```text
error[E0382]: use of moved value: text
  --> example-02.ls:4:20
   |
 4 |     println!("{}", text);
   |                    ^^^^
   = note: example-02.ls:3:17: value moved here
   = note: text has type String, which is not Copy, so the value is moved whenever it is used by value; use .clone() to copy it first

lsaca: example-02.ls: could not compile example-02.ls due to previous error(s)
```

A function that takes a `String` by value takes ownership of the caller's value;
the caller cannot use that binding afterwards:

```ls
fn describe(text: String) {
    println!("describe {}", text);
}

fn main() {
    let greeting = String::from("hello");
    describe(greeting); // ownership moves into describe
    println!("done");
}
```

Output:

```text
describe hello
done
```

Two related move errors are worth knowing: moving a field or an indexed element
out of a value (`E0507` — use `.clone()` instead), and moving a value while a
reference to it is live (`E0505`). Both are explained in [Ownership and
moves](11-ownership-and-moves.md).

## Copy types

`Copy` is derived automatically; there is no `impl Copy` syntax. A type is Copy
when every part of it is Copy and it does not implement `Drop`:

* `bool`, all integer and float types, `()`;
* shared and mutable references, `&T` and `&mut T`;
* tuples, `Option<T>` and `Result<T, E>` when all their parts are Copy;
* user structs and enums whose fields or payloads are all Copy, when the type
  has no `Drop` implementation.

A `Copy` value is never moved. Binding it to a second name, passing it to a
function, returning it, storing it in a container and matching it all copy the
bits, and both the old and the new place stay usable. Integers and other scalars
behave this way everywhere in the manual, which is why ordinary arithmetic code
never sees a move error.

## Cloning

Every type has a `.clone()` method, derived automatically like `Copy`. Calling
it on a value leaves the original usable and returns an independent value
(except for `Rc`, which returns another handle to the same cell). There is no
`impl Clone` syntax.

| Type | `.clone()` produces |
| --- | --- |
| `String` | An independent string; later mutations of either value do not affect the other |
| `Vec<T>` | A deep copy: a new vector with cloned elements |
| `Rc<T>` | Another handle to the **same** cell; the reference count grows |
| `Box<T>` | A deep copy: a new box with a cloned payload |
| User structs, enums, tuples, `Option`, `Result` | A field-wise clone, recursively |
| Function values and closures | A value that shares the same closure environment (the environment is not duplicated by the language) |
| `&T` / `&mut T` | The reference layers are stripped and the referred value is cloned into an owned value |
| Copy types | A plain copy; `.clone()` is allowed but unnecessary |

```ls
fn main() {
    let mut text = String::from("abc");
    let text_copy = text.clone();
    text.push_str("def");
    println!("{} {}", text, text_copy);

    let mut numbers = vec![1, 2, 3];
    let snapshot = numbers.clone();
    numbers.push(4);
    println!("{:?} {:?}", numbers, snapshot);
}
```

Output:

```text
abcdef abc
[1, 2, 3, 4] [1, 2, 3]
```

`Clone` is not available for a type that implements `Drop`: cloning a value that
owns resources would duplicate its destructor. The compiler rejects it with
`E0277`; use a reference or restructure the type instead. Prefer moving a value
to cloning it; `.clone()` is an explicit request for a second owned value.

## Value semantics for aggregates

Structs, enums and tuples have value semantics. Moving one moves all its fields
and replacing one releases the old value; cloning one clones every field
recursively. Reading a `Copy` field copies just that field, and a `Copy` field
can be read out of a move-only aggregate. A move-only field cannot be moved out
of its aggregate (`E0507`) — clone it instead.

```ls
struct Account {
    owner: String,
    balance: i64,
}

fn main() {
    let mut a = Account { owner: String::from("alice"), balance: 10 };
    let b = a.clone(); // independent copy: b owns its own owner string
    a.balance += 5;
    a.owner.push_str("!");
    println!("{} {}", a.owner, a.balance);
    println!("{} {}", b.owner, b.balance);
}
```

Output:

```text
alice! 15
alice 10
```

## `Drop`

`Drop` is the one trait with a builtin implementation form (user-defined traits
exist since 1.5.0, but they do not change value semantics). It declares the hook
that runs when a value is released:

```rust
impl Drop for TypeName {
    fn drop(&mut self) {
        // release resources, print a trace, ...
    }
}
```

Rules and behaviour:

* exactly one `Drop` implementation per type (`E0119` on a duplicate); the hook
  must be named `drop`, take `&mut self` as its only parameter and return `()`;
* a type that implements `Drop` is never `Copy` and cannot be cloned (`E0277`);
* no other builtin trait can be implemented; user-defined traits are allowed
  since 1.5.0 but are ordinary method collections. `impl Copy` and `impl Clone`
  are rejected with a message saying those traits are derived automatically;
* the hook receives a mutable reference to the value, so it can read or modify
  fields before they are released;
* the hook runs first, then the fields are released in declaration order;
* local variables are released at the end of their block in reverse order of
  declaration; parameters are released in reverse order when the function body
  exits; temporaries are released at the end of the statement that created them;
* overwriting a `let mut` binding with a new value releases the old value at the
  assignment, before the statement completes;
* a value that has been moved is released exactly once, by its new owner;
* `return`, `break` and `continue` release the values that go out of scope
  exactly as normal scope exit does;
* a normal return from `main` releases `main`'s locals; a process that ends
  through `exit(n)` or a panic skips the pending drops and lets the operating
  system reclaim the memory.

```ls
struct Loud {
    name: String,
}

impl Drop for Loud {
    fn drop(&mut self) {
        println!("drop {}", self.name);
    }
}

struct Pair {
    first: Loud,
    second: Loud,
}

impl Drop for Pair {
    fn drop(&mut self) {
        println!("drop pair");
    }
}

fn main() {
    let first = Loud { name: String::from("first") };
    let p = Pair {
        first: Loud { name: String::from("a") },
        second: Loud { name: String::from("b") },
    };
    println!("body");
}
```

Output:

```text
body
drop pair
drop a
drop b
drop first
```

The pair's hook runs before its fields, the fields are released in declaration
order, and the local declared before the pair is released last.

```ls
struct Loud {
    name: String,
}

impl Drop for Loud {
    fn drop(&mut self) {
        println!("drop {}", self.name);
    }
}

fn main() {
    let mut item = Loud { name: String::from("old") };
    item = Loud { name: String::from("new") };
    println!("after");
}
```

Output:

```text
drop old
after
drop new
```

There is no explicit `drop` builtin and no way to call the hook directly:
`drop(value)` reports `E0425` (no such function), and `value.drop()` reports
`E0599` (no method named `drop`). Release happens through scope exit,
assignment or moving the value somewhere that releases it. See [Ownership and
moves](11-ownership-and-moves.md#the-drop-trait) for the full set of rules and
errors.

## Iterators own the vector they read

`.iter()` consumes a `Vec<T>` and returns an iterator value that owns the
vector. The iterator is an ordinary move-only value: it lives in a local, is
passed and returned like any value, and releases its vector (and its elements)
when it is dropped. Each adapter in a chain such as
`values.iter().filter(...).map(...)` creates a new iterator value, and a terminal
operation (`collect`, `fold`, `count`, `sum`, `any`, `all`, `find`) consumes the
chain and produces an ordinary value. A `for` loop over an iterator releases it
at loop exit. See [Iterators](19-iterators.md).

## HashMap owns its keys and values

A `HashMap<K, V>` is a managed value like `Vec`: it owns every key and value
inserted into it, releases them when the map is dropped, and `insert` returns the
previous value instead of dropping it. `.clone()` produces an independent table
whose keys and values are cloned too. See [HashMap](20-hashmap.md).

## Copy-on-write

`String` and `Vec<T>` may share a buffer between a value and its clone and copy
it lazily on mutation, so cloning and then mutating one side does not have to
duplicate the buffer twice: when a buffer is shared and a handle asks to mutate
it, the runtime copies the buffer first. The language-visible rule is simply that cloned values are independent —
mutating a clone never changes the original, and mutating the original after a
clone never changes the clone. Moving a value transfers the buffer without any
copy at all, which is why moves are preferred to clones.

Acyclic refcounted values are freed deterministically, one allocation at a time,
as their reference counts drop to zero.

## `Rc<T>`: shared mutable cells

`Rc<T>` is the builtin shared cell. `Rc::new(value)` allocates a cell and
returns a handle. An `Rc` is move-only; use `.clone()` to create another handle
to the *same* cell. Mutation through any handle is visible through all of them.
This is the one controlled exception to value semantics, and it is how shared
mutable state is expressed.

The API is small:

| Operation | Meaning |
| --- | --- |
| `Rc::new(v)` | Create a cell holding `v`. Moves `v` into the cell. |
| `r.clone()` | Another handle to the same cell; the reference count grows. |
| `r.get()` | Read the payload (returns an owned copy of it). |
| `r.set(v)` | Replace the payload with `v` (moved in); releases the old payload. Works on an immutable binding; the cell is mutable even though the binding is not. |
| `r.strong_count()` | Number of live handles to the cell, including `r` itself. |

Notes:

* `set` is the one method that does not require a mutable binding, because it
  mutates the shared cell, not the local handle. `get()` returns a copy, so
  reading does not expose a reference.
* Two `Rc` handles compare equal with `==` if and only if they point at the
  **same cell** — identity comparison, not payload comparison. This makes
  comparing handles of a cycle terminate.
* `Display`/`Debug` of an `Rc` print `Rc(payload)`, using the payload's own
  formatting: `Rc(5)`, `Rc("x")` for `Rc<String>`, `Rc([1, 2])` for
  `Rc<Vec<i64>>`.

```ls
struct Counter {
    hits: i64,
}

fn main() {
    let cell = Rc::new(Counter { hits: 0 });
    let first = cell.clone();
    let second = cell.clone();
    println!("{}", first.strong_count());

    second.set(Counter { hits: 7 });
    println!("{} {} {}", cell.get().hits, first.get().hits, second.get().hits);

    println!("{}", first == second);
    println!("{}", cell == Rc::new(Counter { hits: 7 }));
    println!("{:?}", cell);
}
```

Output:

```text
3
7 7 7
true
false
Rc(Counter { hits: 7 })
```

Passing a cloned handle to a helper shares the cell, so the helper's `set` is
visible to the caller. Passing an owned value such as a `String` by value moves
it instead, giving the helper the only handle.

## Cycles

A reference cycle is possible only through `Rc<T>`, because `Rc` is the only
type whose handles share one mutable payload. A cycle exists when a chain of
handles starting from a cell leads back to that same cell:

```rust
struct Node {
    value: i64,
    next: Option<Rc<Node>>, // a handle to another node
}

// first -> second -> first  is a cycle.
```

Acyclic values cannot form cycles: moving an owned `String`, `Vec` or `Box`
never produces a second handle to the same payload. Non-owning references
cannot form a cycle either, because they are never stored inside a value and
never counted. There is also no global mutable state and no weak reference type.

The language does not prevent cycles. Instead, an ORC cycle collector (the
Bacon–Rajan trial-deletion algorithm) runs automatically once enough candidate
subgraphs have accumulated. When it finds a subgraph whose internal references
only point within the subgraph (no external handles), it frees the whole
subgraph, running the `Drop` hooks of the collected values. Its guarantees are:

* Acyclic garbage is reclaimed immediately, by ordinary refcount decrements.
* Cyclic garbage becomes reclaimable once no external handles remain, without
  any user call. There is no `collect()` function and no way to force a
  collection.
* A cycle that is still reachable from a live handle is never collected.
* The collector is not a tracing GC: it never moves objects, and it runs only
  over candidate subgraphs, in bounded increments at allocation and release
  points.

```ls
struct Node {
    value: i64,
    next: Option<Rc<Node>>,
}

impl Drop for Node {
    fn drop(&mut self) {
        println!("drop node {}", self.value);
    }
}

fn main() {
    let first = Rc::new(Node { value: 1, next: None });
    let second = Rc::new(Node { value: 2, next: Some(first.clone()) });
    first.set(Node { value: 1, next: Some(second.clone()) });
    // first -> second -> first is a cycle; both counts include the cycle edges.
    println!("{} {}", first.get().value, second.get().value);
    println!("{} {}", first.strong_count(), second.strong_count());
    // Both local handles are gone at the end of main; the collector reclaims
    // the cycle and its Drop hooks run. With leak tracking enabled this program
    // reports no leaks.
}
```

Output:

```text
drop node 1
1 2
drop node 2
drop node 1
2 2
drop node 2
drop node 1
```

## `Box<T>`: owned indirection

`Box<T>` is an owned heap allocation with ordinary value semantics. It exists
for two reasons: recursive data types need indirection (a struct cannot contain
itself directly), and moving a large value can be done by moving a pointer
instead of copying the payload. A `Box` is move-only; `.clone()` makes a deep
copy of the payload.

The API:

| Operation | Meaning |
| --- | --- |
| `Box::new(v)` | Allocate a box holding `v` (moved in). |
| `b.clone()` | A deep copy: a new box with a cloned payload. |
| `b.get()` | Read the payload (returns an owned copy of it). |
| `b.set(v)` | Replace the payload of this box with `v` (moved in). |

Rules:

* `b.get()` copies the payload out; for a move-only payload the copy is a
  clone-like owned duplicate.
* `==` compares box payloads structurally (unlike `Rc`, which compares
  identities).
* `Display`/`Debug` of a box print the payload with the payload's formatting, so
  `Box<String>` displays as `hi` and debugs as `"hi"`.
* `Box<T>` works with `Vec`, `Option` and `Rc`, and is the standard way to build
  recursive lists and trees. A box inside an `Rc` cell is a shared mutable
  subtree — the `Rc` provides sharing and the `Box` provides the shape.

```ls
struct Node {
    value: i64,
    next: Option<Box<Node>>,
}

fn sum(node: Option<Box<Node>>) -> i64 {
    match node {
        Some(boxed) => {
            let value = boxed.get().value;
            value + sum(boxed.get().next.clone())
        }
        None => 0,
    }
}

fn main() {
    let mut head: Option<Box<Node>> = None;
    for value in 1..=4 {
        head = Some(Box::new(Node { value, next: head }));
    }
    println!("{}", sum(head));

    let boxed = Box::new(10);
    let mut copied = boxed.clone();
    copied.set(20);
    println!("{} {} {}", boxed.get(), copied.get(), boxed == Box::new(10));
}
```

Output:

```text
10
10 20 true
```

Because a box clone is deep, replacing the payload through one handle never
affects another handle. That is exactly the value semantics of every other
owned value; the box only changes *where* the payload lives and enables recursive
types.

## Leak tracking

Reference counting cannot reclaim a cycle if the collector is disabled or if the
program exits abnormally. lsaca therefore provides leak tracking, compiled into
the runtime:

* in the Debug profile (the default for `lsaca run` and `lsaca build`) the
  compiler builds the runtime with `-DLSACA_DEBUG`, which enables the tracking
  code and the report; the direct backend passes the flag to the C compiler,
  and an emitted CMake project sets it in its Debug configuration;
* to track and report in *every* profile, including Release, set the environment
  variable `LSACA_DEBUG_RC=1` when invoking the compiler - not the program. The
  compiler reads it at build time and adds `-DLSACA_DEBUG_RC` to the C
  compilation (`LSACA_DEBUG_RC=1 lsaca build -r prog.ls`); when you configure
  an emitted project with CMake instead, the equivalent is
  `-DLSACA_DEBUG_RC=ON`;
* at shutdown, after the final collection, if allocations are still live the
  runtime prints to stderr:

```text
lsaca: warning: 2 reference-counted allocation(s) leaked
lsaca:   leaked allocation of 40 bytes
lsaca:   leaked allocation of 24 bytes
```

Only allocations that are genuinely unreachable-but-not-freed (or still
reachable from a static, which does not exist) are reported. Reaching the end of
`main` is a normal shutdown and triggers the report; a panic or an explicit
`exit(n)` terminates the process without running the final collection, so those
paths never print a leak report. The same shutdown paths skip pending `Drop`
hooks.

For a correct program the report is empty. A non-empty report means either that
the program used `Rc` in a way the collector did not finish reclaiming (for
example a cycle reachable from nothing but not yet visited, which is a
collector-liveness question, not a memory-safety one) or a compiler/runtime bug.
In either case it is not something user code can control; treat it as a bug
report.

## References: non-owning views

Borrowing is free. Creating a reference with `&place` or `&mut place` produces a
plain C pointer and does not touch any reference count; passing a reference to a
function, returning it, or copying the reference copies that pointer. No
`retain` or `release` happens for references, and a reference never keeps its
referent alive: the owner — a local, a parameter, or a value stored in a local —
is responsible for the value's lifetime.

That split of responsibility is the whole memory model:

* **ORC** owns and frees all values (owned values are refcounted; cycles are
  collected by trial deletion) and runs `Drop` hooks when a value is released.
* **The borrow checker** only checks *aliasing*: it ensures that a reference is
  never used after its referent is gone, that a mutable borrow is exclusive, and
  that a value is not moved while a reference to it is live (`E0505`). It never
  decides when a value is freed.

Version 1.4 also allows mutable borrows of owned heap values: `&mut String`,
`&mut Vec<T>`, `&mut Rc<T>` and `&mut Box<T>` are legal as parameters, locals and
return types, and mutating methods work through them. References still cannot be
stored inside aggregates or captured by closures; they live in locals,
parameters and return values.

```ls
fn append(text: &mut String) {
    text.push_str("!");
}

fn grow(values: &mut Vec<i64>) {
    values.push(4);
}

fn bump(cell: &mut Rc<i64>) {
    cell.set(cell.get() + 1);
}

fn main() {
    let mut text = String::from("hi");
    append(&mut text);
    println!("{}", text);

    let mut values = vec![1, 2, 3];
    grow(&mut values);
    println!("{:?}", values);

    let mut cell = Rc::new(5);
    bump(&mut cell);
    println!("{}", cell.get());
}
```

Output:

```text
hi!
[1, 2, 3, 4]
6
```

A shared reference read is just as cheap:

```ls
fn main() {
    let text = String::from("hello");
    let r = &text;
    println!("{} {}", text.len(), r.len());
}
```

Output:

```text
5 5
```

See [References and borrowing](12-references-and-borrowing.md) for the rules.

## Summary

* A value has one owner. Copy values are copied implicitly; every other owned
  value is moved by default, and using it after a move is `E0382`.
* `.clone()` is derived automatically for every type except `Drop` types and
  produces an independent value (`Rc` clones share the cell).
* `Drop` hooks run when a value is released: hook first, then fields, locals in
  reverse order, parameters in reverse order, temporaries at the end of the
  statement. `exit(n)` and panics skip pending drops.
* `String` and `Vec` clones are independent; the runtime may use copy-on-write
  internally, and moves transfer buffers without copying.
* Iterator values own the `Vec<T>` they were created from; terminal operations
  produce ordinary values. A `HashMap<K, V>` owns its keys and values and
  `.clone()` produces an independent table.
* Since 1.5.0 `==` compares `Vec`, `Option`, `Result`, tuples, user structs and
  enums, references and `HashMap` structurally; function values have no `==`.
* `Rc<T>` provides shared mutable cells; its handles are move-only and
  `.clone()` shares the cell; `==` is identity.
* `Box<T>` provides owned indirection with deep-clone value semantics; it
  enables recursive types.
* Cycles exist only through `Rc<T>` and are collected automatically by the
  trial-deletion collector, which runs the collected values' `Drop` hooks.
* References (`&T`/`&mut T`) are non-owning pointers: borrowing never changes a
  reference count and never keeps a value alive; the borrow checker enforces
  aliasing and rejects moves of borrowed values.
* Leak tracking is opt-in and reports only at a normal shutdown.
* Panics and `exit` terminate the process; the operating system reclaims all
  memory.

## What next

* The move-by-move rules: [Ownership and moves](11-ownership-and-moves.md).
* Variables, mutability and constants: [Variables and
  mutability](06-variables-and-mutability.md).
* The `String`, `Vec`, `Option`, `Result`, `Rc` and `Box` APIs:
  [Standard library](15-standard-library.md).
* Non-owning views of values and their rules: [References and
  borrowing](12-references-and-borrowing.md).
* Closures capture by value and move their captures: [Closures](14-closures.md).
* Generics, traits, iterators and the hash map: [Generics](17-generics.md),
  [Traits](18-traits.md), [Iterators](19-iterators.md), [HashMap](20-hashmap.md).
