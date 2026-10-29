# Standard library

lsaca has no imports and no module system: everything in this page is a builtin
that is always in scope. Builtin generic types are `Vec<T>`, `HashMap<K, V>`,
`Option<T>`, `Result<T, E>`, `Rc<T>` and `Box<T>`; `String` and the scalar types
are non-generic. The `fn main()` program can use all of them without
declarations. This page describes the library of version 1.5.0.

The page is organised by builtin: formatting first, because every example in the
documentation uses it, then `String`, `Vec`, iterators, `HashMap`, `Option`
(including its combinators), `Result` (including its combinators and the `?`
operator), `Rc`, `Box`, the free functions and finally the panic and exit-code
rules.

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
| `String` | literal, `String::from` | move-only refcounted buffer; `.clone()` copies the text |
| `Vec<T>` | `vec![…]`, `Vec::new()` | move-only refcounted buffer; `.clone()` copies the elements |
| `Option<T>` | `Some(v)`, `None` | handle if `T` is a handle |
| `Result<T, E>` | `Ok(v)`, `Err(e)` | handle if payload is a handle |
| `Rc<T>` | `Rc::new(v)` | refcounted shared cell |
| `Box<T>` | `Box::new(v)` | move-only owned box; `.clone()` deep-copies the value |
| `HashMap<K, V>` | `HashMap::new()` | move-only table that owns its keys and values |

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
is a move-only value type: assigning it or passing it by value moves it, and
`.clone()` produces an independent copy. Create one with `Vec::new()`, with the
`vec![…]` macro, or with `Vec::with_capacity(n)` when you already know roughly
how large the vector will grow:

```ls
fn main() {
    let explicit: Vec<i64> = Vec::new();
    let literal = vec![1, 2, 3];
    let repeated = vec![7; 4];
    let typed: Vec<String> = vec![];
    let mut reserved: Vec<i64> = Vec::with_capacity(8);
    reserved.push(5);
    println!("{} {} {} {} {}", explicit.len(), literal.len(), repeated.len(), typed.len(), reserved.len());
}
```

Output:
```text
0 3 4 0 1
```

`Vec::new()` needs type context: annotate the binding or use the turbofish
`Vec::<i64>::new()`. The `vec![…]` macro builds a vector from elements or from
`vec![value; count]`. `Vec::with_capacity(n)` reserves room for at least `n`
elements without changing the length; the language has no `capacity()` method,
so the reservation is only observable as avoided reallocations.

| Method | Description |
| --- | --- |
| `len` / `is_empty` | number of elements / emptiness |
| `Vec::with_capacity(n)` | creates a vector with room reserved for at least `n` elements |
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
    for extra in 0..2 {
        numbers.push(100 * (extra + 1));
    }
    println!("{:?}", numbers);

    let more = vec![7, 8];
    numbers.extend(more);
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
[20, 15, 100, 200, 7, 8]
99
6 false
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

## Iterators

Version 1.5 introduced lazy iterators. Asking a vector for an iterator with
`.iter()` consumes the vector (the iterator owns it), and adapters such as
`map`, `filter`, `take`, `skip` and `enumerate` build new iterators; terminal
operations such as `collect`, `sum`, `count`, `fold`, `any`, `all` and `find`
consume the iterator and produce a value.

| Item | Description |
| --- | --- |
| `.iter()` | consumes a `Vec<T>` and returns an iterator over its elements |
| `map(f)` | applies `f: Fn(T) -> U`, producing `U` values |
| `filter(p)` | keeps values for which `p: Fn(&T) -> bool` returns true |
| `take(n)` / `skip(n)` | first `n` values / all but the first `n` values |
| `enumerate()` | yields `(i64, T)` index/value pairs |
| `collect()` | collects into a `Vec<T>`; the target type must be known |
| `fold(init, f)` | folds with `f: Fn(U, T) -> U` |
| `count()` / `sum()` | number of values (i64) / sum of numbers |
| `any(p)` / `all(p)` | short-circuit boolean tests with `p: Fn(&T) -> bool` |
| `find(p)` | first value satisfying `p`, as `Option<T>` |

```ls
fn main() {
    let values = vec![1, 2, 3, 4, 5, 6];
    let even_squares: Vec<i64> = values.iter().filter(|value| *value % 2 == 0).map(|value| value * value).collect();
    println!("{:?}", even_squares);
}
```

Output:
```text
[4, 16, 36]
```

Full details, user-defined iterators and the exact rules are on
[Iterators](19-iterators.md).

## HashMap<K, V>

A `HashMap` is a builtin map from keys to values added in 1.5. Keys are limited
to `i64`, `u64`, `bool` and `String`; values may be any value type. A map is
move-only and owns its keys and values; `.clone()` produces an independent copy
when the key and value types are cloneable.

```ls
fn main() {
    let mut counts: HashMap<String, i64> = HashMap::new();
    counts.insert(String::from("a"), 1);
    counts.insert(String::from("b"), 2);
    counts.insert(String::from("a"), 3);
    println!("{:?}", counts.get(String::from("a")));
    println!("{} {}", counts.len(), counts.contains_key(String::from("b")));
}
```

Output:
```text
Some(3)
2 true
```

| Method | Description |
| --- | --- |
| `HashMap::new()` | creates an empty map; type arguments come from context or the turbofish |
| `insert(key, value)` | moves both in, returns the previous value |
| `get(key)` | returns `Option<V>` (the stored value is cloned) |
| `get_with(key, f)` | applies `f: Fn(&V) -> U` to the stored value, returns `Option<U>` |
| `contains_key(key)` | true when the key is present |
| `remove(key)` | removes the entry, returns `Option<V>` |
| `len()` / `is_empty()` | number of entries / emptiness |
| `clear()` | removes every entry |
| `keys()` / `values()` | clones the keys/values into a `Vec` |

Full details, ownership rules and limitations are on [HashMap](20-hashmap.md).

## Option

`Option<T>` is `Some(value)` or `None`. It is the return type used by `find`,
`get`, `first`, `last`, `pop`, `read_line` and several other builtins.

| Method | Description |
| --- | --- |
| `is_some()` / `is_none()` | tests for the variant |
| `unwrap()` | returns the value, panics on `None` |
| `unwrap_or(default)` | returns the value or the default |
| `expect(message)` | returns the value, panics with the message on `None` |
| `map(f)` | applies `f` to the value: `Some(f(v))`, or `None` |
| `map_or(default, f)` | applies `f` to the value, or returns `default` |
| `and_then(f)` | chains a function that itself returns an `Option` |
| `filter(pred)` | keeps `Some(v)` only when `pred(v)` is `true`, else `None` |
| `ok_or(err)` | converts to `Result<T, E>`: `Ok(v)` or `Err(err)` |

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

### Option combinators

The combinators transform an `Option` without unwrapping it by hand:

```ls
fn main() {
    let some: Option<i64> = Some(21);
    let none: Option<i64> = None;

    println!("{:?}", some.map(|n| n * 2));
    println!("{}", none.map(|n| n * 2).unwrap_or(-1));
    println!("{}", none.map_or(7, |n| n + 1));
    println!("{} {}", some.filter(|n| *n > 10).is_some(), some.filter(|n| *n > 100).is_none());

    match none.ok_or(String::from("missing")) {
        Ok(value) => println!("ok {}", value),
        Err(message) => println!("err {}", message),
    }

    println!("{:?}", some.and_then(|n| if n > 0 { Some(n / 3) } else { None }));
}
```

Output:
```text
Some(42)
-1
7
true true
err missing
Some(7)
```

`map` and `map_or` transform the payload, `and_then` chains a computation that
may fail again, `filter` turns `Some` into `None` when a predicate fails, and
`ok_or` lifts an `Option` into a `Result`. The combinators are ordinary
function calls, so their arguments are evaluated eagerly: the `default` of
`map_or` and the error of `ok_or` are computed even when the `Option` is
`Some`. A plain function item can be passed where a closure is expected, for
example `some.map(double)`.

Printing an `Option` shows its structure: `Some(42)` or `None`. Access the
payload with `match`/`if let`/`unwrap`; since 1.5.0 `==` compares `Option`
payloads structurally.
`unwrap`, `expect` and `unwrap_or` consume the `Option`, so clone it first if
the payload is move-only and the original is needed afterwards.

## Result

`Result<T, E>` is `Ok(value)` or `Err(error)`. It is used by `parse_i64`,
`parse_f64`, `read_file` and `write_file`. A `Result` holding a move-only
payload (`String`, `Vec`, `Rc`, `Box`) is move-only too; `.clone()` produces an
independent copy of both variants, so `good.clone().unwrap()` and
`bad.clone().unwrap_or(0)` consume the clone and leave the original usable.

| Method | Description |
| --- | --- |
| `is_ok()` / `is_err()` | tests for the variant |
| `unwrap()` | returns the value, panics on `Err` |
| `unwrap_or(default)` | returns the value or the default |
| `expect(message)` | returns the value, panics with the message on `Err` |
| `ok()` | converts to `Option<T>`, discarding the error |
| `err()` | converts to `Option<E>`, discarding the value |
| `map(f)` | applies `f` to the `Ok` value: `Ok(f(v))` or the original `Err` |
| `map_err(f)` | applies `f` to the `Err` value: `Err(f(e))` or the original `Ok` |
| `and_then(f)` | chains a function that itself returns a `Result` |

```ls
fn main() {
    let good: Result<i64, String> = Ok(10);
    let bad: Result<i64, String> = Err(String::from("boom"));
    println!("{} {} {} {}", good.is_ok(), good.is_err(), bad.is_ok(), bad.is_err());
    println!("{}", good.unwrap());

    let bad_default: Result<i64, String> = Err(String::from("boom"));
    println!("{}", bad_default.unwrap_or(0));

    let good_expect: Result<i64, String> = Ok(10);
    println!("{}", good_expect.expect("must be ok"));

    println!("{:?}", bad.clone().ok());
    println!("{:?}", bad.clone().err());
}
```

Output:
```text
true false false true
10
0
10
None
Some("boom")
```

### Result combinators

`map` transforms the success value, `map_err` transforms the error, and
`and_then` chains another fallible step:

```ls
fn double(n: i64) -> i64 {
    n * 2
}

fn main() {
    let good: Result<i64, String> = Ok(5);
    let bad: Result<i64, String> = Err(String::from("boom"));

    println!("{}", good.clone().map(double).unwrap());
    println!("{:?}", bad.clone().map_err(|e| e + "!"));
    println!("{:?}", good.clone().and_then(|n| if n > 0 { Ok(n + 1) } else { Err(String::from("negative")) }));
    println!("{:?}", bad.clone().map(double));
}
```

Output:
```text
10
Err("boom!")
Ok(6)
Err("boom")
```

`Result::map` leaves an `Err` untouched and `Result::map_err` leaves an `Ok`
untouched. Unlike `Option`, `Result` has no `map_or` method (Rust does not
have one either); match on the result, or use `map`/`map_err`/`and_then`.
Error values are not modified by `map`, so the error type of `good.map(double)`
stays `String`.

Like `Option`, `Result` compares structurally with `==` since 1.5.0; match on
it to handle the variants.

## Error propagation with `?`

The postfix `?` operator is the concise way to propagate failure. It applies to
an `Option<T>` or a `Result<T, E>`:

* on `Some(v)` or `Ok(v)` it evaluates to `v`;
* on `None` it returns `None` from the enclosing function;
* on `Err(e)` it returns `Err(e)` from the enclosing function.

The enclosing function must return the same kind of value: `?` on an `Option`
requires the function to return an `Option`, and `?` on a `Result` requires the
function to return a `Result`. `Result` error types must match exactly: `?`
does **not** convert error types (there is no `From` conversion), so
`Result<T, String>` cannot propagate an `Err(MyError)`.

```ls
fn half(n: i64) -> Option<i64> {
    if n % 2 == 0 {
        Some(n / 2)
    } else {
        None
    }
}

fn quarter(n: i64) -> Option<i64> {
    let h = half(n)?;
    Some(h / 2)
}

fn main() {
    println!("{:?}", quarter(12));
    println!("{:?}", quarter(10));
    println!("{:?}", quarter(7));
}
```

Output:
```text
Some(3)
Some(2)
None
```

`quarter` reads as "unwrap `half(n)` or return `None`". The operator is
postfix and binds tighter than any binary operator: `half(n)? + 1` is
`(half(n)?) + 1`. The operand is evaluated exactly once:

```ls
fn first_with_count(values: Vec<i64>, counter: Rc<i64>) -> Option<i64> {
    counter.set(counter.get() + 1);
    values.first()
}

fn pick(values: Vec<i64>, counter: Rc<i64>) -> Option<i64> {
    let value = first_with_count(values, counter)?;
    Some(value)
}

fn main() {
    let counter = Rc::new(0);
    let empty: Vec<i64> = Vec::new();
    println!("{:?}", pick(empty, counter.clone()));
    println!("{}", counter.get());
}
```

Output:
```text
None
1
```

Only one call is recorded even though the operand fails: `?` desugars to a
`match` on the operand value, not to a re-evaluation. Attach `?` to any
expression — a call, a method call, a variable, or a block-like expression such
as `if`/`else`, `match` or a plain block (1.5.0 accepts these; 1.4.0 did not).
Wrapping a block-like operand in parentheses usually reads better:
`let value = (if ok { Ok(1) } else { Err(String::from("e")) })?;`.

The same works for `Result`; an error early-return keeps its type:

```ls
fn parse_pair(a: String, b: String) -> Result<i64, String> {
    let x = a.parse_i64()?;
    let y = b.parse_i64()?;
    Ok(x + y)
}

fn main() {
    println!("{:?}", parse_pair(String::from("20"), String::from("22")));
    println!("{:?}", parse_pair(String::from("20"), String::from("x")));
}
```

Output:
```text
Ok(42)
Err("invalid integer")
```

A method may use `?` on its receiver data, as long as the method itself returns
an `Option` or `Result`:

```ls
struct Bag {
    values: Vec<i64>,
}

impl Bag {
    fn first_plus(&self) -> Option<i64> {
        let first = self.values.first()?;
        Some(first + 1)
    }
}

fn main() {
    let bag = Bag { values: vec![4, 5] };
    println!("{:?}", bag.first_plus());

    let empty = Bag { values: Vec::new() };
    println!("{:?}", empty.first_plus());
}
```

Output:
```text
Some(5)
None
```

The `?` operator cannot be used everywhere:

* **`main` returns `()`**, so `?` cannot appear in `main` itself; catch the
  error with `match`/`if let`, or move the fallible work into a helper function
  that returns `Option`/`Result`.
* **Closures** must declare (or otherwise fix) an `Option`/`Result` return
  type before `?` can be used inside; annotate the closure's return type or
  give it a `fn` type through a `let` binding.
* **Constant initializers** cannot use `?` because the operator performs an
  early return.
* The operand must be an `Option` or `Result`; other types are rejected.

These rules produce clear front-end errors:

```ls,compile-fail
fn main() {
    let value: Option<i64> = Some(1);
    let inner = value?;
    println!("{}", inner);
}
```

Error:
```text
error[E0277]: the ? operator can only be used in a function that returns Option, but this function returns ()
  --> example-27.ls:3:17
   |
 3 |     let inner = value?;
   |                 ^^^^^^
   = note: propagate the value with an explicit match, or change the function to return Option

lsaca: example-27.ls: could not compile example-27.ls due to previous error(s)
```

```ls,compile-fail
fn number() -> Option<i64> {
    let value: i64 = 42;
    let inner = value?;
    Some(inner)
}

fn main() {}
```

Error:
```text
error[E0277]: the ? operator can only be applied to Option or Result values, but found i64
  --> example-28.ls:3:17
   |
 3 |     let inner = value?;
   |                 ^^^^^^
   = note: the ? operator propagates None or Err out of the enclosing function

lsaca: example-28.ls: could not compile example-28.ls due to previous error(s)
```

```ls,compile-fail
fn convert() -> Result<i64, String> {
    let other: Result<i64, i64> = Err(1);
    let value = other?;
    Ok(value)
}

fn main() {}
```

Error:
```text
error[E0308]: the ? operator cannot convert error types: expected String, found i64
  --> example-29.ls:3:17
   |
 3 |     let value = other?;
   |                 ^^^^^^
   = note: the ? operator requires the error types to match exactly; use map_err or an explicit match to convert

lsaca: example-29.ls: could not compile example-29.ls due to previous error(s)
```

```ls,compile-fail
fn main() {
    let bad = |value: Option<i64>| {
        let inner = value?;
        Some(inner)
    };
    println!("{:?}", bad(Some(1)));
}
```

Error:
```text
error[E0277]: the ? operator requires the enclosing closure to declare an Option return type
  --> example-30.ls:3:21
   |
 3 |         let inner = value?;
   |                     ^^^^^^
   = note: add an explicit return type such as -> Option<i64> to this closure

lsaca: example-30.ls: could not compile example-30.ls due to previous error(s)
```

```ls,compile-fail
const VALUE: i64 = 1?;

fn main() {
    println!("{}", VALUE);
}
```

Error:
```text
error[E0015]: the ? operator is not allowed in a constant initializer
  --> example-31.ls:1:20
   |
 1 | const VALUE: i64 = 1?;
   |                    ^^

error[E0015]: constant initializer must be a constant expression
  --> example-31.ls:1:20
   |
 1 | const VALUE: i64 = 1?;
   |                    ^^

lsaca: example-31.ls: could not compile example-31.ls due to previous error(s)
```

A closure can use `?` when its return type is known, for example through a
`fn` type annotation on the binding:

```ls
fn main() {
    let parse: fn(Option<i64>) -> Option<i64> = |value| {
        let inner = value?;
        Some(inner + 1)
    };
    println!("{:?} {:?}", parse(Some(1)), parse(None));
}
```

Output:
```text
Some(2) None
```

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
    let b = a.clone();
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
false 2
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

    let mut changed = boxed.clone();
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
            inner.value + sum(inner.next.clone())
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

Cloning a `Box` deep-copies the payload, so a box stored in a struct field is
independent after the struct is cloned. Boxes compose with `Vec` and `Rc`:
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
2
alpha
beta
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
  --> example-41.ls:2:24
   |
 2 |     println!("{}", pow(2, 10));
   |                        ^
   = note: expected f64, found {integer}

error[E0308]: mismatched types
  --> example-41.ls:2:27
   |
 2 |     println!("{}", pow(2, 10));
   |                           ^^
   = note: expected f64, found {integer}

lsaca: example-41.ls: could not compile example-41.ls due to previous error(s)
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
thread 'main' panicked at example-43.ls:3:20:
called `Option::unwrap()` on a `None` value
```

```ls,panic
fn main() {
    panic!("explicit panic");
}
```

Panic:
```text
thread 'main' panicked at example-44.ls:2:5:
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
leak report, which Debug builds print by default and other profiles print when
the compiler is invoked with `LSACA_DEBUG_RC=1`; see
[Memory model](05-memory-model.md#leak-tracking).

## What next

* How ownership, moves, Drop and ORC behave: [Memory model](05-memory-model.md).
* Lazy iterators in depth: [Iterators](19-iterators.md).
* Hash maps in depth: [HashMap](20-hashmap.md).
* User-defined generic types: [Generics](17-generics.md).
* Formatting inside user types: [Structs, enums and impl](10-structs-enums-and-impl.md#printing-and-equality).
* Every known gap of this library: [Roadmap](16-roadmap.md).
