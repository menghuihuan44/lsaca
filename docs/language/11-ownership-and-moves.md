# Ownership and moves

Version 1.4 adds ownership to lsaca. Every value has one owner, and when that owner is released the value is released too: the compiler runs the `Drop` hook if the type declares one, then releases the value's fields. Assigning a value, passing it to a function or returning it may either copy it or move it:

* Copy types (numbers, booleans, references, small aggregates) are copied; both the old and the new binding stay usable.
* Move-only types (String, Vec, Rc, Box, function values, types with a Drop impl) are moved; ownership transfers and the old binding can no longer be used.

When you want a second, independent value for a move-only type you must ask for it explicitly with `.clone()`. Borrowing with `&` and `&mut` never moves a value: the reference is a borrowed view and ownership stays with the original binding.

This page covers Copy versus move, the consuming contexts that trigger moves, move errors (E0382, E0507, E0505, E0509), the Drop trait and drop order, `.clone()`, and how all of this interacts with references and ORC. Reference syntax and the alias checker are covered in [References and borrowing](12-references-and-borrowing.md); the runtime representation of values (reference counting, copy-on-write, Rc and Box) is in [Memory model](05-memory-model.md). A complete runnable program that exercises a move, a `.clone()` and a `Drop` hook is [`examples/ownership.ls`](../../examples/ownership.ls) in the repository.

## Copy and move-only types

lsaca decides at compile time whether a type is Copy. The rule is structural: a type is Copy when every part of it is Copy and it does not implement `Drop`. Types that own heap data, own a closure environment, or implement Drop are move-only.

| Kind | Types |
| --- | --- |
| Copy | `i64` and the other integer types, `f64` and the other float types, `bool`, `char`, unit `()`; shared references `&T` and mutable references `&mut T`; tuples, `Option` and `Result` whose parts are all Copy; user structs and enums whose fields are all Copy and that do not implement `Drop` |
| Move-only | `String`, `Vec<T>`, `Rc<T>`, `Box<T>`; function values and closures; user structs and enums that contain a move-only type or implement `Drop` |

Copy is derived automatically. There is no `impl Copy for ...` syntax in version 1.6.0; writing one is rejected. Move-only is the default for owned data, so the compiler derives move-onlyness the same way: a struct with a `String` field is move-only, while a struct with only `i64` fields is Copy. There are no ownership annotations to write: the type structure decides.

### Copy values can be used repeatedly

```ls
struct Point {
    x: i64,
    y: i64,
}

fn main() {
    let point = Point { x: 1, y: 2 };
    let copy = point;
    println!("{} {} {} {}", point.x, copy.x, point.y, copy.y);
}
```

Output:

```text
1 1 2 2
```

Copying happens on assignment, as an argument and on return. The same holds for `Option`, `Result`, tuples and references:

```ls
fn main() {
    let maybe = Some(3);
    let copied = maybe;
    println!("{:?} {:?}", maybe, copied);

    let result: Result<i64, i64> = Ok(1);
    let result_copy = result;
    println!("{:?} {:?}", result, result_copy);
}
```

Output:

```text
Some(3) Some(3)
Ok(1) Ok(1)
```

### Move-only values transfer ownership

```ls
fn main() {
    let name = String::from("lsaca");
    let moved = name;
    println!("{}", moved);
}
```

Output:

```text
lsaca
```

After `let moved = name;` the binding `name` no longer owns a value. Using it again is the error E0382 (see [Use of a moved value](#use-of-a-moved-value-e0382)). The value is not duplicated: there is still exactly one `String`, now owned by `moved`.

A user type becomes move-only as soon as one field is move-only:

```ls
struct Record {
    name: String,
    count: i64,
}

fn main() {
    let record = Record { name: String::from("lsaca"), count: 3 };
    println!("{} {}", record.name, record.count);
}
```

Output:

```text
lsaca 3
```

## Moves

A move transfers ownership of a value from one place to another. The compiler inserts a move whenever a move-only value is used in a position that needs the value itself rather than a reference. The most common form is a `let` with a single identifier pattern:

```ls
fn consume(text: String) -> String {
    println!("{} consumed", text);
    text
}

fn main() {
    let text = String::from("lsaca");
    let returned = consume(text);
    println!("{}", returned);
}
```

Output:

```text
lsaca consumed
lsaca
```

The argument to `consume` moves the `String` into the function; the function's return value moves it back out. The caller's `text` binding is unusable after the call. The compiler reports the location of the move in the diagnostic note, which makes it easy to see which use consumed the value.

### Consuming contexts

| Context | Effect |
| --- | --- |
| `let moved = value;` (single identifier binding) | moves a move-only value |
| assignment with `=` | moves the right-hand side; the old left-hand value is released |
| argument of a by-value parameter, including by-value `self` | moves the argument |
| `return` expression and block tail | moves the result out |
| struct literal field, tuple literal, `vec!` element, `Some(...)` / `Ok(...)` / `Err(...)` payload | moves the payload |
| `Box::new` / `Rc::new` payload; `Box::set` / `Rc::set` payload | moves the payload |
| `Vec::push` (element and vector) and other self-consuming builtins such as `unwrap` and `expect` | moves the receiver |
| `for` loop over a `Vec<T>` | moves the vector |
| `match`, `if let` and `while let` scrutinee when it is move-only | moves the scrutinee |
| closure capture | captures by value (Copy values copy, move-only values move) |
| discarded expression statement | the value is released at the end of the statement |
| arguments of `println!`, `print!` and `format!`, operands of `assert!` and `assert_eq!`, operands of `==` and `!=`, `&expr`, reading a Copy value | the value is borrowed, not moved |

The last row is the reason a `String` can be printed or compared over and over without being moved: macros and comparison operators take their operands by reference. The following example uses `text` four times and still owns it afterwards:

```ls
fn main() {
    let text = String::from("hi");
    println!("{}", text);
    assert_eq!(text, String::from("hi"));
    println!("{}", text == String::from("hi"));
    println!("{}", text);
}
```

Output:

```text
hi
true
hi
```

Two special cases deserve their own note:

* `let _ = value;` does not move the value: the wildcard pattern `_` binds nothing, so `value` is still usable afterwards.
* A `match` on a move-only scrutinee consumes it even when the pattern is the wildcard `_`; after the match the original binding is gone.

## Errors when moving

### Use of a moved value (E0382)

```ls,compile-fail
fn main() {
    let name = String::from("lsaca");
    let moved = name;
    println!("{}", moved);
    println!("{}", name);
}
```

Error:

```text
error[E0382]: use of moved value: name
  --> example-07.ls:5:20
   |
 5 |     println!("{}", name);
   |                    ^^^^
   = note: example-07.ls:3:17: value moved here
   = note: name has type String, which is not Copy, so the value is moved whenever it is used by value; use .clone() to copy it first

lsaca: example-07.ls: could not compile example-07.ls due to previous error(s)
```

The note points at the place where the value was moved and suggests `.clone()` when an independent copy is what you want.

### Moving out of a place (E0507)

A move takes the whole value. You cannot move one field out of a value, one element out of a vector or the payload out of a dereference; those operations would leave the original place partially uninitialised, which version 1.5 does not support. Clone the field instead, or move the whole value and destructure it.

```ls,compile-fail
struct Config {
    name: String,
}

fn main() {
    let config = Config { name: String::from("lsaca") };
    let name = config.name;
    println!("{}", name);
}
```

Error:

```text
error[E0507]: cannot move out of a field of type String
  --> example-08.ls:7:16
   |
 7 |     let name = config.name;
   |                ^^^^^^^^^^^
   = note: use .clone() to copy the value before consuming it

lsaca: example-08.ls: could not compile example-08.ls due to previous error(s)
```

The same rule applies to indexed elements (`words[0]`) and to dereferenced values (`*reference`); each produces E0507 with a note suggesting `.clone()`. Reading a Copy field out of a move-only struct is fine, because that reads a copy rather than moving the field.

### Moving while borrowed (E0505)

A value cannot move while a borrow of it is still live:

```ls,compile-fail
fn main() {
    let text = String::from("lsaca");
    let borrowed = &text;
    let moved = text;
    println!("{} {}", borrowed, moved);
}
```

Error:

```text
error[E0505]: cannot move out of text because it is borrowed
  --> example-09.ls:4:17
   |
 4 |     let moved = text;
   |                 ^^^^
   = note: example-09.ls:3:20: shared borrow occurs here
   = note: a move would leave the reference pointing at a value the program no longer owns; clone the value instead of moving it

lsaca: example-09.ls: could not compile example-09.ls due to previous error(s)
```

A borrow ends at its last use, so moving the value after that point is allowed:

```ls
fn main() {
    let text = String::from("hi");
    let borrowed = &text;
    println!("{}", borrowed);
    let moved = text;
    println!("{}", moved);
}
```

Output:

```text
"hi"
hi
```

### Drop values in patterns (E0509)

A value whose type implements `Drop` owns resources that its hook must release. Moving such a value through a *destructuring* pattern would split the ownership of the hook, so destructuring bindings of drop types are rejected. Whole-value bindings are allowed: `let guard = value;` and a match arm that binds the whole value with a plain identifier (`match value { guard => ... }`) move the value and drop it exactly once. Destructuring with `match`/`if let`/`while let` must borrow instead.

```ls,compile-fail
struct Guard {
    name: String,
}

impl Drop for Guard {
    fn drop(&mut self) {
        println!("drop {}", self.name);
    }
}

fn main() {
    let pair = (Guard { name: String::from("first") }, 7);
    let (guard, count) = pair;
    println!("{} {}", guard.name, count);
}
```

Error:

```text
error[E0509]: cannot move out of a value of type Guard in a pattern because it implements Drop; use references instead
   --> example-11.ls:13:10
    |
 13 |     let (guard, count) = pair;
    |          ^^^^^
    = note: by-value bindings of types with drop glue are only allowed with a simple identifier binding; match or borrow through a reference instead

lsaca: example-11.ls: could not compile example-11.ls due to previous error(s)
```

## Moves in loops, matches and closures

### for loops

A `for` loop over a `Vec<T>` moves the vector; the loop owns it while it runs. When the elements themselves are move-only they are moved out one at a time and released at the end of each iteration:

```ls
struct Item {
    name: String,
}

impl Drop for Item {
    fn drop(&mut self) {
        println!("drop {}", self.name);
    }
}

fn main() {
    let items = vec![
        Item { name: String::from("a") },
        Item { name: String::from("b") },
    ];
    for item in items {
        println!("item {}", item.name);
    }
    println!("done");
}
```

Output:

```text
item a
drop a
item b
drop b
done
```

Iterating over a `Vec<i64>` also moves the vector, even though its elements are Copy. After the loop the original binding is gone; use a range or a fresh vector when the loop must not consume it. Version 1.4 has no `for` loop over a reference (`for x in &vec` is rejected).

### match, if let and while let

A `match`, `if let` or `while let` on a move-only value consumes the value. Patterns can then bind its parts by value:

```ls
struct Holder {
    name: String,
    count: i64,
}

fn main() {
    let holder = Holder { name: String::from("lsaca"), count: 3 };
    let Holder { name, count } = holder;
    println!("{} {}", name, count);
}
```

Output:

```text
lsaca 3
```

When the scrutinee must stay usable afterwards, match on a reference instead: `match &value { ... }` binds references to the parts and does not move the value. The same applies to `if let` and `while let`: if the body only needs a look at the content, borrow the scrutinee.

An arm may also bind the whole scrutinee with a plain identifier, including when its type has a `Drop` impl. The binding owns the value for the arm body, and the value is dropped once at the end of the match statement:

```ls
struct Loud {
    name: String,
}

impl Drop for Loud {
    fn drop(&mut self) {
        println!("drop {}", self.name);
    }
}

fn main() {
    let loud = Loud { name: String::from("x") };
    match loud {
        value => println!("matched {}", value.name),
    }
    println!("after");
}
```

Output:

```text
matched x
drop x
after
```

Guard expressions may read the bound value but must not consume it. A match guard cannot move a value out; the compiler reports E0507.

### Closures capture by move

Closures capture their environment by value: Copy variables are copied into the closure, move-only variables are moved into it. Using the original binding after the closure has been created is an E0382:

```ls
fn main() {
    let text = String::from("lsaca");
    let show = || println!("{}", text);
    show();
}
```

Output:

```text
lsaca
```

Use `.clone()` before creating the closure when both the closure and the caller need the value, or borrow it explicitly. A closure is move-only, but it can be cloned if its captured environment can be cloned (see [Cloning values](#cloning-values)).

## The Drop trait

### Declaring Drop

`Drop` is the only builtin trait with a special implementation form; since 1.5.0 a program can also declare and implement its own traits (see [Traits](18-traits.md)), but only `Drop` changes code generation. The implementation has exactly one hook, `fn drop(&mut self)`, which runs just before the value's fields are released. Inside the hook the value is still fully usable through `&mut self`.

```ls
struct Loud {
    name: String,
}

impl Drop for Loud {
    fn drop(&mut self) {
        println!("drop {}", self.name);
    }
}

fn main() {
    let loud = Loud { name: String::from("x") };
    println!("body");
}
```

Output:

```text
body
drop x
```

The hook in this example runs when `loud` goes out of scope at the end of `main`. Rules for the implementation:

* The trait must be `Drop` and the impl target must be a struct or enum type defined in the program.
* The impl contains exactly one function, named `drop`, with receiver `&mut self`, no other parameters and unit return.
* There is at most one Drop impl per type (a duplicate is error E0119).
* An inherent method named `drop` is rejected; the name is reserved for the trait.
* Types with a Drop impl can neither implement Copy nor Clone: both traits are derived automatically and a type with `Drop` is move-only. Writing `impl Copy` or `impl Clone` for any type is an error.

A value with a Drop impl also cannot be moved out of a pattern (E0509, above), and it cannot be cloned (see [Cloning values](#cloning-values)).

### Drop order and timing

Drop order is deterministic in version 1.6.0. When a value is released:

1. The type's own hook runs first (if it has one).
2. Then the value's fields are released in declaration order, each running its own hook in turn.

For a sequence of values held in one scope, the rules are:

* Local variables are released in reverse declaration order at the end of their scope.
* Function parameters are released in reverse parameter order when the function returns, before the caller continues.
* Temporaries (such as the result of a call used as a statement) are released at the end of the statement, unless they are returned or bound.
* Overwriting an existing binding with `=` releases the previous value immediately, before the assignment completes.
* Vector elements are released in index order; the payload of a Box or Rc is released when its last handle is released.
* A moved value is released exactly once, by whichever value ends up owning it; the moved-from binding does not release it again.

The following example shows the hook-then-fields order:

```ls
struct Loud {
    name: String,
}

impl Drop for Loud {
    fn drop(&mut self) {
        println!("drop {}", self.name);
    }
}

struct Pair {
    first: Loud,
    second: Loud,
}

impl Drop for Pair {
    fn drop(&mut self) {
        println!("drop pair");
    }
}

fn main() {
    let pair = Pair {
        first: Loud { name: String::from("first") },
        second: Loud { name: String::from("second") },
    };
    println!("body");
}
```

Output:

```text
body
drop pair
drop first
drop second
```

Locals and temporaries:

```ls
struct Loud {
    name: String,
}

impl Drop for Loud {
    fn drop(&mut self) {
        println!("drop {}", self.name);
    }
}

fn make() -> Loud {
    Loud { name: String::from("temp") }
}

fn main() {
    let first = Loud { name: String::from("first") };
    make();
    let second = Loud { name: String::from("second") };
    println!("body");
}
```

Output:

```text
drop temp
body
drop second
drop first
```

The temporary returned by `make` is discarded immediately and released at the end of that statement; the locals are then released in reverse declaration order at the end of `main`.

Overwriting a binding releases the old value at the assignment point:

```ls
struct Loud {
    name: String,
}

impl Drop for Loud {
    fn drop(&mut self) {
        println!("drop {}", self.name);
    }
}

fn main() {
    let mut loud = Loud { name: String::from("old") };
    println!("before");
    loud = Loud { name: String::from("new") };
    println!("after");
}
```

Output:

```text
before
drop old
after
drop new
```

Drop does not run for values that are still alive when the program terminates abnormally. A `panic!` abandons the stack without releasing pending values, and so does an explicit call that exits the process. Under normal termination, values are released exactly as described above, because returning from `main` drops its locals. There is no separate shutdown pass and no way to run a hook at exit.

### No explicit drop function

Version 1.4 has no function that releases a value on demand. `drop(value)` is an unknown function and `value.drop()` is an unknown method; the hook also cannot be called directly. A value is released when its owner goes out of scope, when it is overwritten, or when the aggregate or handle that contains it is released.

```ls,compile-fail
struct Loud {
    name: String,
}

impl Drop for Loud {
    fn drop(&mut self) {
        println!("drop {}", self.name);
    }
}

fn main() {
    let loud = Loud { name: String::from("x") };
    drop(loud);
}
```

Error:

```text
error[E0425]: cannot find function drop in this scope
   --> example-20.ls:13:5
    |
 13 |     drop(loud);
    |     ^^^^^^^^^^

lsaca: example-20.ls: could not compile example-20.ls due to previous error(s)
```

## Cloning values

Every clonable type has a `.clone()` method. Clone is derived automatically too: there is no `impl Clone` syntax, and a type is clonable exactly when it does not implement `Drop` and all of its parts are clonable. Clone takes no arguments and returns an independent value.

| Type | What `.clone()` does |
| --- | --- |
| `String` | copies the string; the copy owns its own buffer, so mutating one does not affect the other (copy-on-write makes the initial copy cheap) |
| `Vec<T>` | copies the vector and clones each element, producing a deep copy |
| `Rc<T>` | returns a new handle to the same cell and increments its strong count; the payload is shared, not copied |
| `Box<T>` | copies the box and clones its payload, producing a deep copy |
| user struct or enum | clones each field, producing a field-wise copy |
| function value or closure | returns a clone of the function value that shares the captured environment |
| `&T`, `&mut T` | strips the reference layers and clones the value they point at; cloning `&&String` produces a `String` |
| Copy value | returns a Copy of the value; both stay usable |

```ls
fn main() {
    let mut first = String::from("lsaca");
    let second = first.clone();
    first.push_str("!");
    println!("{} {}", first, second);

    let mut words = vec![String::from("alpha")];
    let copy = words.clone();
    words.push(String::from("beta"));
    println!("{:?} {:?}", words, copy);
}
```

Output:

```text
lsaca! lsaca
["alpha", "beta"] ["alpha"]
```

A user struct with a String and a Vec clones field by field:

```ls
struct Record {
    name: String,
    values: Vec<i64>,
}

fn main() {
    let first = Record { name: String::from("first"), values: vec![1, 2] };
    let second = first.clone();
    println!("{} {:?} | {} {:?}", first.name, first.values, second.name, second.values);
}
```

Output:

```text
first [1, 2] | first [1, 2]
```

An `Rc` clone shares the cell, and `Box` clone copies the payload:

```ls
fn main() {
    let mut first = Rc::new(String::from("hi"));
    let second = first.clone();
    first.set(String::from("changed"));
    println!("{} {}", first.get(), second.get());
    println!("{}", first.strong_count());

    let mut boxed = Box::new(String::from("old"));
    let boxed_copy = boxed.clone();
    boxed.set(String::from("new"));
    println!("{} {}", boxed.get(), boxed_copy.get());
}
```

Output:

```text
changed changed
2
new old
```

Cloning a reference strips it; this is the usual way to obtain an owned value from a borrowed one:

```ls
fn main() {
    let text = String::from("hi");
    let borrowed = &text;
    let double = &borrowed;
    let owned: String = double.clone();
    println!("{}", owned);
}
```

Output:

```text
hi
```

A closure clones its function value:

```ls
fn main() {
    let text = String::from("hi");
    let show = || println!("{}", text);
    let copy = show.clone();
    show();
    copy();
}
```

Output:

```text
hi
hi
```

Types with a Drop impl cannot be cloned. The compiler rejects `.clone()` on them because duplicating the value would duplicate its hook, and the resulting two hooks would release the same resources:

```ls,compile-fail
struct Guard {
    name: String,
}

impl Drop for Guard {
    fn drop(&mut self) {
        println!("drop {}", self.name);
    }
}

fn main() {
    let guard = Guard { name: String::from("x") };
    let copy = guard.clone();
    println!("{}", copy.name);
}
```

Error:

```text
error[E0277]: type Guard implements Drop and cannot be cloned; use references instead
   --> example-26.ls:13:22
    |
 13 |     let copy = guard.clone();
    |                      ^^^^^

lsaca: example-26.ls: could not compile example-26.ls due to previous error(s)
```

The same rejection applies to containers whose elements have drop glue: `Vec<Guard>`, `Option<Guard>`, `(Guard, i64)` and `Box<Guard>` all implement Drop through their elements and cannot be cloned.

## References, borrowing and ORC

Borrowing never moves a value. A `&` or `&mut` binding is a view of the original value, and the original binding remains the owner:

```ls
fn main() {
    let text = String::from("hi");
    let borrowed = &text;
    println!("{}", text);
    println!("{}", borrowed);
}
```

Output:

```text
hi
"hi"
```

A value can be moved once every borrow of it has ended. The alias rules that decide when a borrow is live are described in [References and borrowing](12-references-and-borrowing.md).

Version 1.4 also allows mutable references to the built-in heap containers. A `&mut String`, `&mut Vec<T>`, `&mut Rc<T>` or `&mut Box<T>` can be used to mutate the value behind the reference:

```ls
fn append(text: &mut String) {
    text.push_str("!");
}

fn grow(values: &mut Vec<i64>) {
    values.push(4);
}

fn main() {
    let mut text = String::from("hi");
    append(&mut text);
    println!("{}", text);

    let mut values = vec![1, 2, 3];
    grow(&mut values);
    println!("{:?}", values);
}
```

Output:

```text
hi!
[1, 2, 3, 4]
```

Mutable references to a user struct or enum are supported only when the type is Copy. A type with a heap field (a `String`, `Vec` or `Rc` field, for example) or with a Drop impl cannot be borrowed mutably, because mutating such a value through a reference needs code generation support that version 1.5 does not have yet. Passing such a value to a function still works by value (moving it) or by shared reference (`&T`).

`Rc` and `Box` are handles: moving a handle does not change the reference count, and cloning an `Rc` creates a second handle to the same cell. `get()` on a Box or Rc returns an owned copy of the payload, and `set()` replaces the payload, releasing the previous one immediately. Both are ordinary builtins described in [Memory model](05-memory-model.md) and [Standard library](15-standard-library.md).

## Differences from Rust

* Copy and Clone are derived automatically. Writing `impl Copy for T` or `impl Clone for T` is an error, and there is no derive attribute.
* Ordinary value moves need no keyword or annotation; they happen implicitly in the consuming contexts listed above. Closures accept the Rust-style `move` prefix, but it changes nothing, because closures always capture by value in lsaca (see [Closures](14-closures.md)).
* Types with a Drop impl are never Copy and cannot be cloned; Rust allows `#[derive(Clone)]` on such types and supports partial moves out of them.
* There is no `std::mem::drop` and no explicit drop call; the hook cannot be called directly.
* Drop hooks do not run on panic or on process exit; Rust unwinds and runs destructors during unwinding.
* Function values are move-only, unlike Rust function pointers; since 1.5.0 they can be stored in struct fields, enum variants, tuples, `Vec`, `Box` and as HashMap values.
* References are Copy, including `&mut` references; the alias checker still enforces borrowing rules.
* User-defined generics and traits exist since 1.5.0, but `Copy`, `Clone` and `Drop` remain the only compiler-known traits; there are no trait objects and no dynamic dispatch (see [Roadmap and deviations from Rust](16-roadmap.md)).
* Since 1.5.0 user structs, enums, tuples, `Vec`, `Option`, `Result`, references and `HashMap` compare with `==`; function values do not (`E0369`).

## Known limitations in version 1.6.0

* `Copy` and `Clone` cannot be implemented (both are derived automatically); `Drop` keeps its special form, and user-defined traits can be implemented since 1.5.0.
* There is no explicit lifetime syntax and references cannot be stored in struct or enum fields.
* A value with a Drop impl cannot be cloned, and a container whose elements have drop glue (for example `Vec<Guard>`) cannot be cloned either.
* Moving out of a field, an indexed element or a dereference is rejected (E0507); clone the part or move the whole value and destructure it.
* Partial moves are not supported: a value is either moved as a whole or not at all.
* Mutable references are supported for Copy values and for `String`, `Vec<T>`, `Rc<T>` and `Box<T>`; mutable references to a user struct or enum with heap fields are rejected (E0498).
* `for` loops consume a vector and there is no iteration over `&Vec`.
* Drop hooks do not run on panic or exit.
* `if let Some(value) = maybe` where the payload type has drop glue is rejected (E0509); match a reference (`if let Some(value) = &maybe`) instead.
* Generics are monomorphized and checked per instantiation, so an unused generic body is never checked; there are no trait objects or dynamic dispatch (see [Generics](17-generics.md) and [Traits](18-traits.md)).
* Function and closure values cannot be compared with `==`.

## Summary

* A value is copied when its type is Copy, and moved otherwise. Copy is derived from the type structure; there is nothing to annotate.
* String, Vec, Rc, Box, function values and types with a Drop impl are move-only.
* Using a move-only value by value transfers ownership; the old binding cannot be used afterwards (E0382). Fields, indexed elements and dereferences cannot be moved out (E0507), and a borrowed value cannot be moved (E0505).
* `.clone()` produces an independent value for clonable types; Rc clones share the cell, String and Vec clones are independent.
* `impl Drop for T` declares a hook that runs when the value is released: hook first, then fields; locals in reverse declaration order; parameters in reverse order; temporaries at the end of their statement; overwritten values immediately.
* Drop hooks do not run on panic or exit, and there is no explicit drop function.
* Borrowing is how you observe a value without moving it; mutable references to String, Vec, Rc and Box are legal since version 1.4.

## What next

* Learn the reference and alias rules: [References and borrowing](12-references-and-borrowing.md).
* See how values are represented at runtime: [Memory model](05-memory-model.md).
* Learn about Rc, Box and the standard library: [Standard library](15-standard-library.md).
* User-defined generics and traits: [Generics](17-generics.md) and [Traits](18-traits.md).
* What is planned next: [Roadmap and deviations from Rust](16-roadmap.md).
