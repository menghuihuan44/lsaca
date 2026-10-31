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
function, returned from a function, and called. A `fn` value is **move-only**:
binding, passing or returning it moves it, and `.clone()` keeps a second usable
handle. A call borrows the value, so the same binding can be called repeatedly.
Since 1.5.0 function values can also be stored in structures (see
[Closures](14-closures.md#storing-closures-in-aggregates)). A closure with no
captures can be used wherever a `fn` value is expected ([Closures](14-closures.md)).

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

One limitation remains in 1.6.0: function values do **not** support `==`, so
comparing two of them is an error (`E0369`):

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

## Generic functions

Function declarations can be generic since 1.5.0. Type parameters are written
in angle brackets after the name, and each can carry inline bounds. Generic
functions are static: the compiler instantiates and checks the body once per
concrete set of type arguments (see
[Generics](17-generics.md#how-generics-are-compiled)). Type arguments
come from the arguments and the expected type, or from an explicit turbofish:

```ls
fn identity<T>(value: T) -> T {
    value
}

fn apply_twice<F: Fn(i64) -> i64>(f: F, value: i64) -> i64 {
    f(f(value))
}

fn main() {
    println!("{}", identity(7));
    let text: String = identity(String::from("lsaca"));
    println!("{}", text);
    println!("{}", apply_twice(|x| x + 1, 20));
    println!("{}", identity::<i64>(9));
}
```

Output:
```text
7
lsaca
22
9
```

The closure passed to `apply_twice` is written without a parameter
annotation: its type comes from the `F: Fn(i64) -> i64` bound on the function
parameter.

Bounds are checked per instantiation: calling a bounded generic function with
a type that does not implement the bound is an error (`E0277`). See
[Generics](17-generics.md) for multiple parameters, bounds, `Self` and the
turbofish.

## Calls and argument checking

A call passes each argument by value. `Copy` arguments are copied; move-only
arguments (`String`, `Vec`, `Rc`, `Box`, function values and the user types that
contain them) are **moved** into the parameter, so the caller's binding cannot be
used after the call (see [Ownership and moves](11-ownership-and-moves.md)). Pass
a reference or `.clone()` the argument when the caller still needs it. A
move-only return value — and a tail expression — moves ownership out of the
callee to the caller the same way. Arguments must match the parameter count and
types exactly — there are no default arguments, no variadic functions, no
implicit conversions and no overloading.

```ls
fn take(text: String) -> i64 {
    text.len()
}

fn peek(text: &String) -> i64 {
    text.len()
}

fn main() {
    let owned = String::from("lsaca");
    println!("{}", peek(&owned));
    println!("{}", take(owned));   // moves `owned`
}
```

Output:

```text
5
5
```

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
  --> example-11.ls:6:20
   |
 6 |     println!("{}", add(1));
   |                    ^^^^^^

lsaca: example-11.ls: could not compile example-11.ls due to previous error(s)
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
  --> example-12.ls:6:27
   |
 6 |     println!("{}", add(1, 2.5));
   |                           ^^^
   = note: expected i64, found {float}

lsaca: example-12.ls: could not compile example-12.ls due to previous error(s)
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
  --> example-13.ls:3:20
   |
 3 |     println!("{}", n());
   |                    ^^^

lsaca: example-13.ls: could not compile example-13.ls due to previous error(s)
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
  --> example-14.ls:2:4
   |
 2 | fn double(f: f64) -> f64 { f * 2.0 }
   |    ^^^^^^
   = note: example-14.ls:1:4: previous definition of 'double' here

lsaca: example-14.ls: could not compile example-14.ls due to previous error(s)
```

## Reference parameters and returns

A parameter type may be a reference. A `&T` parameter is a read-only view of the
caller's value; a `&mut T` parameter lets the function write through the
reference. Callers pass a borrow expression:

```ls
fn double(value: &i64) -> i64 {
    *value * 2
}

fn bump(value: &mut i64) {
    *value += 1;
}

fn main() {
    let mut n = 5;
    bump(&mut n);
    println!("{}", double(&n));
}
```

Output:
```text
12
```

Inside a `&mut` parameter, whole-place writes use `*value = ...`; field writes
auto-dereference too, so `counter.count += 1` and `(*counter).count += 1` both
write the caller's field:

```ls
struct Counter {
    count: i64,
}

fn bump_count(counter: &mut Counter) {
    counter.count += 1;
}

fn main() {
    let mut counter = Counter { count: 0 };
    bump_count(&mut counter);
    bump_count(&mut counter);
    println!("{}", counter.count);
}
```

Output:
```text
2
```

A function may also **return** a reference. Because lsaca has no lifetime
syntax, the returned reference's source is inferred by elision:

* In a method, a returned reference is borrowed from the receiver (`&self` or
  `&mut self`).
* In a free function, the return type may contain a reference only if the
  function has **exactly one** reference parameter; that parameter is the
  source. Zero or several reference parameters give `E0106` (missing lifetime
  specifier).

```ls
fn first(values: &Vec<i64>) -> &i64 {
    &values[0]
}

fn main() {
    let values = vec![10, 20, 30];
    let head = first(&values);
    println!("{}", *head);
}
```

Output:
```text
10
```

The returned reference must point into that source: returning a reference to a
local (`E0515`) or to a value that does not live long enough (`E0597`) is
rejected. When the function has more than one reference parameter and the
answer is ambiguous, the compiler reports `E0106`:

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
  --> example-18.ls:1:41
   |
 1 | fn choose(first: &i64, second: &i64) -> &i64 {
   |                                         ^
   = note: this function's return type contains a borrowed value, but the signature does not say which parameter it is borrowed from

lsaca: example-18.ls: could not compile example-18.ls due to previous error(s)
```

At calls, returns and `let` bindings, a few reference coercions happen
implicitly:

* `&mut T` coerces to `&T`;
* `&Rc<T>` coerces to `&T`;
* `&Box<T>` coerces to `&T`.

Methods with `&self`/`&mut self` receivers are covered in [Structs, enums and
impl](10-structs-enums-and-impl.md#methods-and-associated-functions-with-impl);
the full borrowing rules are in [References and
borrowing](12-references-and-borrowing.md).

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

* Closures and capture-by-value: [Closures](14-closures.md).
* The full function-value type syntax: [Types](04-types.md#function-types).
* Borrowing in parameters and return values: [References and
  borrowing](12-references-and-borrowing.md).
* Recursion and control flow: [Statements and control flow](08-statements-and-control-flow.md).
