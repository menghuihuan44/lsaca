# core: Option, Result and Iterator

**Status in 2.0.0.** The `Option`, `Result` and `Iterator` surfaces are the ones that
existed in 1.7 as part of the builtin prelude. In 2.0.0 they live in the `std` crate
(`std::core`) and are re-exported unqualified, so existing programs keep working.
Behaviour, panics and formatting are unchanged.

## Option<T>

`Option<T>` is either `Some(value)` or `None`. It is a handle when `T` is a handle
and a plain value otherwise; it is `Copy` when `T` is `Copy`.

| Method | Signature | Meaning |
| --- | --- | --- |
| `is_some` / `is_none` | → `bool` | variant test |
| `unwrap` | → `T` | the value; panics on `None` |
| `unwrap_or` | `(T) -> T` | the value or the default |
| `expect` | `(String) -> T` | the value; panics with the message on `None` |
| `map` | `(Fn(T) -> U)` | applies `f` to a `Some` value |
| `map_or` | `(U, Fn(T) -> U)` | applies `f` or returns the default |
| `and_then` | `(Fn(T) -> Option<U>)` | chains an `Option`-returning function |
| `filter` | `(Fn(&T) -> bool)` | keeps `Some(v)` only when the predicate holds |
| `ok_or` | `(E) -> Result<T, E>` | converts to a `Result` |

Combinators may be chained; the closure argument receives the value by value except
for `filter`, which receives a reference. The full combinator walkthrough is in the
[Standard library chapter](../language/15-standard-library.md#option-combinators).

## Result<T, E>

`Result<T, E>` is either `Ok(value)` or `Err(error)`.

| Method | Signature | Meaning |
| --- | --- | --- |
| `is_ok` / `is_err` | → `bool` | variant test |
| `unwrap` / `expect` | → `T` | the value; panics on `Err` |
| `unwrap_or` | `(T) -> T` | the value or the default |
| `ok` / `err` | → `Option` | drops one side |
| `map` | `(Fn(T) -> U)` | applies `f` to an `Ok` value |
| `map_err` | `(Fn(E) -> F)` | applies `f` to the error |
| `and_then` | `(Fn(T) -> Result<U, E>)` | chains a fallible function |

The `?` operator propagates an `Err` from a function whose return type is a
compatible `Result`; it is a language feature documented in the
[Standard library chapter](../language/15-standard-library.md#error-propagation-with-).

## Iterator<T>

`Iterator<T>` is the trait every iterator implements:

```rust
pub trait Iterator<T> {
    fn next(&mut self) -> Option<T>;
}
```

It is a normal trait: user types can implement it, and generic code can bound on it
(see [Iterators](../language/19-iterators.md)). The adapters and terminal operations
live in [iter](iter.md).

## Planned for a later release

* Iterator fusion in the compiler (an optimisation, no API change).
* No `core`-level error trait in 2.0.0: `std::error::Error` is defined in the error
  module and used by `io` and `fs`.

## See also

* [Standard library chapter](../language/15-standard-library.md) - full 1.7 method
  reference and the `?` operator.
* [Iterators](iter.md) and [Collections](collections/README.md).
