# Functions

Functions are introduced with `fn`. A function has a name, a parameter list of
`name: Type` pairs, an optional return type after `->`, and a block body:

```ls
fn add(a: i64, b: i64) -> i64 {
    a + b
}

fn main() {
    println!("{}", add(2, 3));
}
```

Output:
```text
5
```

Parameter types are mandatory, and a parameter must be a simple name — patterns
such as `(x, y): (i64, i64)` are not allowed in a parameter position. The return
type may be omitted; a function without `-> T` returns `()`.

```ls
fn greet(name: String) {
    println!("hello, {}", name);
}

fn noop() {}

fn main() {
    greet(String::from("lsaca"));
    noop();
}
```

Output:
```text
hello, lsaca
```

## Return values

The body of a function is an expression. Its value is the return value, and the
usual way to return something is a tail expression — an expression at the end of
the body without a semicolon. The `return` statement exits early and may be used
anywhere in the body:

```ls
fn absolute(n: i64) -> i64 {
    if n < 0 {
        return -n;
    }
    n
}

fn main() {
    println!("{} {}", absolute(-5), absolute(5));
}
```

Output:
```text
5 5
```

Forgetting the tail expression is a type error: a block whose last statement ends
with `;` has type `()` and cannot be used as an `i64` return value.

```ls,compile-fail
fn add(a: i64, b: i64) -> i64 {
    a + b;
}

fn main() {
    println!("{}", add(1, 2));
}
```

Error:
```text
error[E0308]: mismatched types
  --> example-04.ls:1:31
   |
 1 | fn add(a: i64, b: i64) -> i64 {
   |                               ^
   = note: expected i64, found ()

lsaca: example-04.ls: could not compile example-04.ls due to previous error(s)
```

A bare `return;` and `return expr;` follow the same rules as the return type; a
value returned from a `()` function is a compile error (`E0308`).

## Recursion

Functions can call themselves and each other. Top-level items are visible
throughout the compilation unit regardless of their order, so mutual recursion
needs no forward declarations:

```ls
fn factorial(n: i64) -> i64 {
    if n <= 1 {
        1
    } else {
        n * factorial(n - 1)
    }
}

fn is_even(n: i64) -> bool {
    if n == 0 { true } else { is_odd(n - 1) }
}

fn is_odd(n: i64) -> bool {
    if n == 0 { false } else { is_even(n - 1) }
}

fn main() {
    println!("{} {}", factorial(6), factorial(0));
    println!("{} {}", is_even(10), is_odd(10));
}
```

Output:
```text
720 1
true false
```

Recursion depth is limited by the native call stack (each lsaca call is a C call);
there is no tail-call optimization and no way to catch stack exhaustion — a
runaway recursion crashes the process.

## Function values

A function name used as a value has the type `fn(ArgTypes) -> ReturnType`. Such
values are first-class: they can be bound to a variable, passed to another
function, returned from a function, and called. They are ordinary lsaca values
(refcounted, copied on assignment). A closure with no captures can be used
wherever a `fn` value is expected (`[Closures](12-closures.md)`).

```ls
fn inc(n: i64) -> i64 {
    n + 1
}

fn apply(f: fn(i64) -> i64, x: i64) -> i64 {
    f(x)
}

fn main() {
    let incrementer: fn(i64) -> i64 = inc;
    println!("{}", incrementer(41));
    println!("{}", apply(inc, 41));

    let doubler: fn(i64) -> i64 = |x| x * 2;
    println!("{}", apply(doubler, 21));
}
```

Output:
```text
42
42
42
```

Functions can also return function values. Since lsaca captures by value, this
is the idiomatic substitute for closures over mutable state:

```ls
fn adder(n: i64) -> fn(i64) -> i64 {
    |x| x + n
}

fn main() {
    let add_five = adder(5);
    let add_ten = adder(10);
    println!("{} {}", add_five(3), add_ten(3));
}
```

Output:
```text
8 13
```

Two limitations apply in 1.2.0:

* function values **cannot be stored in struct fields or in a `Vec`** — the C
  backend does not generate a valid representation for them there; using them
  only as parameters, locals and return values is supported;
* function values do **not** support `==`; comparing two of them is an error
  (`E0369`):

```ls,compile-fail
fn inc(n: i64) -> i64 { n + 1 }
fn dec(n: i64) -> i64 { n - 1 }

fn main() {
    let a: fn(i64) -> i64 = inc;
    let b: fn(i64) -> i64 = dec;
    println!("{}", a == b);
}
```

Error:
```text
error[E0369]: binary operation == cannot be applied to type fn(i64) -> i64
  --> example-08.ls:7:20
   |
 7 |     println!("{}", a == b);
   |                    ^^^^^^

lsaca: example-08.ls: could not compile example-08.ls due to previous error(s)
```

## Calls and argument checking

A call passes each argument by value: the callee receives copies (handles are
copied, scalars are copied). Arguments must match the parameter count and types
exactly — there are no default arguments, no variadic functions, no implicit
conversions and no overloading.

```ls,compile-fail
fn add(a: i64, b: i64) -> i64 {
    a + b
}

fn main() {
    println!("{}", add(1));
}
```

Error:
```text
error[E0061]: this function takes 2 argument(s) but 1 were supplied
  --> example-09.ls:6:20
   |
 6 |     println!("{}", add(1));
   |                    ^^^^^^

lsaca: example-09.ls: could not compile example-09.ls due to previous error(s)
```

```ls,compile-fail
fn add(a: i64, b: i64) -> i64 {
    a + b
}

fn main() {
    println!("{}", add(1, 2.5));
}
```

Error:
```text
error[E0308]: mismatched types
  --> example-10.ls:6:27
   |
 6 |     println!("{}", add(1, 2.5));
   |                           ^^^
   = note: expected i64, found {float}

lsaca: example-10.ls: could not compile example-10.ls due to previous error(s)
```

Calling something that is not a function is a compile error as well:

```ls,compile-fail
fn main() {
    let n = 42;
    println!("{}", n());
}
```

Error:
```text
error[E0618]: call expression requires a function, found {integer}
  --> example-11.ls:3:20
   |
 3 |     println!("{}", n());
   |                    ^^^

lsaca: example-11.ls: could not compile example-11.ls due to previous error(s)
```

Similarly, two functions with the same name are rejected even when their
parameter lists differ; there is no overloading:

```ls,compile-fail
fn double(n: i64) -> i64 { n * 2 }
fn double(f: f64) -> f64 { f * 2.0 }

fn main() {
    println!("{}", double(2));
}
```

Error:
```text
error[E0428]: the name 'double' is defined multiple times
  --> example-12.ls:2:4
   |
 2 | fn double(f: f64) -> f64 { f * 2.0 }
   |    ^^^^^^
   = note: example-12.ls:1:4: previous definition of 'double' here

lsaca: example-12.ls: could not compile example-12.ls due to previous error(s)
```

## Nested functions

Functions may be declared inside blocks and inside other functions. Nested items
are hoisted to the whole enclosing body, so a nested function can be called
before the line that declares it, but it is not visible outside that body:

```ls
fn main() {
    println!("{}", square(7));

    fn square(n: i64) -> i64 {
        n * n
    }

    fn apply_twice(f: fn(i64) -> i64, x: i64) -> i64 {
        f(f(x))
    }

    println!("{}", apply_twice(square, 3));
}
```

Output:
```text
49
81
```

A nested function still cannot capture the locals of the enclosing function —
capturing is what closures are for. If the nested function needs an outer value,
pass it as a parameter.

## The entry point

Every program must define exactly one top-level `fn main()` that takes no
parameters and returns `()`. See
[Program structure](02-program-structure.md#the-entry-point-main) for the exact error
messages when a program violates this. `main` is an ordinary function apart from
those restrictions: it can call anything and declare nested items.

## What next

* Closures and capture-by-value: [Closures](12-closures.md).
* The full function-value type syntax: [Types](04-types.md#function-types).
* Recursion and control flow: [Statements and control flow](08-statements-and-control-flow.md).
