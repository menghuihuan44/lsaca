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
| 12 | `-` `!` | prefix | Unary negation and logical/bitwise NOT. |
| 13 | `f(x)` `x.m()` `x.f` `x[i]` | left | Calls, method calls, field access, indexing. |

Two consequences worth remembering: `1 << 3 + 1` is `16` because `+` binds tighter
than `<<`, and a cast applies to the operand to its left, so `-1 as u8` is
`(-1) as u8` = `255` — not the negation of a cast.

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
available.

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

Concatenation copies a handle, not the characters: the left operand remains
usable afterwards, as the example shows. `+=` on a `String` follows
copy-on-write, so it mutates in place when the buffer is unique and clones first
otherwise (see [Memory model](05-memory-model.md)).

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
    let a = step(counter) && step(counter);
    println!("{} {}", a, counter.get());

    let b = step(counter) || step(counter);
    println!("{} {}", b, counter.get());
}
```

Output:
```text
true 2
true 4
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
* tuples, structs, enums, `Vec<T>`, `Option<T>` and `Result<T, E>` do **not**
  support `==` in 1.1.0, and the compiler reports `E0369`. Compare them by
  pattern matching instead;
* `()` does not support `==` either.

```ls
fn main() {
    let a = Rc::new(5);
    let alias = a;
    let other = Rc::new(5);
    println!("{} {}", a == alias, a == other);

    let boxed = Box::new(5);
    let separate = Box::new(5);
    println!("{}", boxed == separate);

    println!("{} {}", 1 == 1, 1 != 2);
    println!("{}", "abc" == "abc");
}
```

Output:
```text
true false
true
true true
true
```

Using `==` on a container type is rejected:

```ls,compile-fail
fn main() {
    let a = vec![1, 2, 3];
    let b = vec![1, 2, 3];
    println!("{}", a == b);
}
```

Error:
```text
error[E0369]: binary operation == cannot be applied to type Vec<{integer}>
  --> example-15.ls:4:20
   |
 4 |     println!("{}", a == b);
   |                    ^^^^^^

lsaca: example-15.ls: could not compile example-15.ls due to previous error(s)
```

## Casts

The `as` operator converts between scalar types. It binds tighter than
arithmetic and looser than unary operators, and it can be chained
(`x as i32 as f64`). The conversion rules — truncation for integers, saturation
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
* Operator behavior of floats and strings in the built-in API:
  [Standard library](13-standard-library.md).
