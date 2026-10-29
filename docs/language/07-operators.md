# Operators

lsaca has the familiar C-family operator set with Rust's precedence. Every
operator is statically typed, and most binary operators require both operands to
have the **same type**: there are no implicit integer promotions and no implicit
numeric conversions. When the types differ, convert explicitly with `as`.

## Precedence and associativity

From loosest to tightest binding:

| Level | Operators | Associativity | Notes |
| --- | --- | --- | --- |
| 1 | `=` `+=` `-=` `*=` `/=` `%=` `&=` `|=` `^=` `<<=` `>>=` | right | Assignment; left side must be a place. |
| 2 | `||` | left | Short-circuit logical OR. |
| 3 | `&&` | left | Short-circuit logical AND. |
| 4 | `==` `!=` `<` `<=` `>` `>=` | left | Comparisons produce `bool`. |
| 5 | `|` | left | Bitwise OR. |
| 6 | `^` | left | Bitwise XOR. |
| 7 | `&` | left | Bitwise AND. |
| 8 | `<<` `>>` | left | Shifts. |
| 9 | `+` `-` | left | Addition and subtraction, also `String` concatenation. |
| 10 | `*` `/` `%` | left | Multiplication, division, remainder. |
| 11 | `as` | left | Type cast. |
| 12 | `-` `!` `*` `&` `&mut` | prefix | Unary negation, logical/bitwise NOT, dereference and borrow. |
| 13 | `f(x)` `x.m()` `x.f` `x[i]` `x?` | left | Calls, method calls, field access, indexing, error propagation. |

Two consequences worth remembering: `1 << 3 + 1` is `16` because `+` binds tighter
than `<<`, and a cast applies to the operand to its left, so `-1 as u8` is
`(-1) as u8` = `255` — not the negation of a cast.

The postfix `?` operator is at the same (tightest) level as calls and field
access: it applies to the value to its left, so `half(n)? + 1` is
`(half(n)?) + 1`. When it propagates `None`/`Err` it returns from the enclosing
function immediately, so nothing to its right is evaluated. See
[Error propagation with `?`](15-standard-library.md#error-propagation-with-).

```ls
fn main() {
    println!("{}", 1 + 2 * 3);
    println!("{}", (1 + 2) * 3);
    println!("{}", 2 * 3 % 4);
    println!("{}", 1 << 3 + 1);
    println!("{}", -2 * 3);
    println!("{}", !true);
    println!("{}", 1 < 2 && 3 > 2);
    println!("{}", 5 & 3 | 8);
}
```

Output:
```text
7
9
2
16
-6
false
true
9
```

## Arithmetic on integers

`+ - * / %` work on all integer types, and both operands must have the same
type (after inference of literals). Division truncates toward zero; the
remainder has the sign of the dividend, as in C and Rust.

```ls
fn main() {
    println!("{} {}", 7 / 2, -7 / 2);
    println!("{} {}", 7 % 2, -7 % 2);
    println!("{}", 7 / -2);
}
```

Output:
```text
3 -3
1 -1
-3
```

Because operands are not promoted, a mixed-type arithmetic expression is a type
error; cast one side explicitly. This is one of the main differences from C:

```ls,compile-fail
fn main() {
    let a: i64 = 1;
    let b: u8 = 2;
    println!("{}", a + b);
}
```

Error:
```text
error[E0308]: mismatched types
  --> example-03.ls:4:20
   |
 4 |     println!("{}", a + b);
   |                    ^^^^^
   = note: expected i64, found u8

lsaca: example-03.ls: could not compile example-03.ls due to previous error(s)
```

### Overflow and division by zero

In the default **Debug** profile every integer arithmetic operation is checked.
An overflow, a division by zero and a remainder with a zero divisor all panic
with a message naming the operation and the source location:

```ls,panic
fn main() {
    let big: i64 = 9_223_372_036_854_775_807;
    println!("{}", big + 1);
}
```

Panic:
```text
thread 'main' panicked at example-04.ls:3:20:
attempt to add with overflow
```

```ls,panic
fn main() {
    let zero = 0;
    println!("{}", 10 / zero);
}
```

Panic:
```text
thread 'main' panicked at example-05.ls:3:20:
attempt to divide by zero
```

```ls,panic
fn main() {
    let zero = 0;
    println!("{}", 10 % zero);
}
```

Panic:
```text
thread 'main' panicked at example-06.ls:3:20:
attempt to calculate the remainder with a divisor of zero
```

In a **Release** build integer arithmetic wraps around (two's complement), which
is the behavior selected by `-r`/`--release` and by the generated project's
Release configuration (`-O2 -DNDEBUG`):

```ls,release
fn main() {
    let big: i64 = 9_223_372_036_854_775_807;
    let small: i64 = -9_223_372_036_854_775_806;
    println!("{}", big + 1);
    println!("{}", small - 2);
}
```

Output:
```text
-9223372036854775808
-9223372036854775808
```

The behavior is chosen when the runtime is compiled, not by the program: the
same `main.c` compiled with `-DLSACA_DEBUG` panics and compiled with `-DNDEBUG`
wraps. See [Getting started](01-getting-started.md) for the build profiles.

## Arithmetic on floats

`+ - * /` work on `f32` and `f64` with IEEE-754 semantics. `%` is also
available for floats and computes the C `fmod` remainder. Division by zero
produces an infinity; `0.0 / 0.0` produces NaN, and printing yields `inf`,
`-inf` and `NaN`.

```ls
fn main() {
    println!("{} {}", 1.5 + 2.25, 10.0 * 0.1);
    println!("{}", 7.5 % 2.0);
    println!("{} {}", 1.0 / 0.0, -1.0 / 0.0);
    println!("{} {}", 0.0 / 0.0, 2.0 / 0.5);
}
```

Output:
```text
3.75 1
1.5
inf -inf
NaN 4
```

Beyond IEEE-754 there are no surprises: NaN is not equal to anything including
itself, and `0.0 == -0.0` is true.

## Strings: concatenation and comparison

`String` supports `+` (concatenation) and the compound `+=`. The right-hand side
of `+` may be a `String` or a string literal. Comparisons `==`, `!=`, `<`, `<=`,
`>`, `>=` compare the UTF-8 bytes lexicographically, which for valid UTF-8 is
the same as comparing by code point; this makes the usual sorting operators
available. Comparisons borrow both operands, so `==` and the ordering operators
do not move the values being compared; both stay usable afterwards.

```ls
fn main() {
    let first = String::from("hello");
    let second = String::from(", world");
    let joined = first + second;
    println!("{}", joined);
    println!("{}", first);

    let mut text = String::from("abc");
    text += "def";
    text += String::from("!");
    println!("{}", text);

    println!("{} {} {}", "apple" < "banana", "Z" < "a", String::from("a") == String::from("a"));
}
```

Output:
```text
hello, world
hello
abcdef!
true true true
```

Concatenation does not consume its operands: `+` borrows both sides, so the left
operand stays usable afterwards (and a cloned handle shares the buffer until it is
written, copy-on-write). `+=` mutates the string in place when its buffer is
unique and clones first otherwise (see [Memory model](05-memory-model.md)).

## Boolean operators

`&&`, `||` and `!` work on `bool` and are the only operators with
**short-circuit** evaluation: `a && b` does not evaluate `b` when `a` is false,
and `a || b` does not evaluate `b` when `a` is true. This is observable with a
function that mutates an `Rc` cell:

```ls
fn step(counter: Rc<i64>) -> bool {
    counter.set(counter.get() + 1);
    true
}

fn main() {
    let counter = Rc::new(0);
    let a = step(counter.clone()) && step(counter.clone());
    println!("{} {}", a, counter.get());

    let b = step(counter.clone()) || step(counter.clone());
    println!("{} {}", b, counter.get());
}
```

Output:
```text
true 2
true 3
```

The first expression evaluates both calls (the left side is true, so the right
side is needed) and increments the counter to 2; the second evaluation starts at
2, the left side is true, so `||` short-circuits and the counter stays 2.

Both operands must be `bool`; the compiler rejects integers with `E0308`
(`true && 1` does not compile). Conditions in `if`, `while` and `match` guards
follow the same rule — see [Statements and control
flow](08-statements-and-control-flow.md#conditions).

## Bitwise operators

`&`, `|`, `^`, `!` and the shifts `<<`, `>>` work on integers. `!` is a
**bitwise** NOT on integers (so `!0` is `-1`) and a logical NOT on `bool`. The
shift count must have the same type as the shifted value, `i64` by default;
shifting by a negative count or by a count greater than or equal to the bit width
panics in Debug builds.

```ls
fn main() {
    let a: u8 = 0b1100;
    let b: u8 = 0b1010;
    println!("{} {} {}", a & b, a | b, a ^ b);
    println!("{}", !a);

    let n: i64 = 0;
    println!("{}", !n);
    println!("{} {}", 1 << 4, 256 >> 2);
    println!("{}", 0b1010_1010 & 0b1111_0000);
}
```

Output:
```text
8 14 6
243
-1
16 64
160
```

Right shift of a signed negative value is arithmetic (fills with the sign bit);
left shift of a signed value is a logical shift on the two's-complement bits:

```ls
fn main() {
    println!("{}", -8 >> 1);
    println!("{}", 1 << 62);
}
```

Output:
```text
-4
4611686018427387904
```

A shift that overflows the width panics in Debug builds:

```ls,panic
fn main() {
    println!("{}", 1 << 70);
}
```

Panic:
```text
thread 'main' panicked at example-13.ls:2:20:
attempt to shift left with overflow
```

## Comparisons and equality

Comparison operators produce `bool` and require both operands to have the same
type:

* integers compare numerically, signed and unsigned according to their types;
* floats compare by IEEE-754 rules (NaN compares false with everything);
* `String` compares byte-wise as described above;
* `bool` compares with `==`/`!=` only; ordering is not available;
* `Rc<T>` compares by **identity** — two handles are equal when they point at the
  same cell;
* `Box<T>` compares by payload, structurally;
* since 1.5.0, tuples, user structs and enums, `Vec<T>`, `Option<T>`, `Result<T,
  E>`, references and `HashMap<K, V>` also support `==` and compare structurally
  (a reference compares the pointed-to value; a map compares contents ignoring
  insertion order);
* function values do **not** support `==`; the compiler reports `E0369`;
* `()` does not support `==` either.

```ls
fn main() {
    let a = Rc::new(5);
    let alias = a.clone();
    let other = Rc::new(5);
    println!("{} {}", a == alias, a == other);

    let boxed = Box::new(5);
    let separate = Box::new(5);
    println!("{}", boxed == separate);

    println!("{} {}", 1 == 1, 1 != 2);
    println!("{}", "abc" == "abc");

    let left = vec![1, 2, 3];
    let right = vec![1, 2, 3];
    println!("{} {}", left == right, Some(4) == Some(4));

    let pair = (1, "x");
    println!("{} {}", pair == (1, "x"), 4 == 5);
}
```

Output:
```text
true false
true
true true
true
true true
true false
```

Using `==` on a type that does not implement equality is rejected. The unit
type is the simplest example (the aggregate types above all gained `==` in
1.5.0):

```ls,compile-fail
fn main() {
    let left = ();
    let right = ();
    println!("{}", left == right);
}
```

Error:
```text
error[E0369]: binary operation == cannot be applied to type ()
  --> example-15.ls:4:20
   |
 4 |     println!("{}", left == right);
   |                    ^^^^^^^^^^^^^

lsaca: example-15.ls: could not compile example-15.ls due to previous error(s)
```

## Borrowing and dereferencing

The unary `&` and `&mut` operators form a *borrow expression*; their operand
must be a **place** (a variable, parameter, field, index, or a dereferenced
reference). Borrowing a temporary such as `&(1 + 2)`, `&String::from("hi")` or a
constant is rejected with E0745 — bind the value to a local first. `&place`
creates a shared reference; `&mut place` creates a mutable one and requires the
place itself to be mutable.

The unary `*` operator is a *dereference*: `*r` reads the value a reference
points to, and `*r = value` writes through a mutable reference. Dereference
binds tighter than every binary operator, so `*r + 1` means `(*r) + 1`, while
`&n + 1` parses as `(&n) + 1` and is a type error.

Field access, method calls and indexing auto-dereference, so `r.field` reads
or writes a field through a `&mut` reference and `r[i]` indexes through it;
the explicit `(*r).field = value` form is equivalent. References can be
compared with `==` since 1.5.0 — the comparison reads the pointed-to values, so
`a == b` means `*a == *b` — but they cannot be converted with `as`.

```ls
fn main() {
    let mut n = 10;
    let r = &n;
    println!("{}", *r + 1);

    let m = &mut n;
    *m = *m + 5;
    println!("{}", n);
}
```

Output:
```text
11
15
```

This is a short summary; the full aliasing rules, reference parameters,
elision and match ergonomics are on [References and
borrowing](12-references-and-borrowing.md).

```ls,compile-fail
fn main() {
    let n = 1;
    println!("{}", &n + 1);
}
```

Error:
```text
error[E0308]: mismatched types
  --> example-17.ls:3:20
   |
 3 |     println!("{}", &n + 1);
   |                    ^^^^^^
   = note: expected &{integer}, found {integer}

lsaca: example-17.ls: could not compile example-17.ls due to previous error(s)
```

## Casts

The `as` operator converts between scalar types. It binds tighter than
arithmetic and looser than unary operators, and it can be chained
(`x as i32 as f64`). References cannot be converted with `as`. The conversion rules — truncation for integers, saturation
for float-to-integer, rounding for large integer-to-float — are listed in
[Types](04-types.md#casts).

```ls
fn main() {
    println!("{}", 300 as u8);
    println!("{}", -1 as u8);
    println!("{}", 3.99 as i64);
    println!("{}", 65 as f64);
    println!("{}", 200 as u8 as i64);
}
```

Output:
```text
44
255
3
65
200
```

## What next

* Blocks and control flow, where these operators live:
  [Statements and control flow](08-statements-and-control-flow.md).
* The complete type rules behind the operators: [Types](04-types.md).
* Borrowing, dereferencing and the aliasing rules behind `&`, `&mut` and `*`:
  [References and borrowing](12-references-and-borrowing.md).
* Operator behavior of floats and strings in the built-in API:
  [Standard library](15-standard-library.md).
