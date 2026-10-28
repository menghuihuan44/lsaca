# Variables and mutability

Bindings are introduced with `let`. By default a binding is immutable; `let mut`
makes the binding mutable. This page covers bindings, shadowing, assignment,
compound assignment, destructuring and constants.

## Bindings

The form is:

```rust
let name = expression;
let name: Type = expression;
let mut name = expression;
let mut name: Type = expression;
```

Rules:

* An initializer is **required**. There is no uninitialized declaration and no
  deferred initialization: the type and the value are fixed at the point of
  declaration.
* The type annotation is optional; the type of the binding is the type of the
  initializer, after inference.
* A binding is immutable unless declared `mut`. Immutability is about the
  *binding*, not the value: an immutable binding cannot be assigned to, but an
  `Rc` handle in an immutable binding can still be used with `set` (the cell is
  shared state, see [Memory model](05-memory-model.md)).
* The initializer may be an arbitrary expression, including a block or an
  `if` expression, and it is evaluated exactly once.
* For move-only types (`String`, `Vec`, `Rc`, `Box`, function values and the user
  types that contain them) the initializer is **moved** into the new binding when
  it names an existing place, so the old binding cannot be used afterwards.
  `Copy` values are copied instead. Use `.clone()` when both names are needed
  (see [Ownership and moves](11-ownership-and-moves.md)).

```ls
fn main() {
    let answer = 42;
    let pi: f64 = 3.5;
    let mut counter = 0;
    counter += 1;
    counter += 1;
    let label: String = if counter > 1 { String::from("many") } else { String::from("few") };
    println!("{} {} {} {}", answer, pi, counter, label);
}
```

Output:
```text
42 3.5 2 many
```

A move-only value is moved into its new binding; a `Copy` value is copied and
the source stays usable:

```ls
fn main() {
    let first = String::from("lsaca");
    let second = first;                 // moves `first`
    println!("{}", second);

    let number = 7;
    let copy = number;                  // Copy: `number` is still usable
    println!("{} {}", number, copy);
}
```

Output:

```text
lsaca
7 7
```

Using the moved binding is a compile error:

```ls,compile-fail
fn main() {
    let first = String::from("lsaca");
    let second = first;
    println!("{} {}", first, second);
}
```

Error:

```text
error[E0382]: use of moved value: first
  --> example-03.ls:4:23
   |
 4 |     println!("{} {}", first, second);
   |                       ^^^^^
   = note: example-03.ls:3:18: value moved here
   = note: first has type String, which is not Copy, so the value is moved whenever it is used by value; use .clone() to copy it first

lsaca: example-03.ls: could not compile example-03.ls due to previous error(s)
```

Declaring a binding without an initializer is rejected:

```ls,compile-fail
fn main() {
    let x: i64;
    println!("{}", x);
}
```

Error:
```text
error[E0001]: cannot declare a binding without an initializer
  --> example-04.ls:2:5
   |
 2 |     let x: i64;
   |     ^^^^^^^^^^^

lsaca: example-04.ls: could not compile example-04.ls due to previous error(s)
```

## Immutability

Assigning to a binding that was not declared `mut` is a compile-time error, and
the diagnostic explains that the binding is not mutable. This applies to every
kind of assignment, including compound assignment and assignment to a field or
index of an immutable binding.

```ls,compile-fail
fn main() {
    let count = 1;
    count = count + 1;
    println!("{}", count);
}
```

Error:
```text
error[E0596]: cannot assign to immutable value: the binding has not been declared as mutable
  --> example-05.ls:3:5
   |
 3 |     count = count + 1;
   |     ^^^^^

lsaca: example-05.ls: could not compile example-05.ls due to previous error(s)
```

The same program with `let mut count = 1;` compiles and prints `2`:

```ls
fn main() {
    let mut count = 1;
    count = count + 1;
    println!("{}", count);
}
```

Output:
```text
2
```

Note that a mutable *field* still needs a mutable *binding*: `let p = Point { ... }`
followed by `p.x = 1;` is rejected, even though the field is conceptually a slot.
Mutability is a property of the binding path, and `mut` must appear at the root.

```ls,compile-fail
struct Point {
    x: i64,
    y: i64,
}

fn main() {
    let p = Point { x: 1, y: 2 };
    p.x = 10;
    println!("{}", p.x);
}
```

Error:
```text
error[E0596]: cannot assign to immutable value: the binding has not been declared as mutable
  --> example-07.ls:8:5
   |
 8 |     p.x = 10;
   |     ^^^

lsaca: example-07.ls: could not compile example-07.ls due to previous error(s)
```

## Shadowing

A new `let` can reuse an existing name. Shadowing creates a *new* binding that
has its own type and mutability; the previous binding still exists until the end
of its scope, but it is no longer reachable by name. Shadowing is allowed in the
same block and in inner blocks; an inner block's shadow disappears when the
block ends.

Shadowing is frequently used to change a value's type or to refine a
computation:

```ls
fn main() {
    let value = "42";
    let value = value.parse_i64().unwrap();
    let value = value * 2;
    println!("{}", value);

    let text = String::from("outer");
    {
        let text = 5;
        println!("{}", text);
    }
    println!("{}", text);
}
```

Output:
```text
84
5
outer
```

Shadowing is **not** mutation: the old binding is not modified. Shadowing a
move-only value with an unrelated initializer leaves the old value alone; if the
new initializer uses the old binding, the value is moved into the new binding
exactly like any other use. If you need to change a value, use `let mut`, not a
shadow.

## Reading before declaration

A name can only be used after its `let` (and after the declaration of the item
that introduces it, for types and functions — items are hoisted, variables are
not). Using a variable before its declaration is a name-resolution error:

```ls,compile-fail
fn main() {
    println!("{}", later);
    let later = 1;
}
```

Error:
```text
error[E0425]: cannot find value later in this scope
  --> example-09.ls:2:20
   |
 2 |     println!("{}", later);
   |                    ^^^^^

lsaca: example-09.ls: could not compile example-09.ls due to previous error(s)
```

## Assignment and compound assignment

Assignment is an expression statement of the form `place = value;`, where `place`
is a mutable binding, a field path rooted in a mutable binding, or an index
expression rooted in a mutable binding. The value is evaluated first, then stored.
The assignment expression itself has type `()`.

Assignment also moves. For a move-only type the right-hand side is transferred
into the place and the place's previous value is released immediately, running
its `Drop` hook; the right-hand-side binding is unusable afterwards. Compound
assignment reads and writes the place in one step, so `text += "..."` does not
move `text`.

Compound assignment combines a binary operator with assignment:

| Operator | Equivalent to |
| --- | --- |
| `+=` | `place = place + value` |
| `-=` | `place = place - value` |
| `*=` | `place = place * value` |
| `/=` | `place = place / value` (integer division truncates; division by zero panics) |
| `%=` | `place = place % value` |
| `&=` `|=` `^=` | Bitwise assignment on integer places. |
| `<<=` `>>=` | Shift assignment on integer places. |

The compound form evaluates the place once, and the binary operation follows
the same typing, overflow and zero-division rules as the plain operator (see
[Operators](07-operators.md)).

```ls
fn main() {
    let mut n = 10;
    n += 5;
    n -= 3;
    n *= 2;
    n /= 4;
    n %= 5;
    println!("{}", n);

    let mut bits = 0b1100;
    bits &= 0b1010;
    bits |= 0b0001;
    bits ^= 0b0010;
    bits <<= 2;
    bits >>= 1;
    println!("{}", bits);

    let mut text = String::from("ab");
    text += "cd";
    println!("{}", text);

    let mut numbers = vec![1, 2, 3];
    numbers[1] += 10;
    println!("{:?}", numbers);

    let mut point = Point { x: 1, y: 2 };
    point.x += 5;
    println!("{} {}", point.x, point.y);
}

struct Point {
    x: i64,
    y: i64,
}
```

Output:
```text
1
22
abcd
[1, 12, 3]
6 2
```

Overwriting a place releases the old value first:

```ls
struct Tracked {
    name: String,
}

impl Drop for Tracked {
    fn drop(&mut self) {
        println!("drop {}", self.name);
    }
}

fn main() {
    let mut slot = Tracked { name: String::from("first") };
    slot = Tracked { name: String::from("second") };
    println!("after replace");
}
```

Output:

```text
drop first
after replace
drop second
```

Assignment to a function call or another non-place expression is rejected. The
following program does not parse, because the left-hand side of `=` must be a
place:

```ls,compile-fail
fn answer() -> i64 {
    42
}

fn main() {
    answer() = 1;
}
```

Error:
```text
error[E0596]: cannot assign to this expression: the left-hand side is not a place expression
  --> example-12.ls:6:5
   |
 6 |     answer() = 1;
   |     ^^^^^^^^

lsaca: example-12.ls: could not compile example-12.ls due to previous error(s)
```

## Destructuring bindings

A `let` pattern can destructure a tuple or a struct into several bindings at
once. The pattern must be irrefutable: tuples of the right shape and struct
patterns without `Some`/`Ok`-style alternatives always are. `mut` may be applied
to individual bindings inside the pattern.

Destructuring a move-only value moves its parts into the new bindings (`Copy`
parts are copied). A struct or tuple is destructured as a whole, so a `String`
field moves out cleanly; moving a field out of a value *without* destructuring it
is still rejected (`E0507`).

```ls
struct Point {
    x: i64,
    y: i64,
}

fn main() {
    let pair = (1, String::from("one"));
    let (number, name) = pair;
    println!("{} {}", number, name);

    let origin = Point { x: 3, y: 4 };
    let Point { x, y } = origin;
    println!("{} {}", x, y);

    let (mut total, label) = (0, "sum");
    total += x + y;
    println!("{} = {}", label, total);
}
```

Output:
```text
1 one
3 4
sum = 7
```

## Constants

A `const` item declares a compile-time constant. Unlike `let`, a constant is an
item: it may be declared at the top level or inside a block, it can be used
anywhere its scope reaches (including before its textual position, because
items are hoisted), and it requires an explicit type annotation.

```rust
const NAME: Type = constant_expression;
```

The type must be a scalar type or `String`. Structs, enums, `Vec`, `Option`,
`Result`, `Rc` and `Box` cannot be constant types in 1.4.0; a structurally
constant `Point` value is rejected even if all its fields are literals. The
initializer must be a constant expression:

* literals of the supported types: integers, floats, booleans and string
  literals (`"text"` has type `String` and is accepted);
* unary `-` and `!` on constant expressions;
* arithmetic `+ - * / %`, bitwise `& | ^` and shifts `<< >>` on integers;
* references to previously declared constants;
* parenthesized constant expressions.

Comparison operators and function calls are **not** constant expressions, and
neither is `String::from`.

```ls
const WIDTH: i64 = 80;
const HEIGHT: i64 = WIDTH / 2 + 5;
const FLAG: bool = !false;
const MASK: u64 = (1 << 8) | 0x0F;
const GREETING: String = "hello, constants";

fn main() {
    println!("{} {} {}", WIDTH, HEIGHT, FLAG);
    println!("{}", MASK);
    println!("{}", GREETING);
}
```

Output:
```text
80 45 true
271
hello, constants
```

A missing type annotation is a parse error, because the grammar requires it:

```ls,compile-fail
const LIMIT = 10;

fn main() {
    println!("{}", LIMIT);
}
```

Error:
```text
error: expected ':' after constant name, found =
  --> example-15.ls:1:13
   |
 1 | const LIMIT = 10;
   |             ^

error: expected type name, found =
  --> example-15.ls:1:13
   |
 1 | const LIMIT = 10;
   |             ^

error: expected '=' in constant declaration, found integer literal 10
  --> example-15.ls:1:15
   |
 1 | const LIMIT = 10;
   |               ^^

lsaca: example-15.ls: could not compile example-15.ls due to previous error(s)
```

Comparison is not constant-foldable in 1.4.0:

```ls,compile-fail
const SMALL: bool = 1 < 2;

fn main() {
    println!("{}", SMALL);
}
```

Error:
```text
error[E0015]: constant initializer must be a constant expression
  --> example-16.ls:1:21
   |
 1 | const SMALL: bool = 1 < 2;
   |                     ^^^^^

lsaca: example-16.ls: could not compile example-16.ls due to previous error(s)
```

Constants of a non-scalar type are rejected as well:

```ls,compile-fail
struct Point {
    x: i64,
    y: i64,
}

const ORIGIN: Point = Point { x: 0, y: 0 };

fn main() {
    println!("{}", ORIGIN.x);
}
```

Error:
```text
error[E0001]: constants of type Point are not supported; constant types must be scalars or String
  --> example-17.ls:6:15
   |
 6 | const ORIGIN: Point = Point { x: 0, y: 0 };
   |               ^^^^^

error[E0609]: no field x on type ()
  --> example-17.ls:9:20
   |
 9 |     println!("{}", ORIGIN.x);
   |                    ^^^^^^^^

lsaca: example-17.ls: could not compile example-17.ls due to previous error(s)
```

Constants are substituted at compile time: they cost nothing at run time, they
cannot be mutated, and they do not create a storage location. A shadowing `let`
or a nested item may reuse the name. There are no global variables and no
`static` items: the only mutable state outside a function is inside an `Rc<T>`
cell passed around explicitly.

## Parameters and loop variables

Function parameters are immutable bindings owned by the function body. A
move-only argument is moved into the parameter, so the caller cannot use its
binding afterwards (see [Functions](09-functions.md#calls-and-argument-checking)).
Writing `mut` on a parameter parses but has no effect in 1.4.0: assigning to the
parameter is still rejected with `E0596` (see the example in
[Memory model](05-memory-model.md)). To modify a value, take the value and
return the new one, pass an `Rc` cell, or take a mutable reference to a builtin
heap type (`&mut String`, `&mut Vec<T>`, `&mut Rc<T>` or `&mut Box<T>`).

The variable bound by a `for` loop is a normal, immutable binding, and it may be
declared `mut`: `for mut i in 0..3` parses and permits reassigning `i` inside the
loop body. This is a lenient corner of the 1.4.0 parser rather than a design
feature; iterating with a separate mutable accumulator is clearer (see
[Statements and control flow](08-statements-and-control-flow.md)).

## What next

* The operators used in the examples: [Operators](07-operators.md).
* Blocks, `if` and loops: [Statements and
  control flow](08-statements-and-control-flow.md).
* Why mutation of shared values needs `Rc`: [Memory model](05-memory-model.md).
