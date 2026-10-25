# Closures

A closure is an anonymous function value written inline with `|params|`. Closures
have the same type shape as functions — `fn(Args) -> Ret` — and can be bound,
passed, returned and called wherever a function value is expected. The one
semantic difference from Rust is that lsaca captures **by value**: a closure
holds copies of the variables it mentions, not references to them.

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

When a closure mentions an outer variable, it captures a **copy** of that
variable at the moment the closure is created. Later changes to the outer
variable are not seen by the closure, and changes the closure makes to its copy
are not seen outside. This is the defining difference from Rust closures:

```ls
fn main() {
    let mut name = String::from("ana");
    let visit = || {
        println!("visiting {}", name);
    };

    name.push_str("!");
    println!("outer name: {}", name);
    visit();
    visit();
}
```

Output:
```text
outer name: ana!
visiting ana
visiting ana
```

The integer case shows the snapshot rule especially clearly: the closure keeps
the value the variable had when it was created.

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

Captured values are ordinary lsaca values. A captured `String` or `Vec` shares
its buffer until one side writes (copy-on-write), so capture is cheap; a captured
`Rc` copies the handle and therefore shares the cell.

## The move keyword

lsaca accepts the Rust `move` keyword on closures (`move |x| …`), but because
capture is already by value it changes nothing. It exists only to let Rust code
compile unchanged:

```ls
fn main() {
    let label = String::from("moved");
    let show = move || println!("{}", label);
    show();
    show();
    println!("{}", label);
}
```

Output:
```text
moved
moved
moved
```

## Shared state with Rc

Since capture copies handles, an `Rc<T>` captured by a closure is shared with the
outer scope: mutations through one handle are visible through the other. This is
the idiomatic way to give a closure mutable shared state:

```ls
fn main() {
    let counter = Rc::new(0);
    let bump = || counter.set(counter.get() + 1);
    bump();
    bump();
    bump();
    println!("{}", counter.get());

    let read = || counter.get();
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

Closures that capture values can be returned from functions. The captured copies
travel with the closure, which is how a function can produce a specialized
function value:

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

### Inference

A closure's parameter and return types are usually inferred. The compiler uses
the annotation on the binding, the type expected where the closure is used, and
— for a closure that is never annotated — the argument types of its first call:

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
  --> example-09.ls:4:29
   |
 4 |     println!("{}", identity(3.5));
   |                             ^^^
   = note: expected {integer}, found {float}

lsaca: example-09.ls: could not compile example-09.ls due to previous error(s)
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
  --> example-10.ls:2:22
   |
 2 |     let negate = |x| -x;
   |                      ^^

lsaca: example-10.ls: could not compile example-10.ls due to previous error(s)
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

## Known limitations in 1.2.0

Two restrictions come from the C backend rather than the type checker:

* closures and function values **cannot be stored in struct fields or in a
  `Vec`**; the generated C does not have a representation for function values in
  those positions;
* closures and function values cannot be compared with `==`.

See [Roadmap](14-roadmap.md) for the full list.

## What next

* Functions and higher-order functions: [Functions](09-functions.md).
* Where function values are used in the standard library:
  [Standard library](13-standard-library.md).
* The exact `fn` type syntax: [Types](04-types.md#function-types).
