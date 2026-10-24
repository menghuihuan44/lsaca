# Statements and control flow

lsaca is expression-oriented: blocks, `if`/`else`, `match` and loops are
expressions, and some of them produce a value you can bind. This page covers
every statement form and every loop in the language. Pattern matching gets its
own page — [Pattern matching](11-pattern-matching.md).

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
nested `if`). `continue` skips to the next iteration. A loop has type `()`:
`break` cannot carry a value in 1.1.0.

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

Breaking with a value is not supported:

```ls,compile-fail
fn main() {
    let x = loop {
        break 5;
    };
}
```

Error:
```text
error[E0308]: break with a value is not supported; loops have type ()
  --> example-11.ls:3:15
   |
 3 |         break 5;
   |               ^

lsaca: example-11.ls: could not compile example-11.ls due to previous error(s)
```

## for and ranges

`for` iterates over a value. The two range forms are:

* `start..end` — from `start` up to but not including `end`;
* `start..=end` — from `start` up to and including `end`.

Both endpoints are `i64` (the default integer type); cast if you computed them in
another type. The loop variable is a fresh immutable binding, but — a 1.1.0
leniency — `for mut i in …` is accepted and lets you reassign it inside the body;
that does not change the iteration.

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

`for` over a `Vec<T>` visits every element. Iteration yields **copies** of the
elements, so the original vector stays usable and unmodified afterwards:

```ls
fn main() {
    let numbers = vec![3, 1, 4, 1, 5];
    let mut total = 0;
    for n in numbers {
        total += n;
    }
    println!("{} {}", total, numbers.len());
}
```

Output:
```text
14 5
```

Because iteration is by value, iterating a `Vec<String>` gives you copies of the
strings too (copy-on-write makes that cheap). To walk an index sequence instead,
iterate a range and index the vector — remember that indexing is bounds checked
and panics on a bad index:

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

The `mut` on a loop variable is accepted in 1.1.0; the assignment does not affect
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

## break and continue

`break` and `continue` are valid only inside `loop`, `while` or `for`. `break`
leaves the innermost loop, `continue` starts the next iteration of the innermost
loop. There are no labels in 1.1.0, so to leave nested loops use a flag or a
function with `return`.

Outside a loop, `continue` (and `break`) is a compile error:

```ls,compile-fail
fn main() {
    continue;
}
```

Error:
```text
error[E0268]: continue outside of a loop
  --> example-17.ls:2:5
   |
 2 |     continue;
   |     ^^^^^^^^

lsaca: example-17.ls: could not compile example-17.ls due to previous error(s)
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
  [Pattern matching](11-pattern-matching.md).
* The operators used in these conditions: [Operators](07-operators.md).
* Closures, which appear wherever a function value is expected:
  [Closures](12-closures.md).
