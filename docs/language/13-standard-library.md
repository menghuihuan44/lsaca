# Standard library

lsaca has no imports and no module system: everything in this page is a builtin
that is always in scope. Builtin generic types are `Vec<T>`, `Option<T>`,
`Result<T, E>`, `Rc<T>` and `Box<T>`; `String` and the scalar types are
non-generic. The `fn main()` program can use all of them without declarations.

The page is organised by builtin: formatting first, because every example in the
documentation uses it, then `String`, `Vec`, `Option`, `Result`, `Rc`, `Box`, the
free functions and finally the panic and exit-code rules.

## Formatting and printing

### The formatting macros

| Macro | Output | Trailing newline |
| --- | --- | --- |
| `print!(fmt, args…)` | stdout | no |
| `println!(fmt, args…)` | stdout | yes |
| `eprint!(fmt, args…)` | stderr | no |
| `eprintln!(fmt, args…)` | stderr | yes |
| `format!(fmt, args…)` | returns a `String` | — |
| `panic!(fmt, args…)` | aborts with a panic message | — |

A format string is a `String` literal containing replacement fields. `{}` prints
the argument with `Display`; `{:?}` prints it with `Debug`. `{` and `}` are
escaped by doubling them.

```ls
fn main() {
    println!("hello, world");
    eprintln!("this goes to stderr");
    print!("no newline");
    print!(" then more\n");
    let line = format!("{} + {} = {}", 1, 2, 1 + 2);
    println!("{}", line);
    println!("a literal {{brace}} and a value {}", 5);
}
```

Output:
```text
hello, world
no newline then more
1 + 2 = 3
a literal {brace} and a value 5
```

The number of replacement fields must equal the number of arguments, and the
arguments must line up with the field specifiers.

### Format specifier grammar

A replacement field is:

```text
{ [[fill] align] [sign] ['0'] [width] ['.' precision] ['?'] }
```

| Component | Values | Applies to | Meaning |
| --- | --- | --- | --- |
| fill | one character | any | padding character, default space |
| align | `<` `>` `^` | any | left, right, center |
| sign | `+` | signed integers, floats | always print a sign |
| `0` | flag | numeric types only | zero padding, needs a width |
| width | integer ≤ 10000 | any | minimum width, counted in characters |
| precision | `.N`, N ≤ 10000 | floats and `String` only | digits after the point / chars kept |
| `?` | flag | any | use Debug formatting inside the field |

Defaults: strings and booleans align left, numbers align right; padding uses
spaces; floats print with the shortest representation that round-trips, and
precision rounds like C's `printf` (round-half-even on exact ties).

```ls
fn main() {
    println!("[{:>8}]", "lsaca");
    println!("[{:<8}]", "lsaca");
    println!("[{:^8}]", "lsaca");
    println!("[{:*>8}]", "lsaca");

    println!("[{:5}]", "ab");
    println!("[{:5}]", 42);
    println!("[{:?}]", "hi");
    println!("[{:>6?}]", "hi");
}
```

Output:
```text
[   lsaca]
[lsaca   ]
[ lsaca  ]
[***lsaca]
[ab   ]
[   42]
["hi"]
[  "hi"]
```

### Width, fill and alignment

For numbers the width is the minimum number of characters of the formatted
value, including a sign. Zero padding inserts zeros between the sign and the
digits when `0` is used with a width; when `0` is combined with an explicit fill
character, the fill character pads the whole formatted string including the sign.

```ls
fn main() {
    println!("[{:04}]", 7);
    println!("[{:04}]", -7);
    println!("[{:08.2}]", 3.14159);
    println!("[{:+}] [{:+}] [{:+}]", 42, -7, 0);
    println!("[{:+05}] [{:+05}]", 5, -5);
    println!("[{:0>8}]", -42);
    println!("[{:0<8}]", 42);
}
```

Output:
```text
[0007]
[-007]
[00003.14]
[+42] [-7] [+0]
[+0005] [-0005]
[00000-42]
[42000000]
```

Width counts **characters**, not bytes, so non-ASCII text lines up:

```ls
fn main() {
    println!("[{:^6}]", "你好");
    println!("[{:>8}]", "héllo");
}
```

Output:
```text
[  你好  ]
[   héllo]
```

### Precision

Precision has meaning only for floats and strings; using it on an integer,
boolean or other type is a compile error (`E0001`). For floats it is the number
of digits after the decimal point; for strings it is the maximum number of
characters kept, counted in characters.

```ls
fn main() {
    println!("[{:.2}]", 3.14159);
    println!("[{:.0}]", 2.5);
    println!("[{:.0}]", 3.5);
    println!("[{:.1}]", 2.25);
    println!("[{:.3}]", "lsaca");
    println!("[{:.2}]", "你好世界");
    println!("[{:8.3}]", 3.5);
    println!("[{:.2}]", 1.0 / 0.0);
}
```

Output:
```text
[3.14]
[2]
[4]
[2.2]
[lsa]
[你好]
[   3.500]
[inf]
```

### Debug formatting

`{:?}` prints strings with quotes and escapes, and is identical to `Display` for
scalars. For composites it prints the same structural form as `Display`, but
strings nested inside composites are always quoted:

```ls
fn main() {
    println!("{:?} {}", "hi", "hi");
    println!("{:?}", vec!["a", String::from("b")]);
    println!("{:?}", Some("x"));
    println!("{:?}", (String::from("q"), 1));
}
```

Output:
```text
"hi" hi
["a", "b"]
Some("x")
("q", 1)
```

Width applies before the value is quoted, so the padding surrounds the whole
quoted string. Precision and the `?` flag cannot be combined.

### Formatting errors

All formatting mistakes are compile-time errors (`E0001`): unsupported
specifiers, a wrong number of arguments, a precision on a non-float/string, a
sign on an unsigned integer, or a zero flag on a non-numeric type.

```ls,compile-fail
fn main() {
    println!("{:q}", 1);
}
```

Error:
```text
error[E0001]: unsupported format specifier '{:q}' in format string
  --> example-07.ls:2:14
   |
 2 |     println!("{:q}", 1);
   |              ^^^^^^

lsaca: example-07.ls: could not compile example-07.ls due to previous error(s)
```

```ls,compile-fail
fn main() {
    println!("{} {}", 1);
}
```

Error:
```text
error[E0001]: 2 format argument(s) required, but 1 were supplied
  --> example-08.ls:2:5
   |
 2 |     println!("{} {}", 1);
   |     ^^^^^^^^^^^^^^^^^^^^

lsaca: example-08.ls: could not compile example-08.ls due to previous error(s)
```

```ls,compile-fail
fn main() {
    println!("{:.2}", 5);
}
```

Error:
```text
error[E0001]: precision is only supported for floats and strings
  --> example-09.ls:2:23
   |
 2 |     println!("{:.2}", 5);
   |                       ^

lsaca: example-09.ls: could not compile example-09.ls due to previous error(s)
```

```ls,compile-fail
fn main() {
    println!("{:04}", "ab");
}
```

Error:
```text
error[E0001]: the '0' flag is only supported for numeric values
  --> example-10.ls:2:23
   |
 2 |     println!("{:04}", "ab");
   |                       ^^^^

lsaca: example-10.ls: could not compile example-10.ls due to previous error(s)
```

Named arguments (`{name}`) and positional indices (`{0}`) are not supported:
the arguments are consumed left to right in field order. There is no `{:#x}`
alternate formatting, no width/alignment from an argument, and no code to print
an integer in another base — write a helper function if you need one.

## Types at a glance

| Builtin | Created with | Treated by ORC as |
| --- | --- | --- |
| `String` | literal, `String::from` | refcounted handle, copy-on-write |
| `Vec<T>` | `vec![…]`, `Vec::new()` | refcounted handle, copy-on-write |
| `Option<T>` | `Some(v)`, `None` | handle if `T` is a handle |
| `Result<T, E>` | `Ok(v)`, `Err(e)` | handle if payload is a handle |
| `Rc<T>` | `Rc::new(v)` | refcounted shared cell |
| `Box<T>` | `Box::new(v)` | refcounted owned box, deep-copied on assignment |

## String

A `String` is an owned UTF-8 string. Literals have type `String` already, and
`String::from` accepts a string literal (or another `String`). String
concatenation uses `+` and `+=`; see [Operators](07-operators.md#strings-concatenation-and-comparison).

| Method | Signature (informal) | Description |
| --- | --- | --- |
| `len` | `→ i64` | length in **bytes**, not characters |
| `is_empty` | `→ bool` | true when the byte length is 0 |
| `push_str` | `(text) ` | appends text, mutating in place |
| `clear` | `()` | removes all contents |
| `contains` | `(needle) → bool` | substring test |
| `starts_with` | `(prefix) → bool` | prefix test |
| `ends_with` | `(suffix) → bool` | suffix test |
| `find` | `(needle) → Option<i64>` | byte index of the first occurrence |
| `replace` | `(from, to) → String` | replaces every occurrence, returns a new string |
| `to_uppercase` | `→ String` | ASCII uppercasing only |
| `to_lowercase` | `→ String` | ASCII lowercasing only |
| `trim` | `→ String` | strips leading/trailing ASCII whitespace |
| `split` | `(separator) → Vec<String>` | splits on every occurrence |
| `substring` | `(start, len) → String` | byte-based slice; panics when out of range |
| `repeat` | `(n) → String` | concatenates `n` copies |
| `parse_i64` | `→ Result<i64, String>` | decimal parse |
| `parse_f64` | `→ Result<f64, String>` | decimal float parse |

Because all lengths and offsets are byte-based, `len` of a non-ASCII string is
larger than its character count, and `find` returns a byte offset. Keep this in
mind when slicing:

```ls
fn main() {
    let text = String::from("héllo");
    println!("{} {}", text.len(), "hello".len());
    println!("{}", text.substring(1, 3));
    println!("{}", text.to_uppercase());
}
```

Output:
```text
6 5
él
HéLLO
```

A tour of the remaining methods:

```ls
fn main() {
    let mut text = String::from("  hello world  ");
    let trimmed = text.trim();
    println!("[{}]", trimmed);
    println!("{} {} {}", trimmed.contains("lo wo"), trimmed.starts_with("hello"), trimmed.ends_with("world"));
    println!("{:?}", trimmed.find("world"));
    println!("{}", trimmed.replace("world", "lsaca"));

    println!("{:?}", trimmed.split(" ").len());
    println!("{:?}", trimmed.split(" "));

    let mut built = String::from("ab");
    built.push_str("cd");
    println!("{} {}", built, built.is_empty());
    built.clear();
    println!("[{}] {}", built, built.is_empty());

    println!("{}", "ab".repeat(3));
    println!("{}", "[]".repeat(0));
}
```

Output:
```text
[hello world]
true true true
Some(6)
hello lsaca
2
["hello", "world"]
abcd false
[] true
ababab
```

Parsing returns a `Result` that carries a message describing the failure; the
message is a `String` and can be printed:

```ls
fn main() {
    println!("{:?}", "42".parse_i64());
    println!("{:?}", "-17".parse_i64());
    println!("{:?}", "4x2".parse_i64());
    println!("{:?}", " 42".parse_i64());
    println!("{:?}", "2.5".parse_f64());
    println!("{:?}", "nope".parse_f64());
}
```

Output:
```text
Ok(42)
Ok(-17)
Err("invalid integer")
Err("invalid integer")
Ok(2.5)
Err("invalid float")
```

Parsing is strict: no leading or trailing whitespace, no suffixes, no hex. A
successful parse returns `Ok(value)`; a failure returns `Err(message)` where the
message is `"invalid integer"` or `"invalid float"`.

## Vec

A `Vec<T>` is a growable array. Elements are stored contiguously and the vector
is a refcounted handle: copies share the buffer until one is modified
(copy-on-write). Two ways to create one:

```ls
fn main() {
    let explicit: Vec<i64> = Vec::new();
    let literal = vec![1, 2, 3];
    let repeated = vec![7; 4];
    let typed: Vec<String> = vec![];
    println!("{} {} {} {}", explicit.len(), literal.len(), repeated.len(), typed.len());
}
```

Output:
```text
0 3 4 0
```

`Vec::new()` needs type context: annotate the binding or use the turbofish
`Vec::<i64>::new()`. The `vec![…]` macro builds a vector from elements or from
`vec![value; count]`.

| Method | Description |
| --- | --- |
| `len` / `is_empty` | number of elements / emptiness |
| `push(v)` | appends a value |
| `pop()` | removes and returns the last element as `Option<T>` |
| `insert(i, v)` | inserts at index `i`, shifting later elements right |
| `remove(i)` | removes and returns the element at `i`, shifting later elements left |
| `clear()` | removes all elements |
| `first()` / `last()` | first/last element as `Option<T>` |
| `get(i)` | element at `i` as `Option<T>`, or `None` for an out-of-range index |
| `extend(other)` | appends all elements of another vector |
| `reverse()` | reverses in place |
| `v[i]` | element copy; **panics** when `i` is out of bounds |
| `v[i] = value` | replaces an element; panics when out of bounds |

`get` is the non-panicking accessor: it returns `None` instead of panicking.
Indexing returns a **copy** of the element, so mutating the result does not
change the vector; assign through the index to modify it.

```ls
fn main() {
    let mut numbers: Vec<i64> = Vec::new();
    numbers.push(10);
    numbers.push(20);
    numbers.push(30);
    numbers.insert(1, 15);
    println!("{:?}", numbers);
    println!("{:?} {:?}", numbers.first(), numbers.last());
    println!("{:?} {:?}", numbers.get(2), numbers.get(99));

    let popped = numbers.pop();
    println!("{:?} {:?}", popped, numbers);
    println!("{}", numbers.remove(0));
    println!("{:?}", numbers);

    numbers.reverse();
    numbers.extend(vec![100, 200]);
    println!("{:?}", numbers);

    numbers[0] = 99;
    println!("{}", numbers[0]);
    println!("{} {}", numbers.len(), numbers.is_empty());
    numbers.clear();
    println!("{} {:?}", numbers.len(), numbers);
}
```

Output:
```text
[10, 15, 20, 30]
Some(10) Some(30)
Some(20) None
Some(30) [10, 15, 20]
10
[15, 20]
[20, 15, 100, 200]
99
4 false
0 []
```

Because indexing is checked, an out-of-range index panics at runtime with the
length and the index in the message:

```ls,panic
fn main() {
    let values = vec![1, 2, 3];
    println!("{}", values[5]);
}
```

Panic:
```text
thread 'main' panicked at example-16.ls:3:20:
index out of bounds: the len is 3 but the index is 5
```

`get` is the safe alternative when the index comes from data; write through `v[i] = value` when you know the index is in range. A
vector whose element type is itself a handle (`Vec<Vec<i64>>`, `Vec<String>`,
`Vec<Rc<T>>`, `Vec<Box<T>>`) works; nested `Vec` display is structural.

### Known limitation: Vec::with_capacity

`Vec::with_capacity(n)` exists in the source and parses, but the generated C in
1.1.0 is invalid, so it cannot be used. Use `Vec::new()` and let the vector grow.

## Option

`Option<T>` is `Some(value)` or `None`. It is the return type used by `find`,
`get`, `first`, `last`, `pop`, `read_line` and several other builtins.

| Method | Description |
| --- | --- |
| `is_some()` / `is_none()` | tests for the variant |
| `unwrap()` | returns the value, panics on `None` |
| `unwrap_or(default)` | returns the value or the default |
| `expect(message)` | returns the value, panics with the message on `None` |

```ls
fn main() {
    let some: Option<i64> = Some(42);
    let none: Option<i64> = None;
    println!("{} {} {} {}", some.is_some(), some.is_none(), none.is_some(), none.is_none());
    println!("{} {}", some.unwrap(), none.unwrap_or(0));
    println!("{}", none.unwrap_or(7));
    println!("{}", some.expect("must be there"));

    let values = vec![1, 2, 3];
    println!("{}", values.get(1).unwrap_or(-1));
    println!("{}", values.get(9).unwrap_or(-1));
}
```

Output:
```text
true false false true
42 0
7
42
2
-1
```

Printing an `Option` shows its structure: `Some(42)` or `None`. Access the
payload with `match`/`if let`/`unwrap`; there is no `==` for `Option`.

## Result

`Result<T, E>` is `Ok(value)` or `Err(error)`. It is used by `parse_i64`,
`parse_f64`, `read_file` and `write_file`.

| Method | Description |
| --- | --- |
| `is_ok()` / `is_err()` | tests for the variant |
| `unwrap()` | returns the value, panics on `Err` |
| `unwrap_or(default)` | returns the value or the default |
| `expect(message)` | returns the value, panics with the message on `Err` |
| `ok()` | converts to `Option<T>`, discarding the error |
| `err()` | converts to `Option<E>`, discarding the value |

```ls
fn main() {
    let good: Result<i64, String> = Ok(10);
    let bad: Result<i64, String> = Err(String::from("boom"));
    println!("{} {} {} {}", good.is_ok(), good.is_err(), bad.is_ok(), bad.is_err());
    println!("{} {}", good.unwrap(), bad.unwrap_or(0));
    println!("{}", good.expect("must be ok"));
    println!("{:?}", bad.ok());
    println!("{:?}", bad.err());
}
```

Output:
```text
true false false true
10 0
10
None
Some("boom")
```

Like `Option`, `Result` has no `==`; match on it. There is no `ok_or` and no
`and_then`/`map`/`?` operator in 1.1.0 — write `match` or `if let`.

## Rc

`Rc<T>` is a shared, reference-counted cell. All handle copies see the same
payload; `get` reads it and `set` writes it, without needing the handle to be
`mut`. Equality between `Rc` values is identity, and the cell may contain a
cycle through another `Rc` — see [Memory model](05-memory-model.md#rct-shared-mutable-cells)
for the full model and the cycle collector.

| Function | Description |
| --- | --- |
| `Rc::new(v)` | creates a cell |
| `rc.get()` | reads the payload (copy) |
| `rc.set(v)` | replaces the payload, visible through every handle |
| `rc.strong_count()` | number of handles to this cell |

```ls
fn main() {
    let a = Rc::new(5);
    let b = a;
    println!("{} {} {}", a.get(), b.get(), a.strong_count());
    b.set(9);
    println!("{} {} {}", a.get(), b.get(), a == b);

    let other = Rc::new(9);
    println!("{} {}", a == other, a.strong_count());
}
```

Output:
```text
5 5 2
9 9 true
false 3
```

`Rc` display shows the wrapper: `Rc(9)`.

## Box

`Box<T>` is an owned heap value with **value semantics**: every copy deep-copies
the payload, unlike `Rc`. It is the builtin type for recursive data structures
such as lists and trees. Equality is structural (compares payloads) and
formatting shows the payload directly.

| Function | Description |
| --- | --- |
| `Box::new(v)` | allocates an owning box |
| `boxed.get()` | reads the payload (copy) |
| `boxed.set(v)` | replaces the payload |

```ls
fn main() {
    let boxed = Box::new(20);
    let copy = Box::new(20);
    println!("{} {}", boxed.get(), boxed == copy);

    let mut changed = boxed;
    changed.set(30);
    println!("{} {}", boxed.get(), changed.get());

    let text = Box::new(String::from("hi"));
    println!("{} {:?}", text, text);
}
```

Output:
```text
20 true
20 30
hi "hi"
```

The classic recursive structure — `Option<Box<Node>>` — is a linked list:

```ls
struct Node {
    value: i64,
    next: Option<Box<Node>>,
}

fn sum(node: Option<Box<Node>>) -> i64 {
    match node {
        Some(boxed) => {
            let inner = boxed.get();
            inner.value + sum(inner.next)
        }
        None => 0,
    }
}

fn main() {
    let mut head: Option<Box<Node>> = None;
    for value in 1..=5 {
        head = Some(Box::new(Node { value, next: head }));
    }
    println!("{}", sum(head));
}
```

Output:
```text
15
```

Copying a `Box` deep-copies the payload, so a box stored in a struct field is
independent after a struct copy. Boxes compose with `Vec` and `Rc`:
`Vec<Box<String>>`, `Rc<Box<Node>>` and `Box<Vec<i64>>` all work.

## Free functions

These functions are always in scope.

### I/O and process

| Function | Signature | Description |
| --- | --- | --- |
| `read_line()` | `→ Option<String>` | one line from stdin, newline stripped; `None` at EOF |
| `args()` | `→ Vec<String>` | command-line arguments after the program name |
| `exit(code)` | `(i32)` | terminates immediately with the exit code |
| `read_file(path)` | `→ Result<String, String>` | reads a UTF-8 text file |
| `write_file(path, contents)` | `→ Result<(), String>` | writes a text file |

`read_line` returns lines without the trailing newline and `None` once the input
is exhausted, so a read loop is naturally `while let Some(line) = read_line()`:

```ls,stdin=alpha|beta|gamma
fn main() {
    let mut n = 0;
    while let Some(line) = read_line() {
        n += 1;
        println!("{}: {}", n, line);
    }
    println!("eof after {}", n);
}
```

Output:
```text
1: alpha
2: beta
3: gamma
eof after 3
```

`args` excludes the program name, so the first user argument is index 0. The
expected fence attribute passes two arguments `alpha` and `beta`:

```ls,args=alpha,beta
fn main() {
    let all = args();
    println!("{}", all.len());
    if all.len() > 0 {
        println!("{}", all[0]);
    }
    if all.len() > 1 {
        println!("{}", all[1]);
    }
}
```

Output:
```text
1
alpha
```

`exit` stops the process immediately with the given status; nothing after the
call runs and destructors/leak reports are skipped:

```ls,exit=3
fn main() {
    println!("before");
    exit(3);
    println!("never printed");
}
```

Output:
```text
before
```

File I/O reports failures through `Result` instead of panicking. `read_file`
reads a UTF-8 file into a `String`; `write_file` returns `Ok(())` on success:

```ls
fn main() {
    let written = write_file(String::from("note.txt"), String::from("hello file"));
    println!("{}", written.is_ok());

    let read = read_file(String::from("note.txt"));
    println!("{}", read.is_ok());
    if let Ok(text) = read {
        println!("{}", text);
    }

    let missing = read_file(String::from("does-not-exist.txt"));
    println!("{}", missing.is_err());
}
```

Output:
```text
true
true
hello file
true
```

### Math

| Function | Signature | Description |
| --- | --- | --- |
| `sqrt(x)` | `f64 → f64` | square root |
| `abs(x)` | integers or `f64` | absolute value |
| `min(a, b)` / `max(a, b)` | integers or `f64` | smaller/larger value |
| `pow(a, b)` | `(f64, f64) → f64` | `a` raised to `b` |
| `floor(x)` / `ceil(x)` / `round(x)` | `f64 → f64` | rounding |
| `sin(x)` `cos(x)` `tan(x)` | `f64 → f64` | trigonometry |
| `ln(x)` / `log10(x)` / `exp(x)` | `f64 → f64` | natural log, base-10 log, exponential |

`abs`, `min` and `max` accept integers as well as floats, but each call must be
consistent: mixing integer and float arguments is a type error. `pow` is
float-only — convert both arguments, e.g. `pow(2.0, 10.0)`.

```ls
fn main() {
    println!("{} {}", sqrt(2.0), sqrt(9.0));
    println!("{} {}", abs(-5), abs(-2.5));
    println!("{} {}", min(3, 7), max(2.5, 1.5));
    println!("{}", pow(2.0, 10.0));
    println!("{} {} {}", floor(3.7), ceil(3.2), round(2.5));
    println!("{} {} {}", sin(0.0), cos(0.0), tan(0.0));
    println!("{} {} {}", ln(1.0), log10(100.0), exp(0.0));
}
```

Output:
```text
1.4142135623730951 3
5 2.5
3 2.5
1024
3 4 3
0 1 0
0 2 1
```

A call with the wrong argument types is rejected by the type checker:

```ls,compile-fail
fn main() {
    println!("{}", pow(2, 10));
}
```

Error:
```text
error[E0308]: mismatched types
  --> example-27.ls:2:24
   |
 2 |     println!("{}", pow(2, 10));
   |                        ^
   = note: expected f64, found {integer}

error[E0308]: mismatched types
  --> example-27.ls:2:27
   |
 2 |     println!("{}", pow(2, 10));
   |                           ^^
   = note: expected f64, found {integer}

lsaca: example-27.ls: could not compile example-27.ls due to previous error(s)
```

## assert and vec macros

| Macro | Behavior |
| --- | --- |
| `assert!(condition)` | `condition` must be a `bool`; panics with `assertion failed` when it is false |
| `assert_eq!(left, right)` | panics when `left != right`; prints both operands |
| `vec![…]` / `vec![value; count]` | builds a `Vec` |

`assert_eq!` needs `==` support for the operand type, so it works on integers,
floats, booleans, `String` and `Box`, but not on `Vec`, `Option`, `Result`,
tuples, structs or enums.

The argument of `assert!` must be `bool`; `assert!(n)` for an integer `n` is a
type error (`E0308`). Write the comparison explicitly: `assert!(n > 0)`.

```ls
fn main() {
    assert!(1 + 1 == 2);
    assert_eq!(2 + 2, 4);
    assert_eq!(String::from("a"), String::from("a"));
    println!("assertions passed");
}
```

Output:
```text
assertions passed
```

## Panics

A panic aborts the program immediately. The runtime prints a two-part message to
stderr and exits with code `101`:

```text
thread 'main' panicked at <file>:<line>:<column>:
<message>
```

The location is the source location of the operation that panicked. Panics
unwind nothing and run no cleanup (the process exits; the operating system
reclaims memory).

```ls,panic
fn main() {
    let maybe: Option<i64> = None;
    println!("{}", maybe.unwrap());
}
```

Panic:
```text
thread 'main' panicked at example-29.ls:3:20:
called `Option::unwrap()` on a `None` value
```

```ls,panic
fn main() {
    panic!("explicit panic");
}
```

Panic:
```text
thread 'main' panicked at example-30.ls:2:5:
explicit panic
```

The panic conditions in the builtin library are:

| Operation | Message |
| --- | --- |
| `Option::unwrap` on `None` | `called Option::unwrap() on a None value` |
| `Option::expect` on `None` | the message passed to `expect` |
| `Result::unwrap` on `Err` | `called Result::unwrap() on an Err value` |
| `Result::expect` on `Err` | the message passed to `expect` |
| integer division by zero | `attempt to divide by zero` |
| integer remainder by zero | `attempt to calculate the remainder with a divisor of zero` |
| index out of bounds | `index out of bounds: the len is N but the index is M` |
| `substring` out of range | `index out of bounds` with start + len as the index |
| checked integer overflow (Debug) | `attempt to add with overflow` (also subtract/multiply) |
| shift overflow (Debug) | `attempt to shift left with overflow` |
| `assert!(false)` | `assertion failed` |
| `assert_eq!` mismatch | `assertion left == right failed` plus `left:`/`right:` lines |
| `panic!(message)` | the formatted message |

## Exit codes

A normal return from `main` exits with code `0`. `exit(code)` chooses an explicit
code. The compiler's own command-line exit codes are `0` success, `1` compile or
build error and `2` command-line usage error; panics inside the compiled program
produce `101`. See [Getting started](01-getting-started.md#exit-codes) for the
CLI contract.

Whatever the exit path — return from `main`, `exit` or a panic — the operating
system reclaims all memory. Cycles and leaked allocations only matter for the
optional leak report (`LSACA_DEBUG_RC`), described in
[Memory model](05-memory-model.md#leak-tracking).

## What next

* How the refcounting and copy-on-write behave:
  [Memory model](05-memory-model.md).
* Formatting inside user types: [Structs, enums and impl](10-structs-enums-and-impl.md#printing-and-equality).
* Every known gap of this library: [Roadmap](14-roadmap.md).
