# Arrays

Version 2.2 adds fixed-size arrays. An array type is written `[T; N]`: `N`
values of type `T` stored inline, with the length fixed at compile time. The
length is part of the type, so `[i64; 3]` and `[i64; 4]` are different types.
Arrays are values like any other: they can be copied, compared, printed, passed
to functions and stored in aggregate types. A [slice](26-slices-and-str.md) is a
borrowed view of a sequence, a `Vec<T>` is a growable heap vector, and an array
is the fixed-size value with no allocation of its own.

## Array types and literals

The type is written `[T; N]` and a literal lists the elements between square
brackets. All elements must unify to the same type, and the number of elements
must match `N`:

```ls
fn main() {
    let mut xs: [i64; 4] = [1, 2, 3, 4];
    xs[0] = 9;
    println!("{} {} {}", xs.len(), xs.is_empty(), xs[2]);
    println!("{:?}", xs);
}
```

Output:

```text
4 false 3
[9, 2, 3, 4]
```

A repeated-element literal `[value; N]` builds an array of `N` copies of
`value`; a zero-length array `[T; 0]` is legal, and the empty literal `[]`
needs a type annotation (otherwise the element type cannot be inferred):

```ls
const WIDTH: usize = 3;

fn main() {
    let zeroes = [0i64; 5];
    let names = [String::from("a"), String::from("b")];
    let empty: [i64; 0] = [];
    let grid: [[i64; 2]; 2] = [[1, 2], [3, 4]];
    let fixed: [i64; WIDTH] = [7, 8, 9];
    println!("{:?} {:?}", zeroes, names);
    println!("{} {} {:?}", empty.len(), grid[1][0], fixed);
}
```

Output:

```text
[0, 0, 0, 0, 0] ["a", "b"]
0 3 [7, 8, 9]
```

The length expression in a type position must be an integer literal or an
integer `const` path. An arbitrary expression such as `1 + 2` is rejected
with `error[E0435]: the array length must be an integer literal or an integer constant`.
A literal whose length does not match the annotated type is a type error
(`error[E0308]: mismatched types`); an empty literal with no expected type is
reported as `error[E0282]: type annotations needed: cannot infer the element type of an empty array`.

## Indexing and length

Arrays are indexed with `array[index]`; the index type is `usize`. The expression
is a place, so it can be assigned to and borrowed mutably (`&mut array[index]`).
`.len()` and `.is_empty()` behave like their `Vec` and slice counterparts:

```ls
fn main() {
    let xs: [i64; 4] = [1, 2, 3, 4];
    let mut sum = 0i64;
    for index in 0..xs.len() {
        sum += xs[index];
    }
    println!("{}", sum);
}
```

Output:

```text
10
```

An index must be `usize`; passing an `i64` is a type error. Indexing is checked
at run time, and an out-of-bounds index panics with the length and the index:

```ls,panic
fn main() {
    let xs: [i64; 3] = [10, 20, 30];
    let index = 3;
    println!("{}", xs[index]);
}
```

Panic:

```text
thread 'main' panicked at example-04.ls:4:20:
index out of bounds: the len is 3 but the index is 3
```

## Copy and move

Arrays follow the same copy rule as the types they contain: an array is `Copy`
when its element type is `Copy`, so assigning one array to another copies all
elements. Arrays of non-`Copy` elements such as `String` move instead:

```ls
fn main() {
    let xs = [1i64, 2, 3];
    let copy = xs;
    let mut changed = copy;
    changed[0] = 9;
    println!("{:?} {:?}", xs, changed);
}
```

Output:

```text
[1, 2, 3] [9, 2, 3]
```

## Borrowing arrays and slicing

An array can be borrowed for reading, and a `&array` coerces implicitly to the
slice `&[T]`. Range indexing of a borrowed array produces a slice:
`&array[a..b]`, `&array[..b]`, `&array[a..]` and `&array[..]` are all
available, as is indexing a single element, which yields a reference to the
element:

```ls
fn total(values: &[i64]) -> i64 {
    let mut sum = 0i64;
    for index in 0..values.len() {
        sum += values[index];
    }
    return sum;
}

fn main() {
    let xs: [i64; 4] = [1, 2, 3, 4];
    let view: &[i64] = &xs;
    let middle: &[i64] = &xs[1..3];
    println!("{} {} {:?}", total(&xs), view.len(), middle);
}
```

Output:

```text
10 4 [2, 3]
```

A single-element borrow is a reference to the element, not a slice:

```ls
fn first(values: &[i64]) -> i64 {
    return values[0];
}

fn main() {
    let xs: [i64; 3] = [10, 20, 30];
    let element: &i64 = &xs[1];
    println!("{} {} {}", first(&xs), *element, xs.len());
}
```

Output:

```text
10 20 3
```

## Arrays in aggregate types

Arrays can be struct fields, enum payloads, function parameters and return
values, type arguments of `Option` and `Vec`, and elements of other arrays.
Printing is structural and equality is element-wise:

```ls
struct Row {
    cells: [i64; 3],
}

enum Cell {
    Empty,
    Row([i64; 2]),
}

fn main() {
    let row = Row { cells: [1, 2, 3] };
    let cell = Cell::Row([3, 4]);
    let maybe: Option<[i64; 2]> = Some([5, 6]);
    let cells: Vec<[i64; 2]> = vec![[1, 2], [3, 4]];
    match cell {
        Cell::Empty => { println!("empty"); }
        Cell::Row(v) => { println!("{:?}", v); }
    }
    println!("{:?} {:?}", row.cells, maybe);
    println!("{} {:?}", cells.len(), cells[1]);
}
```

Output:

```text
[3, 4]
[1, 2, 3] Some([5, 6])
2 [3, 4]
```

## Comparing arrays

Arrays compare with `==` and `!=` when their element type supports equality.
The comparison is structural and respects the element order:

```ls
fn main() {
    let zeroes = [0i64; 3];
    let equal = [1i64, 2, 3] == [1, 2, 3];
    let different = [1i64, 2, 3] != [1, 2, 4];
    println!("{:?} {} {}", zeroes, equal, different);
}
```

Output:

```text
[0, 0, 0] true true
```

## Iterating over an array

An array is not iterable by value. `for element in array` is rejected with
`error[E0277]: cannot iterate over an array by value; arrays are not iterable in this language version`
and a note suggesting the index route. Arrays do not have an `.iter()` method
either; iterate over the indices with `0..array.len()`, or pass `&array` as
`&[T]` and iterate over that slice from inside a function that takes `&[T]`:

```ls
fn total(values: &[i64]) -> i64 {
    let mut sum = 0i64;
    for index in 0..values.len() {
        sum += values[index];
    }
    return sum;
}

fn main() {
    let xs: [i64; 4] = [1, 2, 3, 4];
    println!("{}", total(&xs));
}
```

Output:

```text
10
```

## How arrays are represented

An array is a value type: it is stored inline in its containing value, with no
heap allocation. Copying an array copies every element, and a borrowed array
does not allocate. In the generated C project an array type is a struct wrapper
around a C array of the element type, so an array by value and an array field
behave like any other aggregate.

## Known limitations in 2.2.0

* Arrays are not iterable by value, have no `.iter()` or `.iter_mut()` method,
  and there is no array pattern syntax: `let [a, b] = [1, 2];` and patterns
  such as `[1, 2, 3]` are parse errors.
* References cannot be array elements: `[&i64; 2]` is rejected with
  `error[E0498]: references cannot be stored in this type (found &i64)`.
  References can be stored in struct, enum, tuple and `Option` fields (see
  [Lifetimes](29-lifetimes.md)), but not inside arrays.
* The length must be an integer literal or an integer `const`; there are no
  const generics and no general constant expressions in type positions.
* There is no runtime-length array; use [`Vec`](15-standard-library.md#vec) when
  the length is not known at compile time.
* Array-to-slice coercions cover `&array` to `&[T]` and range indexing; a
  mutable whole-array view is not part of the 2.2.0 surface described here.

## Summary

* An array type is `[T; N]`, with `N` a compile-time integer literal or
  integer `const` path; `N` may be zero.
* Literals are `[a, b, c]`, `[value; N]`, and `[]` with a type annotation.
* Indexing uses `usize`, produces a place, and panics when out of bounds.
* `.len()` is a `usize`; `.is_empty()` is a `bool`.
* Arrays are `Copy` exactly when their element type is `Copy`; they compare
  with `==` and `!=` and print as `[1, 2, 3]`.
* `&array` coerces to `&[T]`, and range indexing of a borrowed array yields a
  slice.
* Iterate with `for index in 0..array.len()`, or pass `&array` to a function
  that takes `&[T]`.

## What next

* [Slices and str](26-slices-and-str.md) covers the borrowed view of sequences.
* [Lifetimes](29-lifetimes.md) covers storing references in aggregates.
* [Loop labels](28-loop-labels.md) covers exiting nested loops.
* [The standard library](15-standard-library.md) covers `Vec` and the rest of
  the prelude.
