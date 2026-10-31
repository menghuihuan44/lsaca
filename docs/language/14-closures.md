# Closures

A closure is an anonymous function value written inline with `|params|`. Closures
have the same type shape as functions — `fn(Args) -> Ret` — and can be bound,
passed, returned and called wherever a function value is expected. The one
semantic difference from Rust is that lsaca captures **by value**: each variable
the closure mentions is copied if it is `Copy` and **moved** into the closure
otherwise, so using a move-only variable after the closure is created is a
compile error (`E0382`) — clone it first if both are needed. A closure never
holds references to captured variables. Since 1.5.0 closures and function values
can also be stored in structs, enums, tuples, `Vec`, `Box` and as HashMap values;
see [Storing closures in aggregates](#storing-closures-in-aggregates).

## Syntax

The general form is:

```rust
|param: Type, ...| -> ReturnType { statements; tail }
```

The parameter types and the return type are optional, and a single-expression
body does not need braces:

```ls
fn main() {
    let double: fn(i64) -> i64 = |x| x * 2;
    println!("{}", double(21));

    let add = |a: i64, b: i64| -> i64 { a + b };
    println!("{}", add(2, 3));

    let greet = |name: String| { println!("hello, {}", name); };
    greet(String::from("lsaca"));
}
```

Output:
```text
42
5
hello, lsaca
```

Like functions, a closure without a tail expression returns `()`. A closure's
body is an expression; use `{ … }` whenever you need several statements.

## Capture by value

When a closure mentions an outer variable, it captures that variable **by
value** at the moment the closure is created. Copy values are copied, so the
closure keeps a snapshot; move-only values (`String`, `Vec<T>`, `Rc<T>`, `Box<T>`,
function values) are **moved** into the closure, so the outer binding cannot be
used afterwards. This is the same capture rule as Rust, except that lsaca has no
borrow capture at all: closures never capture a reference.

```ls
fn main() {
    let name = String::from("ana");
    let visit = || println!("visiting {}", name);
    visit();
    visit();
}
```

Output:
```text
visiting ana
visiting ana
```

`name` is moved into the closure when `visit` is created; using `name` after that
line is a compile error (`E0382`, use of moved value), so clone the value before
capture when both the closure and the outer scope need it.

Copy values show the snapshot rule directly: the closure keeps the value the
variable had when it was created.

```ls
fn main() {
    let mut count = 0;
    let read = || count;

    count += 10;
    count += 1;

    println!("{} {}", count, read());
}
```

Output:
```text
11 0
```

Captured values are ordinary lsaca values; a closure owns whatever it captured.

## The move keyword

lsaca accepts the Rust `move` keyword on closures (`move |x| …`). Capture is
already by value, so the keyword changes nothing; it exists only so that Rust
code that spells `move` compiles unchanged:

```ls
fn main() {
    let label = String::from("moved");
    let show = move || println!("{}", label);
    show();
    show();
}
```

Output:
```text
moved
moved
```

`label` is moved into the closure either way, so the outer binding is unusable
after the closure is created (without cloning first).

## Shared state with Rc

A captured `Rc<T>` handle is moved into the closure, and every other handle still
refers to the same cell, so mutations through the closure are visible outside.
Clone the outer handle before capture when the surrounding scope also needs to
read the cell:

```ls
fn main() {
    let counter = Rc::new(0);
    let handle = counter.clone();
    let bump = || handle.set(handle.get() + 1);
    bump();
    bump();
    bump();
    println!("{}", counter.get());

    let read_handle = counter.clone();
    let read = || read_handle.get();
    println!("{}", read());
}
```

Output:
```text
3
3
```

## Closures as function values

A closure with an annotation or a context that determines its signature
implements the corresponding `fn` type and can be passed to any function that
expects one. A closure that captures nothing is interchangeable with a plain
function:

```ls
fn apply(f: fn(i64) -> i64, x: i64) -> i64 {
    f(x)
}

fn twice(f: fn(i64) -> i64, x: i64) -> i64 {
    f(f(x))
}

fn main() {
    println!("{}", apply(|x| x + 1, 41));
    println!("{}", twice(|x| x * 3, 2));
    println!("{}", apply(|x| x - 5, 12));
}
```

Output:
```text
42
18
7
```

Closures that capture values can be returned from functions. The captured values
travel with the closure (it owns them), which is how a function can produce a
specialized function value:

```ls
fn multiplier(factor: i64) -> fn(i64) -> i64 {
    |x| x * factor
}

fn main() {
    let triple = multiplier(3);
    let tenfold = multiplier(10);
    println!("{} {}", triple(7), tenfold(7));
}
```

Output:
```text
21 70
```

### Storing closures in aggregates

Since version 1.5.0 a closure or function value can be stored in a struct
field, an enum variant, a tuple, `Vec<T>`, `Box<T>` and as a HashMap value. The
value stays **move-only**: moving it into the aggregate consumes it, and an
aggregate that holds a function value is itself move-only. Calling a stored
value borrows it, so the field or element stays usable:

```ls
struct Rule {
    limit: i64,
    accepts: fn(i64) -> bool,
}

fn main() {
    let rule = Rule {
        limit: 3,
        accepts: |value| value < 5,
    };
    println!("{} {}", rule.limit, (rule.accepts)(4));

    let steps: Vec<fn(i64) -> i64> = vec![|x| x + 1, |x| x * 2];
    println!("{} {}", (steps[0])(41), (steps[1])(21));
}
```

Output:
```text
3 true
42 42
```

An aggregate holding a function value is move-only. `.clone()` on the closure
or on the aggregate clones the **handle** and shares the captured environment
(its reference count goes up); it does not deep-copy captured values. For an
`Rc<T>` captured by the closure, both handles therefore see the same cell, and
the environment is released when the last handle is dropped.

### Inference

A closure's parameter and return types are usually inferred. The compiler uses
the annotation on the binding, the type expected where the closure is used — a
`fn` parameter type, or a generic parameter with an `Fn(..) -> ..` bound (see
[Bounds](17-generics.md#bounds)) — and, for a closure that has no annotation and
no expected type, the argument types of its first call:

```ls
fn main() {
    let id = |x| x;
    println!("{}", id(String::from("lsaca")));

    let f: fn(i64) -> i64 = |x| x + 1;
    println!("{}", f(41));
}
```

Output:
```text
lsaca
42
```

The first call fixes the parameter type. The closure is monomorphic, so a later
call with a different argument type is a type error:

```ls,compile-fail
fn main() {
    let identity = |x| x;
    println!("{}", identity(2));
    println!("{}", identity(3.5));
}
```

Error:
```text
error[E0308]: mismatched types
  --> example-10.ls:4:29
   |
 4 |     println!("{}", identity(3.5));
   |                             ^^^
   = note: expected {integer}, found {float}

lsaca: example-10.ls: could not compile example-10.ls due to previous error(s)
```

Some closure bodies use the parameter in a way that needs a concrete type before
any call is available — unary `-` is one example. There the parameter must be
annotated, otherwise the front end reports `E0282`:

```ls,compile-fail
fn main() {
    let negate = |x| -x;
    println!("{}", negate(3));
}
```

Error:
```text
error[E0282]: type annotations needed for the operand of unary -
  --> example-11.ls:2:22
   |
 2 |     let negate = |x| -x;
   |                      ^^

lsaca: example-11.ls: could not compile example-11.ls due to previous error(s)
```

Adding an annotation fixes it:

```ls
fn main() {
    let negate = |x: i64| -x;
    println!("{}", negate(3));
}
```

Output:
```text
-3
```

Function values and closures do not support `==` (`E0369`), and a closure cannot
call itself — there is no way to name it from inside its own body. To write a
recursive function value, use a named `fn`.

## Known limitations in 1.6.0

* closures and function values are move-only and cannot be compared with `==`
  (`E0369`);
* capture is by value only: there is no borrow capture (`E0498`) and no
  `FnMut`/`FnOnce` distinction. The `move` keyword is accepted but changes
  nothing;
* a closure cannot call itself because there is no way to name it inside its
  own body — use a named `fn` for recursive function values;
* `.clone()` on a closure or on an aggregate holding one shares the captured
  environment instead of copying it (see above).

See [Roadmap](16-roadmap.md) for the full list.

## What next

* Functions and higher-order functions: [Functions](09-functions.md).
* Where function values are used in the standard library:
  [Standard library](15-standard-library.md).
* The exact `fn` type syntax: [Types](04-types.md#function-types).
