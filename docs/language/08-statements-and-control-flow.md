# Statements and control flow

lsaca is expression-oriented: blocks, `if`/`else`, `match` and loops are
expressions, and some of them produce a value you can bind. This page covers
every statement form and every loop in the language. Pattern matching gets its
own page — [Pattern matching](13-pattern-matching.md).

## Blocks

A block is a list of statements in braces. It is itself an expression: the value
of a block is the value of its final expression, provided that final expression
is **not** followed by a semicolon. A block whose last statement ends with a
semicolon — or which has no final expression at all — has type `()`.

```ls
fn main() {
    let area = {
        let width = 3;
        let height = 4;
        width * height
    };
    println!("{}", area);

    let unit = {
        1;
        2;
        3
    };
    println!("{}", unit);

    let trailing = {
        let a = 2;
        let b = 3;
        a * b
    };
    println!("{}", trailing);
}
```

Output:
```text
12
3
6
```

Semicolons are significant. `{ 1; 2; 3 }` evaluates each expression statement in
turn and yields `3` because the last expression has no semicolon; adding a
semicolon after `3` would make the block yield `()` instead.

### Statements

The statements allowed inside a block are:

* `let` statements, including destructuring `let` (`let (a, b) = pair;`);
* expression statements (`foo();`, `x = y;`, `x += 1;`, `push(v, 1);` …);
* nested items — `fn`, `struct`, `enum`, `impl`, `const` and `type` — which are
  hoisted to the whole enclosing body (see
  [Program structure](02-program-structure.md#nested-items-and-hoisting));
* other control-flow statements: `if`, `match`, `while`, `loop`, `for`,
  `break`, `continue`, `return`.

An expression statement is an expression followed by a semicolon. Assignments
are expressions of type `()`, so `x = 1;` is valid as a statement, and as a value
an assignment evaluates to `()`:

```ls
fn main() {
    let mut x = 0;
    let unit = { x = 7; };
    println!("{} {}", x, unit);
}
```

Output:
```text
7 ()
```

## if / else

`if` takes a condition and a block. The optional `else` branch may be another
`if` (an `else if` chain) or a block. Both branches must have the same type when
the `if` is used as a value, and an `if` without `else` always has type `()`.

```ls
fn main() {
    let a = 7;
    let b = 3;
    let larger = if a > b { a } else { b };
    println!("{}", larger);

    let score = 83;
    let grade = if score >= 90 {
        4
    } else if score >= 80 {
        3
    } else if score >= 70 {
        2
    } else {
        0
    };
    println!("{}", grade);
}
```

Output:
```text
7
3
```

When the `if` is used as a statement, no value is expected and either branch
type is allowed; the value of the branch is simply discarded:

```ls
fn main() {
    for n in 1..20 {
        if n % 15 == 0 {
            println!("fizzbuzz");
        } else if n % 3 == 0 {
            println!("fizz");
        } else if n % 5 == 0 {
            println!("buzz");
        }
    }
}
```

Output:
```text
fizz
buzz
fizz
fizz
buzz
fizz
fizzbuzz
fizz
```

An `if` without `else` yields `()`, so it cannot be used where a value is
required:

```ls,compile-fail
fn main() {
    let x = if true { 1 };
}
```

Error:
```text
error[E0308]: mismatched types
  --> example-05.ls:2:21
   |
 2 |     let x = if true { 1 };
   |                     ^^^^^
   = note: expected (), found {integer}

lsaca: example-05.ls: could not compile example-05.ls due to previous error(s)
```

### Conditions

The condition of `if` and `while` must be a `bool` expression. There is no
implicit conversion from numbers: `if 3`, `if 1.5` and `if x` where `x` is an
inferred integer are all type errors. The same rule applies to the operands of
`&&` and `||`, to `match` guards and to the argument of `assert!`: the compiler
reports `E0308` with `expected bool` and the found type, which is shown as
`{integer}` or `{float}` when the type was only inferred:

```ls,compile-fail
fn main() {
    let zero = 0;
    if zero {
        println!("zero is true");
    }
}
```

Error:
```text
error[E0308]: mismatched types
  --> example-06.ls:3:8
   |
 3 |     if zero {
   |        ^^^^
   = note: expected bool, found {integer}

lsaca: example-06.ls: could not compile example-06.ls due to previous error(s)
```

A non-boolean operand of `&&` is rejected the same way:

```ls,compile-fail
fn main() {
    let flag = true;
    let count = 1;
    if flag && count {
        println!("both");
    }
}
```

Error:
```text
error[E0308]: mismatched types
  --> example-07.ls:4:16
   |
 4 |     if flag && count {
   |                ^^^^^
   = note: expected bool, found {integer}

lsaca: example-07.ls: could not compile example-07.ls due to previous error(s)
```

Write the comparison out. The condition slot takes an ordinary expression, so a
comparison, a boolean variable or a call that returns `bool` are all fine:

```ls
fn main() {
    let zero = 0;
    let nonzero = 7;
    if zero != 0 {
        println!("zero is true");
    } else {
        println!("zero is false");
    }
    if nonzero != 0 {
        println!("nonzero is true");
    }
    assert!(nonzero > 0);
}
```

Output:
```text
zero is false
nonzero is true
```

There are no `if let` chains combined with `&&`; write nested `if let` blocks or a
`match` instead.

## while

`while` repeats a block as long as its condition is true. The condition is
evaluated before every iteration:

```ls
fn main() {
    let mut n = 10;
    while n > 0 {
        if n % 2 == 0 {
            println!("even {}", n);
        }
        n -= 3;
    }
    println!("done {}", n);
}
```

Output:
```text
even 10
even 4
done -2
```

## loop

`loop` repeats its block forever. Leave it with `break` (optionally from inside a
nested `if`). `continue` skips to the next iteration. Since 2.4.0 a `loop` can
produce a value: `break value` makes the loop expression evaluate to `value`, and
a bare `break` gives `()`. Every `break` that leaves the same loop must produce
the same type. Since 2.2.0 a loop may also be labelled, which lets `break` and
`continue` name an outer loop; see [Loop labels](28-loop-labels.md).

A bare `break` loop still has type `()`:

```ls
fn main() {
    let mut total = 0;
    let mut n = 0;
    loop {
        n += 1;
        if n == 10 {
            break;
        }
        if n % 2 == 0 {
            continue;
        }
        total += n;
    }
    println!("{}", total);
}
```

Output:
```text
25
```

To produce a value, use `break value` inside a `loop`. The value becomes the
result of the loop expression, so it can be bound, returned or passed to another
expression:

```ls
fn main() {
    let answer = loop {
        break 42;
    };
    println!("{}", answer);

    let mut n = 0;
    let doubled = loop {
        n += 1;
        if n == 3 {
            break n * 2;
        }
    };
    println!("{} {}", n, doubled);
}
```

Output:
```text
42
3 6
```

The value type is inferred from the `break` expressions and can be any type,
including an owned `String` or a tuple. A bare `break` has type `()`, so mixing
`break;` and `break value;` in one loop is a type mismatch (`E0308`).

`while` and `for` loops always have type `()`; a value attached to a `break` in
one of them is rejected with `E0571`. Move the value out in a variable, or use a
`loop` when the loop itself must produce a result. `continue` never accepts a
value: `continue value;` is rejected with `E0308` and a note that the language
has no continue values.

## for and ranges

`for` iterates over a value. The two range forms are:

* `start..end` — from `start` up to but not including `end`;
* `start..=end` — from `start` up to and including `end`.

The endpoints must have the same integer type, and the loop variable has that
type; any integer type works, not just `i64`. Unsuffixed literal endpoints
default to `i32`, so `for i in 0..5` iterates `i32` values, while a range over a
`usize` count takes the `usize` type because the unsuffixed `0` adapts to it. The
loop variable is a fresh immutable binding, but — a parser leniency — `for mut i
in …` is accepted and lets you reassign it inside the body; that does not change
the iteration.

```ls
fn main() {
    for i in 0..5 {
        println!("{}", i);
    }
    for i in 3..=6 {
        println!("inclusive {}", i);
    }
    for i in 0..3 {
        println!("mut {}", i);
    }
}
```

Output:
```text
0
1
2
3
4
inclusive 3
inclusive 4
inclusive 5
inclusive 6
mut 0
mut 1
mut 2
```

A reversed or empty range simply iterates zero times; it does not panic:

```ls
fn main() {
    for i in 5..2 {
        println!("never {}", i);
    }
    for i in 3..=1 {
        println!("never {}", i);
    }
    println!("empty ranges are fine");
}
```

Output:
```text
empty ranges are fine
```

### Iterating a Vec

`for` over a `Vec<T>` visits every element, but it **consumes the vector**: the
vector is moved into the loop, so the binding cannot be used afterwards.
Elements are yielded by value — Copy elements as copies, move-only elements as
moves out of the vector. Since 2.3.0, `for n in &numbers` borrows the vector
instead and yields `&T` items, `for n in &mut numbers` yields `&mut T`, and
the vector remains usable after the loop:

```ls
fn main() {
    let numbers = vec![3, 1, 4, 1, 5];
    let mut total = 0;
    for n in numbers {
        total += n;
    }
    println!("{}", total);
}
```

Output:
```text
14
```

Because the vector is consumed, walking a `Vec<String>` yields owned `String`
values. To keep using the vector afterwards, iterate a range and index it
instead — indexing in a borrowed context such as `println!` reads the element
without moving it, so the vector stays usable. Remember that indexing is bounds checked and panics on a bad index:

```ls
fn main() {
    let names = vec![String::from("a"), String::from("b")];
    for i in 0..names.len() {
        println!("{}: {}", i, names[i]);
    }
}
```

Output:
```text
0: a
1: b
```

The `mut` on a loop variable is accepted in 2.2.0; the assignment does not affect
the sequence of values:

```ls
fn main() {
    for mut i in 0..3 {
        i += 10;
        println!("{}", i);
    }
}
```

Output:
```text
10
11
12
```

### Iterating an iterator

Since 1.5.0 a `for` loop also accepts any value whose type implements
`Iterator<T>`. The iterator is moved into the loop and released when the loop
ends; `for` over a `Vec` value and over an integer range keep their own
lowering (see [Iterators](19-iterators.md)):

```ls
fn main() {
    let words = vec![String::from("alpha"), String::from("beta")];
    for (index, word) in words.iter().enumerate() {
        // enumerate yields a usize index and the element type; `.iter()`
        // borrows `words`, so the item type here is `&String`
        println!("{}: {}", index, word.as_str());
    }
}
```

Output:
```text
0: alpha
1: beta
```

Since 2.3.0 a `for` loop can also iterate over a borrowed vector with
`for value in &numbers` (shared items) or `for value in &mut numbers`
(mutable items). The loop borrows the vector for its duration and leaves it
usable afterwards:

```ls
fn main() {
    let numbers = vec![1, 2, 3];
    let borrowed = &numbers;
    for value in borrowed {
        println!("{}", *value);
    }
    println!("{}", numbers.len());
}
```

Output:
```text
1
2
3
3
```

The same applies to `&[T]`/`&mut [T]` slices and to `&HashMap<K, V>`; see
[Iterators](19-iterators.md#borrowing-loops-and-slices).

## break and continue

`break` and `continue` are valid only inside `loop`, `while` or `for`. `break`
leaves the innermost loop, `continue` starts the next iteration of the innermost
loop.

Since 2.2.0 a loop may be labelled with a lifetime token and a colon —
`'outer: loop`, `'outer: while cond`, `'outer: for value in values`, and
`'outer: while let Some(value) = …` — and `break 'outer` or `continue 'outer`
then names that loop instead of the innermost one:

```rust
'outer: for row in 0..4 {
    for column in 0..4 {
        if row * column > 4 {
            break 'outer;
        }
    }
}
```

The label name may shadow an outer label of the same name (the innermost wins),
a label before a non-loop item is a syntax error, and a label that no enclosing
loop declares is `E0426`. A values-out-of-`break` loop follows the same rules
when labelled: `break 'outer value;` produces the value for the labelled loop.
The full rules, including `while let`, are on
[Loop labels](28-loop-labels.md).

A labelled `loop` can both choose an outer loop and produce a value:

```ls
fn main() {
    let found = 'outer: loop {
        let mut n = 0;
        loop {
            n += 1;
            if n == 5 {
                break 'outer n;
            }
        }
    };
    println!("{}", found);
}
```

Output:
```text
5
```

Outside a loop, `continue` (and `break`) is a compile error:

```ls,compile-fail
fn main() {
    continue;
}
```

Error:
```text
error[E0268]: continue outside of a loop
  --> example-20.ls:2:5
   |
 2 |     continue;
   |     ^^^^^^^^

lsaca: example-20.ls: could not compile example-20.ls due to previous error(s)
```

## if let and while let

`if let` matches a refutable pattern and runs the block when the pattern matches,
with an optional `else` branch. It is the idiomatic way to unwrap one `Option` or
`Result` layer without a full `match`:

```ls
fn main() {
    let some: Option<i64> = Some(42);
    let none: Option<i64> = None;

    if let Some(value) = some {
        println!("got {}", value);
    } else {
        println!("nothing");
    }

    if let None = none {
        println!("confirmed none");
    }
}
```

Output:
```text
got 42
confirmed none
```

`while let` repeats a block as long as the pattern keeps matching. It is the
standard way to drain a `Vec<T>` with `pop`:

```ls
fn main() {
    let mut stack = vec![10, 20, 30];
    let mut sum = 0;
    while let Some(top) = stack.pop() {
        sum += top;
    }
    println!("{} {}", sum, stack.len());
}
```

Output:
```text
60 0
```

## return

`return` leaves the enclosing function immediately with an optional value. A
bare `return;` is only valid in a function returning `()`. The usual style is a
tail expression for the final value and `return` for early exits:

```ls
fn classify(n: i64) -> String {
    if n < 0 {
        return String::from("negative");
    }
    if n == 0 {
        return String::from("zero");
    }
    String::from("positive")
}

fn main() {
    println!("{} {} {}", classify(-2), classify(0), classify(9));
}
```

Output:
```text
negative zero positive
```

For the full function rules — return types, tail expressions and the restrictions
on `main` — see [Functions](09-functions.md).

## What next

* Match expressions, exhaustiveness and pattern syntax:
  [Pattern matching](13-pattern-matching.md).
* The operators used in these conditions: [Operators](07-operators.md).
* Closures, which appear wherever a function value is expected:
  [Closures](14-closures.md).

