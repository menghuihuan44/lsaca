# References and borrowing

Version 1.3 added references to lsaca, and version 1.4 adds ownership, moves, the
`Drop` trait and `.clone()` (see [Ownership and moves](11-ownership-and-moves.md)).
A reference is a **non-owning view of a value that already exists somewhere else**. You borrow a value with `&` (shared) or `&mut`
(mutable), use it through a dereference, and give it back — the ORC memory model keeps
the underlying value alive and tracks every allocation exactly as before.

References are the second way to avoid copying data (the first is `Rc<T>`, the shared
*mutable* cell). They are deliberately small: there are **no lifetimes you can write**,
no reference-typed fields, no references inside closures, and no raw pointers. The
compiler infers how long each borrow lives — the borrow ends at the last point where the
reference is used — and rejects code that would let two conflicting accesses happen at
the same time.

This page covers:

- the two reference types and how to create them;
- dereferencing, auto-dereferencing, and writing through a reference;
- the aliasing rules (shared vs. mutable borrows) and what "the borrow ends at its last
  use" means in practice;
- reference parameters, reference returns, and receiver methods;
- match ergonomics (matching a reference without writing `&` everywhere);
- where references are allowed and where the compiler refuses them;
- the interaction with ORC, `Rc<T>` and `Box<T>`, plus the generated C representation.

If you have written Rust, the syntax will look familiar, but the checker underneath is
not Rust's borrow checker. A reference is still a non-owning, checked pointer: it does
not keep its referent alive on its own, and the compiler only accepts a borrow when an
owner provably outlives its last use. There are no lifetime annotations — a borrow ends at
the last point where the reference is used — and the checker is conservative inside a
single expression (two mutable borrows there are rejected even when Rust would accept
them; see [Known limitations](#known-limitations-in-version-160)). Since 1.4 the ownership
and move checks (E0382, E0507, E0505) run alongside the reference checks, so moving a
value while a borrow of it is live is rejected too.

## Reference types

There are exactly two reference types:

| Type | Written | Meaning |
| --- | --- | --- |
| shared reference | `&T` | read-only view of a `T`; any number of shared references to the same value may exist at once |
| mutable reference | `&mut T` | view that also permits writing the referent; at most one mutable reference to a value may exist at a time, and no shared reference may overlap with it |

`T` can be any type lsaca supports, including `String`, `Vec<i64>`, `Rc<i64>`,
`Box<i64>`, tuples, structs and enums:

```ls
struct Point { x: i64, y: i64 }
fn main() {
    let number = 10;
    let a: &i64 = &number;
    let b: &&i64 = &a;
    let text = String::from("hi");
    let r: &String = &text;
    let values = vec![1, 2, 3];
    let rv: &Vec<i64> = &values;
    let point = Point { x: 1, y: 2 };
    let rp: &Point = &point;
    println!("{} {} {} {} {}", *a, **b, r.len(), rv.len(), rp.x);
}
```

Output:
```text
10 10 2 3 1
```

There is no lifetime syntax in lsaca: you cannot write `&'a T`, and `&` never takes a
label. The compiler computes lifetimes itself, using the elision rules described in
[References and functions](#references-and-functions).

A reference is a value in its own right: it can be copied, reassigned, stored in a
local, passed to a function and returned from one. Copying a reference never copies or
retains the referent:

```ls
fn main() {
    let value = 10;
    let first = &value;
    let second = first;
    let third = second;
    println!("{} {} {}", *first, *second, *third);
}
```

Output:
```text
10 10 10
```

## Borrow expressions

You create a reference with a *borrow expression*: `&place` or `&mut place`. The
operand must be a **place expression** — something that lives in memory, not a computed
temporary:

| Place expression | Example |
| --- | --- |
| a local binding or parameter | `&value`, `&mut count` |
| a field of a struct or enum payload | `&point.x` |
| a tuple element | `&pair.0` |
| a vector element | `&values[1]` |
| a dereferenced reference | `&*r`, `&mut *r` |
| a constant | not a place: constants are inlined (error E0745) |
| a temporary (`1 + 2`, `String::from("x")`, a call result) | not a place (error E0745) |

Borrowing a local, a field, a tuple element and a vector element:

```ls
struct Point { x: i64, y: i64 }
fn main() {
    let pair = (1, 2);
    let point = Point { x: 3, y: 4 };
    let values = vec![5, 6];
    let first = &pair.0;
    let field = &point.x;
    let element = &values[1];
    println!("{} {} {}", *first, *field, *element);
}
```

Output:
```text
1 3 6
```

The mutable variant requires a mutable place — the local must be declared `let mut`:

```ls
struct Point { x: i64, y: i64 }
fn main() {
    let mut pair = (1, 2);
    let a = &mut pair.0;
    *a = 10;
    let mut point = Point { x: 0, y: 0 };
    let b = &mut point.x;
    *b = 7;
    let mut values = vec![1, 2, 3];
    let c = &mut values[1];
    *c = 20;
    println!("{} {} {} {:?}", pair.0, point.x, values[1], values);
}
```

Output:
```text
10 7 20 [1, 20, 3]
```

You can reborrow through an existing reference with `&*r` / `&mut *r`, and a reference
can itself be borrowed (`&&mut i64` is a valid type):

```ls
fn main() {
    let mut number = 1;
    let exclusive = &mut number;
    let shared: &i64 = &*exclusive;
    println!("{}", *shared);
    let nested: &&mut i64 = &exclusive;
    println!("{}", **nested);
}
```

Output:
```text
1
1
```

Trying to take a mutable reference to an immutable binding is an error:

```ls,compile-fail
fn main() {
    let value = 1;
    let r = &mut value;
    *r = 2;
    println!("{}", value);
}
```

Error:
```text
error[E0596]: cannot borrow this value as mutable: the binding is not declared as mutable
  --> example-06.ls:3:13
   |
 3 |     let r = &mut value;
   |             ^^^^^^^^^^

lsaca: example-06.ls: could not compile example-06.ls due to previous error(s)
```

Temporaries and constants cannot be borrowed at all — bind the value to a local first:

```ls,compile-fail
fn main() {
    let r = &(1 + 2);
    println!("{}", *r);
}
```

Error:
```text
error[E0745]: cannot borrow a temporary value, bind it to a local first
  --> example-07.ls:2:13
   |
 2 |     let r = &(1 + 2);
   |             ^^^^^^^

lsaca: example-07.ls: could not compile example-07.ls due to previous error(s)
```

```ls,compile-fail
const LIMIT: i64 = 10;
fn main() {
    let r = &LIMIT;
    println!("{}", *r);
}
```

Error:
```text
error[E0745]: cannot borrow a temporary value, bind it to a local first
  --> example-08.ls:3:13
   |
 3 |     let r = &LIMIT;
   |             ^^^^^^

lsaca: example-08.ls: could not compile example-08.ls due to previous error(s)
```

## Dereferencing

The unary `*` operator reads the value a reference points at. It is a real operator — it
binds tighter than binary operators, so `*r + 1` means `(*r) + 1`, and `*r * 2` means
`(*r) * 2`:

```ls
fn main() {
    let number = 5;
    let r = &number;
    println!("{} {}", *r + 1, *r * 2);
}
```

Output:
```text
6 10
```

For a mutable reference, the dereference is also an assignable place: `*r = value` and
compound assignments such as `*r += 1` write through the reference into the referent:

```ls
fn main() {
    let mut number = 5;
    {
        let r = &mut number;
        *r += 4;
    }
    println!("{}", number);
    {
        let r = &mut number;
        *r = 1;
    }
    println!("{}", number);
}
```

Output:
```text
9
1
```

For aggregates, field, tuple and index syntax works directly through a reference: the
same auto-dereference that applies to reads also applies to writes through a `&mut`
reference (`r.x += 10`). Dereferencing the whole value and then using normal field
syntax (`(*r).x`) is equivalent:

```ls
struct Point { x: i64, y: i64 }
fn main() {
    let mut point = Point { x: 1, y: 2 };
    {
        let r = &mut point;
        r.x += 10;
    }
    println!("{} {}", point.x, point.y);
    {
        let r = &mut point;
        *r = Point { x: 5, y: 6 };
    }
    println!("{} {}", point.x, point.y);
}
```

Output:
```text
11 2
5 6
```

### Auto-dereferencing

You do not have to spell out `(*r)` for reads, and field assignment through a `&mut`
reference auto-dereferences too. Field access, method calls and indexing automatically
dereference as many references as needed:

```ls
struct Bag { values: Vec<i64> }
impl Bag {
    fn first(&self) -> &i64 { &self.values[0] }
}
fn main() {
    let bag = Bag { values: vec![7, 8] };
    let r = &bag;
    println!("{}", r.first());
    println!("{}", r.values[1]);
    let values = vec![10, 20];
    let rv = &values;
    println!("{}", rv.len());
    println!("{}", rv[1]);
}
```

Output:
```text
7
8
2
20
```

Writing a field through a reference does not require the reference binding itself to be
mutable (`let r = &mut x` is enough) — only the referent must have been borrowed
mutably. Both the plain field form and the explicit dereference form write the referent:

```ls
struct Point { x: i64 }
fn main() {
    let mut point = Point { x: 1 };
    let r = &mut point;
    r.x = 2;
    (*r).x = 3;
    println!("{}", point.x);
}
```

Output:
```text
3
```

Writing through a shared reference is always an error:

```ls,compile-fail
fn main() {
    let value = 1;
    let r = &value;
    *r = 2;
    println!("{}", value);
}
```

Error:
```text
error[E0596]: cannot assign to immutable value: the binding has not been declared as mutable
  --> example-14.ls:4:5
   |
 4 |     *r = 2;
   |     ^^

lsaca: example-14.ls: could not compile example-14.ls due to previous error(s)
```

The dereference operator only works on references. `Rc<T>` and `Box<T>` are not
pointers and cannot be dereferenced — use their `get`/`set` methods instead:

```ls,compile-fail
fn main() {
    let boxed = Box::new(1);
    println!("{}", *boxed);
}
```

Error:
```text
error[E0614]: type Box<{integer}> cannot be dereferenced
  --> example-15.ls:3:20
   |
 3 |     println!("{}", *boxed);
   |                    ^^^^^^

lsaca: example-15.ls: could not compile example-15.ls due to previous error(s)
```

### Dereferencing in expressions and conditions

A dereference is an ordinary expression, so it works anywhere a value is expected:
conditions, arguments, arithmetic, format arguments and method arguments:

```ls
fn main() {
    let number = 3;
    let r = &number;
    if *r > 2 {
        println!("big");
    }
    let mut count = 0;
    let c = &mut count;
    while *c < 2 {
        *c = *c + 1;
    }
    println!("{}", count);
    assert_eq!(*r, 3);
}
```

Output:
```text
big
2
```

## Aliasing rules

The borrow checker enforces one rule, applied conservatively:

- **Any number of shared references** (`&T`) to the same place may exist at once.
- **At most one mutable reference** (`&mut T`) to a place may exist at a time.
- While a mutable reference is alive, **no shared reference to the same place may be
  created or used**, and the place may not be assigned directly.
- While shared references are alive, the place may not be mutated and no mutable
  reference may be created.

A borrow **ends at its last use**, not at the end of the enclosing block (a lightweight
form of non-lexical lifetimes). That means sequential borrows are fine:

```ls
fn main() {
    let mut value = 1;
    let first = &mut value;
    *first = 2;
    let second = &mut value;
    *second = 5;
    println!("{}", value);
}
```

Output:
```text
5
```

Borrows can be split across blocks so that mutable borrows never overlap:

```ls
fn main() {
    let mut values = vec![1, 2, 3];
    {
        let a = &mut values[0];
        *a = 10;
    }
    {
        let b = &mut values[1];
        *b = 20;
    }
    println!("{:?}", values);
}
```

Output:
```text
[10, 20, 3]
```

Inside a loop the borrow is re-created on every iteration and ends before the next
iteration begins:

```ls
fn main() {
    let mut values = vec![1, 2, 3];
    let mut total = 0;
    let mut i = 0;
    while i < values.len() {
        let r = &mut values[i];
        *r = *r + 10;
        total += *r;
        i += 1;
    }
    println!("{} {:?}", total, values);
}
```

Output:
```text
36 [11, 12, 13]
```

Two mutable borrows of the same place that overlap are rejected:

```ls,compile-fail
fn main() {
    let mut value = 1;
    let first = &mut value;
    let second = &mut value;
    *first = 2;
    *second = 3;
    println!("{}", value);
}
```

Error:
```text
error[E0499]: cannot borrow value as mutable more than once at a time
  --> example-20.ls:4:18
   |
 4 |     let second = &mut value;
   |                  ^^^^^^^^^^
   = note: example-20.ls:3:17: first mutable borrow occurs here

error[E0506]: cannot assign to value because it is borrowed
  --> example-20.ls:5:5
   |
 5 |     *first = 2;
   |     ^^^^^^
   = note: example-20.ls:4:18: mutable borrow occurs here

lsaca: example-20.ls: could not compile example-20.ls due to previous error(s)
```

Creating a shared borrow while a mutable borrow is still alive is rejected:

```ls,compile-fail
fn main() {
    let mut value = 1;
    let writer = &mut value;
    let reader = &value;
    *writer = 2;
    println!("{}", *reader);
}
```

Error:
```text
error[E0502]: cannot borrow value as immutable because it is also borrowed as mutable
  --> example-21.ls:4:18
   |
 4 |     let reader = &value;
   |                  ^^^^^^
   = note: example-21.ls:3:18: mutable borrow occurs here

error[E0506]: cannot assign to value because it is borrowed
  --> example-21.ls:5:5
   |
 5 |     *writer = 2;
   |     ^^^^^^^
   = note: example-21.ls:4:18: shared borrow occurs here

lsaca: example-21.ls: could not compile example-21.ls due to previous error(s)
```

Assigning to a place while it is borrowed is rejected:

```ls,compile-fail
fn main() {
    let mut value = 1;
    let reader = &value;
    value = 2;
    println!("{}", *reader);
}
```

Error:
```text
error[E0506]: cannot assign to value because it is borrowed
  --> example-22.ls:4:5
   |
 4 |     value = 2;
   |     ^^^^^
   = note: example-22.ls:3:18: shared borrow occurs here

lsaca: example-22.ls: could not compile example-22.ls due to previous error(s)
```

### Field and element granularity

For structs, different fields are treated as different places: you can hold a mutable
reference to one field and a shared or mutable reference to another field at the same
time.

```ls
struct Pair { left: i64, right: i64 }
fn main() {
    let mut pair = Pair { left: 1, right: 2 };
    let a = &mut pair.left;
    let b = &mut pair.right;
    *a = 10;
    *b = 20;
    println!("{} {}", pair.left, pair.right);
}
```

Output:
```text
10 20
```

Vectors are **not** split per element: borrowing one element borrows the whole vector,
so two mutable borrows of different elements overlap and are rejected. Update elements
one at a time, inside separate statements or blocks:

```ls,compile-fail
fn main() {
    let mut values = vec![1, 2, 3];
    let a = &mut values[0];
    let b = &mut values[1];
    *a = 10;
    *b = 20;
    println!("{:?}", values);
}
```

Error:
```text
error[E0499]: cannot borrow values as mutable more than once at a time
  --> example-24.ls:4:13
   |
 4 |     let b = &mut values[1];
   |             ^^^^^^^^^^^^^^
   = note: example-24.ls:3:13: first mutable borrow occurs here

error[E0506]: cannot assign to values because it is borrowed
  --> example-24.ls:5:5
   |
 5 |     *a = 10;
   |     ^^
   = note: example-24.ls:4:13: mutable borrow occurs here

lsaca: example-24.ls: could not compile example-24.ls due to previous error(s)
```

## References and moves

References never own their referent, so ownership still decides when the value
itself can be moved. A live borrow keeps the referent in place: moving it while
the borrow is still in use is `E0505`. Because a borrow ends at its last use, a
move afterwards is fine:

```ls
fn main() {
    let text = String::from("hi");
    let r = &text;
    println!("{}", *r);
    let owned = text;          // the borrow ended: the move is allowed
    println!("{}", owned);
}
```

Output:

```text
hi
hi
```

```ls,compile-fail
fn main() {
    let text = String::from("hi");
    let r = &text;
    let owned = text;          // error: text is borrowed by r
    println!("{} {}", *r, owned);
}
```

Error:

```text
error[E0505]: cannot move out of text because it is borrowed
  --> example-26.ls:4:17
   |
 4 |     let owned = text;          // error: text is borrowed by r
   |                 ^^^^
   = note: example-26.ls:3:13: shared borrow occurs here
   = note: a move would leave the reference pointing at a value the program no longer owns; clone the value instead of moving it

lsaca: example-26.ls: could not compile example-26.ls due to previous error(s)
```

References themselves are `Copy`: binding or passing a reference copies the
pointer and never moves the referent. A `&mut` reference is also exclusive — no
other borrow and no move of the referent may overlap it.

## References and functions

A reference parameter is declared with the reference type in the parameter list, and a
reference return type with `-> &T` or `-> &mut T`. A reference argument is passed
exactly like any other argument.

```ls
struct Point { x: i64, y: i64 }
fn double(value: &i64) -> i64 { *value * 2 }
fn first(values: &Vec<i64>) -> &i64 { &values[0] }
fn sum(point: &Point) -> i64 { point.x + point.y }
fn bump(point: &mut Point) {
    (*point).x = (*point).x + 1;
    (*point).y = (*point).y + 1;
}
fn main() {
    let n = 21;
    println!("{}", double(&n));
    let values = vec![1, 2];
    println!("{}", *first(&values));
    let mut point = Point { x: 1, y: 2 };
    println!("{}", sum(&point));
    bump(&mut point);
    println!("{}", sum(&point));
}
```

Output:
```text
42
1
3
5
```

Inside a `&mut` parameter, field writes auto-dereference just like reads: `point.x = ...`
writes the referent. The example above spells out `(*point).x = ...` to show which
borrowed value is being written; both forms are equivalent.

### Returning references and elision

A function may return a reference only when the compiler can tell which input it
borrows from. Since lsaca has no lifetime annotations, it uses two elision rules:

1. **Method receiver rule.** A method with a `&self` or `&mut self` receiver can return
   a reference derived from `self`; the receiver decides the lifetime even if the method
   has further reference parameters.
2. **Single input rule.** A free function must have *exactly one* reference parameter;
   the returned reference is assumed to borrow from that parameter. Zero reference
   parameters or two or more reference parameters are errors (E0106).

The returned reference must actually borrow from the chosen source — returning a
reference to a local value is an error (E0515):

```ls
struct Counter { count: i64 }
impl Counter {
    fn count(&self) -> &i64 { &self.count }
}
fn identity(counter: &Counter) -> &Counter { counter }
fn first(values: &Vec<i64>) -> &i64 { &values[0] }
fn main() {
    let counter = Counter { count: 7 };
    println!("{}", *counter.count());
    println!("{}", identity(&counter).count);
    let values = vec![4, 5];
    println!("{}", *first(&values));
}
```

Output:
```text
7
7
4
```

A `&mut` return type must borrow from a `&mut` source: returning a `&mut` that came from
a shared reference is rejected.

```ls
fn forward(value: &mut i64) -> &mut i64 { value }
fn to_shared(value: &mut i64) -> &i64 { value }
fn main() {
    let mut n = 4;
    let r = forward(&mut n);
    *r = 6;
    let shared = to_shared(&mut n);
    println!("{} {}", n, *shared);
}
```

Output:
```text
6 6
```

These are the resulting diagnostics for the invalid cases:

```ls,compile-fail
fn bad() -> &i64 {
    let value = 1;
    &value
}
fn main() {
    println!("{}", *bad());
}
```

Error:
```text
error[E0106]: missing lifetime specifier
  --> example-30.ls:1:13
   |
 1 | fn bad() -> &i64 {
   |             ^
   = note: this function's return type contains a borrowed value, but there is no reference input to borrow from

error[E0515]: returns a reference to a local value
  --> example-30.ls:3:5
   |
 3 |     &value
   |     ^^^^^^
   = note: the returned reference must borrow from a function parameter

lsaca: example-30.ls: could not compile example-30.ls due to previous error(s)
```

```ls,compile-fail
fn choose(first: &i64, second: &i64) -> &i64 {
    first
}
fn main() {
    let a = 1;
    let b = 2;
    println!("{}", *choose(&a, &b));
}
```

Error:
```text
error[E0106]: missing lifetime specifier
  --> example-31.ls:1:41
   |
 1 | fn choose(first: &i64, second: &i64) -> &i64 {
   |                                         ^
   = note: this function's return type contains a borrowed value, but the signature does not say which parameter it is borrowed from

lsaca: example-31.ls: could not compile example-31.ls due to previous error(s)
```

```ls,compile-fail
fn bad(source: &i64) -> &i64 {
    let local = 1;
    &local
}
fn main() {
    let n = 1;
    println!("{}", *bad(&n));
}
```

Error:
```text
error[E0515]: returns a reference to a local value
  --> example-32.ls:3:5
   |
 3 |     &local
   |     ^^^^^^
   = note: the returned reference must borrow from a function parameter

lsaca: example-32.ls: could not compile example-32.ls due to previous error(s)
```

A reference cannot outlive the value it borrows. The common case is storing a reference
to a block-local variable in a variable declared outside the block:

```ls,compile-fail
fn main() {
    let outer = 1;
    let mut r = &outer;
    {
        let inner = 2;
        r = &inner;
    }
    println!("{}", *r);
}
```

Error:
```text
error[E0597]: inner does not live long enough
  --> example-33.ls:6:13
   |
 6 |         r = &inner;
   |             ^^^^^^
   = note: example-33.ls:5:13: inner is dropped here
   = note: the reference must not outlive the value it points to; keep the referent alive longer or copy the value

lsaca: example-33.ls: could not compile example-33.ls due to previous error(s)
```

### Implicit coercions

References are converted automatically in these directions:

| From | To | Where |
| --- | --- | --- |
| `&mut T` | `&T` | calls, returns, `let` annotations, struct/tuple/vector literals |
| `&Rc<T>` | `&T` | same positions |
| `&Box<T>` | `&T` | same positions |

```ls
fn read(value: &i64) -> i64 { *value }
fn main() {
    let rc = Rc::new(3);
    let boxed = Box::new(7);
    let mut n = 9;
    println!("{} {}", read(&rc), read(&boxed));
    let shared: &i64 = &mut n;
    println!("{}", *shared);
}
```

Output:
```text
3 7
9
```

There are no other implicit conversions: an owned `String` does not coerce to a string
slice (there is no `&str` type), and a `Vec<i64>` value cannot be passed where
`&Vec<i64>` is expected without writing `&`.

## Method receivers

Methods take `self` by value, `&self` (shared view) or `&mut self` (mutable view). The
receiver forms are described in detail in
[Structs, enums and impl](10-structs-enums-and-impl.md); from the reference point of
view the important facts are:

- calling a `&self` method borrows the receiver immutably for the call; the receiver
  may itself be a reference (`r.add(4)` where `r: &mut Counter` auto-dereferences);
- calling a `&mut self` method requires that the receiver be mutable and not otherwise
  borrowed;
- a `&self` method may return a shared reference derived from `self`;
- a `&mut self` method may mutate `self` and may return a shared or mutable reference to
  its fields; a `&self` method can only return shared references.

Matching `self` inside a method follows the same ergonomics as matching any other
reference (see [Match ergonomics](#match-ergonomics)): the patterns bind references to
the fields.

```ls
struct Counter { count: i64 }
impl Counter {
    fn increment(&mut self) -> i64 {
        self.count = self.count + 1;
        self.count
    }
    fn current(&self) -> i64 { self.count }
}
fn main() {
    let mut counter = Counter { count: 0 };
    println!("{} {}", counter.increment(), counter.increment());
    let r = &mut counter;
    r.increment();
    println!("{} {}", counter.current(), r.current());
}
```

Output:
```text
1 2
3 3
```

A `&mut self` method may return a mutable reference to its fields, and callers can write
through it:

```ls
struct Counter { count: i64 }
impl Counter {
    fn count_mut(&mut self) -> &mut i64 {
        &mut self.count
    }
}
fn main() {
    let mut counter = Counter { count: 0 };
    *counter.count_mut() += 1;
    println!("{}", counter.count);
}
```

Output:
```text
1
```

Returning a mutable reference from a shared receiver is still rejected:

```ls,compile-fail
struct Counter { count: i64 }
impl Counter {
    fn count_mut(&self) -> &mut i64 {
        &mut self.count
    }
}
fn main() {}
```

Error:
```text
error[E0596]: cannot return a mutable reference from a shared reference source
  --> example-37.ls:3:28
   |
 3 |     fn count_mut(&self) -> &mut i64 {
   |                            ^
   = note: a &mut return type must borrow from a &mut parameter or from a &mut self receiver

error[E0596]: cannot borrow this value as mutable: the binding is not declared as mutable
  --> example-37.ls:4:9
   |
 4 |         &mut self.count
   |         ^^^^^^^^^^^^^^^

lsaca: example-37.ls: could not compile example-37.ls due to previous error(s)
```

## Match ergonomics

When the scrutinee of a `match`, `if let` or `while let` is a reference, the compiler
matches the *referent* and turns every binding into a reference automatically. This is
called match ergonomics: you write the plain pattern (`Some(value)`, `Point { x, y }`,
`(a, b)`) and the bindings come out as `&i64`, `&String`, and so on.

```ls
struct Point { x: i64, y: i64 }
enum Shape { Circle(f64), Square(f64) }

fn describe(point: &Point) -> i64 {
    match point {
        Point { x, y } => *x + *y,
    }
}

fn area(shape: &Shape) -> f64 {
    match shape {
        Shape::Circle(radius) => 3.0 * (*radius) * (*radius),
        Shape::Square(side) => (*side) * (*side),
    }
}

impl Shape {
    fn area_value(&self) -> f64 {
        match self {
            Shape::Circle(radius) => 3.0 * (*radius) * (*radius),
            Shape::Square(side) => (*side) * (*side),
        }
    }
}

fn main() {
    let point = Point { x: 3, y: 4 };
    println!("{}", describe(&point));
    let shape = Shape::Circle(2.0);
    println!("{}", area(&shape));
    println!("{}", shape.area_value());
    let maybe = Some(7);
    match &maybe {
        Some(value) => println!("{}", *value),
        None => println!("none"),
    }
    if let Some(value) = &maybe {
        println!("{}", *value);
    }
    let pair = (1, 2);
    match &pair {
        (a, b) => println!("{} {}", *a, *b),
    }
}
```

Output:
```text
7
12
12
7
7
1 2
```

The `&self` method above is worth a second look: matching `self` follows the same
ergonomics as matching any reference-typed local. The patterns bind references to the
fields (`radius` is `&f64` here), so the body dereferences with `*radius`. To match
*through* a reference with ergonomics, match a reference-typed local or parameter, as
`describe` and `area` do — or `self`, which behaves the same way.

Mutable references behave the same way: matching a `&mut` aggregate (or scalar) binds
`&mut` references, so the payload can be read with `*` and assigned through the binding,
with any number of bindings and with either a block arm or an expression arm:

```ls
struct Point { x: i64, y: i64 }
fn main() {
    let mut point = Point { x: 1, y: 2 };
    match &mut point {
        Point { x, y } => {
            *x = 10;
            *y = 20;
        }
    }
    println!("{} {}", point.x, point.y);
}
```

Output:
```text
10 20
```

A multi-binding pattern reads just as well, and the expression-arm form works for
`&mut` scalars too:

```ls
fn main() {
    let mut counter = 3;
    let r = &mut counter;
    match r {
        value => *value = 7,
    }
    println!("{}", counter);
}
```

Output:
```text
7
```

`while let` and `if let` bindings on a mutable reference are writable as well:

```ls
fn main() {
    let mut maybe = Some(1);
    while let Some(value) = &mut maybe {
        *value = *value + 1;
        break;
    }
    println!("{:?}", maybe);
}
```

Output:
```text
Some(2)
```

The same holds for an `if let` scrutinee:

```ls
fn main() {
    let mut maybe = Some(1);
    if let Some(value) = &mut maybe {
        *value = 9;
    }
    println!("{:?}", maybe);
}
```

Output:
```text
Some(9)
```

Reference patterns (`let &x = r`) are not supported; match the reference scrutinee
directly instead:

```ls,compile-fail
fn main() {
    let n = 5;
    let r = &n;
    let &x = r;
    println!("{}", x);
}
```

Error:
```text
error: reference patterns are not supported; match a reference scrutinee directly, bindings become references
  --> example-43.ls:4:9
   |
 4 |     let &x = r;
   |         ^

lsaca: example-43.ls: could not compile example-43.ls due to previous error(s)
```

## Where references are allowed

References are restricted to the places where their lifetime is obvious. They may be:

- local bindings, function parameters and function return values;
- method receivers (`&self`, `&mut self`);
- elements of expressions and arguments;
- reborrowed through other references;
- mutable references to owned heap values: `&mut String`, `&mut Vec<T>`,
  `&mut Rc<T>` and `&mut Box<T>` (new in version 1.4).

They may **not** be:

| Use | Result |
| --- | --- |
| stored in a struct or enum field | error E0498 |
| stored in a tuple, `Vec<T>`, `Option<T>`, `Result<T, E>` or `Box<T>` | error E0498 |
| captured by a closure | error E0498 |
| returned from a function that cannot name its source | error E0106 |
| used as the `for` iterator | error E0277 |

Since 1.5.0 references can be compared with `==`; the comparison reads the
pointed-to values, so `a == b` is the same as `*a == *b`.

Some examples of the storage and capture errors:

```ls,compile-fail
struct Holder { value: &i64 }
fn main() {
    let n = 1;
    let holder = Holder { value: &n };
    println!("{}", *holder.value);
}
```

Error:
```text
error[E0498]: references cannot be stored in this type (found &i64)
  --> example-44.ls:1:17
   |
 1 | struct Holder { value: &i64 }
   |                 ^^^^^

error[E0498]: references cannot be stored in this type (found &i64)
  --> example-44.ls:4:34
   |
 4 |     let holder = Holder { value: &n };
   |                                  ^^

lsaca: example-44.ls: could not compile example-44.ls due to previous error(s)
```

A closure may take references as parameters, but it may not capture one: closures
capture by value, references cannot be stored in the captured environment
(`E0498`), and the closure could outlive the borrowed value.

```ls,compile-fail
fn main() {
    let n = 5;
    let r = &n;
    let f = || *r;
    println!("{}", f());
}
```

Error:
```text
error[E0498]: references cannot be captured by closures
  --> example-45.ls:4:17
   |
 4 |     let f = || *r;
   |                 ^

lsaca: example-45.ls: could not compile example-45.ls due to previous error(s)
```

Mutable references to owned heap types are supported in version 1.6.0: a
`&mut String`, `&mut Vec<T>`, `&mut Rc<T>` or `&mut Box<T>` can be a parameter, a
local binding or a return value, and the callee mutates the payload in place:

```ls
fn append(text: &mut String) {
    text.push_str("!");
}
fn main() {
    let mut text = String::from("hi");
    append(&mut text);
    println!("{}", text);
}
```

Output:
```text
hi!
```

This makes in-place updates through a reference possible without cloning the
container. The restrictions on *storing* references still apply: a `&mut String`
cannot be a struct field or a `Vec` element (error `E0498`).

For loops iterate over a `Vec<T>` **by value** or over an integer range. A `&Vec<T>` is
not an iterator, so this is an error:

```ls,compile-fail
fn main() {
    let values = vec![1, 2, 3];
    let r = &values;
    for value in r {
        println!("{}", value);
    }
}
```

Error:
```text
error[E0277]: cannot iterate over a value of type &Vec<{integer}>; for loops require a range, a Vec<T>, or a type that implements Iterator<T>
  --> example-47.ls:4:18
   |
 4 |     for value in r {
   |                  ^

lsaca: example-47.ls: could not compile example-47.ls due to previous error(s)
```

Use an index loop when you want to read a vector through a shared reference without
copying it:

```ls
fn main() {
    let values = vec![1, 2, 3];
    let r = &values;
    let mut i = 0;
    let mut total = 0;
    while i < r.len() {
        total += r[i];
        i += 1;
    }
    println!("{}", total);
}
```

Output:
```text
6
```

## References and the ORC memory model

ORC owns memory: every `String`, `Vec<T>`, aggregate and `Rc<T>` handle participates in
reference counting, and cyclic garbage is reclaimed by the Bacon–Rajan cycle collector.
References do not participate in any of that. A reference:

- does not increment or decrement a refcount;
- does not keep its referent alive on its own — the borrow checker guarantees that some
  owner outlives the borrow;
- does not copy, move or modify the referent, except through assignment on a `&mut`
  (`*r = value` or `r.field = value`);
- never introduces a cycle, because it is never stored anywhere that outlives it.

The observable consequence is that borrowing is free and invisible: `Rc::strong_count`
does not change and no leak is reported.

```ls
fn main() {
    let cell = Rc::new(5);
    let before = cell.strong_count();
    let borrowed = &cell;
    let after = cell.strong_count();
    println!("{} {} {}", borrowed.get(), before, after);
}
```

Output:
```text
5 1 1
```

Display and Debug print a reference as its referent, with one quirk worth remembering:
a **top-level** `&String` prints *quoted* in both `{}` and `{:?}`, while an owned
`String` prints unquoted with `{}`. Dereference to print the text itself.

```ls
fn main() {
    let text = String::from("hi");
    let r = &text;
    let values = vec![1, 2];
    let rv = &values;
    println!("{}", r);
    println!("{}", *r);
    println!("{:?}", r);
    println!("{}", rv);
    println!("{:?}", rv);
}
```

Output:
```text
"hi"
hi
"hi"
[1, 2]
[1, 2]
```

### References compared with Rc and Box

| | `&T` / `&mut T` | `Rc<T>` | `Box<T>` |
| --- | --- | --- | --- |
| owns the value | no | shared ownership | yes |
| creation | borrow of an existing place | `Rc::new(value)` | `Box::new(value)` |
| copying | copies the pointer, no refcount change | increments the refcount | deep-copies the payload |
| mutation | through `&mut` (`*r = v`) | `set()` on any handle, interior mutability | copies / no shared mutation |
| can outlive the borrowed value | no — borrow checker forbids it | yes | yes |
| can be stored in aggregates | no | yes | yes |
| cycles | never | possible | never |

## Under the hood

The generated C represents references with plain pointers, so they cost nothing at
runtime:

| lsaca type | C type |
| --- | --- |
| `&T` | `const T*` |
| `&mut T` | `T*` |
| `&self` receiver | `T*` |
| `&mut self` receiver | `T*` |

Both receiver forms erase to the same non-const pointer parameter: the receiver's
mutability is enforced by the borrow checker, not by C `const`. A reference parameter
produces a pointer parameter and a reference return a pointer return:

```c
const int64_t* lsaca_fn_first(const LsVec_i64* p0);
int64_t        lsaca_fn_sum(const LsCounter* p0);
const int64_t* lsaca_thunk_0(void* env, const LsCounter* p0);
```

A borrow expression takes the address of the place, a dereference reads or writes
through the pointer, and the runtime refcount helpers are not called at all:

```c
/* let r = &mut counter; */
v4 = (LsCounter*)&(v3);

/* let shared = &counter; */
v5 = (const LsCounter*)&(v3);

/* let field = &counter.count; */
v6 = (const int64_t*)&(((*(p0))).f_count);

/* let value = *r; */
t28 = (*(t27));

/* (*r).count = 2; */
((*(t19))).f_count = t20;
```

This is why references cannot escape into aggregates: a raw pointer has no lifetime
information, so the compiler only permits references where it can prove the referent
stays alive.

## Diagnostics

| Code | Message (abbreviated) | Typical cause |
| --- | --- | --- |
| E0106 | `missing lifetime specifier` | a function returns a reference but has zero or two or more reference parameters |
| E0498 | `references cannot be stored in this type (found &T)` | `&T` inside a struct, enum, tuple, `Vec`, `Option`, `Result` or `Box` |
| E0498 | `references cannot be captured by closures` | a closure body uses a captured reference |
| E0499 | `cannot borrow X as mutable more than once at a time` | two overlapping mutable borrows, including two mutable borrows inside one expression |
| E0502 | `cannot borrow X as immutable because it is also borrowed as mutable` | shared borrow while a mutable borrow is alive |
| E0506 | `cannot assign to X because it is borrowed` | assignment while a borrow is still in use |
| E0515 | `returns a reference to a local value` | returning `&local` from a function |
| E0596 | `cannot borrow this value as mutable: the binding is not declared as mutable` | `&mut` on a non-`mut` binding |
| E0596 | `cannot mutate through this receiver: the binding is not declared as mutable` | `&mut self` method on an immutable binding |
| E0596 | `cannot assign to immutable value` | `*r = value` through a shared reference |
| E0596 | `cannot return a mutable reference from a shared reference source` | `&self` method returning `&mut` |
| E0597 | `X does not live long enough` | assigning a borrow of a block-local to an outer variable |
| E0614 | `type Box<{integer}> cannot be dereferenced` | unary `*` on `Rc` or `Box` |
| E0745 | `cannot borrow a temporary value, bind it to a local first` | `&(1 + 2)`, `&Some(1)`, `&String::from(..)`, borrowing a constant |
| E0277 | `cannot iterate over a value of type &Vec<T>` | `for x in &vec` |

Errors without a code (parser level): `reference patterns are not supported; match a
reference scrutinee directly, bindings become references`.

## Known limitations in version 1.6.0

Version 1.3.0 introduced references, 1.4.0 added ownership and moves on top of
them, and 1.5.0 added `==` between references. The current restrictions are:

1. **No lifetime syntax.** Only the two elision rules (receiver, single reference
   parameter) are available; there are no explicit `'a` annotations, labels or `'static`.
2. **References cannot be stored.** Struct fields, enum payloads, tuples, `Vec`,
   `Option`, `Result`, `Box` and closures cannot hold references. Keep the reference in
   a local for the duration of the computation instead.
3. **Mutable references to user aggregates with heap fields are rejected.** `&mut` works
   for Copy values and for `String`, `Vec<T>`, `Rc<T>` and `Box<T>`, but not for a user
   struct or enum that contains a `String`, `Vec`, `Rc`, `Box` or Drop field (E0498).
   Borrow those with a shared reference, or move them by value.
4. **By-value `self` cannot return a reference.** A method that takes `self` by value
   and returns `&T` or `&mut T` is rejected with `E0106` ("missing lifetime specifier");
   use a `&self` or `&mut self` receiver to return a field reference.
5. **Reference patterns are not supported.** `let &x = r` is a parser error; match the
   reference scrutinee directly and the bindings become references.
6. **Vector borrows are container-granular**: two mutable references to different
   elements conflict. Struct fields are per-field.
7. **No references in `for` loops** (`for x in &vec` is an error) and no indexing a
   `&String`; `==` on references compares the pointed-to values.
8. **No raw pointers and no casts to or from references.**
9. **The borrow checker is conservative inside a single expression.** Passing two
   mutable borrows as the two arguments of one call, or combining them with `+`,
   is rejected with `E0499` even though the operations run in order. The
   short-circuit operators are the exception: `side(&mut c) || side(&mut c)` and
   the `&&` form are accepted, because each operand becomes its own condition.
   Bind each result to a local first and combine the locals.

## Summary

- `&T` is a shared, read-only view; `&mut T` is an exclusive, writable view. Both are
  copied freely and neither owns the value.
- Create a reference with `&place` / `&mut place`; the operand must be a local, field,
  tuple element, element or dereference — not a temporary or a constant.
- Read through a reference with `*r`; write with `*r = v`, `*r += v`, `r.field = v` or
  `(*r).field = v`. Field reads, field writes, method calls and indexing auto-dereference.
- Shared borrows may overlap; a mutable borrow must be alone. A borrow ends at its last
  use, which makes sequential and loop-local borrows legal.
- Functions can take and return references, using the receiver rule and the
  single-reference-parameter rule to infer lifetimes; the returned reference must point
  into an input.
- Matching a reference binds references to the referent (match ergonomics) — for shared
  and mutable references alike, so `&mut` bindings can be assigned through — and matching
  `self` inside a method follows the same rule.
- References never retain or release ORC allocations and cannot create cycles; ORC
  remains the only owner.
- References cannot be stored in aggregates or captured by closures; they can be
  compared with `==` since 1.5.0 (the comparison reads the pointed-to values).
  Mutable references to `String`, `Vec`, `Rc` and `Box` *are* allowed as
  parameters, locals and return values in version 1.6.0.

## What next

- [Memory model (ORC)](05-memory-model.md) — ownership, copy-on-write, `Rc<T>` and the
  cycle collector that references cooperate with.
- [Types](04-types.md) — where `&T` and `&mut T` sit in the type system.
- [Functions](09-functions.md) — parameter passing, return values, reference elision.
- [Structs, enums and impl](10-structs-enums-and-impl.md) — receivers and methods.
- [Pattern matching](13-pattern-matching.md) — the pattern language used by match
  ergonomics.
- [Standard library](15-standard-library.md) — the owned containers references point
  into.
- [Roadmap](16-roadmap.md) — what is still planned (reference fields, lifetimes,
  generics, traits and iterators).
- [Benchmarks](21-benchmarks.md) — measured performance of the reference and ORC
  runtime.
