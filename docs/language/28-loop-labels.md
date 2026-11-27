# Loop Labels

Version 2.2 adds loop labels. A label names a loop so that `break` and
`continue` can target that loop from inside a nested loop. The syntax is a
lifetime-like name followed by a colon before the loop keyword:

```text
'name: loop { ... }
'name: while condition { ... }
'name: for pattern in iterable { ... }
'name: while let pattern = expression { ... }
```

Inside the loop, `break 'name;` leaves that loop and `continue 'name;` starts
its next iteration; a bare `break` or `continue` still applies to the
innermost loop. A label is a compile-time name, not a value, and the leading
apostrophe is the same character used by [lifetimes](29-lifetimes.md); the two
kinds of names occupy separate namespaces.

## Leaving a nested loop

The typical use is leaving an outer loop from inside an inner one:

```ls
fn main() {
    let mut found = 0;
    'outer: for row in 0..3 {
        for column in 0..3 {
            if row == 1 && column == 2 {
                found = row * 10 + column;
                break 'outer;
            }
        }
    }
    println!("{}", found);
}
```

Output:

```text
12
```

## Continuing an outer loop

`continue 'outer;` abandons the rest of the inner loop and starts the next
iteration of the named outer loop:

```ls
fn main() {
    let mut count = 0;
    'rows: for row in 0..3 {
        for column in 0..3 {
            if column == 1 {
                continue 'rows;
            }
            count += 1;
        }
    }
    println!("{}", count);
}
```

Output:

```text
3
```

## Labels on while and while let

A label can precede `while`, `while let`, `loop` and `for`. This example
leaves a `while let` when the iterator runs out:

```ls
fn main() {
    let values = vec![Some(1), Some(2), None, Some(3)];
    let mut index = 0;
    let mut total = 0;
    'each: while let Some(item) = values.get(index) {
        index += 1;
        match item {
            Some(value) => { total += value; }
            None => { break 'each; }
        }
    }
    println!("{}", total);
}
```

Output:

```text
3
```

## Labels and shadowing

A label is resolved to the innermost enclosing loop with that name, so an inner
loop may reuse a name already used by an outer loop. `break` or `continue`
with the reused name targets the inner loop:

```ls
fn main() {
    let mut count = 0;
    'a: for i in 0..2 {
        'a: for j in 0..2 {
            if j == 1 {
                continue 'a;
            }
            count += 1;
        }
    }
    println!("{}", count);
}
```

Output:

```text
2
```

## Breaking with a value

Since 2.4.0 a labelled `loop` can carry a value out of a nested loop. The value
is written after the label, `break 'name value;`, and becomes the result of
that loop expression. A label on a `while` or `for` loop does not change the
rule that those loops have type `()`: a value on their `break` is `E0571`.
`continue value;` is always rejected with `E0308` and a note that the language
has no continue values.

```ls
fn main() {
    let mut n = 0;
    let result = 'search: loop {
        n += 1;
        if n == 4 {
            break 'search n * 10;
        }
    };
    println!("{}", result);
}
```

Output:

```text
40
```

The same rules apply to unlabelled loops: `break value` gives the innermost
`loop` that value's type, and every `break` that leaves the loop must agree on
the type. See [Statements and control flow](08-statements-and-control-flow.md#loop).

## Label errors

An unknown label is reported as
`error[E0426]: use of undeclared label 'missing'`. A label is only allowed
directly before a loop: putting it before a statement such as `let` is
`error[E0001]: a loop label can only precede 'loop', 'while' or 'for', found let`,
and a missing colon is
`error[E0001]: expected ':' after a loop label`. A value on a `break` that
targets a `while` or `for` loop is `E0571`, even when the loop is labelled;
`continue value;` is `E0308`.

## Known limitations in 2.4.0

* A value on `break` is only meaningful for `loop`; `while` and `for` still have
  type `()`, and `break value` in them is `E0571`.
* Labels are only allowed on `loop`, `while`, `while let` and `for`; there are
  no labeled blocks and no labeled `match` or `if`.
* The label must be written directly before the loop keyword. A colon is
  mandatory, and labels cannot be used as ordinary identifiers or values.

## Summary

* `'name: loop`, `'name: while`, `'name: while let` and `'name: for` attach
  a label to a loop.
* `break 'name;` and `continue 'name;` target the named loop; a bare
  `break` or `continue` targets the innermost loop.
* Reused names resolve to the innermost loop with that name.
* Unknown labels are `E0426`; a misplaced or malformed label is `E0001`.
* `break 'name value;` makes a labelled `loop` produce that value; `while` and
  `for` keep type `()`, and `continue value;` is rejected.

## What next

* [Statements and control flow](08-statements-and-control-flow.md) covers the
  loops themselves.
* [Lifetimes](29-lifetimes.md) covers the other use of the apostrophe prefix.
* [Arrays](27-arrays.md) covers fixed-size sequences.
