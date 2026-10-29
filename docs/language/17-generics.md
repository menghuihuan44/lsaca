# Generics

Version 1.5.0 adds user-defined generics. Structs, enums, free functions and
impl blocks can take type parameters, so one definition can be used with
several concrete types. Traits are covered on the [next page](18-traits.md); a trait can carry type
parameters (as `Iterator<T>` does), but a trait method cannot itself be
generic. There is no dynamic dispatch: every use of a generic item is instantiated with concrete types,
checked, and compiled separately (see [How generics are
compiled](#how-generics-are-compiled)).

## Generic structs

A struct declares its type parameters after its name:

```ls
struct Pair<T> {
    first: T,
    second: T,
}

fn main() {
    let numbers: Pair<i64> = Pair { first: 1, second: 2 };
    let words: Pair<String> = Pair {
        first: String::from("lsaca"),
        second: String::from("1.5"),
    };
    println!("{} {}", numbers.first, numbers.second);
    println!("{} {}", words.first, words.second);
}
```

Output:
```text
1 2
lsaca 1.5
```

Each use of `Pair` names a concrete type: `Pair<i64>` and `Pair<String>` are
different types with the same field layout. A field of a generic struct can
hold any value type, including move-only values such as `String` and `Vec<T>`.
The struct becomes move-only or Copy according to the same rules as any other
struct: `Pair<i64>` is Copy, `Pair<String>` is move-only.

## Generic functions

Type parameters on a function are inferred from the arguments or written with
the turbofish:

```ls
struct Pair<T> {
    first: T,
    second: T,
}

fn pair<T>(first: T, second: T) -> Pair<T> {
    Pair { first, second }
}

fn main() {
    let numbers: Pair<i64> = pair(1, 2);
    let words: Pair<String> = pair(String::from("a"), String::from("b"));
    println!("{} {}", numbers.first, numbers.second);
    println!("{} {}", words.first, words.second);
}
```

Output:
```text
1 2
a b
```

The return type `Pair<T>` reuses the function's type parameter. Arguments are
moved into the function as usual, so calling `pair` with a `String` moves it.

## Generic enums

Enums take type parameters the same way:

```ls
enum Slot<T> {
    Empty,
    Full(T),
}

fn main() {
    let slot: Slot<String> = Slot::Full(String::from("data"));
    match slot {
        Slot::Empty => println!("empty"),
        Slot::Full(value) => println!("{}", value),
    }
}
```

Output:
```text
data
```

A variant payload may refer to the enum's parameters, and matching works on
the instantiated enum exactly like on a non-generic one.

### Recursive generic enums

A generic enum can contain itself through `Box<T>`:

```ls
enum Tree<T> {
    Leaf,
    Node(Box<Tree<T>>, T, Box<Tree<T>>),
}

fn sum(tree: Tree<i64>) -> i64 {
    match tree {
        Tree::Leaf => 0,
        Tree::Node(left, value, right) => sum(left.get()) + value + sum(right.get()),
    }
}

fn main() {
    let tree: Tree<i64> = Tree::Node(
        Box::new(Tree::Leaf),
        2,
        Box::new(Tree::Node(
            Box::new(Tree::Leaf),
            3,
            Box::new(Tree::Leaf),
        )),
    );
    println!("{}", sum(tree));
}
```

Output:
```text
5
```

`Box::get()` returns an owned copy of the boxed value, which is why the
recursive call can consume it; see [Box](15-standard-library.md#box).

## Generic impl blocks

An impl block can be generic too, and `Self` refers to the instantiated type:

```ls
struct Pair<T> {
    first: T,
    second: T,
}

impl<T> Pair<T> {
    fn new(first: T, second: T) -> Self {
        Self { first, second }
    }

    fn first(&self) -> T {
        self.first
    }
}

fn main() {
    let pair = Pair::<i64>::new(3, 4);
    println!("{}", pair.first());
}
```

Output:
```text
3
```

`Self::new(...)` and `Self { ... }` are both available inside an impl block;
the associated function is called with the turbofish form `Pair::<i64>::new(3,
4)`.

The body of `first` reads a field out of a borrowed `self`, which is only
possible when `T` is Copy. That is fine for the instantiation above, and it is
an example of late checking: the same generic body is rejected for
`Pair<String>` only if that instantiation is actually used (see [How generics
are compiled](#how-generics-are-compiled)).

## Bounds

A type parameter may require trait bounds with `: Bound`. Trait names and the
builtin `Drop` bound are supported; the callable bounds `Fn(A) -> B` constrain
function and closure values:

```ls
fn apply_twice<F: Fn(i64) -> i64>(f: F, value: i64) -> i64 {
    f(f(value))
}

fn main() {
    let output = apply_twice(|x| x * 2, 3);
    println!("{}", output);
}
```

Output:
```text
12
```

Closure parameter types are inferred from the `Fn(i64) -> i64` bound, so
`|x| x * 2` needs no annotation.

A bound is checked when the generic item is instantiated; a value that does
not satisfy it is rejected with `E0277`. Types that implement a user-defined
trait can be used as bounds too — see [Traits](18-traits.md).

The turbofish names type arguments explicitly, which is useful when they
cannot be inferred from arguments or context:

```ls
struct Pair<T> {
    first: T,
    second: T,
}

fn pair<T>(first: T, second: T) -> Pair<T> {
    Pair { first, second }
}

fn main() {
    let numbers = pair::<i64>(1, 2);
    println!("{} {}", numbers.first, numbers.second);
}
```

Output:
```text
1 2
```

A generic item can also have unused type parameters; the definition below is
accepted even though `T` does not appear in a field. (Rust rejects this with
`E0392`; lsaca accepts it, so phantom parameters are allowed.)

```ls
struct Tagged<T> {
    value: i64,
}

fn main() {
    let tagged: Tagged<String> = Tagged { value: 7 };
    println!("{}", tagged.value);
}
```

Output:
```text
7
```

## How generics are compiled

Generics in lsaca are **monomorphized**: each concrete instantiation of a
generic function produces its own checked function and its own C function,
with a deterministic mangled name such as `Pair_i64` or `Tree_String`. There
is no runtime type information and no dynamic dispatch.

Instantiation is **late-checked**, in the style of C++ templates:

* The body of a generic function or method is not type-checked when the
  definition is parsed; it is checked once per concrete instantiation, the
  first time that instantiation is used.
* A generic body that is invalid for a particular instantiation reports an
  error at that instantiation. For example `Pair<String>::first()` fails
  because it would move a `String` out of a borrowed value, while
  `Pair<i64>::first()` is fine.
* A generic item that is never instantiated is never checked. An unused
  generic function whose body would be invalid is not reported.
* Because each instantiation is cached, ordinary recursion terminates the
  checker: `fn walk<T>(value: T, n: i64) -> i64` calling `walk(value, n - 1)`
  creates one instantiation, not infinitely many.
* A chain of *different* instantiations that nests more than 128 levels deep
  is rejected with `E0275` (the recursion limit for instantiation).

Methods of an instantiated generic type are looked up like methods of a
concrete type. The examples below show errors reported at the point of
instantiation.

An invalid generic body is only rejected when the instantiation is used:

```ls,compile-fail
struct Pair<T> {
    first: T,
    second: T,
}

impl<T> Pair<T> {
    fn first(&self) -> T {
        self.first
    }
}

fn main() {
    let pair: Pair<String> = Pair {
        first: String::from("a"),
        second: String::from("b"),
    };
    println!("{}", pair.first());
}
```

Error:
```text
error[E0507]: cannot move out of a field of type String
  --> example-09.ls:8:9
   |
 8 |         self.first
   |         ^^^^^^^^^^
   = note: use .clone() to copy the value before consuming it

lsaca: example-09.ls: could not compile example-09.ls due to previous error(s)
```

The wrong number of type arguments is reported when the item is named:

```ls,compile-fail
struct Pair<T> {
    first: T,
    second: T,
}

fn main() {
    let pair: Pair<i64, i64> = Pair { first: 1, second: 2 };
    println!("{}", pair.first);
}
```

Error:
```text
error[E0107]: struct Pair takes 1 type argument but 2 were supplied
  --> example-10.ls:7:15
   |
 7 |     let pair: Pair<i64, i64> = Pair { first: 1, second: 2 };
   |               ^^^^

error[E0282]: type annotations needed
  --> example-10.ls:7:32
   |
 7 |     let pair: Pair<i64, i64> = Pair { first: 1, second: 2 };
   |                                ^^^^

lsaca: example-10.ls: could not compile example-10.ls due to previous error(s)
```

A call whose type parameters cannot be inferred is an error:

```ls,compile-fail
fn label<T>() -> i64 {
    7
}

fn main() {
    let value = label();
    println!("{}", value);
}
```

Error:
```text
error[E0283]: type annotations needed for generic arguments
  --> example-11.ls:6:17
   |
 6 |     let value = label();
   |                 ^^^^^^^

lsaca: example-11.ls: could not compile example-11.ls due to previous error(s)
```

And a value that does not satisfy a bound is rejected when the generic
function is instantiated:

```ls,compile-fail
fn apply_twice<F: Fn(i64) -> i64>(f: F, value: i64) -> i64 {
    f(f(value))
}

fn main() {
    let not_callable = 5;
    println!("{}", apply_twice(not_callable, 1));
}
```

Error:
```text
error[E0277]: the trait bound (i64: fn(i64) -> i64) is not satisfied
  --> example-12.ls:1:19
   |
 1 | fn apply_twice<F: Fn(i64) -> i64>(f: F, value: i64) -> i64 {
   |                   ^^^^^^^^^^^^^^

error[E0618]: call expression requires a function, found i64
  --> example-12.ls:2:5
   |
 2 |     f(f(value))
   |     ^^^^^^^^^^^

error[E0618]: call expression requires a function, found i64
  --> example-12.ls:2:7
   |
 2 |     f(f(value))
   |       ^^^^^^^^

lsaca: example-12.ls: could not compile example-12.ls due to previous error(s)
```

## What generics do not include

* No `where` clauses; bounds are written inline in the parameter list.
* No associated types, no generic type aliases, no generic constants.
* No `dyn`, no trait objects, and no dynamic dispatch; every generic use is
  monomorphic.
* No const generics, no default type parameters, no specialization.
* A generic function's body only has to be valid for the instantiations that
  are actually used; invalid unused instantiations are not diagnosed.
* Generic items are limited to structs, enums, free functions, inherent impls
  and trait impls.

See [Roadmap and deviations from Rust](16-roadmap.md) for the full list.

## Summary

* Structs, enums, functions, inherent impls and trait impls can be generic with
  `<T>` parameter lists and `: Bound` bounds.
* `Self` and `Self::new(...)` / `Self { ... }` work inside generic impls.
* Types are inferred from arguments and context, or named with the turbofish
  form `pair::<i64>(...)`.
* Instantiation is monomorphized and late-checked: each concrete use is
  checked once, never-used instantiations are never checked, and an
  instantiation chain deeper than 128 is `E0275`.
* Unused type parameters are accepted, unlike Rust.
* There are no `where` clauses, associated types, const generics, defaults or
  trait objects.

## What next

* Traits and bounds on user types: [Traits](18-traits.md).
* Iterators are built on generic structs: [Iterators](19-iterators.md).
* The standard library: [Standard library](15-standard-library.md).
* Deviations and limitations: [Roadmap and deviations from Rust](16-roadmap.md).
