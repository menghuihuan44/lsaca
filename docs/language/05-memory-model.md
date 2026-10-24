# Memory model: ORC

lsaca uses **ORC** — *Owned Reference Counting*: automatic reference counting
for owned values plus a trial-deletion cycle collector. It gives deterministic
destruction of acyclic data, transparent sharing of immutable buffers, and safe
collection of cycles, without the move/borrow rules of Rust and without a
stop-the-world tracing garbage collector as in Go or the JVM.

This page is the authoritative description of value ownership, copying,
copy-on-write mutation, `Rc<T>`, `Box<T>`, cycles and leak tracking. Reading it
once makes the rest of the language predictable; the rules are few.

## The two kinds of representation

Every lsaca value is represented in one of two ways:

1. **Plain values** are stored directly and copied bit by bit: the scalar types
   `bool`, all integer and float types, and `()`. Copying an `i64` copies eight
   bytes; there is no sharing and nothing to free.
2. **Reference-counted handles** are stored indirectly: the payload lives in a
   heap allocation with a reference count, and the value you hold is a pointer
   plus a way to adjust that count. Copying the value increments the count;
   dropping the last handle frees the payload.

Types whose values are refcounted handles, possibly at several levels:

* `String` and every `Vec<T>`;
* `Option<T>` and `Result<T, E>` when `T` or `E` contains a handle (an
  `Option<i64>` is a tag plus a plain integer, not a heap pointer — but the
  language treats ownership uniformly);
* tuples, structs and enums that contain any handle;
* `Box<T>` (always an indirection, added in 1.1.0);
* `Rc<T>` handles (the cell itself is shared and mutable);
* function values and closures.

A program never observes counts directly except through `Rc<T>::strong_count`;
the representation is an implementation detail unless you enable leak tracking
or measure performance.

## Ownership and copying

**lsaca has no concept of a move.** Every assignment, argument passing, return,
capture and container insertion *copies* the value. For refcounted values the
copy is cheap (a reference-count increment); for plain values it is a bit copy.

Consequences that differ from Rust:

* Using a variable after giving its value away is always allowed. There is no
  "use of moved value" error.
* There is no `Clone` trait to call: copying is implicit, so `String::from(s)` is
  only needed to construct from a literal, never to "clone".
* A function receives its arguments by value (conceptually); the caller's
  bindings remain valid and keep their values.

```ls
fn describe(text: String, number: i64) {
    println!("{} / {}", text, number);
}

fn main() {
    let greeting = String::from("hello");
    let count = 42;
    describe(greeting, count);
    // Both original bindings are still usable: the call copied them.
    println!("{} / {}", greeting, count);
}
```

Output:
```text
hello / 42
hello / 42
```

## Destruction

A value is destroyed when its last handle disappears. In practice:

* a local variable is released at the end of its block, in reverse order of
  declaration, after the block's tail expression has been evaluated;
* function parameters belong to the function body and are released when the
  body exits;
* temporaries produced inside a statement are released at the end of that
  statement, except that a returned value is transferred to the caller;
* early exit — `return`, `break`, `continue`, or a panic — releases everything
  that goes out of scope exactly as normal scope exit would.

There is no `drop` method and no way for user code to observe destruction
directly, except that dropping the last `Rc` handle of an unreachable cycle
eventually triggers the cycle collector.

```ls
fn main() {
    {
        let inner = String::from("inner");
        println!("{}", inner);
    } // 'inner' is released here

    let outer = String::from("outer");
    println!("{}", outer);
    // 'outer' is released when main returns
}
```

Output:
```text
inner
outer
```

## Copy-on-write mutation

`String` and `Vec<T>` support mutation through `let mut` bindings. The buffer is
shared between copies, so mutating a shared buffer would violate the value
semantics that everything else in the language follows. The runtime therefore
implements **copy-on-write**: when a buffer's reference count is 1 it is mutated
in place; when it is shared, the mutating handle first clones the buffer and
mutates the clone. The other handles keep the old buffer.

The consequence is simple and important: **two variables copied from the same
`String` or `Vec` never observe each other's mutations.**

```ls
fn main() {
    let mut original = vec![1, 2, 3];
    let snapshot = original;
    original.push(4);
    original.push(5);
    println!("{:?}", original);
    println!("{:?}", snapshot);

    let mut text = String::from("abc");
    let text_copy = text;
    text.push_str("def");
    println!("{} {}", text, text_copy);
}
```

Output:
```text
[1, 2, 3, 4, 5]
[1, 2, 3]
abcdef abc
```

Copy-on-write is the only place where the runtime optimizes sharing for owned
values. It is invisible in the language semantics: `snapshot` is simply a vector
with the old contents. Acyclic refcounted values are freed deterministically,
one allocation at a time, as counts drop to zero.

## Value semantics for aggregates

Structs, enums and tuples have value semantics. Copying one copies all its
fields; for refcounted fields the copy increments their counts (so the payloads
are shared until a mutation forces COW, for `String`/`Vec`), for plain fields it
copies the bits.

```ls
struct Account {
    owner: String,
    balance: i64,
}

fn main() {
    let mut a = Account { owner: String::from("alice"), balance: 10 };
    let b = a; // value copy: b is independent
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

## `Rc<T>`: shared mutable cells

`Rc<T>` is the builtin shared cell. `Rc::new(value)` allocates a cell and
returns a handle; copying the handle (assignment, passing, returning, storing in
a container, capturing) shares the *same* cell. There is no copy-on-write for
`Rc`: mutation through any handle is visible through all of them. This is the
one controlled exception to value semantics, and it is how shared mutable state
is expressed.

The API is small:

| Operation | Meaning |
| --- | --- |
| `Rc::new(v)` | Create a cell holding `v`. |
| `r.get()` | Read the payload (returns a copy). |
| `r.set(v)` | Replace the payload. Works on an immutable binding; the cell is mutable even though the handle binding is not. |
| `r.strong_count()` | Number of live handles to the cell, including `r` itself. |

Notes:

* `set` is the one method that does not require a mutable binding, because it
  mutates the shared cell, not a local value. `get()` returns a copy, so reading
  does not expose a reference.
* Two `Rc` handles compare equal with `==` if and only if they point at the
  **same cell** — identity comparison, not payload comparison. This makes
  comparing handles of a cycle terminate.
* `Display`/`Debug` of an `Rc` print `Rc(payload)`, using the payload's own
  formatting: `Rc(5)`, `Rc("x")` for `Rc<String>`, `Rc([1, 2])` for `Rc<Vec<i64>>`.

```ls
struct Counter {
    hits: i64,
}

fn main() {
    let cell = Rc::new(Counter { hits: 0 });
    let first = cell;
    let second = cell;
    println!("{}", cell.strong_count());

    second.set(Counter { hits: 7 });
    println!("{} {} {}", cell.get().hits, first.get().hits, third(cell));

    println!("{}", first == second);
    println!("{}", cell == Rc::new(Counter { hits: 7 }));
    println!("{:?}", cell);
}

fn third(cell: Rc<Counter>) -> i64 {
    cell.get().hits
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

Because a parameter is an owned copy of the argument, passing an `Rc` handle to a helper
shares the cell: the helper can mutate it through `set` and the caller observes the change.
That is the intended way to write mutable helper functions; passing an owned value
such as `String` by value always gives the helper an independent copy.

## Cycles

A reference cycle is possible only through `Rc<T>`, because `Rc` is the only
type whose copies share one mutable payload. A cycle exists when a chain of
handles starting from a cell leads back to that same cell:

```rust
struct Node {
    value: i64,
    next: Option<Rc<Node>>, // a handle to another node
}

// first -> second -> first  is a cycle.
```

Acyclic values cannot form cycles: copying an owned `String`, `Vec` or `Box`
never produces a second handle to the *same* payload, and there are no
references or pointers. There is also no global mutable state and no weak
reference type.

The language does not prevent cycles. Instead, an ORC cycle collector (the
Bacon–Rajan trial-deletion algorithm) runs automatically in the background once
enough candidate subgraphs have accumulated. When it finds a subgraph whose
internal references only point within the subgraph (no external handles), it
frees the whole subgraph. Its guarantees are:

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

fn main() {
    let first = Rc::new(Node { value: 1, next: None });
    let second = Rc::new(Node { value: 2, next: Some(first) });
    first.set(Node { value: 1, next: Some(second) });
    // first -> second -> first is a cycle; both counts include the cycle edges.
    println!("{} {}", first.get().value, second.get().value);
    println!("{} {}", first.strong_count(), second.strong_count());
    // Both local bindings are gone at the end of main; the collector reclaims
    // the cycle. With leak tracking enabled this program reports no leaks.
}
```

Output:
```text
1 2
2 2
```

## `Box<T>`: owned indirection

`Box<T>`, new in 1.1.0, is an owned heap allocation with ordinary value
semantics. It exists for two reasons: recursive data types need indirection (a
struct cannot contain itself directly), and moving a large value can be done by
moving a pointer instead of copying the payload — except that lsaca's `Box`
copies deeply when copied, so it is primarily a *structural* tool.

The API:

| Operation | Meaning |
| --- | --- |
| `Box::new(v)` | Allocate a box holding `v`. |
| `b.get()` | Read the payload (returns a copy). |
| `b.set(v)` | Replace the payload of this box. |

Rules:

* A `Box` copy is a **deep copy**: the payload is copied, so two boxes never
  alias. `b.get()` also copies the payload out.
* `==` compares box payloads structurally (unlike `Rc`, which compares
  identities).
* `Display`/`Debug` of a box print the payload with the payload's formatting, so
  `Box<String>` displays as `hi` and debugs as `"hi"`.
* `Box<T>` works with `Vec`, `Option` and `Rc`, and is the standard way to build
  recursive lists and trees. Because a box copies deeply, a box inside an
  `Rc` cell is a shared mutable subtree — the `Rc` provides sharing and the
  `Box` provides the shape.

```ls
struct Node {
    value: i64,
    next: Option<Box<Node>>,
}

fn sum(node: Option<Box<Node>>) -> i64 {
    match node {
        Some(boxed) => {
            let inner = boxed.get();
            inner.value + sum(inner.next)
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
    let mut copied = boxed;
    copied.set(20);
    println!("{} {} {}", boxed.get(), copied.get(), boxed == Box::new(10));
}
```

Output:
```text
10
10 20 true
```

Because `Box` copies deeply, replacing the payload through one handle never
affects another handle. That is exactly the value semantics of every other
owned value; the box only changes *where* the payload lives and enables recursive
types.

## Leak tracking

Reference counting cannot reclaim a cycle if the collector is disabled or if the
program exits abnormally. lsaca therefore provides optional leak tracking,
compiled into the Debug runtime:

* the compiler's generated project uses `-DLSACA_DEBUG` in the Debug profile,
  which enables the tracking code;
* set the environment variable `LSACA_DEBUG_RC=1` when running the program (or
  pass `-DLSACA_DEBUG_RC=ON` to CMake when building a Release project) to turn
  reporting on;
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
paths never print a leak report.

For a correct program the report is empty. A non-empty report means either that
the program used `Rc` in a way the collector did not finish reclaiming (for
example a cycle reachable from nothing but not yet visited, which is a
collector-liveness question, not a memory-safety one) or a compiler/runtime bug.
In either case it is not something user code can control; treat it as a bug
report.

## Summary

* Owned values are refcounted handles; scalars are plain.
* Copying is always allowed and always cheap for owned values; there are no
  move errors.
* `String` and `Vec` mutate copy-on-write; structs, enums and tuples have value
  semantics.
* `Rc<T>` provides shared mutable cells; copies of a handle share one payload;
  `==` is identity.
* `Box<T>` provides owned indirection with deep-copy value semantics; it enables
  recursive types.
* Cycles exist only through `Rc<T>` and are collected automatically by the
  trial-deletion collector.
* Leak tracking is opt-in and reports only at a normal shutdown.
* Panics and `exit` terminate the process; the operating system reclaims all
  memory.

## What next

* Variables, mutability and constants: [Variables and
  mutability](06-variables-and-mutability.md).
* The `String`, `Vec`, `Option`, `Result`, `Rc` and `Box` APIs:
  [Standard library](13-standard-library.md).
* Closures capture by value; read about them with the memory model in mind:
  [Closures](12-closures.md).
