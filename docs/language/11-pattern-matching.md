# Pattern matching

`match` is lsaca's destructuring construct: it takes a value, tries a series of
patterns from top to bottom, and evaluates the expression of the first pattern
that matches. It is an expression, so a `match` can be bound, returned or used
inside another expression. Except where noted under
[known limitations](#known-limitations-in-110), matching is exhaustive and checked.

```ls
fn describe(n: i64) -> String {
    match n {
        0 => String::from("zero"),
        1 => String::from("one"),
        2 => String::from("two"),
        _ => String::from("many"),
    }
}

fn main() {
    println!("{} {} {} {}", describe(0), describe(1), describe(2), describe(9));
}
```

Output:
```text
zero one two many
```

Each arm is `pattern => expression` followed by a comma. Braces in the expression
form a block; the comma after a block arm is still required. Every arm must have
the same type, and the whole `match` has that type.

## Patterns

The pattern forms available in 1.1.0 are:

| Pattern | Example | Matches |
| --- | --- | --- |
| Wildcard | `_` | anything, binds nothing |
| Binding | `n`, `mut n` | anything, binds the value |
| Integer literal | `0`, `-1`, `42` | that exact integer |
| Boolean literal | `true`, `false` | that boolean |
| Unit | `()` | the unit value |
| Tuple | `(a, b)`, `()` | a tuple of that length (flat tuples only) |
| Struct | `Point { x, y }`, `Point { x, .. }` | a struct with those fields |
| Enum variant | `Shape::Empty`, `Shape::Circle(r)` | that variant, binding the payload |
| Or-pattern | `0 \| 1`, `E::A(x) \| E::B(x)` | at least one alternative matches; every alternative binds the same names |

Patterns nest: a variant pattern can contain a struct pattern, a struct pattern
can contain tuple patterns, and so on. There are no string-literal, range, slice,
`@`-binding or `ref` patterns. Or-patterns (`p | q`) and match guards
(`pattern if condition`) are fully supported in 1.1.0 — see
[Or-patterns](#or-patterns) and [Match guards](#match-guards).

### Binding and wildcards

An identifier pattern binds the matched value, and that binding is immutable
unless written `mut`. `_` discards the value and binds nothing. Both are
irrefutable; a match that consists of a single binding arm is legal but
pointless:

```ls
fn main() {
    let pair = (10, 20);
    match pair {
        (left, mut right) => {
            right += left;
            println!("{} {}", left, right);
        }
    }

    let ignored = match 5 {
        _ => 100,
    };
    println!("{}", ignored);
}
```

Output:
```text
10 30
100
```

### Integer, boolean and unit patterns

Literal patterns compare with `==` semantics for the corresponding scalar type.
Negative integer literals are allowed. A match on integers must cover every
possible value, which in practice means a final wildcard (or a binding) arm.

```ls
fn sign(n: i64) -> String {
    match n {
        0 => String::from("zero"),
        -1 => String::from("minus one"),
        1 => String::from("one"),
        _ => String::from("other"),
    }
}

fn main() {
    println!("{} {} {} {}", sign(0), sign(1), sign(-1), sign(7));

    let flag = true;
    let label = match flag {
        true => String::from("on"),
        false => String::from("off"),
    };
    println!("{}", label);

    let unit = ();
    let also = match unit {
        () => 5,
    };
    println!("{}", also);
}
```

Output:
```text
zero one minus one other
on
5
```

### Tuple patterns

Tuple patterns match the tuple's arity and bind each component. All tuple
patterns must be **flat**: nested tuple values and nested tuple patterns are a
known codegen limitation in 1.1.0 (see [Roadmap](14-roadmap.md)).

```ls
fn main() {
    let pair = (1, String::from("lsaca"));
    match pair {
        (number, text) => println!("{} {}", number, text),
    }

    let triple = (1, 2.5, String::from("x"));
    match triple {
        (a, b, c) => println!("{} {} {}", a, b, c),
    }
}
```

Output:
```text
1 lsaca
1 2.5 x
```

### Struct patterns

A struct pattern names a struct and its fields. Fields can be matched by
shorthand (`Point { x, y }` binds `x` and `y`), or skipped with `..`. `..` may
appear anywhere in the field list:

```ls
struct Point {
    x: i64,
    y: i64,
    label: String,
}

fn main() {
    let p = Point { x: 3, y: 4, label: String::from("corner") };

    match p {
        Point { x, y, .. } => println!("{} {}", x, y),
    }

    match p {
        Point { label, .. } => println!("{}", label),
    }
}
```

Output:
```text
3 4
corner
```

### Enum variant patterns

Variant patterns use `Enum::Variant`; tuple variants take a pattern per payload
value, unit variants take nothing. This is how `Option`, `Result` and user enums
are inspected.

```ls
enum Shape {
    Circle(f64),
    Square(f64),
    Empty,
}

fn area(shape: Shape) -> f64 {
    match shape {
        Shape::Circle(r) => 3.14159 * r * r,
        Shape::Square(s) => s * s,
        Shape::Empty => 0.0,
    }
}

fn main() {
    println!("{}", area(Shape::Circle(2.0)));
    println!("{}", area(Shape::Square(3.0)));
    println!("{}", area(Shape::Empty));
}
```

Output:
```text
12.56636
9
0
```

Because `Option<T>` and `Result<T, E>` are ordinary enums, their patterns are the
familiar Rust ones:

```ls
fn divide(a: i64, b: i64) -> Result<i64, String> {
    if b == 0 {
        Err(String::from("division by zero"))
    } else {
        Ok(a / b)
    }
}

fn main() {
    match divide(10, 2) {
        Ok(value) => println!("ok {}", value),
        Err(message) => println!("error: {}", message),
    }
    match divide(1, 0) {
        Ok(value) => println!("ok {}", value),
        Err(message) => println!("error: {}", message),
    }

    let maybe: Option<String> = Some(String::from("present"));
    match maybe {
        Some(text) => println!("{}", text),
        None => println!("nothing"),
    }
}
```

```text
ok 5
error: division by zero
present
```

### Or-patterns

An or-pattern joins alternatives with `|` and matches when **any** alternative
matches. The alternatives are tried left to right, and the whole or-pattern
counts as a single pattern:

```ls
fn label(n: i64) -> String {
    match n {
        0 | 1 => String::from("small"),
        2 | 3 | 4 => String::from("medium"),
        _ => String::from("large"),
    }
}

fn main() {
    println!("{} {} {}", label(1), label(3), label(9));
}
```

Output:
```text
small medium large
```

Because the arm covers several values at once, an or-pattern that covers the
whole matched type makes the `match` exhaustive on its own:

```ls
fn main() {
    let flag = true;
    let label = match flag {
        true | false => String::from("bool"),
    };
    println!("{}", label);
}
```

Output:
```text
bool
```

The alternatives of one or-pattern must bind the **same names**, and each name
must be bound in every alternative. The arm body then reads the bindings of the
alternative that actually matched:

```ls
enum E {
    Pair(i64, String),
    Single(String),
}

fn text(e: E) -> String {
    match e {
        E::Pair(_, s) | E::Single(s) => s,
    }
}

fn main() {
    println!("{}", text(E::Pair(1, String::from("pair"))));
    println!("{}", text(E::Single(String::from("single"))));
}
```

Output:
```text
pair
single
```

Different patterns can be grouped this way as long as the bound names agree:
`E::Pair(_, s) | E::Single(s)` binds only `s`. If the alternatives bind different
names the compiler reports `error[E0408]`:

```ls,compile-fail
enum E {
    A(i64),
    B(i64),
}

fn f(e: E) -> i64 {
    match e {
        E::A(x) | E::B(y) => x,
    }
}

fn main() {
    println!("{}", f(E::A(2)));
}
```

Error:
```text
error[E0408]: variable x is not bound in all patterns of this or-pattern
  --> example-11.ls:8:9
   |
 8 |         E::A(x) | E::B(y) => x,
   |         ^^^^^^^^^^^^^^^^^

error[E0408]: variable y is not bound in all patterns of this or-pattern
  --> example-11.ls:8:9
   |
 8 |         E::A(x) | E::B(y) => x,
   |         ^^^^^^^^^^^^^^^^^

lsaca: example-11.ls: could not compile example-11.ls due to previous error(s)
```

Or-patterns also nest inside other patterns: a struct field or a tuple component
may itself be an or-pattern:

```ls
struct P {
    x: i64,
    y: i64,
}

fn main() {
    let p = P { x: 2, y: 9 };
    match p {
        P { x: 0 | 1, y } => println!("low {}", y),
        P { x: 2 | 3, y } => println!("mid {}", y),
        _ => println!("other"),
    }

    let pair = (3, 4);
    match pair {
        (1 | 2, b) => println!("a {}", b),
        (3, b) => println!("b {}", b),
        _ => println!("c"),
    }
}
```

Output:
```text
mid 9
b 4
```

Or-patterns are not limited to `match` arms: `if let` and `while let` take a
pattern too, and that pattern may be an or-pattern:

```ls
fn main() {
    let x = 2;
    if let 1 | 2 = x {
        println!("matched");
    } else {
        println!("nope");
    }
}
```

Output:
```text
matched
```

### Match guards

A match arm may add `if condition` between the pattern and `=>`. The arm is
taken only when its pattern matches **and** the guard is true. When the pattern
matches but the guard is false, the remaining arms are tried as usual. The guard
can use the variables bound by its pattern:

```ls
fn grade(n: i64) -> String {
    match n {
        n if n > 10 => String::from("big"),
        5 => String::from("five"),
        _ => String::from("other"),
    }
}

fn main() {
    println!("{} {} {}", grade(11), grade(5), grade(2));
}
```

Output:
```text
big five other
```

Here `grade(5)` first matches `n` with a false guard (`5 > 10` is false), so the
literal arm `5` is tried next. A chain of guarded arms works the same way, and
each guard sees the bindings of its own arm:

```ls
fn main() {
    let v = 5;
    match v {
        n if n > 10 => println!("big"),
        n if n > 0 => println!("small"),
        _ => println!("zero"),
    }

    let w = 0;
    match w {
        n if n > 10 => println!("big"),
        n if n > 0 => println!("small"),
        _ => println!("zero"),
    }
}
```

Output:
```text
small
zero
```

Guards are often used with enum variants to refine an arm. The binding is
available in the guard even when it is an owned `String`:

```ls
enum Msg {
    Text(String),
    Num(i64),
}

fn classify(msg: Msg) -> i64 {
    match msg {
        Msg::Text(s) if s == "hi" => 1,
        Msg::Text(_) => 2,
        Msg::Num(n) if n > 5 => 3,
        Msg::Num(_) => 4,
    }
}

fn main() {
    println!("{} {}", classify(Msg::Text(String::from("hi"))), classify(Msg::Text(String::from("yo"))));
    println!("{} {}", classify(Msg::Num(9)), classify(Msg::Num(2)));
}
```

Output:
```text
1 2
3 4
```

Or-patterns and guards combine. Every alternative must bind the same names, the
guard runs once on those bindings, and a false guard continues with later arms:

```ls
enum Shape {
    A(i64),
    B(i64),
}

fn size(shape: Shape) -> String {
    match shape {
        Shape::A(x) | Shape::B(x) if x > 10 => String::from("big"),
        Shape::A(_) | Shape::B(_) => String::from("small"),
    }
}

fn main() {
    println!("{}", size(Shape::A(12)));
    println!("{}", size(Shape::B(3)));
}
```

Output:
```text
big
small
```

A guard must be a boolean expression, and guarded arms do **not** count toward
exhaustiveness: a guard can fail, so it never proves that its pattern is
covered. The compiler reports `error[E0004]` unless an unguarded arm covers the
remaining values:

```ls,compile-fail
fn main() {
    let n = 5;
    match n {
        n if n > 10 => println!("big"),
        n if n <= 10 => println!("small"),
    }
}
```

Error:
```text
error[E0004]: non-exhaustive patterns: 0 not covered
  --> example-18.ls:3:5
   |
 3 |     match n {
   |     ^^^^^^^^^
   = note: match must be exhaustive; add a wildcard arm or cover the missing patterns

lsaca: example-18.ls: could not compile example-18.ls due to previous error(s)
```

The guard itself is an ordinary expression, so it must have type `bool`. An
integer or float guard is a type error (`E0308`), even when the value's type was
only inferred:

```ls,compile-fail
fn main() {
    let n = 4;
    match n {
        n if n => println!("guard"),
        _ => println!("none"),
    }
}
```

Error:
```text
error[E0308]: mismatched types
  --> example-19.ls:4:14
   |
 4 |         n if n => println!("guard"),
   |              ^
   = note: expected bool, found {integer}

lsaca: example-19.ls: could not compile example-19.ls due to previous error(s)
```

### Nested patterns

Patterns compose. A variant payload can be a struct or tuple pattern, and a
struct field can contain a variant pattern:

```ls
struct Point {
    x: i64,
    y: i64,
}

enum Item {
    Nothing,
    At(Point),
    Pair(Point, Point),
}

fn main() {
    let item = Item::At(Point { x: 1, y: 2 });
    match item {
        Item::Nothing => println!("nothing"),
        Item::At(Point { x, y }) => println!("at {} {}", x, y),
        Item::Pair(a, b) => println!("pair {} {}", a.x, b.y),
    }
}
```

Output:
```text
at 1 2
```

## Exhaustiveness

The compiler requires a `match` to cover every possible value of the matched
type. If an arm set is incomplete you get `error[E0004]` with the missing
patterns named, and the program does not compile:

```ls,compile-fail
fn main() {
    let n = 5;
    let text = match n {
        0 => String::from("zero"),
        1 => String::from("one"),
    };
    println!("{}", text);
}
```

Error:
```text
error[E0004]: non-exhaustive patterns: 2 not covered
  --> example-21.ls:3:16
   |
 3 |     let text = match n {
   |                ^^^^^^^^^
   = note: match must be exhaustive; add a wildcard arm or cover the missing patterns

lsaca: example-21.ls: could not compile example-21.ls due to previous error(s)
```

```ls,compile-fail
fn main() {
    let maybe: Option<i64> = Some(1);
    match maybe {
        Some(v) => println!("{}", v),
    }
}
```

Error:
```text
error[E0004]: non-exhaustive patterns: None not covered
  --> example-22.ls:3:5
   |
 3 |     match maybe {
   |     ^^^^^^^^^^^^^
   = note: match must be exhaustive; add a wildcard arm or cover the missing patterns

lsaca: example-22.ls: could not compile example-22.ls due to previous error(s)
```

A wildcard `_` or a binding covers everything and silences the exhaustiveness
check. Note that the unit type `()` is a single value, so the arm `()` alone is
exhaustive.

## Refutable patterns outside match

`let`, `for` and closure parameters need **irrefutable** patterns — patterns that
always match. `let (a, b) = pair;` and `let Point { x, y } = p;` are fine, but a
pattern that may fail (`Some(x)`, literals) must be written with `match`, `if let`
or `while let`. Using one in `let` is an error:

```ls,compile-fail
fn main() {
    let maybe: Option<i64> = Some(1);
    let Some(value) = maybe;
    println!("{}", value);
}
```

Error:
```text
error[E0005]: refutable pattern in local binding: patterns in this position must be irrefutable
  --> example-23.ls:3:9
   |
 3 |     let Some(value) = maybe;
   |         ^^^^^^^^^^^
   = note: only wildcards, bindings and single-variant destructuring are allowed here

lsaca: example-23.ls: could not compile example-23.ls due to previous error(s)
```

The irrefutable forms `if let` and `while let` are described in
[Statements and control flow](08-statements-and-control-flow.md#if-let-and-while-let).

## Unreachable arms

If an arm can never be selected because earlier arms cover its values, the
compiler emits a `warning[unreachable_patterns]` diagnostic but still compiles
the program (exit code 0). Warnings never make a build fail:

```ls
fn main() {
    let n = 1;
    let text = match n {
        _ => String::from("wildcard first"),
        1 => String::from("one"),
    };
    println!("{}", text);
}
```

Output:
```text
wildcard first
```

## Known limitations in 1.1.0

Or-patterns and match guards are implemented correctly in 1.1.0. The pattern
restrictions that remain are:

* **Nested tuples** — tuple patterns inside tuple patterns still produce invalid
  C in the generated project; keep tuple patterns flat and destructure nested
  pairs in stages.
* **Guards only in `match`** — `if let` and `while let` take a pattern but no
  guard; nest an `if` inside the body instead.
* **No string, range or slice patterns** — match on integers, booleans, enums,
  structs and tuples, then compare strings in the arm body. The full list of
  missing pattern forms is in [Roadmap](14-roadmap.md).

The nested-tuple gap is listed again in [Roadmap](14-roadmap.md) together with
the other known limitations of the release.

## What next

* Control-flow statements that complement `match`:
  [Statements and control flow](08-statements-and-control-flow.md).
* Declaring the types you match on:
  [Structs, enums and impl](10-structs-enums-and-impl.md).
* `Option` and `Result` API: [Standard library](13-standard-library.md#option).
